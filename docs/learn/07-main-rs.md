# 7. Walkthrough: `src/main.rs`

**Job:** run once, refresh everything in Supabase, exit. Open the file alongside; it's commented.

## Shape of the file

```
mod platforms;  use ...;           ← imports
struct Supabase + impl Supabase    ← tiny REST client for Supabase
struct Profile / Account / ...     ← shapes of rows we read and write
fn previous_stats(...)             ← last snapshot's stats for one person
fn fetch_users(...)                ← refresh people → insert snapshots
fn fetch_contests(...)             ← refresh contests → upsert
fn main()                          ← entry point: wire it all together
```

## `main()`: the whole program in 6 lines

```rust
fn main() -> Result<()> {
    let api = Supabase::from_env()?;              // read secrets from env vars
    let client = reqwest::blocking::Client::builder().user_agent("cp-leaderboard").build()?;
    fetch_users(&api, &client)?;
    fetch_contests(&api, &client)?;
    Ok(())
}
```

Every `?` means "stop and report if this failed". If any step fails, the GitHub Actions run turns red.

## The `Supabase` helper

Supabase exposes every table over HTTP ("PostgREST"). Filters go in the URL:

| URL | SQL equivalent |
|-----|----------------|
| `profiles?select=id,name&order=name.asc` | `SELECT id, name FROM profiles ORDER BY name` |
| `snapshots?select=stats&user_id=eq.X&order=taken_at.desc&limit=1` | `… WHERE user_id = 'X' ORDER BY taken_at DESC LIMIT 1` |
| `accounts?user_id=eq.X&platform=eq.Y` (PATCH) | `UPDATE accounts SET … WHERE user_id='X' AND platform='Y'` |
| `contests?start=lt.T` (DELETE) | `DELETE FROM contests WHERE start < T` |

Methods: `get` (read and parse into any type `T`), `post` (insert), `patch` (update),
`delete_expired_contests`. `authorize` adds the **service-role key**, which bypasses row-level
security. That's why this key lives only in GitHub Actions secrets, never in the website.

## Row structs

```rust
#[derive(Deserialize)] struct Profile { id: String, name: String }       // read
#[derive(Serialize)]   struct SnapshotRow { user_id, points, solved, … } // write
```

`Deserialize` means "can be built from JSON" (reading); `Serialize` means "can be turned into JSON"
(writing). Field names must match column names.

## `fetch_users`, step by step

1. Load all `profiles` and all `accounts`.
2. Group accounts by user into a `BTreeMap<user_id, Vec<Account>>`.
3. For each profile:
   1. Start from their **previous** stats (`previous_stats`), so a failed fetch keeps old numbers
      instead of dropping them to zero.
   2. If they **unlinked** a platform, zero that platform's fields.
   3. For each linked account, `match` on the platform and call the fetcher with `&mut stats`.
      - success → PATCH `verified = true` on that account,
      - failure → log with `eprintln!` and keep going.
   4. Build a `SnapshotRow` (computing `points()` / `solved()` before moving `stats` in).
4. Insert all rows in one POST.

## `fetch_contests`

Get both sites' contests (logging, not failing, if one site is down), convert unix seconds to
dates, and **upsert**: `on_conflict=platform,url` + `resolution=merge-duplicates` updates
existing rows instead of erroring. Then delete contests that started over a day ago.

## Environment variables

| Variable | Where it comes from |
|----------|---------------------|
| `SUPABASE_URL` | GitHub secret, or `export SUPABASE_URL=...` locally |
| `SUPABASE_SERVICE_ROLE_KEY` | GitHub secret only. Never commit it or put it in the site. |

To run locally:

```bash
export SUPABASE_URL="https://<project>.supabase.co"
export SUPABASE_SERVICE_ROLE_KEY="<from Supabase → Settings → API>"
cargo run
```

Running it writes real snapshots to your database, just like the scheduled job.
