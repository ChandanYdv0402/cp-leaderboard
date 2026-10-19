# Plan: self sign-up with Supabase

Goal: anyone can open the site, log in, connect their Codeforces / LeetCode handles, and appear on the
leaderboard — no GitHub account, no PRs. Everything stays on free tiers.

## Architecture

```
Static site (GitHub Pages)  ──supabase-js──►  Supabase (free)
  login, "connect handles",                     ├─ Auth: GitHub / Google / email magic link
  leaderboard, contests                         ├─ Postgres: profiles, accounts, snapshots, contests
                                                ├─ RLS: a user can only edit their own rows
                                                └─ RPC: leaderboard(days)
GitHub Actions cron (every 6h)
  Rust fetcher  ──service-role key──►  reads accounts, writes snapshots + contests
```

What changes vs today:

| Today | After |
|---|---|
| `users.toml` (edited by PR) | `profiles` + `accounts` tables (edited by users on the site) |
| `data/snapshots/*.json` committed by bot | `snapshots` table |
| `site/leaderboard.json` | `leaderboard(days)` SQL function, queried live |
| `site/contests.json` | `contests` table |
| Bot commits every 6h | No commits; Actions only runs the fetcher |

## Database schema

```sql
-- One row per signed-up person (id = Supabase auth user id)
create table profiles (
  id          uuid primary key references auth.users on delete cascade,
  name        text not null unique check (char_length(name) between 2 and 32),
  created_at  timestamptz not null default now()
);

-- Linked platform handles
create table accounts (
  user_id   uuid not null references profiles on delete cascade,
  platform  text not null check (platform in ('codeforces', 'leetcode')),
  handle    text not null check (handle ~ '^[A-Za-z0-9_.-]{1,40}$'),
  verified  boolean not null default false,       -- set by fetcher once the handle resolves
  primary key (user_id, platform),
  unique (platform, handle)                       -- a handle can belong to only one person
);

-- Written only by the fetcher (service role)
create table snapshots (
  id         bigint generated always as identity primary key,
  user_id    uuid not null references profiles on delete cascade,
  taken_at   timestamptz not null default now(),
  points     int not null,
  solved     int not null,
  cf_rating  int,
  lc_rating  int,
  stats      jsonb not null                       -- full Stats struct for future use
);
create index on snapshots (user_id, taken_at);

create table contests (
  platform  text not null,
  name      text not null,
  start     timestamptz not null,
  duration  int not null,                         -- seconds
  url       text not null,
  primary key (platform, url)
);
```

### Row-level security

```sql
alter table profiles  enable row level security;
alter table accounts  enable row level security;
alter table snapshots enable row level security;
alter table contests  enable row level security;

-- Everyone (even logged out) can read the board
create policy "public read" on profiles  for select using (true);
create policy "public read" on accounts  for select using (true);
create policy "public read" on snapshots for select using (true);
create policy "public read" on contests  for select using (true);

-- Users manage only their own profile and handles
create policy "own profile" on profiles for insert with check (auth.uid() = id);
create policy "own profile" on profiles for update using (auth.uid() = id);
create policy "own accounts" on accounts for all
  using (auth.uid() = user_id) with check (auth.uid() = user_id);

-- No insert/update policies on snapshots/contests: only the service-role key (fetcher) can write.
```

### Leaderboard function

Same logic as the current Rust code: latest snapshot minus the last snapshot at or before the
window start (or the earliest one if history is shorter).

```sql
create or replace function leaderboard(days int default null)
returns table (name text, points int, solved int, cf_rating int, lc_rating int)
language sql stable as $$
  with cur as (
    select distinct on (user_id) * from snapshots order by user_id, taken_at desc
  ),
  base as (
    select distinct on (s.user_id) s.*
    from snapshots s
    where days is not null
      and s.taken_at <= coalesce(
            (select max(taken_at) from snapshots) - make_interval(days => days),
            '-infinity')
    order by s.user_id, s.taken_at desc
  ),
  first as (
    select distinct on (user_id) * from snapshots order by user_id, taken_at asc
  )
  select p.name,
         greatest(c.points - coalesce(b.points, case when days is null then 0 else f.points end), 0),
         greatest(c.solved - coalesce(b.solved, case when days is null then 0 else f.solved end), 0),
         c.cf_rating, c.lc_rating
  from cur c
  join profiles p on p.id = c.user_id
  left join base b  on b.user_id = c.user_id
  left join first f on f.user_id = c.user_id
  order by 2 desc;
$$;
```

Site calls: `supabase.rpc('leaderboard', { days: 7 })` (7 / 14 / 30 / null).

## Site changes (`site/index.html`, still static)

- Load `@supabase/supabase-js` from a CDN; configure with the **project URL + anon key** (both are
  safe to publish — RLS is what protects data).
- **Header:** "Log in" button → `supabase.auth.signInWithOAuth({ provider: 'github' })` (or Google /
  magic link). Shows the user's name + "Log out" when signed in.
- **"My profile" panel** (logged-in only): name, Codeforces handle, LeetCode username → `upsert`
  into `profiles` / `accounts`. Show a "pending verification" badge until `verified = true`.
- **Leaderboard tabs:** call `rpc('leaderboard')` instead of fetching `leaderboard.json`.
- **Contests:** `from('contests').select().gte('start', now).order('start')`.
- Optional: a "Validate" check in the browser before saving (call Codeforces `user.info` — it
  allows CORS; LeetCode does not, so LeetCode is verified only by the fetcher).

## Fetcher changes (Rust)

- Add `postgrest` crate (or plain `reqwest` against Supabase's REST API) — no direct DB connection needed.
- Env vars from GitHub Secrets: `SUPABASE_URL`, `SUPABASE_SERVICE_ROLE_KEY`.
- Flow per run:
  1. `GET /rest/v1/accounts?select=*` → group handles by `user_id`.
  2. Fetch stats exactly as now (`platforms.rs` unchanged).
  3. Mark `verified = true` for handles that resolved; leave others false (shown as pending on the site).
  4. `POST /rest/v1/snapshots` one row per user (`points`, `solved`, ratings, `stats` JSON).
  5. Upsert upcoming contests into `contests`; delete rows with `start < now() - 1 day`.
- Delete `users.toml`, `data/`, and the JSON-writing code in `main.rs`.
- Workflow: drop the "Commit snapshot" step and `contents: write`; Pages deploy only needs to run
  when `site/` changes (split into a separate `pages.yml`).

## Secrets & keys

| Key | Where | Public? |
|---|---|---|
| Project URL | `site/index.html`, GitHub Secret | yes |
| `anon` key | `site/index.html` | yes (RLS enforces access) |
| `service_role` key | **GitHub Secret only** | **never** — bypasses RLS |

## Migration of existing data

One-off: existing JSON snapshots can be dropped (only a few days of history), or imported with a
small script mapping `name → user_id` once the friends have signed up.

## Free-tier notes

- Supabase free: 500 MB DB, 50k monthly active users, 2 projects. Projects **pause after ~7 days
  without activity** — the 6-hourly fetcher keeps it active.
- Snapshot growth: ~4 rows/user/day → 100 users ≈ 150k rows/year, tiny. Can prune to one row per
  day after 30 days if ever needed.

## Implementation steps

1. **You:** create a Supabase project; enable GitHub (and/or Google) auth provider with
   redirect URL `https://chandanydv0402.github.io/cp-leaderboard/`.
2. Run the schema + RLS + `leaderboard()` SQL in the Supabase SQL editor (save it as
   `supabase/schema.sql` in the repo).
3. **You:** add `SUPABASE_URL` and `SUPABASE_SERVICE_ROLE_KEY` as GitHub repo secrets.
4. Update the Rust fetcher to read/write Supabase; test locally with a `.env` (git-ignored).
5. Update `site/index.html` with login, profile panel, and live leaderboard.
6. Split workflows (`update.yml` = fetcher cron, `pages.yml` = deploy site).
7. Friends sign up; remove `users.toml` and `data/`.

## Later

Rating charts from `snapshots`, streaks, Discord contest reminders (Supabase cron or Actions),
groups/private leaderboards (`groups`, `group_members` tables + RLS), more platforms (AtCoder,
CodeChef) via the same `accounts.platform` column.
