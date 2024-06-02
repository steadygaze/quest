-- TODO - Consider using citext again.
-- We include triggers for checking the allowed lengths of some fields, but
-- this would be incompatible with allowing per-instance configurable lengths.

create domain email as text
  check ( value ~ '^[a-zA-Z0-9.!#$%&''*+/=?^_`{|}~-]+@[a-zA-Z0-9](?:[a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?(?:\.[a-zA-Z0-9](?:[a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?)*$' );

-- Should permissions be per-account or per-profile?

create table account (
  id uuid primary key default gen_random_uuid(),
  email email unique not null,
  secondary_email email[],
  created_at timestamptz not null default current_timestamp
);

comment on table account is 'An account (lurker or profiled user) that is using our site.';
comment on column account.id is 'Account ID.';
comment on column account.email is 'Primary email.';
comment on column account.secondary_email is 'Secondary emails.';
comment on column account.created_at is 'Created at timestamp.';

create domain username as text
  check ( value ~ '^[a-z0-9]+$' and substring(value, 1, 1) ~ '[a-z]' );

create table profile (
  id uuid primary key default gen_random_uuid(),
  username username unique not null constraint username_not_too_long check (length(username) < 30 and length(username) >= 3),
  account_id uuid references account,
  display_name text constraint display_name_not_too_long check (length(display_name) < 30),
  bio text constraint bio_not_too_long check (length(bio) < 500)
);

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

create domain url_part as text check ( value ~ '^[a-z0-9]+$' );

-- A quest.
create table quest (
  id uuid primary key,
  title text not null,
  slug url_part unique not null,
  short_description text check (length(short_description) <= 250),
  long_description text check (length(long_description) <= 5000),
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
  -- What quest the post is under.
  quest uuid references quest not null,
  -- Actual text of the post.
  body text not null constraint not_empty check (body <> ''),
  -- When the post was created.
  created_at timestamptz not null,
  -- Whether it's been published. Null if unpublished.
  published_at timestamptz null,
  -- What state the post is in.
  state quest_post_state default 'draft'
);

comment on table quest_post is 'A post on a quest, created by the questmaster.';
comment on column quest_post.id is 'Post ID';
comment on column quest_post.quest is 'What quest the post is under.';
comment on column quest_post.body is 'Actual text of the post.';
comment on column quest_post.created_at is 'When the post was created.';
comment on column quest_post.published_at is 'Whether it has been published. Null if unpublished.';
comment on column quest_post.state is 'What state the post is in.';

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
