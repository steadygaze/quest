drop function if exists check_account_default_profile;
drop trigger if exists tcheck_account_default_profile;

drop table if exists username_tombstone_table_template;
drop table if exists username_tombstone;

drop table if exists quest_allowed_user;
drop table if exists quest_comment;
drop table if exists quest_post;
drop table if exists quest;
drop table if exists profile;
drop table if exists account;

drop type if exists quest_comment_type;
drop type if exists quest_post_state;
drop type if exists quest_publish_state;

drop domain if exists email;
drop domain if exists username;
drop domain if exists url_part;

drop extension if exists pg_partman;
drop schema if exists partman;
