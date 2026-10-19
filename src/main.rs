//! Fetches user stats and writes snapshots and contests to Supabase.
//! Run by GitHub Actions on a schedule; also runs locally with `cargo run`.

mod platforms;

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use platforms::{Contest, Stats};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{collections::BTreeMap, env};

struct Supabase {
    url: String,
    service_key: String,
    client: reqwest::blocking::Client,
}

impl Supabase {
    fn from_env() -> Result<Self> {
        Ok(Self {
            url: env::var("SUPABASE_URL").context("SUPABASE_URL is not set")?,
            service_key: env::var("SUPABASE_SERVICE_ROLE_KEY")
                .context("SUPABASE_SERVICE_ROLE_KEY is not set")?,
            client: reqwest::blocking::Client::builder()
                .user_agent("cp-leaderboard")
                .build()?,
        })
    }

    fn endpoint(&self, resource: &str) -> String {
        format!("{}/rest/v1/{resource}", self.url.trim_end_matches('/'))
    }

    fn authorize(&self, request: reqwest::blocking::RequestBuilder) -> reqwest::blocking::RequestBuilder {
        request
            .header("apikey", &self.service_key)
            .bearer_auth(&self.service_key)
    }

    fn get<T: serde::de::DeserializeOwned>(&self, resource: &str) -> Result<T> {
        Ok(self
            .authorize(self.client.get(self.endpoint(resource)))
            .send()?
            .error_for_status()?
            .json()?)
    }

    fn post<T: Serialize>(&self, resource: &str, body: &T, prefer: &str) -> Result<()> {
        self.authorize(self.client.post(self.endpoint(resource)))
            .header("Prefer", prefer)
            .json(body)
            .send()?
            .error_for_status()?;
        Ok(())
    }

    fn patch<T: Serialize>(&self, resource: &str, body: &T) -> Result<()> {
        self.authorize(self.client.patch(self.endpoint(resource)))
            .header("Prefer", "return=minimal")
            .json(body)
            .send()?
            .error_for_status()?;
        Ok(())
    }

    fn delete_expired_contests(&self) -> Result<()> {
        let cutoff = (Utc::now() - chrono::Duration::days(1)).to_rfc3339();
        self.authorize(self.client.delete(self.endpoint("contests")))
            .query(&[("start", format!("lt.{cutoff}"))])
            .send()?
            .error_for_status()?;
        Ok(())
    }
}

#[derive(Deserialize)]
struct Profile {
    id: String,
    name: String,
}

#[derive(Deserialize)]
struct Account {
    user_id: String,
    platform: String,
    handle: String,
}

#[derive(Deserialize)]
struct StoredSnapshot {
    stats: Stats,
}

#[derive(Serialize)]
struct SnapshotRow {
    user_id: String,
    points: u32,
    solved: u32,
    cf_rating: Option<i64>,
    lc_rating: Option<i32>,
    stats: Stats,
}

#[derive(Serialize)]
struct ContestRow {
    platform: &'static str,
    name: String,
    start: DateTime<Utc>,
    duration: i64,
    url: String,
}

fn previous_stats(api: &Supabase, user_id: &str) -> Result<Stats> {
    let rows: Vec<StoredSnapshot> = api.get(&format!(
        "snapshots?select=stats&user_id=eq.{user_id}&order=taken_at.desc&limit=1"
    ))?;
    Ok(rows.into_iter().next().map_or_else(Stats::default, |row| row.stats))
}

fn fetch_users(api: &Supabase, client: &reqwest::blocking::Client) -> Result<()> {
    let profiles: Vec<Profile> = api.get("profiles?select=id,name&order=name.asc")?;
    let accounts: Vec<Account> = api.get("accounts?select=user_id,platform,handle")?;
    let mut by_user: BTreeMap<String, Vec<Account>> = BTreeMap::new();
    for account in accounts {
        by_user.entry(account.user_id.clone()).or_default().push(account);
    }

    let mut rows = Vec::with_capacity(profiles.len());
    for profile in &profiles {
        let mut stats = previous_stats(api, &profile.id)?;
        let user_accounts = by_user.get(&profile.id).map(Vec::as_slice).unwrap_or_default();
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
            let result = match account.platform.as_str() {
                "codeforces" => platforms::fetch_codeforces(client, &account.handle, &mut stats),
                "leetcode" => platforms::fetch_leetcode(client, &account.handle, &mut stats),
                _ => continue,
            };
            match result {
                Ok(()) => api.patch(
                    &format!(
                        "accounts?user_id=eq.{}&platform=eq.{}",
                        profile.id, account.platform
                    ),
                    &json!({ "verified": true }),
                )?,
                Err(error) => eprintln!("[{}] {}: {error}", profile.name, account.platform),
            }
        }

        println!("{}: {} solved, {} pts", profile.name, stats.solved(), stats.points());
        rows.push(SnapshotRow {
            user_id: profile.id.clone(),
            points: stats.points(),
            solved: stats.solved(),
            cf_rating: stats.cf_rating,
            lc_rating: stats.lc_rating.map(|rating| rating.round() as i32),
            stats,
        });
    }
    api.post("snapshots", &rows, "return=minimal")
}

fn fetch_contests(api: &Supabase, client: &reqwest::blocking::Client) -> Result<()> {
    let mut contests: Vec<Contest> = Vec::new();
    match platforms::codeforces_contests(client) {
        Ok(found) => contests.extend(found),
        Err(error) => eprintln!("codeforces contests: {error}"),
    }
    match platforms::leetcode_contests(client) {
        Ok(found) => contests.extend(found),
        Err(error) => eprintln!("leetcode contests: {error}"),
    }

    let rows: Result<Vec<ContestRow>> = contests
        .into_iter()
        .map(|contest| {
            Ok(ContestRow {
                platform: contest.platform,
                name: contest.name,
                start: DateTime::from_timestamp(contest.start, 0)
                    .context("contest has invalid start timestamp")?,
                duration: contest.duration,
                url: contest.url,
            })
        })
        .collect();
    api.post(
        "contests?on_conflict=platform,url",
        &rows?,
        "resolution=merge-duplicates,return=minimal",
    )?;
    api.delete_expired_contests()
}

fn main() -> Result<()> {
    let api = Supabase::from_env()?;
    let client = reqwest::blocking::Client::builder()
        .user_agent("cp-leaderboard")
        .build()?;
    fetch_users(&api, &client)?;
    fetch_contests(&api, &client)?;
    Ok(())
}
