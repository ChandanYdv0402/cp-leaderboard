//! Codeforces (official API) and LeetCode (unofficial GraphQL) fetchers.
//!
//! This module only talks to the two coding sites. It knows nothing about Supabase:
//! `main.rs` calls these functions and stores whatever they return.
//!
//! Items marked `pub` are public (usable from `main.rs`); everything else is private to this file.

use anyhow::{bail, Result}; // `bail!("...")` = return an error immediately (like `throw new Error`)
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
// `Value` = "any JSON value". Handy when we only need a few fields from a big, loosely-typed response.
use serde_json::{json, Value};
use std::{collections::HashSet, thread, time::Duration};

/// One person's numbers on both sites. Stored as JSON in each snapshot's `stats` column.
///
/// Derived traits:
///   - `Serialize, Deserialize`: convert to/from JSON
///   - `Clone`: allow making a full copy with `.clone()`
///   - `Default`: `Stats::default()` gives all zeros / `None` (used for brand-new users)
#[derive(Serialize, Deserialize, Clone, Default)]
pub struct Stats {
    // `Option` because someone may never have entered a rated contest.
    pub cf_rating: Option<i64>,
    pub cf_solved: u32,
    pub cf_points: u32,
    // `f64` = 64-bit decimal number; LeetCode ratings come back as decimals like 1963.4.
    pub lc_rating: Option<f64>,
    pub lc_easy: u32,
    pub lc_medium: u32,
    pub lc_hard: u32,
}

impl Stats {
    /// Scoring: CF problem = rating/100, LC easy/medium/hard = 5/10/20.
    ///
    /// `&self` = read the struct without changing it. `-> u32` = returns an unsigned integer.
    pub fn points(&self) -> u32 {
        self.cf_points + self.lc_easy * 5 + self.lc_medium * 10 + self.lc_hard * 20
    }

    /// Total problems solved across both sites.
    pub fn solved(&self) -> u32 {
        self.cf_solved + self.lc_easy + self.lc_medium + self.lc_hard
    }
}

/// An upcoming contest from either site, in a common shape.
#[derive(Serialize)]
pub struct Contest {
    pub platform: &'static str, // "codeforces" or "leetcode" (fixed text, so `&'static str`)
    pub name: String,
    pub start: i64,    // unix timestamp in seconds
    pub duration: i64, // length in seconds
    pub url: String,
}

// ---------- Codeforces ----------
// Docs: https://codeforces.com/apiHelp. Every response looks like
// { "status": "OK", "result": ... } or { "status": "FAILED", "comment": "why" }.

/// GET a Codeforces API method and return its `result` part.
fn cf_get(c: &Client, path: &str) -> Result<Value> {
    // Codeforces allows ~1 request per 2 seconds.
    // `thread::sleep` pauses the program; that's fine here because nothing else runs in parallel.
    thread::sleep(Duration::from_millis(2100));
    // `.send()?` does the request, `.json()?` parses the body. Each `?` returns early on failure.
    let v: Value = c.get(format!("https://codeforces.com/api/{path}")).send()?.json()?;
    // Indexing a `Value` with ["key"] never panics; a missing key just gives `Value::Null`.
    if v["status"] != "OK" {
        bail!("codeforces {path}: {}", v["comment"]);
    }
    Ok(v["result"].clone())
}

/// Fill in `s` (the person's stats) with their Codeforces rating, solved count and points.
///
/// `s: &mut Stats` = a *mutable borrow*: we may change the caller's `Stats` in place,
/// and the caller sees the changes. Rust allows only one mutable borrow at a time,
/// which prevents two pieces of code from editing the same data at once.
pub fn fetch_codeforces(c: &Client, handle: &str, s: &mut Stats) -> Result<()> {
    let info = cf_get(c, &format!("user.info?handles={handle}"))?;
    // `.as_i64()` returns `Option<i64>`: `Some(rating)`, or `None` if the user is unrated.
    s.cf_rating = info[0]["rating"].as_i64();

    // Every submission the user ever made (can be thousands).
    let subs = cf_get(c, &format!("user.status?handle={handle}"))?;
    // A set of problem keys we've already counted, so solving the same problem twice counts once.
    let mut seen = HashSet::new();
    s.cf_solved = 0;
    s.cf_points = 0;
    // `.as_array()` gives `Option<&Vec<Value>>`; `.into_iter().flatten()` loops over the items,
    // or over nothing if the result wasn't an array. A common Rust trick to skip `if let` nesting.
    for sub in subs.as_array().into_iter().flatten() {
        if sub["verdict"] != "OK" {
            continue; // only accepted submissions count
        }
        let p = &sub["problem"];
        // `HashSet::insert` returns `true` only if the key wasn't there yet.
        if seen.insert(format!("{}-{}-{}", p["contestId"], p["index"], p["name"])) {
            s.cf_solved += 1;
            // Unrated problems count as 800. `as u32` converts the number type.
            s.cf_points += (p["rating"].as_u64().unwrap_or(800) / 100) as u32;
        }
    }
    Ok(())
}

/// Codeforces contests that haven't started yet (`phase == "BEFORE"`).
pub fn codeforces_contests(c: &Client) -> Result<Vec<Contest>> {
    let list = cf_get(c, "contest.list?gym=false")?;
    // An *iterator chain*, like JS `array.filter(...).map(...)`:
    //   .filter(|x| ...) keeps items where the closure returns true
    //   .map(|x| ...)    turns each item into a `Contest`
    //   .collect()       gathers the results into a `Vec<Contest>` (the return type tells it which collection)
    Ok(list
        .as_array()
        .into_iter()
        .flatten()
        .filter(|x| x["phase"] == "BEFORE")
        .map(|x| Contest {
            platform: "codeforces",
            // `.as_str()` -> Option<&str>; `.unwrap_or_default()` -> "" if missing;
            // `.to_string()` turns the borrowed `&str` into an owned `String`.
            name: x["name"].as_str().unwrap_or_default().to_string(),
            start: x["startTimeSeconds"].as_i64().unwrap_or_default(),
            duration: x["durationSeconds"].as_i64().unwrap_or_default(),
            url: format!("https://codeforces.com/contests/{}", x["id"]),
        })
        .collect())
}

// ---------- LeetCode ----------
// LeetCode has no official API. The website itself uses a GraphQL endpoint, so we call that.
// It may change without notice; if LeetCode numbers break, look here first.

/// Run a GraphQL query against LeetCode and return its `data` part.
fn lc_query(c: &Client, query: &str, variables: Value) -> Result<Value> {
    let v: Value = c
        .post("https://leetcode.com/graphql")
        .header("Referer", "https://leetcode.com") // LeetCode rejects requests without it
        // `json!({ ... })` builds a JSON object inline; `.json(...)` sends it as the request body.
        .json(&json!({ "query": query, "variables": variables }))
        .send()?
        .json()?;
    if v["data"].is_null() {
        bail!("leetcode: {}", v["errors"]);
    }
    Ok(v["data"].clone())
}

/// Fill in `s` with the person's LeetCode contest rating and solved counts per difficulty.
pub fn fetch_leetcode(c: &Client, user: &str, s: &mut Stats) -> Result<()> {
    // `r#"..."#` is a *raw string*: backslashes and quotes inside are taken literally,
    // which makes multi-line queries easy to write.
    let q = r#"query($u: String!) {
        matchedUser(username: $u) { submitStatsGlobal { acSubmissionNum { difficulty count } } }
        userContestRanking(username: $u) { rating }
    }"#;
    let d = lc_query(c, q, json!({ "u": user }))?;
    if d["matchedUser"].is_null() {
        bail!("leetcode user {user} not found");
    }
    s.lc_rating = d["userContestRanking"]["rating"].as_f64();
    // `acSubmissionNum` looks like [{difficulty:"All",count:686}, {difficulty:"Easy",count:126}, ...]
    for row in d["matchedUser"]["submitStatsGlobal"]["acSubmissionNum"]
        .as_array()
        .into_iter()
        .flatten()
    {
        let n = row["count"].as_u64().unwrap_or(0) as u32;
        // Matching on `Option<&str>`: `Some("Easy")` only matches when the value is exactly "Easy".
        match row["difficulty"].as_str() {
            Some("Easy") => s.lc_easy = n,
            Some("Medium") => s.lc_medium = n,
            Some("Hard") => s.lc_hard = n,
            _ => {} // "All" or anything else: ignore (`{}` = do nothing)
        }
    }
    Ok(())
}

/// LeetCode's upcoming weekly/biweekly contests.
pub fn leetcode_contests(c: &Client) -> Result<Vec<Contest>> {
    let d = lc_query(c, "{ upcomingContests { title titleSlug startTime duration } }", json!({}))?;
    Ok(d["upcomingContests"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|x| Contest {
            platform: "leetcode",
            name: x["title"].as_str().unwrap_or_default().to_string(),
            start: x["startTime"].as_i64().unwrap_or_default(),
            duration: x["duration"].as_i64().unwrap_or_default(),
            url: format!("https://leetcode.com/contest/{}", x["titleSlug"].as_str().unwrap_or_default()),
        })
        .collect())
}
