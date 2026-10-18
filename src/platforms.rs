//! Codeforces (official API) and LeetCode (unofficial GraphQL) fetchers.

use anyhow::{bail, Result};
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{collections::HashSet, thread, time::Duration};

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct Stats {
    pub cf_rating: Option<i64>,
    pub cf_solved: u32,
    pub cf_points: u32,
    pub lc_rating: Option<f64>,
    pub lc_easy: u32,
    pub lc_medium: u32,
    pub lc_hard: u32,
}

impl Stats {
    /// Scoring: CF problem = rating/100, LC easy/medium/hard = 5/10/20.
    pub fn points(&self) -> u32 {
        self.cf_points + self.lc_easy * 5 + self.lc_medium * 10 + self.lc_hard * 20
    }

    pub fn solved(&self) -> u32 {
        self.cf_solved + self.lc_easy + self.lc_medium + self.lc_hard
    }
}

#[derive(Serialize)]
pub struct Contest {
    pub platform: &'static str,
    pub name: String,
    pub start: i64,
    pub duration: i64,
    pub url: String,
}

// ---------- Codeforces ----------

fn cf_get(c: &Client, path: &str) -> Result<Value> {
    // Codeforces allows ~1 request per 2 seconds.
    thread::sleep(Duration::from_millis(2100));
    let v: Value = c.get(format!("https://codeforces.com/api/{path}")).send()?.json()?;
    if v["status"] != "OK" {
        bail!("codeforces {path}: {}", v["comment"]);
    }
    Ok(v["result"].clone())
}

pub fn fetch_codeforces(c: &Client, handle: &str, s: &mut Stats) -> Result<()> {
    let info = cf_get(c, &format!("user.info?handles={handle}"))?;
    s.cf_rating = info[0]["rating"].as_i64();

    let subs = cf_get(c, &format!("user.status?handle={handle}"))?;
    let mut seen = HashSet::new();
    s.cf_solved = 0;
    s.cf_points = 0;
    for sub in subs.as_array().into_iter().flatten() {
        if sub["verdict"] != "OK" {
            continue;
        }
        let p = &sub["problem"];
        if seen.insert(format!("{}-{}-{}", p["contestId"], p["index"], p["name"])) {
            s.cf_solved += 1;
            s.cf_points += (p["rating"].as_u64().unwrap_or(800) / 100) as u32;
        }
    }
    Ok(())
}

pub fn codeforces_contests(c: &Client) -> Result<Vec<Contest>> {
    let list = cf_get(c, "contest.list?gym=false")?;
    Ok(list
        .as_array()
        .into_iter()
        .flatten()
        .filter(|x| x["phase"] == "BEFORE")
        .map(|x| Contest {
            platform: "codeforces",
            name: x["name"].as_str().unwrap_or_default().to_string(),
            start: x["startTimeSeconds"].as_i64().unwrap_or_default(),
            duration: x["durationSeconds"].as_i64().unwrap_or_default(),
            url: format!("https://codeforces.com/contests/{}", x["id"]),
        })
        .collect())
}

// ---------- LeetCode ----------

fn lc_query(c: &Client, query: &str, variables: Value) -> Result<Value> {
    let v: Value = c
        .post("https://leetcode.com/graphql")
        .header("Referer", "https://leetcode.com")
        .json(&json!({ "query": query, "variables": variables }))
        .send()?
        .json()?;
    if v["data"].is_null() {
        bail!("leetcode: {}", v["errors"]);
    }
    Ok(v["data"].clone())
}

pub fn fetch_leetcode(c: &Client, user: &str, s: &mut Stats) -> Result<()> {
    let q = r#"query($u: String!) {
        matchedUser(username: $u) { submitStatsGlobal { acSubmissionNum { difficulty count } } }
        userContestRanking(username: $u) { rating }
    }"#;
    let d = lc_query(c, q, json!({ "u": user }))?;
    if d["matchedUser"].is_null() {
        bail!("leetcode user {user} not found");
    }
    s.lc_rating = d["userContestRanking"]["rating"].as_f64();
    for row in d["matchedUser"]["submitStatsGlobal"]["acSubmissionNum"]
        .as_array()
        .into_iter()
        .flatten()
    {
        let n = row["count"].as_u64().unwrap_or(0) as u32;
        match row["difficulty"].as_str() {
            Some("Easy") => s.lc_easy = n,
            Some("Medium") => s.lc_medium = n,
            Some("Hard") => s.lc_hard = n,
            _ => {}
        }
    }
    Ok(())
}

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
