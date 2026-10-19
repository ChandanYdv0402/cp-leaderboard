# Project Context for Coding Assistants

Read this file before making project changes. Keep it current so the next assistant can continue without reconstructing the work.

## Project

CP Leaderboard is a static GitHub Pages site for Codeforces and LeetCode standings and upcoming contests. Supabase is being introduced for authentication, user profiles, linked handles, snapshots, and contests. GitHub Actions runs the Rust collector every six hours.

## Current Progress

- Added `supabase/schema.sql` with `profiles`, `accounts`, `snapshots`, and `contests`, RLS policies, grants, and the `leaderboard(days)` RPC.
- Reworked `src/main.rs` to fetch profiles/accounts from Supabase REST, fetch platform stats using the existing `src/platforms.rs`, preserve stats when an active platform fetch fails, clear stats for unlinked platforms, mark successful handles verified, and write snapshots/contests.
- Updated `site/index.html` with Supabase OAuth/email sign-in, profile and handle editing, leaderboard RPC queries, and upcoming contest queries.
- Split Actions into a scheduled Supabase fetcher (`.github/workflows/update.yml`) and a Pages deployment workflow (`.github/workflows/pages.yml`).
- Updated `README.md` with setup instructions.
- Existing `users.toml`, `data/snapshots/`, `site/leaderboard.json`, and `site/contests.json` have not been removed. Keep them until the Supabase flow is configured and verified.
- The browser currently uses placeholder values for the Supabase project URL and anon key in `site/index.html`.

## Current Blockers / Unverified

- Supabase project setup, schema execution, auth provider configuration, and redirect URL configuration have not been done from this workspace.
- GitHub Actions secrets `SUPABASE_URL` and `SUPABASE_SERVICE_ROLE_KEY` still need to be configured by the repository owner.
- The fetcher has not been compiled or run: Cargo/Rust is unavailable in the current environment.
- Browser auth and database behavior have not been exercised against a real Supabase project.
- Local HTTP preview is available at `http://localhost:8000/` in the current session; it uses a PowerShell server because Python is unavailable.

## Next Steps

1. Owner creates/configures the Supabase project and runs `supabase/schema.sql`.
2. Owner enables GitHub OAuth and/or email OTP, configures the Pages redirect URL, and adds the two GitHub Actions secrets.
3. Replace the placeholder project URL and anon key in `site/index.html` with the public Supabase values. Never put the service-role key in browser code or commit it.
4. Install/use the Rust toolchain, run formatting and `cargo check`, then run the fetcher against a test Supabase project.
5. Verify sign-in, own-profile/account RLS, handle verification, leaderboard periods, contest display, and both Actions workflows.
6. Only after successful verification, decide whether to import or remove the legacy TOML/JSON data.

## Project Conventions

- Keep `src/platforms.rs` as the owner of Codeforces/LeetCode API fetching unless a platform change requires otherwise.
- The browser may contain only the Supabase project URL and anon key. The service-role key is server-side/Actions-only.
- Preserve RLS as the access boundary: signed-in users can edit only their own profile and accounts; snapshots and contests are written by the service-role fetcher.
- Do not delete legacy data or generated JSON until the replacement has been proven operational.
- Use focused checks for each change. If a check cannot run because a tool or credential is missing, record that fact here.

## Assistant Handoff Maintenance

After every project change, update this file in the same task:

- Refresh **Current Progress** and **Current Blockers / Unverified** to reflect the repository accurately.
- Update **Next Steps** when priorities or prerequisites change.
- Record meaningful verification outcomes and any new blockers; do not claim tests passed unless they were actually run.
- Keep this handoff concise and factual. Do not add credentials, tokens, or other secrets.
