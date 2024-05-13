COPY public.account (id, username, display_name, email, bio, created_at) FROM stdin;
b962b6db-18cf-455d-b5a0-b85316755bd3	alice	Alice	alice@example.com	Alice is a very nice person and a well-behaved user.	2023-10-26 03:32:11.051084+00
bd67c25a-3b3e-4cdb-9ef6-bf6a5de97713	bob	Bob	bob@example.com	Bob is just trying to enjoy his life.	2023-10-26 03:32:11.051084+00
b91885ef-6e90-4127-a7f2-890353a81327	carol	Carol	carol@example.com	Carol is having a grand old time.	2023-10-26 03:32:11.051084+00
\.

COPY public.quest (id, questmaster, created_at, unlisted, state, title, slug) FROM stdin;
018b5516-cd36-7b44-9ba0-8cac6b3ef904	b962b6db-18cf-455d-b5a0-b85316755bd3	2023-10-22 01:56:15.396825+00	f	active	The Quest for Alice	quest-alice
018b5519-2163-7895-9cd5-9cdf707f5d66	b962b6db-18cf-455d-b5a0-b85316755bd3	2023-10-22 01:56:49.518985+00	f	active	Wonderland	wonderland
018b551f-b6b0-7996-9413-6371aeff0a1d	bd67c25a-3b3e-4cdb-9ef6-bf6a5de97713	2023-10-22 02:08:30.889012+00	f	active	Bob Quest	bob-quest
\.

