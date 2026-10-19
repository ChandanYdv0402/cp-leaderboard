# CP Leaderboard

Weekly, bi-weekly, monthly, and all-time Codeforces + LeetCode standings, with upcoming contests. Supabase stores profiles, linked handles, snapshots, and contests; GitHub Actions refreshes platform data every six hours and GitHub Pages serves the static site.

## Supabase setup

1. Create a Supabase project and run [`supabase/schema.sql`](supabase/schema.sql) in the SQL Editor.
2. Sign-in uses [Clerk](https://clerk.com) via Supabase third-party auth:
   - Create a Clerk application and pick login methods (Google, GitHub, email code, …) in the Clerk dashboard.
   - In Clerk, open the **Supabase** integration setup and activate it (this adds the `role: authenticated` claim to session tokens). Copy the Clerk domain it shows.
   - In Supabase, **Authentication → Sign In / Providers → Third-Party Auth → Add Clerk**, and paste that domain.
   - Put the Clerk publishable key in `CLERK_PUBLISHABLE_KEY` in `site/index.html` (public, safe to commit).
   - Projects created before Clerk: run [`supabase/clerk_migration.sql`](supabase/clerk_migration.sql) once (user ids become text, RLS reads the JWT `sub`).
   - Clerk production instances need a domain you own; on `*.github.io` use a Clerk development instance (shows a "Development mode" badge).
3. Put the project URL and anon key in the `SUPABASE_URL` and `SUPABASE_ANON_KEY` constants in `site/index.html`. These browser values are public; database access is controlled by RLS.
4. Add GitHub repository Actions secrets named `SUPABASE_URL` and `SUPABASE_SERVICE_ROLE_KEY`. Never put the service-role key in the site or commit it.
5. In repository Settings → Pages, select **GitHub Actions** as the source. Run the `pages` workflow once, then run `update` to populate initial data.

## How it works

- Anyone can view the leaderboard and contests. Signed-in users can create their profile and manage their own Codeforces and LeetCode handles. Handle fields accept a username or profile URL, normalize supported URLs to usernames, and link to the matching public profile.
- The fetcher reads accounts and stores one snapshot per profile each run. The browser calls `leaderboard(days)` for period standings.
- A period's score is the latest snapshot minus the last snapshot at or before the period start, or the earliest available snapshot when history is shorter.
- Scoring: Codeforces problem = rating / 100 (unrated = 8); LeetCode Easy/Medium/Hard = 5/10/20.

## Run locally

Set `SUPABASE_URL` and `SUPABASE_SERVICE_ROLE_KEY` in your shell, then run `cargo run`. Serve `site/` over HTTP to test the browser client; configure the project's URL and anon key in `site/index.html` first.
