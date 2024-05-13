--
-- PostgreSQL database dump
--

-- Dumped from database version 15.4
-- Dumped by pg_dump version 15.4

SET statement_timeout = 0;
SET lock_timeout = 0;
SET idle_in_transaction_session_timeout = 0;
SET client_encoding = 'UTF8';
SET standard_conforming_strings = on;
SELECT pg_catalog.set_config('search_path', '', false);
SET check_function_bodies = false;
SET xmloption = content;
SET client_min_messages = warning;
SET row_security = off;

--
-- Name: email; Type: DOMAIN; Schema: public; Owner: postgres
--

CREATE DOMAIN public.email AS text
	CONSTRAINT email_check CHECK ((VALUE ~ '^[a-zA-Z0-9.!#$%&''*+/=?^_`{|}~-]+@[a-zA-Z0-9](?:[a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?(?:\.[a-zA-Z0-9](?:[a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?)*$'::text));


ALTER DOMAIN public.email OWNER TO postgres;

--
-- Name: quest_post_state; Type: TYPE; Schema: public; Owner: postgres
--

CREATE TYPE public.quest_post_state AS ENUM (
    'draft',
    'accept_command',
    'accept_vote',
    'finalized'
);


ALTER TYPE public.quest_post_state OWNER TO postgres;

--
-- Name: quest_state; Type: TYPE; Schema: public; Owner: postgres
--

CREATE TYPE public.quest_state AS ENUM (
    'preparing',
    'active',
    'hiatus',
    'archived'
);


ALTER TYPE public.quest_state OWNER TO postgres;

--
-- Name: url_part; Type: DOMAIN; Schema: public; Owner: postgres
--

CREATE DOMAIN public.url_part AS text
	CONSTRAINT url_part_check CHECK ((VALUE ~ '^[a-zA-Z0-9-]+$'::text));


ALTER DOMAIN public.url_part OWNER TO postgres;

--
-- Name: username; Type: DOMAIN; Schema: public; Owner: postgres
--

CREATE DOMAIN public.username AS text
	CONSTRAINT username_check CHECK ((VALUE ~ '^[a-z_0-9]+$'::text));


ALTER DOMAIN public.username OWNER TO postgres;

SET default_tablespace = '';

SET default_table_access_method = heap;

--
-- Name: _sqlx_migrations; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public._sqlx_migrations (
    version bigint NOT NULL,
    description text NOT NULL,
    installed_on timestamp with time zone DEFAULT now() NOT NULL,
    success boolean NOT NULL,
    checksum bytea NOT NULL,
    execution_time bigint NOT NULL
);


ALTER TABLE public._sqlx_migrations OWNER TO postgres;

--
-- Name: account; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.account (
    id uuid NOT NULL,
    username public.username NOT NULL,
    display_name text,
    email public.email NOT NULL,
    bio text,
    created_at timestamp with time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    CONSTRAINT bio_not_too_long CHECK ((length(bio) < 5000)),
    CONSTRAINT display_name_not_too_long CHECK ((length(display_name) < 30)),
    CONSTRAINT username_not_too_long CHECK ((length((username)::text) < 30))
);


ALTER TABLE public.account OWNER TO postgres;

--
-- Name: TABLE account; Type: COMMENT; Schema: public; Owner: postgres
--

COMMENT ON TABLE public.account IS 'An account (player, QM, or both) that is using our site.';


--
-- Name: COLUMN account.id; Type: COMMENT; Schema: public; Owner: postgres
--

COMMENT ON COLUMN public.account.id IS 'Account ID';


--
-- Name: COLUMN account.username; Type: COMMENT; Schema: public; Owner: postgres
--

COMMENT ON COLUMN public.account.username IS 'Username';


--
-- Name: COLUMN account.display_name; Type: COMMENT; Schema: public; Owner: postgres
--

COMMENT ON COLUMN public.account.display_name IS 'Display name';


--
-- Name: COLUMN account.email; Type: COMMENT; Schema: public; Owner: postgres
--

COMMENT ON COLUMN public.account.email IS 'Email';


--
-- Name: COLUMN account.bio; Type: COMMENT; Schema: public; Owner: postgres
--

COMMENT ON COLUMN public.account.bio IS 'User bio';


--
-- Name: quest; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.quest (
    id uuid NOT NULL,
    questmaster uuid NOT NULL,
    created_at timestamp with time zone NOT NULL,
    unlisted boolean NOT NULL,
    state public.quest_state DEFAULT 'preparing'::public.quest_state NOT NULL,
    title text NOT NULL,
    slug public.url_part NOT NULL
);


ALTER TABLE public.quest OWNER TO postgres;

--
-- Name: TABLE quest; Type: COMMENT; Schema: public; Owner: postgres
--

COMMENT ON TABLE public.quest IS 'A quest';


--
-- Name: COLUMN quest.id; Type: COMMENT; Schema: public; Owner: postgres
--

COMMENT ON COLUMN public.quest.id IS 'Quest ID';


--
-- Name: COLUMN quest.questmaster; Type: COMMENT; Schema: public; Owner: postgres
--

COMMENT ON COLUMN public.quest.questmaster IS 'Who the questmaster, the author of the quest, is.';


--
-- Name: COLUMN quest.created_at; Type: COMMENT; Schema: public; Owner: postgres
--

COMMENT ON COLUMN public.quest.created_at IS 'When the quest was created';


--
-- Name: COLUMN quest.unlisted; Type: COMMENT; Schema: public; Owner: postgres
--

COMMENT ON COLUMN public.quest.unlisted IS 'Whether the quest should be listed in explore feeds.';


--
-- Name: COLUMN quest.state; Type: COMMENT; Schema: public; Owner: postgres
--

COMMENT ON COLUMN public.quest.state IS 'What state the quest is in.';


--
-- Name: COLUMN quest.title; Type: COMMENT; Schema: public; Owner: postgres
--

COMMENT ON COLUMN public.quest.title IS 'Title of the quest, shown in the UI.';


--
-- Name: COLUMN quest.slug; Type: COMMENT; Schema: public; Owner: postgres
--

COMMENT ON COLUMN public.quest.slug IS 'Short name used in the URL.';


--
-- Name: quest_comment; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.quest_comment (
    id uuid NOT NULL,
    commenter uuid,
    created_at timestamp with time zone NOT NULL,
    quest_post uuid NOT NULL,
    reply_to uuid,
    is_command boolean DEFAULT false,
    qm_excluded boolean,
    qm_exclusion_reason text,
    body text NOT NULL,
    CONSTRAINT command_is_not_reply CHECK (((NOT is_command) OR (reply_to IS NULL))),
    CONSTRAINT exclusion_reason_only_if_excluded CHECK ((qm_excluded OR (qm_exclusion_reason IS NULL))),
    CONSTRAINT not_empty CHECK ((body <> ''::text))
);


ALTER TABLE public.quest_comment OWNER TO postgres;

--
-- Name: TABLE quest_comment; Type: COMMENT; Schema: public; Owner: postgres
--

COMMENT ON TABLE public.quest_comment IS 'A comment on a quest.';


--
-- Name: COLUMN quest_comment.id; Type: COMMENT; Schema: public; Owner: postgres
--

COMMENT ON COLUMN public.quest_comment.id IS 'Quest comment';


--
-- Name: COLUMN quest_comment.commenter; Type: COMMENT; Schema: public; Owner: postgres
--

COMMENT ON COLUMN public.quest_comment.commenter IS 'The one who commented.';


--
-- Name: COLUMN quest_comment.created_at; Type: COMMENT; Schema: public; Owner: postgres
--

COMMENT ON COLUMN public.quest_comment.created_at IS 'Created at.';


--
-- Name: COLUMN quest_comment.quest_post; Type: COMMENT; Schema: public; Owner: postgres
--

COMMENT ON COLUMN public.quest_comment.quest_post IS 'Quest post that the command is under.';


--
-- Name: COLUMN quest_comment.reply_to; Type: COMMENT; Schema: public; Owner: postgres
--

COMMENT ON COLUMN public.quest_comment.reply_to IS 'Comment being replied to, if replying to a comment.';


--
-- Name: COLUMN quest_comment.is_command; Type: COMMENT; Schema: public; Owner: postgres
--

COMMENT ON COLUMN public.quest_comment.is_command IS 'Intended to be a command by the commenter.';


--
-- Name: COLUMN quest_comment.qm_excluded; Type: COMMENT; Schema: public; Owner: postgres
--

COMMENT ON COLUMN public.quest_comment.qm_excluded IS 'Whether QM excluded the command from selection.';


--
-- Name: COLUMN quest_comment.qm_exclusion_reason; Type: COMMENT; Schema: public; Owner: postgres
--

COMMENT ON COLUMN public.quest_comment.qm_exclusion_reason IS 'Why the QM excluded the command from selection.';


--
-- Name: COLUMN quest_comment.body; Type: COMMENT; Schema: public; Owner: postgres
--

COMMENT ON COLUMN public.quest_comment.body IS 'Actual text of the comment.';


--
-- Name: quest_post; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.quest_post (
    id uuid NOT NULL,
    quest uuid NOT NULL,
    body text NOT NULL,
    created_at timestamp with time zone NOT NULL,
    published_at timestamp with time zone,
    state public.quest_post_state DEFAULT 'draft'::public.quest_post_state,
    CONSTRAINT not_empty CHECK ((body <> ''::text))
);


ALTER TABLE public.quest_post OWNER TO postgres;

--
-- Name: TABLE quest_post; Type: COMMENT; Schema: public; Owner: postgres
--

COMMENT ON TABLE public.quest_post IS 'A post on a quest, created by the questmaster.';


--
-- Name: COLUMN quest_post.id; Type: COMMENT; Schema: public; Owner: postgres
--

COMMENT ON COLUMN public.quest_post.id IS 'Post ID';


--
-- Name: COLUMN quest_post.quest; Type: COMMENT; Schema: public; Owner: postgres
--

COMMENT ON COLUMN public.quest_post.quest IS 'What quest the post is under.';


--
-- Name: COLUMN quest_post.body; Type: COMMENT; Schema: public; Owner: postgres
--

COMMENT ON COLUMN public.quest_post.body IS 'Actual text of the post.';


--
-- Name: COLUMN quest_post.created_at; Type: COMMENT; Schema: public; Owner: postgres
--

COMMENT ON COLUMN public.quest_post.created_at IS 'When the post was created.';


--
-- Name: COLUMN quest_post.published_at; Type: COMMENT; Schema: public; Owner: postgres
--

COMMENT ON COLUMN public.quest_post.published_at IS 'Whether it has been published. Null if unpublished.';


--
-- Name: COLUMN quest_post.state; Type: COMMENT; Schema: public; Owner: postgres
--

COMMENT ON COLUMN public.quest_post.state IS 'What state the post is in.';


--
-- Data for Name: _sqlx_migrations; Type: TABLE DATA; Schema: public; Owner: postgres
--

COPY public._sqlx_migrations (version, description, installed_on, success, checksum, execution_time) FROM stdin;
20231026031049	initial	2023-10-26 03:31:38.759797+00	t	\\x4ae3ba60dbbb5feb8f6c685797fa93edba6a416e718c0be289765e927265b53c1e8ed03c94cbbd8d192f1fa444e74f93	16362909
\.


--
-- Data for Name: account; Type: TABLE DATA; Schema: public; Owner: postgres
--

COPY public.account (id, username, display_name, email, bio, created_at) FROM stdin;
b962b6db-18cf-455d-b5a0-b85316755bd3	alice	Alice	alice@example.com	Alice is a very nice person and a well-behaved user.	2023-10-26 03:32:11.051084+00
bd67c25a-3b3e-4cdb-9ef6-bf6a5de97713	bob	Bob	bob@example.com	Bob is just trying to enjoy his life.	2023-10-26 03:32:11.051084+00
b91885ef-6e90-4127-a7f2-890353a81327	carol	Carol	carol@example.com	Carol is having a grand old time.	2023-10-26 03:32:11.051084+00
\.


--
-- Data for Name: quest; Type: TABLE DATA; Schema: public; Owner: postgres
--

COPY public.quest (id, questmaster, created_at, unlisted, state, title, slug) FROM stdin;
018b5516-cd36-7b44-9ba0-8cac6b3ef904	b962b6db-18cf-455d-b5a0-b85316755bd3	2023-10-22 01:56:15.396825+00	f	active	The Quest for Alice	quest-alice
018b5519-2163-7895-9cd5-9cdf707f5d66	b962b6db-18cf-455d-b5a0-b85316755bd3	2023-10-22 01:56:49.518985+00	f	active	Wonderland	wonderland
018b551f-b6b0-7996-9413-6371aeff0a1d	bd67c25a-3b3e-4cdb-9ef6-bf6a5de97713	2023-10-22 02:08:30.889012+00	f	active	Bob Quest	bob-quest
\.


--
-- Data for Name: quest_comment; Type: TABLE DATA; Schema: public; Owner: postgres
--

COPY public.quest_comment (id, commenter, created_at, quest_post, reply_to, is_command, qm_excluded, qm_exclusion_reason, body) FROM stdin;
\.


--
-- Data for Name: quest_post; Type: TABLE DATA; Schema: public; Owner: postgres
--

COPY public.quest_post (id, quest, body, created_at, published_at, state) FROM stdin;
\.


--
-- Name: _sqlx_migrations _sqlx_migrations_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public._sqlx_migrations
    ADD CONSTRAINT _sqlx_migrations_pkey PRIMARY KEY (version);


--
-- Name: account account_email_key; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.account
    ADD CONSTRAINT account_email_key UNIQUE (email);


--
-- Name: account account_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.account
    ADD CONSTRAINT account_pkey PRIMARY KEY (id);


--
-- Name: account account_username_key; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.account
    ADD CONSTRAINT account_username_key UNIQUE (username);


--
-- Name: quest_comment quest_comment_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.quest_comment
    ADD CONSTRAINT quest_comment_pkey PRIMARY KEY (id);


--
-- Name: quest quest_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.quest
    ADD CONSTRAINT quest_pkey PRIMARY KEY (id);


--
-- Name: quest_post quest_post_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.quest_post
    ADD CONSTRAINT quest_post_pkey PRIMARY KEY (id);


--
-- Name: quest quest_slug_key; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.quest
    ADD CONSTRAINT quest_slug_key UNIQUE (slug);


--
-- Name: quest_comment quest_comment_commenter_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.quest_comment
    ADD CONSTRAINT quest_comment_commenter_fkey FOREIGN KEY (commenter) REFERENCES public.account(id);


--
-- Name: quest_comment quest_comment_quest_post_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.quest_comment
    ADD CONSTRAINT quest_comment_quest_post_fkey FOREIGN KEY (quest_post) REFERENCES public.quest_post(id);


--
-- Name: quest_comment quest_comment_reply_to_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.quest_comment
    ADD CONSTRAINT quest_comment_reply_to_fkey FOREIGN KEY (reply_to) REFERENCES public.quest_comment(id);


--
-- Name: quest_post quest_post_quest_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.quest_post
    ADD CONSTRAINT quest_post_quest_fkey FOREIGN KEY (quest) REFERENCES public.quest(id);


--
-- Name: quest quest_questmaster_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.quest
    ADD CONSTRAINT quest_questmaster_fkey FOREIGN KEY (questmaster) REFERENCES public.account(id);


--
-- PostgreSQL database dump complete
--

