# 8. Exercises

Small, safe changes to build familiarity. After each one, run `cargo check`. It needs no network
or secrets and shows compiler errors. Undo any change with `git checkout -- <file>`.

## Warm-ups (read & predict)

1. In `platforms.rs`, what does `points()` return for 2 Codeforces problems rated 800 and 1500,
   plus 1 LeetCode Medium? *(Answer: 8 + 15 + 10 = 33.)*
2. In `fetch_users`, why is `stats` declared `let mut`? *(It's lent as `&mut stats` to the fetchers.)*
3. Why does `fetch_users` take `&Supabase` instead of `Supabase`? *(So `main` keeps ownership and
   can pass it to `fetch_contests` afterwards. See chapter 3.)*

## Break it on purpose (learn the compiler errors)

4. In `fetch_users`, move the `stats,` line **above** `points: stats.points(),` inside
   `SnapshotRow { … }`. Run `cargo check` and read the "borrow of moved value" error. Put it back.
5. Remove `mut` from `let mut stats`. Read the "cannot borrow as mutable" error. Put it back.
6. Delete one `?` after `.send()` in `cf_get`. The compiler explains the type mismatch:
   you'd have a `Result` where it expected a response.

## Small features

7. **Print a summary.** At the end of `fetch_users`, print how many snapshot rows were written:
   `println!("wrote {} snapshots", rows.len());`. Put it before the final `api.post(...)` line.
   `api.post` only borrows `rows` (`&rows`), but it's the function's last expression, so add a
   `let result = api.post(...); println!(...); result` or print first.
8. **Count contests.** In `fetch_contests`, print how many contests were found per site
   (`found.len()` inside each `Ok(found)` arm).
9. **Change scoring.** Make LeetCode Hard worth 25. Edit `points()`, **and** the SQL
   `leaderboard()` function (`supabase/schema.sql`, which recomputes LC points), so both agree.

## Bigger: add a platform (AtCoder)

10. Outline:
    - `Stats`: add `pub ac_rating: Option<i64>` and `pub ac_solved: u32`. Thanks to
      `#[derive(Default)]`, old snapshots without these fields still load. Add `#[serde(default)]`
      above the fields to be safe.
    - `platforms.rs`: write `pub fn fetch_atcoder(c: &Client, user: &str, s: &mut Stats) -> Result<()>`
      using `https://atcoder.jp/users/<user>/history/json` (a list of contests with `NewRating`).
    - `main.rs`: add `"atcoder" => platforms::fetch_atcoder(client, &account.handle, &mut stats),`
      to the `match`, and an "unlinked → reset" block like the other two.
    - Database: allow `'atcoder'` in the `accounts.platform` check constraint.
    - Website: add an AtCoder input to the profile form.

## Tools worth running

```bash
rustup component add clippy rustfmt
cargo clippy   # suggestions like "this .clone() is unnecessary"
cargo fmt      # formats everything the standard way
```
