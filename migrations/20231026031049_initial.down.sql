drop table if exists quest_comment;
drop table if exists quest_post;
drop table if exists quest;
drop table if exists profile;
drop table if exists oauth_redirect_pending;
drop table if exists account_creation_pending;
drop table if exists active_session;
drop table if exists account;

drop type if exists quest_comment_type;
drop type if exists quest_visibility;
drop type if exists quest_state;
drop type if exists quest_post_state;

drop domain if exists email;
drop domain if exists username;
drop domain if exists url_part;
