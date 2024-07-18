-- TODO - Consider using citext again.

-- pg_partman extension on schema partman.
create schema partman;
create extension pg_partman schema partman;

-- 254 characters is the maximum length of an email address per the spec.
create domain email as varchar(254)
  check ( value ~ '^[a-zA-Z0-9.!#$%&''*+/=?^_`{|}~-]+@[a-zA-Z0-9](?:[a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?(?:\.[a-zA-Z0-9](?:[a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?)*$' );

create table account (
  id uuid primary key default gen_random_uuid(),
  email email unique not null,
  secondary_email email[],
  created_at timestamptz not null default current_timestamp,
  ask_for_profile_on_login boolean not null default false
);

comment on table account is 'An account (lurker or profiled user) that is using our site.';
comment on column account.id is 'Account ID.';
comment on column account.email is 'Primary email.';
comment on column account.secondary_email is 'Secondary emails.';
comment on column account.created_at is 'Created at timestamp.';
comment on column account.ask_for_profile_on_login is 'Setting to ask for what profile to log in as every login.';

create domain username as varchar(20)
  check ( value ~ '^[a-z0-9]+$' and substring(value, 1, 1) ~ '[a-z]' );

create table profile (
  id uuid primary key default gen_random_uuid(),
  username username unique not null constraint username_not_too_long check (length(username) >= 3),
  account_id uuid references account not null,
  display_name varchar(30),
  bio varchar(500)
);

alter table account add column default_profile uuid references profile;
comment on column account.default_profile is 'Default profile. Null is reader mode.';

create or replace function check_account_default_profile() returns trigger as $$
  declare
    profile_not_same_account boolean;
  begin
    if new.default_profile is not null
        and new.default_profile is distinct from old.default_profile then
      select not exists(
        select 1
        from profile
        where id = new.default_profile
          and account_id = new.id limit 1
      ) into profile_not_same_account;
      if profile_not_same_account then
        raise exception 'default_profile must exist in the profile table and be owned by the same account';
      end if;
    end if;
    return new;
  end;
$$ language plpgsql;

create constraint trigger tcheck_account_default_profile
  after insert or update on account
  deferrable initially deferred
  for each row execute function check_account_default_profile();

comment on table profile is 'User profile, for non-lurker users.';
comment on column profile.id is 'Profile ID. Unchanging in case the username is changed. Private/transparent to users.';
comment on column profile.username is 'Username.';
comment on column profile.account_id is 'Account association.';
comment on column profile.display_name is 'Display name; shown in UI.';
comment on column profile.bio is 'User bio.';

-- Whether a quest is active or not. Does not impact visibility, posting permissions, etc. Is used as a signal to readers in the UI.
create type quest_publish_state as enum (
  -- Quest is still a draft/idea.
  'prepping',
  -- Quest is active.
  'active',
  -- Quest is on hiatus.
  'hiatus',
  -- Quest is cancelled.
  'cancelled'
  -- Quest is complete.
  'complete'
);

create domain url_part as varchar(30) check ( value ~ '^[a-z0-9]+$' );

-- A quest.
create table quest (
  id uuid primary key,
  title varchar(250) not null,
  slug url_part unique not null,
  short_description varchar(250),
  long_description varchar(5000),
  questmaster uuid references account not null,
  publish_state quest_publish_state default 'prepping'::quest_publish_state not null,
  created_at timestamptz not null default current_timestamp,
  general_access boolean not null default true,
  general_commenting boolean not null default true,
  listed_in_feeds boolean not null default true,
  require_log_in_to_view boolean not null default false
  /* last_updated timestamptz not null default current_timestamp */
);

comment on table quest is 'A quest';
comment on column quest.id is 'Quest ID';
comment on column quest.title is 'Title of the quest, shown in the UI.';
comment on column quest.slug is 'Short name used in the URL.';
comment on column quest.short_description is 'Short description shown in feeds.';
comment on column quest.long_description is 'Long description shown on the quest page.';
comment on column quest.questmaster is 'Who the questmaster, the author of the quest, is.';
comment on column quest.publish_state is 'Whether the quest is active or not, according to the author.';
comment on column quest.created_at is 'When the quest was created';
comment on column quest.general_access is 'Whether the quest is viewable without explicitly being on an allowlist.';
comment on column quest.general_commenting is 'Whether the quest allows comments without commenters being on an allowlist.';
comment on column quest.listed_in_feeds is 'Whether the quest is listed in feeds, for users who can view it.';
comment on column quest.require_log_in_to_view is 'Whether the quest requires the user to be logged in, independent of general_access.';
/* comment on column quest.last_updated is 'When the QM last updated the quest.'; */

-- Allowlisted users for a given quest.
create table quest_allowed_user (
  quest_id uuid references quest not null,
  profile_id uuid references profile not null
);

-- EVERYTHING BELOW IS VERY NOT FINAL:

-- Whether a quest post is still accepting comments.
create type quest_post_state as enum (
  -- Post is still a draft, visible only to the QM account.
  'draft',
  -- Post is accepting new player commands.
  'accept_command',
  -- Post is not accepting new commands, but is accepting votes or other player feedback. Non-command comments are still accepted.
  'accept_vote',
  -- Post is not accepting new commands or player feedback. Non-command comments are still accepted.
  'finalized'
);

/* create table quest_moderator ( */
/* ); */

-- A post by the questmaster of a quest.
-- Assumption: one questmaster.
create table quest_post (
  id uuid primary key,
  quest uuid references quest not null,
  title text,
  body_markup text not null constraint not_empty check (body_markup <> ''),
  body_html text not null,
  created_at timestamptz not null default current_timestamp,
  published_at timestamptz null,
  state quest_post_state default 'draft'
);

comment on table quest_post is 'A post on a quest, created by the questmaster.';
comment on column quest_post.id is 'Post ID';
comment on column quest_post.quest is 'What quest the post is under.';
comment on column quest_post.title is 'Title of the post.';
comment on column quest_post.body_markup is 'Actual text of the post in markup form.';
comment on column quest_post.body_html is 'Actual text of the post converted to HTML.';
comment on column quest_post.created_at is 'When the post was created.';
comment on column quest_post.published_at is 'Whether it has been published. Null if unpublished.';
comment on column quest_post.state is 'What state the post is in.';

create table username_tombstone (
    username username not null,
    account uuid not null,
    deleted_at timestamptz not null default now()
) partition by range (deleted_at);

-- Insertion of old data may go into the DEFAULT partition, which isn't subject
-- to partman's retention policy, until `select
-- partman.partition_data_time('public.username_tombstone');` is run. This may
-- result in it living forever.

-- TODO: create a trigger to keep out old data.

comment on table username_tombstone is 'Tombstones used to prevent usage of usernames that were recently changed.';
comment on column username_tombstone.username is 'Username that is tombstoned.';
comment on column username_tombstone.account is 'Prior account the username was associated with.';
comment on column username_tombstone.deleted_at is 'Timestamp at deletion time';

create table username_tombstone_table_template (like username_tombstone);
alter table username_tombstone_table_template add primary key (username);
create index on username_tombstone (account);

select partman.create_parent(
    p_parent_table := 'public.username_tombstone',
    p_control := 'deleted_at',
    p_interval := '1 day',
    p_template_table := 'public.username_tombstone_table_template',
    p_type := 'native'
);

update partman.part_config
set retention_keep_table = false, retention = '30 days'
where parent_table = 'public.username_tombstone';

-- Whether a quest post is still accepting comments.
create type quest_comment_type as enum (
  -- Comment is just a comment.
  'comment',
  -- Comment is a command intended for the QM to execute next turn.
  'command',
  -- Comment is a question for the QM.
  'question'
);

-- A comment on a quest.
create table quest_comment (
  id uuid primary key,
  -- The one who commented.
  commenter uuid references account not null,
  -- Created at.
  created_at timestamptz not null,
  -- Quest post that the command is under.
  quest_post uuid references quest_post not null,
  -- Comment being replied to, if replying to a comment.
  reply_to uuid references quest_comment,
  -- Intended to be a command by the commenter.
  comment_type quest_comment_type default 'comment'::quest_comment_type,
  -- Whether QM excluded the command from selection.
  qm_excluded boolean default false not null,
  -- Why the QM excluded the command from selection.
  qm_exclusion_reason text,
  -- Actual text of the comment.
  body text not null constraint not_empty check (body <> ''),
  constraint command_is_not_reply check (comment_type <> 'command'::quest_comment_type or reply_to is null),
  constraint exclusion_reason_only_if_excluded check (qm_excluded or qm_exclusion_reason is null)
);

comment on table quest_comment is 'A comment on a quest.';
comment on column quest_comment.id is 'Quest comment';
comment on column quest_comment.commenter is 'The one who commented.';
comment on column quest_comment.created_at is 'Created at.';
comment on column quest_comment.quest_post is 'Quest post that the command is under.';
comment on column quest_comment.reply_to is 'Comment being replied to, if replying to a comment.';
comment on column quest_comment.comment_type is 'What type of comment this is, whether comment, command, or question.';
comment on column quest_comment.qm_excluded is 'Whether QM excluded the command from selection.';
comment on column quest_comment.qm_exclusion_reason is 'Why the QM excluded the command from selection.';
comment on column quest_comment.body is 'Actual text of the comment.';
