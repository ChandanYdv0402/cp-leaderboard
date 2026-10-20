# 1. The big picture

```
            every 6 hours
GitHub Actions ───────────────► Rust program (src/)
 (.github/workflows/update.yml)      │
                                     │ 1. read profiles + handles ─────────┐
                                     │ 2. ask Codeforces & LeetCode         │
                                     │ 3. write snapshots + contests ──────►│  Supabase (Postgres)
                                                                            │   profiles, accounts,
Website (site/index.html on GitHub Pages) ◄── reads leaderboard ───────────┤   snapshots, contests,
   people sign in (Clerk) and save handles ── writes profile/accounts ─────►┘   leaderboard() function
```

## The parts

| Part | Where | Language | Job |
|------|-------|----------|-----|
| **Fetcher** | `src/main.rs`, `src/platforms.rs` | Rust | Every 6h: get fresh stats, store a snapshot per person, store upcoming contests |
| **Database** | Supabase (`supabase/*.sql`) | SQL | Stores everything; `leaderboard(days)` computes standings; row-level security (RLS) decides who may write what |
| **Website** | `site/index.html` | HTML/CSS/JS | Shows leaderboard, charts, ranks, calendar; sign-in; profile form |
| **Sign-in** | Clerk (external service) | — | Logs people in; Supabase trusts Clerk's token |
| **Automation** | `.github/workflows/` | YAML | `update.yml` runs the fetcher on a schedule; `pages.yml` publishes the site |

## Why Rust for the fetcher?

It's a small, fast program that runs, does its job and exits. Rust is a good fit because the
compiler catches most mistakes (wrong types, missing error handling, forgotten `null` checks)
*before* the program ever runs. That matters for something that runs unattended every 6 hours.

## The data flow, step by step

1. **A person signs in** on the website and saves their name + handles.
   Rows go into `profiles` and `accounts` (with `verified = false`).
2. **The fetcher runs** (`cargo run --release` inside GitHub Actions):
   - `main()` reads `SUPABASE_URL` and `SUPABASE_SERVICE_ROLE_KEY` from the environment
     (they're GitHub secrets).
   - `fetch_users()` loads all profiles and accounts and asks each site for stats via
     `platforms.rs`. It marks working handles `verified = true` and inserts one `snapshots` row
     per person.
   - `fetch_contests()` loads upcoming contests from both sites and upserts them into `contests`.
3. **The website** calls `leaderboard(7)` (or 14, 30, all) in Postgres. That compares the newest
   snapshot with an older one to get "points gained this week".

## Key idea: snapshots

Neither site can say "how many problems did you solve this week?". So every 6 hours we save a
**snapshot** of each person's *totals*. Weekly points = latest snapshot − snapshot from 7 days ago.
