# Project Context for Coding Assistants

Read this file before making project changes. Keep it current so the next assistant can continue without reconstructing the work.

## Project

CP Leaderboard is a static GitHub Pages site for Codeforces and LeetCode standings and upcoming contests. Supabase is being introduced for authentication, user profiles, linked handles, snapshots, and contests. GitHub Actions runs the Rust collector every six hours.

## Current Progress

- Added `supabase/schema.sql` with `profiles`, `accounts`, `snapshots`, and `contests`, RLS policies, grants, and the `leaderboard(days)` RPC.
- Reworked `src/main.rs` to fetch profiles/accounts from Supabase REST, fetch platform stats using the existing `src/platforms.rs`, preserve stats when an active platform fetch fails, clear stats for unlinked platforms, mark successful handles verified, and write snapshots/contests.
- Updated `site/index.html` with Supabase OAuth/email sign-in, profile and handle editing, leaderboard RPC queries, and upcoming contest queries.
- Added Google OAuth sign-in to `site/index.html`; the Supabase Google provider still needs a Google OAuth client ID and secret configured by the project owner.
- Email sign-in takes a Gmail username and appends the visible `@gmail.com` suffix.
- The Gmail username draft persists locally across reloads; on first sign-in, empty platform handle fields default to the sanitized email prefix and remain editable.
- Each editable Codeforces/LeetCode handle accepts a username or public profile URL, normalizes supported URLs to usernames, and links to the corresponding public profile page.
- Pushed expanded profile URL handling in commit `ec96a86`; the Pages workflow completed successfully and production shows 200-character handle inputs. Local checks normalized `https://codeforces.com/profile/BihariBull0402` to `BihariBull0402` and a LeetCode `/u/` URL to its username.
- Split Actions into a scheduled Supabase fetcher (`.github/workflows/update.yml`) and a Pages deployment workflow (`.github/workflows/pages.yml`).
- Updated `README.md` with setup instructions.
- Created Supabase project `ttcbdfmwsywdgjcwsllf` and successfully ran `supabase/schema.sql` in its SQL Editor.
- Configured `site/index.html` with the project's URL and publishable browser key.
- Set the Supabase Site URL to the GitHub Pages URL and added the Pages and `http://localhost:8000/` auth redirects.
- Email auth and new-user signups are enabled. The GitHub sign-in button stays hidden until GitHub OAuth credentials are configured.
- The local site connected to this Supabase project successfully; leaderboard and contest queries returned their expected empty states because no accounts or snapshots exist yet.
- Added the non-sensitive `SUPABASE_URL` and sensitive `SUPABASE_SERVICE_ROLE_KEY` as GitHub Actions secrets. Never expose the service-role key in chat or commit it.
- Pushed the Supabase browser configuration; the Pages workflow completed successfully and the production site returned the expected empty leaderboard/contest states.
- Manually ran the `update` workflow successfully; its Rust build completed and it populated upcoming contests. The production site now shows those contests; there are still no profiles or leaderboard snapshots.
- Existing `users.toml`, `data/snapshots/`, `site/leaderboard.json`, and `site/contests.json` have not been removed. Keep them until the Supabase flow is configured and verified.

## Current Blockers / Unverified

- Google OAuth is wired in the site but requires a Google OAuth client ID and secret in Supabase; email OTP has not been tested end to end. GitHub OAuth remains disabled pending its own OAuth credentials.
- No users have signed up or connected handles yet, so per-user snapshots and handle verification remain untested.
- The fetcher compiled and ran successfully in GitHub Actions. Cargo/Rust is unavailable in the local environment.
- Local HTTP preview is available at `http://localhost:8000/` in the current session; it uses a PowerShell server because Python is unavailable.

## Next Steps

1. Sign in with email OTP, create the first profile, and connect platform handles.
2. Run the `update` workflow manually again; verify platform fetches, verification updates, and per-user snapshot writes.
3. Verify own-profile/account RLS and leaderboard periods. Install/use Rust locally for `cargo fmt` if desired.
4. Configure a Google OAuth client and enable the Google provider in Supabase; configure GitHub OAuth separately if GitHub sign-in is wanted.
5. Only after successful verification, decide whether to import or remove the legacy TOML/JSON data.

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
