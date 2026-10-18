//! Fetches every user's stats, saves a snapshot, and writes the static site's JSON.
//! Run by GitHub Actions on a schedule; also runs locally with `cargo run`.

mod platforms;

use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
use platforms::Stats;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{collections::BTreeMap, fs};

const SNAPSHOT_DIR: &str = "data/snapshots";
const SITE_DIR: &str = "site";

#[derive(Deserialize)]
struct Config {
    users: Vec<User>,
}

#[derive(Deserialize)]
struct User {
    name: String,
    codeforces: Option<String>,
    leetcode: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct Snapshot {
    taken_at: DateTime<Utc>,
    users: BTreeMap<String, Stats>,
}

fn load_snapshots() -> Result<Vec<Snapshot>> {
    let mut snaps = Vec::new();
    for entry in fs::read_dir(SNAPSHOT_DIR)? {
        let path = entry?.path();
        if path.extension().is_some_and(|e| e == "json") {
            snaps.push(serde_json::from_str(&fs::read_to_string(path)?)?);
        }
    }
    snaps.sort_by_key(|s: &Snapshot| s.taken_at);
    Ok(snaps)
}

fn take_snapshot(cfg: &Config, prev: Option<&Snapshot>) -> Snapshot {
    let client = reqwest::blocking::Client::builder()
        .user_agent("cp-leaderboard")
        .build()
        .expect("http client");
    let mut users = BTreeMap::new();
    for u in &cfg.users {
        // Start from the previous stats so a failed fetch doesn't zero anyone out.
        let mut s = prev.and_then(|p| p.users.get(&u.name)).cloned().unwrap_or_default();
        if let Some(h) = &u.codeforces {
            if let Err(e) = platforms::fetch_codeforces(&client, h, &mut s) {
                eprintln!("[{}] codeforces: {e}", u.name);
            }
        }
        if let Some(h) = &u.leetcode {
            if let Err(e) = platforms::fetch_leetcode(&client, h, &mut s) {
                eprintln!("[{}] leetcode: {e}", u.name);
            }
        }
        println!("{}: {} solved, {} pts", u.name, s.solved(), s.points());
        users.insert(u.name.clone(), s);
    }
    Snapshot { taken_at: Utc::now(), users }
}

/// Leaderboard for a window = latest snapshot minus the last snapshot taken
/// at or before the window start (or the earliest one, if history is shorter).
fn leaderboard(snaps: &[Snapshot], days: Option<i64>) -> serde_json::Value {
    let cur = snaps.last().expect("at least one snapshot");
    let base = days.map(|d| {
        let start = cur.taken_at - Duration::days(d);
        snaps.iter().rev().find(|s| s.taken_at <= start).unwrap_or(&snaps[0])
    });
    let mut rows: Vec<_> = cur
        .users
        .iter()
        .map(|(name, s)| {
            let b = base.and_then(|b| b.users.get(name));
            let (bp, bs) = b.map_or((0, 0), |b| (b.points(), b.solved()));
            json!({
                "name": name,
                "points": s.points().saturating_sub(bp),
                "solved": s.solved().saturating_sub(bs),
                "cf_rating": s.cf_rating,
                "lc_rating": s.lc_rating.map(|r| r.round()),
            })
        })
        .collect();
    rows.sort_by_key(|r| std::cmp::Reverse(r["points"].as_u64()));
    json!(rows)
}

fn main() -> Result<()> {
    let cfg: Config = toml::from_str(&fs::read_to_string("users.toml")?)?;
    fs::create_dir_all(SNAPSHOT_DIR)?;
    fs::create_dir_all(SITE_DIR)?;

    let mut snaps = load_snapshots()?;
    let snap = take_snapshot(&cfg, snaps.last());
    let file = format!("{SNAPSHOT_DIR}/{}.json", snap.taken_at.format("%Y-%m-%dT%H-%M"));
    fs::write(file, serde_json::to_string_pretty(&snap)?)?;
    snaps.push(snap);

    let board = json!({
        "updated_at": snaps.last().unwrap().taken_at,
        "periods": {
            "week": leaderboard(&snaps, Some(7)),
            "biweek": leaderboard(&snaps, Some(14)),
            "month": leaderboard(&snaps, Some(30)),
            "all": leaderboard(&snaps, None),
        }
    });
    fs::write(format!("{SITE_DIR}/leaderboard.json"), serde_json::to_string_pretty(&board)?)?;

    let client = reqwest::blocking::Client::builder().user_agent("cp-leaderboard").build()?;
    let mut contests = Vec::new();
    match platforms::codeforces_contests(&client) {
        Ok(c) => contests.extend(c),
        Err(e) => eprintln!("codeforces contests: {e}"),
    }
    match platforms::leetcode_contests(&client) {
        Ok(c) => contests.extend(c),
        Err(e) => eprintln!("leetcode contests: {e}"),
    }
    contests.sort_by_key(|c| c.start);
    fs::write(format!("{SITE_DIR}/contests.json"), serde_json::to_string_pretty(&contests)?)?;
    Ok(())
}
