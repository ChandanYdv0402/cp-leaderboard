# Project Context for Coding Assistants

Read this file before making project changes. Keep it current so the next assistant can continue without reconstructing the work.

## Project

CP Leaderboard is a static GitHub Pages site for Codeforces and LeetCode standings and upcoming contests. Supabase is being introduced for authentication, user profiles, linked handles, snapshots, and contests. GitHub Actions runs the Rust collector every six hours.

## Current Progress

- Pushed groups to `main` in merge commit `dbc80d5`, preserving the newer Clerk, themes, registration restriction, and dashboard features. Pages run `40214439859` succeeded; production returned HTTP 200 with group navigation, Clerk, closed registration, and the duel arena present. The hosted groups migration remains pending.
- Added invitation-only groups and merged with the newer `main` dashboard: create/join flows, owner invitations with rotation, removal, leaving/deletion, remembered per-user dashboard selection, and a separate global rankings option. Group selection also scopes ranks, progress charts, duels, and heatmaps; platform filters apply to group totals. Existing profile editing is collapsed; new profiles open for setup.
- Added group tables, member-only RLS, owner-checked RPCs, and `group_leaderboard` to `supabase/schema.sql` and `supabase/migrations/202610250001_groups.sql`. Group ids are UUIDs; user ids are Clerk text JWT subjects. Existing projects need the Clerk and per-platform migrations first. The collector is unchanged. Invitations persist across sign-in tabs until joined or dismissed; closed registration remains enforced.
- Added development-only PGlite/Node checks (`npm ci && npm test`) for actual PostgreSQL RLS/RPCs and browser flows. All 15 checks passed on the merged tree using the Node runtime bundled with VS Code and headless Edge with mocked Clerk/Supabase/Chart.js boundaries. Hosted migration and real-account group flows remain unverified.
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

- Switched sign-in from Supabase Auth (Google/email OTP) to Clerk via Supabase third-party auth: `site/index.html` loads clerk-js from jsDelivr, passes the Clerk session token to supabase-js through `accessToken`, and uses Clerk's sign-in modal. `supabase/schema.sql` now uses text user ids and `auth.jwt() ->> 'sub'` in RLS; `supabase/clerk_migration.sql` converts an existing project. The fetcher needed no change (service-role key, string ids).

- Added Platform (All/LeetCode/Codeforces) and Sort by (Score/Questions solved/Rating) dropdowns. `leaderboard(days)` now also returns `cf_points`, `cf_solved`, `lc_points`, `lc_solved` derived from snapshot `stats` JSON; `supabase/leaderboard_by_platform.sql` drops and recreates it for existing projects. Until it is run, per-platform values show as "–".

- Added RPG party sprites (branch `feat/rank-badges`): `site/index.html` composes a 24x32 pixel character per tier from layered parts (`BODY`, `GEAR`, `SPRITES`), e.g. LC Knight = plumed helm + sword + shield, CF Candidate Master = wizard. Rank cards show both platform characters with XP bars; the legend, table avatars and chart labels use the tier character (portrait crop). Standings bars are segmented like XP bars; Progress lines are stepped with a portrait on each player's latest point. Verified locally only.
- Design tokens: every notched pixel frame uses `box-shadow:var(--px-frame)` with its color in `--px-c`; inset areas (chart, calendar, rank cards) share the `.screen` class. The hero shows the top-tier characters (Legendary GM, Guardian); the top three table ranks get gold/silver/bronze.
- Added the Duel Arena (full-width panel under the hero): pick two players for mirrored characters with HP (share of period points) and XP bars, a VS verdict, tug-of-war stat rounds, generated taunts, and a two-player progress chart. `headOn()` flags the closest pair within max(3, 10%) period points or 30 rating; it drives a blinking HEAD-ON alert strip under the top bar and the arena's default pairing. Key `d` jumps to the arena. Verified locally (alert checked with a temporarily loosened threshold).
- Added all 21 8bitcn/ui themes (dark palettes from its `lib/themes.ts`) as `:root[data-theme="…"]` token blocks; Default keeps the original grayscale. The top-bar Theme select (or key `t`) switches, saves to `localStorage` `cp-theme`, and an inline `<head>` script applies it before paint. Surfaces use `--sel`/`--track`/`--track-2` tokens instead of hex; chart code reads colors through `cssColor()`, which converts oklch/color-mix to rgba for Chart.js. Tier, platform and sprite colors stay fixed across themes.

- Added beginner-oriented comments throughout `src/main.rs`, `src/platforms.rs` and `Cargo.toml` (comments only; a comment-stripped diff against HEAD shows identical code, `cargo check` not run), plus a Rust learning guide in `docs/learn/` (big picture, Rust basics, ownership, Option/Result, Cargo, per-file walkthroughs, exercises). Keep these in sync when the Rust code changes.

- Added two panels under Upcoming contests in `site/index.html`: **Next contest** (per-second countdown over the already-loaded `contestsData`, red digits in the last hour, rolls to the next contest) and **Solve heatmap** (26 weeks × 7 days per player; daily solved = sum of increases between consecutive snapshots, counted on the later snapshot's day; follows the Platform select; grids scroll to the newest weeks on narrow screens). Verified locally on real data (Chandan's 1 solve on 19 Oct), at phone width, and with a non-default theme. Uncommitted.

- Registration is closed: `REGISTRATION_OPEN = false` in `site/index.html` makes the email flow reject unknown emails ("Registration is closed") instead of signing them up, and the dialog says so. `supabase/close_registration.sql` (run manually in the SQL Editor) limits the profile insert policy to already-existing profiles, so new Clerk users can't join even via Google/GitHub; it keeps the policy because the site's upsert checks INSERT policies. Clerk's own sign-up must also be set to Restricted in the dashboard. The file includes reopen steps.

- Layout now mirrors 8bitcn: symmetric header (brand | status + next sync centered | theme + account) and footer, and a 1:2:1 grid — left: Next contest, Contest calendar, Upcoming contests (two-column list); center: Leaderboard, Ranks; right: Rank legend (its own panel now), Solve heatmap (18 weeks to fit). Two columns below 1280px (center spans the top), one below 760px. Type scaled up again (~1.3x small pixel text; hero and chart fonts too) with roomier panel padding; phones get compact table headers. Verified at 1920, 1280 and 375 widths with no horizontal scroll or console errors.

## Current Blockers / Unverified

- The groups migration has not been applied to hosted Supabase. Apply `supabase/migrations/202610250001_groups.sql` once after the Clerk/per-platform prerequisites; fresh projects use the full schema instead. Until then, the UI falls back to global rankings. Verify group access, invitations, and owner actions with separate real accounts after rollout.
- Supabase Auth (Google OAuth / email OTP) is no longer used by the site; login providers are now configured in Clerk. Clerk sign-in, token acceptance by Supabase, and RLS writes with Clerk ids are untested.
- No users have signed up or connected handles yet, so per-user snapshots and handle verification remain untested.
- The fetcher compiled and ran successfully in GitHub Actions. Cargo/Rust is unavailable in the local environment.
- Local HTTP preview is available at `http://localhost:8000/` in the current session; it uses a PowerShell server because Python is unavailable.

## Next Steps

1. Verify Clerk's Supabase integration, third-party auth, and the existing Clerk/per-platform database migrations. Clerk sign-in end to end remains untested.
2. Apply the groups migration once, then verify invitations, outsider isolation, rotation/removal, leaving/deletion, and group/global standings with separate real accounts. Invitations do not reopen registration.
3. Sign in as an existing Clerk player, save a profile/handles, and run the `update` workflow; verify platform fetches, handle verification, and per-user snapshots.
4. Verify own-profile/account RLS and leaderboard periods. Install/use Rust locally for `cargo fmt` if desired.
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
