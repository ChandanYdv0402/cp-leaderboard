# 6. Walkthrough: `src/platforms.rs`

**Job:** talk to Codeforces and LeetCode, and return plain Rust data. It knows nothing about
Supabase. Open the file alongside this page; it's commented line by line.

## What it exports (`pub`)

| Item | Kind | Purpose |
|------|------|---------|
| `Stats` | struct | One person's numbers on both sites |
| `Stats::points()` | method | Score: CF rating/100 per problem + LC 5/10/20 |
| `Stats::solved()` | method | Total problems solved |
| `Contest` | struct | An upcoming contest from either site |
| `fetch_codeforces(client, handle, &mut stats)` | fn | Fill in `cf_rating`, `cf_solved`, `cf_points` |
| `fetch_leetcode(client, user, &mut stats)` | fn | Fill in `lc_rating`, `lc_easy/medium/hard` |
| `codeforces_contests(client)` | fn | Upcoming Codeforces contests |
| `leetcode_contests(client)` | fn | Upcoming LeetCode contests |

The private helpers `cf_get` and `lc_query` handle the HTTP and error checking for each site.

## `Stats`

```rust
#[derive(Serialize, Deserialize, Clone, Default)]
pub struct Stats {
    pub cf_rating: Option<i64>,
    pub cf_solved: u32,
    pub cf_points: u32,
    pub lc_rating: Option<f64>,
    pub lc_easy: u32, pub lc_medium: u32, pub lc_hard: u32,
}
```

This exact struct is stored as JSON in each snapshot's `stats` column. The website and the SQL
`leaderboard()` function read these field names (`cf_points`, `lc_easy`, …), so **renaming a field
breaks them**.

## Codeforces flow

```
fetch_codeforces(handle)
 ├─ cf_get("user.info?handles=…")    → rating
 └─ cf_get("user.status?handle=…")   → every submission
      for each submission with verdict == "OK":
         key = contestId-index-name
         if key not seen before: solved += 1, points += rating/100 (800 if unrated)
```

`cf_get` sleeps 2.1 seconds before **every** call because Codeforces allows about one request per
2 seconds. So each Codeforces user adds ~4 seconds to a run.

### Working with untyped JSON: `serde_json::Value`

Codeforces responses are big, and we need only a few fields. Instead of writing a struct for
everything, we parse into `Value` (any JSON) and index into it:

```rust
let p = &sub["problem"];
p["rating"].as_u64()      // Option<u64>
sub["verdict"] != "OK"    // Value can be compared with a string
```

Indexing a missing key gives `Value::Null` instead of crashing, and `.as_u64()` / `.as_str()` turn
that into `None`. `main.rs` does the opposite with typed structs (`Profile`, `Account`), which is
better when you control the data shape.

## LeetCode flow

LeetCode has no official API. We send the same GraphQL query its website sends:

```graphql
query($u: String!) {
  matchedUser(username: $u) { submitStatsGlobal { acSubmissionNum { difficulty count } } }
  userContestRanking(username: $u) { rating }
}
```

`acSubmissionNum` returns counts for "All", "Easy", "Medium", "Hard"; a `match` picks the three we
store. **If LeetCode changes this endpoint, this is the function to fix.**

## Contests

Both `*_contests` functions turn the site's list into `Vec<Contest>` with an iterator chain
(`filter` → `map` → `collect`). Times are unix seconds; `main.rs` converts them to real dates.

## Try it

- Change scoring: edit `points()`. Then also update the SQL `leaderboard()` function, which
  recomputes LeetCode points itself, so both match.
- Add AtCoder: add fields to `Stats`, a `fetch_atcoder` function, and a `"atcoder"` arm in the
  `match` inside `main.rs`. See [08-exercises.md](08-exercises.md).
