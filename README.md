# CP Leaderboard

Weekly / bi-weekly / monthly / all-time leaderboard for Codeforces + LeetCode, plus upcoming contests.
Runs entirely on GitHub: Actions fetches stats every 6h, commits a snapshot, and deploys `site/` to Pages.

## How it works
- `users.toml` — the list of users (add yourself via PR).
- `cargo run` — fetches stats, writes `data/snapshots/<time>.json`, `site/leaderboard.json`, `site/contests.json`.
- A period's score = latest snapshot − snapshot from the start of the period.
- Scoring: CF problem = rating/100 (unrated = 8), LC Easy/Medium/Hard = 5/10/20.

## Run locally
```
cargo run
cd site && python3 -m http.server 8000   # open http://localhost:8000
```

## Deploy
1. Push to a GitHub repo (branch `main`).
2. Settings → Pages → Source: **GitHub Actions**.
3. Actions → `update` → **Run workflow**.

## Next ideas
Rating charts, streaks, Discord contest reminders, groups, AtCoder/CodeChef.
