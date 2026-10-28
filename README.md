# CP Leaderboard

Weekly, bi-weekly, monthly, and all-time Codeforces + LeetCode + CodeChef standings, with upcoming contests. Supabase stores profiles, linked handles, snapshots, and contests; GitHub Actions refreshes platform data every six hours and GitHub Pages serves the static site.

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

### Upgrade an existing Supabase project for groups

Run [`supabase/migrations/202610250001_groups.sql`](supabase/migrations/202610250001_groups.sql) once in the Supabase SQL Editor. Existing projects must first have applied `clerk_migration.sql` and `leaderboard_by_platform.sql`; the groups migration uses Clerk text user ids and the current per-platform scoring RPC. It adds groups without changing existing profiles, snapshots, or the collector. New projects get the same tables and functions from `schema.sql`; do not run these migrations again after installing the full schema. Until the groups migration is applied, the site falls back to global rankings and reports groups as unavailable.

### Invitation-only groups

For CodeChef support, apply the CodeChef migration described below **after** the groups migration.

1. Sign in and save your profile, then open **Create or join a group**. Create a group with a name or paste an invitation link/code to join one.
2. The group dashboard shows only members, with the existing weekly, bi-weekly, monthly, and all-time scores, member count, and totals. Platform filters also apply to group totals. Rank cards, progress charts, duels, and the solve heatmap follow the selected group. Members awaiting their first collector snapshot appear with zero points and an “awaiting stats” label.
3. The owner can copy an invitation, replace it to invalidate the old link, remove members, or delete the group. Members can leave. Removed members can rejoin with a valid invitation; replace the invitation to stop reuse. Invitations remain available in the same browser through sign-in, including a new email sign-in tab, until joined or dismissed.
4. The site remembers the dashboard selected for each signed-in account. **Global rankings** is available separately.

Group names and membership are readable only by members through RLS; invitation codes are accessible only through owner-checked functions. Anyone holding an invitation can join after signing in and saving a profile. Registration remains closed as configured on `main`: invitations do not bypass the existing player restriction. Individual profiles, handles, and scores remain public in global rankings. Groups do not make those existing public records private.

## How it works

### CodeChef setup for existing projects

Run [`supabase/migrations/202610280001_codechef.sql`](supabase/migrations/202610280001_codechef.sql) in the Supabase SQL Editor to enable linking CodeChef handles and include CodeChef in both leaderboard RPCs. The migration preserves existing accounts/history, works without groups, and can be rerun safely. If you add groups later, rerun the CodeChef migration afterward to update their standings too. Fresh projects receive everything from `schema.sql`.

In **My profile and handles**, enter a CodeChef username or `https://www.codechef.com/users/USERNAME` URL, save, and wait for the next collector run. The optional field starts blank. CodeChef adds 10 points per solved problem; contest rating is displayed separately. Select **CodeChef** in the Platform filter for its standings, progress, and heatmap. CodeChef's rating/star tiers, profile links, and upcoming contests are also included.

The collector reads CodeChef's public profile HTML and contest feed directly, with no third-party proxy or credentials. Changes to those website formats can break collection; failures retain the previous stats instead of writing zeros. Old snapshots without CodeChef fields keep their existing totals. CodeChef fields live in snapshot `stats` JSON, so deploying the collector before the migration does not require a new snapshots column.

- Anyone can view the leaderboard and contests. Existing signed-in players can manage their profile and Codeforces, LeetCode, and CodeChef handles. Handle fields accept a username or profile URL, normalize supported URLs to usernames, and link to the matching public profile.
- The fetcher reads accounts and stores one snapshot per profile each run. The browser calls `leaderboard(days)` for period standings. It returns combined and per-platform (Codeforces / LeetCode) points and solved counts; the site's Platform and Sort by dropdowns filter and re-sort these client-side. Existing projects: run [`supabase/leaderboard_by_platform.sql`](supabase/leaderboard_by_platform.sql) once.
- A period's score is the latest snapshot minus the last snapshot at or before the period start, or the earliest available snapshot when history is shorter.
- Scoring: Codeforces problem = rating / 100 (unrated = 8); LeetCode Easy/Medium/Hard = 5/10/20; CodeChef problem = 10.

## Run locally

Set `SUPABASE_URL` and `SUPABASE_SERVICE_ROLE_KEY` in your shell, then run `cargo run`. Serve `site/` over HTTP to test the browser client; configure the project's URL and anon key in `site/index.html` first.

## Checks

`cargo fmt --check` and `cargo test --locked` check the collector. `cargo test --locked codechef_public_sources -- --ignored` optionally checks the real CodeChef sources. The `checks` GitHub Actions workflow runs Rust and Node checks; the live-source check is advisory because upstream availability can change.

With Node.js 22 or newer, run `npm ci` and `npm test`. The tests execute the SQL migration and access rules in an isolated PGlite PostgreSQL database, with a minimal JWT auth contract using Clerk-style text ids. Browser checks run the real page with mocked Clerk, Supabase responses, and Chart.js rendering. They use Microsoft Edge on Windows or a standard Chromium/Chrome installation on Linux; set `CP_BROWSER` to another Chromium executable if needed. Browser checks are reported as skipped when no browser is available. These checks do not access or change the hosted Supabase project.
