# zero2prod

Code I wrote while following along [zero2prod](https://www.zero2prod.com).

It's not exactly identical because I made some different choices:
* No Redis so I implemented a [session store](https://github.com/vrischmann/zero2prod/blob/master/src/sessions/session_store.rs) using PostgreSQL
* I used [Scaleway TEM](https://www.scaleway.com/fr/betas/#tem-transactional-email) instead of Postmark
* No automatic deployment, I build a deb that I deploy on my server
