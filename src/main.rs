//! Fetches user stats and writes snapshots and contests to Supabase.
//! Run by GitHub Actions on a schedule; also runs locally with `cargo run`.
//!
//! (Lines starting with `//!` are "inner doc comments": they document the file/module itself.
//!  `///` documents the item right below it, and `//` is a normal comment.)
//!
//! Big picture of one run:
//!   1. `main()` reads Supabase credentials from environment variables.
//!   2. `fetch_users()` loads every profile + linked handle from Supabase, asks Codeforces/LeetCode
//!      for fresh stats, and inserts one snapshot row per person.
//!   3. `fetch_contests()` loads upcoming contests from both sites and upserts them into Supabase.

// `mod platforms;` tells the compiler "there is a module in src/platforms.rs, include it".
// Rust doesn't discover files automatically: every module must be declared like this.
mod platforms;

// `use` brings names into scope so we can write `Result` instead of `anyhow::Result`.
// `{A, B}` imports several names from the same path at once.
use anyhow::{Context, Result}; // anyhow = easy error handling; `Context` adds a message to an error
use chrono::{DateTime, Utc}; // chrono = dates and times
use platforms::{Contest, Stats}; // our own types from src/platforms.rs
use serde::{Deserialize, Serialize}; // serde = convert Rust structs <-> JSON
use serde_json::json; // `json!` macro: write JSON literals inside Rust
use std::{collections::BTreeMap, env}; // `std` is Rust's standard library

/// Everything needed to talk to Supabase's REST API.
///
/// A `struct` is like a class with only fields (no methods yet). Methods are added in `impl` below.
struct Supabase {
    // `String` is an owned, growable piece of text (heap-allocated).
    url: String,
    service_key: String,
    // An HTTP client. `reqwest::blocking` = each request waits until it finishes (no async/await needed).
    client: reqwest::blocking::Client,
}

// `impl Supabase { ... }` = the methods that belong to the `Supabase` struct.
impl Supabase {
    /// Build a `Supabase` from environment variables.
    ///
    /// Return type `Result<Self>`: either `Ok(Supabase)` on success or `Err(error)` on failure.
    /// `Self` is shorthand for the type we're implementing, here `Supabase`.
    /// Functions without `self` as the first parameter (like this one) are "associated functions",
    /// similar to static methods. You call them as `Supabase::from_env()`.
    fn from_env() -> Result<Self> {
        // The last expression in a function (no trailing `;`) is its return value.
        // So `Ok(Self { ... })` here is returned without writing `return`.
        Ok(Self {
            // `env::var` returns a Result. `.context("...")` swaps a vague error for a clear one.
            // The `?` operator: if the Result is an `Err`, return it from this function right away;
            // if it's `Ok(value)`, unwrap it and keep going. It's Rust's version of "throw on error".
            url: env::var("SUPABASE_URL").context("SUPABASE_URL is not set")?,
            service_key: env::var("SUPABASE_SERVICE_ROLE_KEY")
                .context("SUPABASE_SERVICE_ROLE_KEY is not set")?,
            // "Builder pattern": start a builder, chain settings, then `.build()`.
            client: reqwest::blocking::Client::builder()
                .user_agent("cp-leaderboard")
                .build()?,
        })
    }

    /// Full REST URL for a table/query, e.g. "snapshots?select=stats" -> "https://…/rest/v1/snapshots?select=stats".
    ///
    /// `&self` = this method borrows the struct read-only (it can't change it).
    /// `&str` = a borrowed view of some text (cheap; no copy). `String` = owned text.
    fn endpoint(&self, resource: &str) -> String {
        // `format!` builds a String, like a template literal. `{resource}` inserts a variable by name,
        // `{}` inserts the next argument. `trim_end_matches('/')` drops a trailing slash.
        format!("{}/rest/v1/{resource}", self.url.trim_end_matches('/'))
    }

    /// Add Supabase's auth headers to a request. The service-role key bypasses row-level security,
    /// which is why it only lives in GitHub Actions secrets, never in the website.
    fn authorize(&self, request: reqwest::blocking::RequestBuilder) -> reqwest::blocking::RequestBuilder {
        // Each `.header(...)` takes the builder and returns it with one more header (method chaining).
        request
            .header("apikey", &self.service_key)
            .bearer_auth(&self.service_key)
    }

    /// GET a resource and parse the JSON response into type `T`.
    ///
    /// `<T: serde::de::DeserializeOwned>` is a *generic* with a *trait bound*:
    /// "T can be any type, as long as serde knows how to build it from JSON".
    /// The caller picks T, e.g. `let rows: Vec<Profile> = api.get(...)?;`
    fn get<T: serde::de::DeserializeOwned>(&self, resource: &str) -> Result<T> {
        Ok(self
            .authorize(self.client.get(self.endpoint(resource)))
            .send()? // do the HTTP request (fails on network errors)
            .error_for_status()? // turn 4xx/5xx HTTP responses into errors
            .json()?) // parse the body as JSON into T
    }

    /// POST a JSON body. `prefer` is Supabase's `Prefer` header (e.g. "return=minimal" = don't echo rows back).
    /// `T: Serialize` = anything serde can turn into JSON.
    fn post<T: Serialize>(&self, resource: &str, body: &T, prefer: &str) -> Result<()> {
        self.authorize(self.client.post(self.endpoint(resource)))
            .header("Prefer", prefer)
            .json(body)
            .send()?
            .error_for_status()?;
        // `()` is the "unit" type: "nothing useful to return". `Ok(())` = success with no value.
        Ok(())
    }

    /// PATCH (partially update) rows matching the filters in `resource`.
    fn patch<T: Serialize>(&self, resource: &str, body: &T) -> Result<()> {
        self.authorize(self.client.patch(self.endpoint(resource)))
            .header("Prefer", "return=minimal")
            .json(body)
            .send()?
            .error_for_status()?;
        Ok(())
    }

    /// Remove contests that started more than a day ago, so the table only holds upcoming ones.
    fn delete_expired_contests(&self) -> Result<()> {
        // `let` declares a variable. Variables are immutable unless declared `let mut`.
        let cutoff = (Utc::now() - chrono::Duration::days(1)).to_rfc3339();
        self.authorize(self.client.delete(self.endpoint("contests")))
            // Supabase filter syntax: `start=lt.<time>` means "start is less than <time>".
            // `&[("a", b)]` is a borrowed array of (key, value) tuples -> becomes `?start=lt....`
            .query(&[("start", format!("lt.{cutoff}"))])
            .send()?
            .error_for_status()?;
        Ok(())
    }
}

// The structs below describe the *shape* of rows we read from / write to Supabase.
//
// `#[derive(...)]` is an attribute that auto-generates code for a struct:
//   - `Deserialize`: serde can build this struct from JSON (used when reading rows)
//   - `Serialize`:   serde can turn this struct into JSON (used when writing rows)
// Field names must match the JSON/column names exactly.

/// One row of the `profiles` table (only the columns we ask for).
#[derive(Deserialize)]
struct Profile {
    id: String,
    name: String,
}

/// One row of the `accounts` table: a person's handle on one platform.
#[derive(Deserialize)]
struct Account {
    user_id: String,
    platform: String,
    handle: String,
}

/// The only column we read back from an old snapshot: its full `stats` JSON.
#[derive(Deserialize)]
struct StoredSnapshot {
    stats: Stats,
}

/// One new row we insert into the `snapshots` table.
#[derive(Serialize)]
struct SnapshotRow {
    user_id: String,
    // `u32` = unsigned 32-bit integer (never negative). `i64` = signed 64-bit, `i32` = signed 32-bit.
    points: u32,
    solved: u32,
    // `Option<T>` = a value that may be missing: `Some(value)` or `None`. Rust has no `null`;
    // `Option` forces you to handle the "missing" case. In JSON, `None` becomes `null`.
    cf_rating: Option<i64>,
    lc_rating: Option<i32>,
    stats: Stats,
}

/// One row we upsert into the `contests` table.
#[derive(Serialize)]
struct ContestRow {
    // `&'static str` = text baked into the program (like "codeforces"), valid forever.
    // `'static` is a *lifetime*: it tells the compiler how long a borrowed value stays valid.
    platform: &'static str,
    name: String,
    start: DateTime<Utc>,
    duration: i64,
    url: String,
}

/// Stats from this person's most recent snapshot (or all zeros if they have none yet).
/// Used as a fallback so one failed fetch doesn't wipe their numbers.
fn previous_stats(api: &Supabase, user_id: &str) -> Result<Stats> {
    // The type annotation `Vec<StoredSnapshot>` tells `api.get` what to parse the JSON into.
    // `Vec<T>` = a growable list (like a JS array / Python list).
    let rows: Vec<StoredSnapshot> = api.get(&format!(
        "snapshots?select=stats&user_id=eq.{user_id}&order=taken_at.desc&limit=1"
    ))?;
    // Read this chain right to left:
    //   rows.into_iter().next()  -> the first row, as Option<StoredSnapshot> (None if no rows)
    //   .map_or_else(A, B)       -> if None, call A (Stats::default = all zeros);
    //                               if Some(row), call B with the row and return row.stats
    // `|row| row.stats` is a *closure* (an inline function, like `row => row.stats` in JS).
    Ok(rows.into_iter().next().map_or_else(Stats::default, |row| row.stats))
}

/// Refresh every person's stats and insert one snapshot row each.
///
/// Parameters are *borrowed* (`&Supabase`, `&Client`): the function can use them but doesn't take
/// ownership, so `main` can keep using them afterwards.
fn fetch_users(api: &Supabase, client: &reqwest::blocking::Client) -> Result<()> {
    let profiles: Vec<Profile> = api.get("profiles?select=id,name&order=name.asc")?;
    let accounts: Vec<Account> = api.get("accounts?select=user_id,platform,handle")?;

    // Group accounts by user id: { user_id -> [their accounts] }.
    // `BTreeMap` is a sorted dictionary. It's `mut` because we add to it in the loop.
    let mut by_user: BTreeMap<String, Vec<Account>> = BTreeMap::new();
    // `for x in collection` *moves* each item out of `accounts` (it can't be used after the loop).
    for account in accounts {
        // `.entry(key).or_default()` = "get the list for this key, creating an empty one if missing".
        // `.clone()` makes a copy of the String, because `account` itself is moved into the list.
        by_user.entry(account.user_id.clone()).or_default().push(account);
    }

    // Pre-size the list: we'll add exactly one row per profile.
    let mut rows = Vec::with_capacity(profiles.len());
    // `&profiles` loops over *references*, so `profiles` stays usable after the loop.
    for profile in &profiles {
        let mut stats = previous_stats(api, &profile.id)?;
        // This person's accounts as a slice (`&[Account]`), or an empty slice if they have none.
        // `.map(Vec::as_slice)` turns `Option<&Vec<Account>>` into `Option<&[Account]>`,
        // and `.unwrap_or_default()` turns `None` into an empty slice.
        let user_accounts = by_user.get(&profile.id).map(Vec::as_slice).unwrap_or_default();

        // If a platform was unlinked, clear its old numbers instead of carrying them forward.
        // `.iter().any(|a| ...)` = "is there at least one item where this is true?" (like JS `.some`).
        if !user_accounts.iter().any(|account| account.platform == "codeforces") {
            stats.cf_rating = None;
            stats.cf_solved = 0;
            stats.cf_points = 0;
        }
        if !user_accounts.iter().any(|account| account.platform == "leetcode") {
            stats.lc_rating = None;
            stats.lc_easy = 0;
            stats.lc_medium = 0;
            stats.lc_hard = 0;
        }

        for account in user_accounts {
            // `match` is Rust's powerful `switch`: compare a value against patterns, top to bottom.
            // `.as_str()` borrows the String as `&str` so it can be matched against string literals.
            // `&mut stats` lends `stats` *mutably* so the fetch function can fill in new numbers.
            let result = match account.platform.as_str() {
                "codeforces" => platforms::fetch_codeforces(client, &account.handle, &mut stats),
                "leetcode" => platforms::fetch_leetcode(client, &account.handle, &mut stats),
                _ => continue, // `_` matches anything else: skip unknown platforms
            };
            // `match` on a Result: handle success and failure separately.
            match result {
                // Success: mark this handle as verified so the site stops showing "pending".
                Ok(()) => api.patch(
                    &format!(
                        "accounts?user_id=eq.{}&platform=eq.{}",
                        profile.id, account.platform
                    ),
                    &json!({ "verified": true }),
                )?,
                // Failure: log it and move on; `stats` keeps the previous numbers for this platform.
                // `eprintln!` prints to stderr (shows up in the GitHub Actions log).
                Err(error) => eprintln!("[{}] {}: {error}", profile.name, account.platform),
            }
        }

        // `println!` prints to stdout. The `!` means it's a *macro*, not a normal function.
        println!("{}: {} solved, {} pts", profile.name, stats.solved(), stats.points());
        rows.push(SnapshotRow {
            user_id: profile.id.clone(),
            points: stats.points(),
            solved: stats.solved(),
            cf_rating: stats.cf_rating,
            // `.map(...)` on an Option transforms the value inside if it's `Some`, leaves `None` alone.
            // `as i32` converts the rounded f64 (decimal) rating into a whole number.
            lc_rating: stats.lc_rating.map(|rating| rating.round() as i32),
            // Field shorthand: `stats` is the same as `stats: stats`. This *moves* `stats` into the row.
            stats,
        });
    }
    // Insert all snapshot rows in a single request. No `;` -> this Result is the function's return value.
    api.post("snapshots", &rows, "return=minimal")
}

/// Load upcoming contests from both sites and upsert them into the `contests` table.
fn fetch_contests(api: &Supabase, client: &reqwest::blocking::Client) -> Result<()> {
    let mut contests: Vec<Contest> = Vec::new();
    // If one site fails, log it and still save the other site's contests.
    match platforms::codeforces_contests(client) {
        Ok(found) => contests.extend(found), // `extend` appends every item from `found`
        Err(error) => eprintln!("codeforces contests: {error}"),
    }
    match platforms::leetcode_contests(client) {
        Ok(found) => contests.extend(found),
        Err(error) => eprintln!("leetcode contests: {error}"),
    }

    // Convert each `Contest` into a `ContestRow` (unix seconds -> real date).
    // Iterator chain: `.into_iter()` -> `.map(convert)` -> `.collect()`.
    // Collecting an iterator of `Result`s into `Result<Vec<_>>` gives `Ok(list)` if every item
    // succeeded, or the first `Err` otherwise.
    let rows: Result<Vec<ContestRow>> = contests
        .into_iter()
        .map(|contest| {
            Ok(ContestRow {
                platform: contest.platform,
                name: contest.name,
                // `from_timestamp` returns an Option (None for impossible dates);
                // `.context(...)` turns that None into a readable error, and `?` returns it.
                start: DateTime::from_timestamp(contest.start, 0)
                    .context("contest has invalid start timestamp")?,
                duration: contest.duration,
                url: contest.url,
            })
        })
        .collect();
    // Upsert: `on_conflict=platform,url` + `merge-duplicates` updates contests that already exist
    // instead of failing on the duplicate key.
    api.post(
        "contests?on_conflict=platform,url",
        &rows?,
        "resolution=merge-duplicates,return=minimal",
    )?;
    api.delete_expired_contests()
}

/// Program entry point: `cargo run` starts here.
///
/// `main` may return a `Result`: if it returns `Err`, Rust prints the error and the process exits
/// with a failure code, which makes the GitHub Actions run show as failed.
fn main() -> Result<()> {
    let api = Supabase::from_env()?;
    // A separate HTTP client for the Codeforces/LeetCode requests.
    let client = reqwest::blocking::Client::builder()
        .user_agent("cp-leaderboard")
        .build()?;
    fetch_users(&api, &client)?;
    fetch_contests(&api, &client)?;
    Ok(())
}
