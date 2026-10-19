// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 The Survivor Planner contributors

//! Loading the survivoR dataset (bundled snapshot, local cache, or fresh download)
//! and indexing it into seasons and players.

use serde::Deserialize;
use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;

/// Raw JSON exports from https://github.com/daviseford/survivoR/tree/master/dev/json
pub const FILES: [&str; 4] = ["castaways", "castaway_details", "season_summary", "episodes"];
pub const BASE_URL: &str = "https://raw.githubusercontent.com/daviseford/survivoR/master/dev/json";

/// The four JSON documents the app needs, as text.
#[derive(Clone)]
pub struct RawFiles {
    pub castaways: String,
    pub castaway_details: String,
    pub season_summary: String,
    pub episodes: String,
}

impl RawFiles {
    pub fn bundled() -> Self {
        Self {
            castaways: include_str!("../data/castaways.json").to_owned(),
            castaway_details: include_str!("../data/castaway_details.json").to_owned(),
            season_summary: include_str!("../data/season_summary.json").to_owned(),
            episodes: include_str!("../data/episodes.json").to_owned(),
        }
    }

    fn get(&self, name: &str) -> &str {
        match name {
            "castaways" => &self.castaways,
            "castaway_details" => &self.castaway_details,
            "season_summary" => &self.season_summary,
            _ => &self.episodes,
        }
    }

    fn from_map(mut m: HashMap<String, String>) -> Option<Self> {
        Some(Self {
            castaways: m.remove("castaways")?,
            castaway_details: m.remove("castaway_details")?,
            season_summary: m.remove("season_summary")?,
            episodes: m.remove("episodes")?,
        })
    }
}

// ---------- raw rows (every field optional so upstream schema drift doesn't break us) ----------

#[derive(Deserialize)]
struct RawCastaway {
    version_season: Option<String>,
    castaway_id: Option<String>,
    castaway: Option<String>,
    full_name: Option<String>,
}

#[derive(Deserialize)]
struct RawDetail {
    castaway_id: Option<String>,
    castaway: Option<String>,
    full_name: Option<String>,
}

#[derive(Deserialize)]
struct RawSeason {
    version: Option<String>,
    version_season: Option<String>,
    season_name: Option<String>,
    season: Option<f64>,
    location: Option<String>,
    country: Option<String>,
    tribe_setup: Option<String>,
    description: Option<String>,
    n_cast: Option<f64>,
}

#[derive(Deserialize)]
struct RawEpisode {
    version_season: Option<String>,
    episode_date: Option<String>,
    episode_length: Option<f64>,
}

// ---------- indexed model ----------

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct Season {
    /// e.g. "US20"
    pub id: String,
    /// e.g. "US"
    pub version: String,
    pub number: u32,
    /// e.g. "Survivor: Heroes vs. Villains"
    pub name: String,
    pub location: String,
    pub country: String,
    pub tribe_setup: String,
    pub description: String,
    pub n_cast: u32,
    /// First episode air date (YYYY-MM-DD), used to order seasons across franchises.
    pub premiere: Option<String>,
    pub episodes: u32,
    pub minutes: u32,
    /// castaway_ids of everyone who played this season
    pub cast: Vec<String>,
}

impl Season {
    /// Name without the "Survivor: " / "Survivor Australia: " prefix.
    pub fn short_name(&self) -> &str {
        match self.name.find(": ") {
            Some(p) if self.name.starts_with("Survivor") => &self.name[p + 2..],
            _ => &self.name,
        }
    }

}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct Player {
    pub id: String,
    pub name: String,
    pub full_name: String,
    /// Indices into `Dataset::seasons`, chronological.
    pub seasons: Vec<usize>,
}

pub struct Dataset {
    /// Sorted chronologically by premiere date, so an index comparison means "aired before".
    pub seasons: Vec<Season>,
    pub index: HashMap<String, usize>,
    pub players: HashMap<String, Player>,
    /// Franchise codes in display order (US first).
    pub versions: Vec<String>,
}

fn parse<T: for<'de> Deserialize<'de>>(name: &str, text: &str) -> Result<Vec<T>, String> {
    serde_json::from_str(text).map_err(|e| format!("{name}.json: {e}"))
}

impl Dataset {
    pub fn from_raw(raw: &RawFiles) -> Result<Self, String> {
        let castaways: Vec<RawCastaway> = parse("castaways", &raw.castaways)?;
        let details: Vec<RawDetail> = parse("castaway_details", &raw.castaway_details)?;
        let summaries: Vec<RawSeason> = parse("season_summary", &raw.season_summary)?;
        let episodes: Vec<RawEpisode> = parse("episodes", &raw.episodes)?;

        // Episode stats per season.
        let mut ep_stats: HashMap<String, (Option<String>, u32, u32)> = HashMap::new();
        for e in episodes {
            let Some(vs) = e.version_season else { continue };
            let entry = ep_stats.entry(vs).or_insert((None, 0, 0));
            if let Some(d) = e.episode_date.filter(|d| d.len() >= 10) {
                if entry.0.as_ref().is_none_or(|cur| d < *cur) {
                    entry.0 = Some(d);
                }
            }
            entry.1 += 1;
            entry.2 += e.episode_length.unwrap_or(0.0).max(0.0) as u32;
        }

        let mut seasons: Vec<Season> = summaries
            .into_iter()
            .filter_map(|s| {
                let id = s.version_season?;
                let (premiere, eps, mins) = ep_stats.get(&id).cloned().unwrap_or((None, 0, 0));
                Some(Season {
                    version: s.version.unwrap_or_else(|| id.chars().take(2).collect()),
                    number: s.season.unwrap_or(0.0) as u32,
                    name: s.season_name.unwrap_or_else(|| id.clone()),
                    location: s.location.unwrap_or_default(),
                    country: s.country.unwrap_or_default(),
                    tribe_setup: s.tribe_setup.unwrap_or_default(),
                    description: s.description.unwrap_or_default(),
                    n_cast: s.n_cast.unwrap_or(0.0) as u32,
                    premiere,
                    episodes: eps,
                    minutes: mins,
                    cast: Vec::new(),
                    id,
                })
            })
            .collect();

        // Chronological. Seasons with no air date go last within their franchise order.
        seasons.sort_by(|a, b| {
            let ka = a.premiere.as_deref().unwrap_or("9999");
            let kb = b.premiere.as_deref().unwrap_or("9999");
            ka.cmp(kb).then(a.version.cmp(&b.version)).then(a.number.cmp(&b.number))
        });
        let index: HashMap<String, usize> =
            seasons.iter().enumerate().map(|(i, s)| (s.id.clone(), i)).collect();

        // Names from castaway_details (canonical), filled in from castaways if missing.
        let mut players: HashMap<String, Player> = HashMap::new();
        for d in details {
            let Some(id) = d.castaway_id else { continue };
            let full = d.full_name.unwrap_or_default();
            players.insert(
                id.clone(),
                Player {
                    name: d.castaway.unwrap_or_else(|| full.clone()),
                    full_name: full,
                    seasons: Vec::new(),
                    id,
                },
            );
        }
        for c in castaways {
            let (Some(id), Some(vs)) = (c.castaway_id, c.version_season) else { continue };
            let Some(&si) = index.get(&vs) else { continue };
            let p = players.entry(id.clone()).or_insert_with(|| {
                let full = c.full_name.clone().unwrap_or_else(|| id.clone());
                Player {
                    name: c.castaway.clone().unwrap_or_else(|| full.clone()),
                    full_name: full,
                    seasons: Vec::new(),
                    id: id.clone(),
                }
            });
            if !p.seasons.contains(&si) {
                p.seasons.push(si);
            }
            if !seasons[si].cast.contains(&id) {
                seasons[si].cast.push(id);
            }
        }
        for p in players.values_mut() {
            p.seasons.sort_unstable();
        }
        players.retain(|_, p| !p.seasons.is_empty());
        for s in &mut seasons {
            s.cast.sort_by(|a, b| players[a].name.cmp(&players[b].name));
        }

        if seasons.is_empty() {
            return Err("dataset contains no seasons".into());
        }

        let mut versions: Vec<String> = seasons.iter().map(|s| s.version.clone()).collect();
        versions.sort_by_key(|v| (version_rank(v), v.clone()));
        versions.dedup();

        Ok(Self { seasons, index, players, versions })
    }

    pub fn player_name<'a>(&'a self, id: &'a str) -> &'a str {
        self.players.get(id).map(|p| p.name.as_str()).unwrap_or(id)
    }

    /// Number of players in season `i` who had played before it aired.
    pub fn returnee_count(&self, i: usize) -> usize {
        self.seasons[i]
            .cast
            .iter()
            .filter(|p| self.players[*p].seasons.first().is_some_and(|&f| f < i))
            .count()
    }
}

fn version_rank(v: &str) -> u8 {
    match v {
        "US" => 0,
        "AU" => 1,
        "NZ" => 2,
        "SA" => 3,
        "UK" => 4,
        _ => 9,
    }
}

pub fn version_label(v: &str) -> &str {
    match v {
        "US" => "United States",
        "AU" => "Australia",
        "NZ" => "New Zealand",
        "SA" => "South Africa",
        "UK" => "United Kingdom",
        other => other,
    }
}

// ---------- storage locations ----------

pub fn app_dir() -> PathBuf {
    dirs::data_dir().unwrap_or_else(|| PathBuf::from(".")).join("SurvivorPlanner")
}

fn cache_dir() -> PathBuf {
    app_dir().join("data")
}

pub enum Source {
    Bundled,
    Cached { fetched: String },
}

impl Source {
    pub fn describe(&self) -> String {
        match self {
            Source::Bundled => "bundled snapshot".into(),
            Source::Cached { fetched } => format!("downloaded {fetched}"),
        }
    }
}

/// Use the downloaded cache if it's complete and parses, otherwise the bundled snapshot.
pub fn load() -> (Dataset, Source) {
    if let Some(raw) = read_cache() {
        if let Ok(ds) = Dataset::from_raw(&raw) {
            let fetched = std::fs::read_to_string(cache_dir().join("fetched.txt"))
                .unwrap_or_else(|_| "earlier".into());
            return (ds, Source::Cached { fetched: fetched.trim().to_owned() });
        }
    }
    let ds = Dataset::from_raw(&RawFiles::bundled()).expect("bundled dataset is valid");
    (ds, Source::Bundled)
}

fn read_cache() -> Option<RawFiles> {
    let dir = cache_dir();
    let mut m = HashMap::new();
    for f in FILES {
        m.insert(f.to_owned(), std::fs::read_to_string(dir.join(format!("{f}.json"))).ok()?);
    }
    RawFiles::from_map(m)
}

/// Download the latest JSON from GitHub, validate it, and write it to the cache.
/// Blocking — call from a background thread.
pub fn refresh() -> Result<(Dataset, Source), String> {
    let agent = ureq::AgentBuilder::new().timeout(std::time::Duration::from_secs(60)).build();
    let mut m = HashMap::new();
    for f in FILES {
        let url = format!("{BASE_URL}/{f}.json");
        let text = agent
            .get(&url)
            .call()
            .map_err(|e| format!("{f}.json: {e}"))?
            .into_string()
            .map_err(|e| format!("{f}.json: {e}"))?;
        m.insert(f.to_owned(), text);
    }
    let raw = RawFiles::from_map(m).ok_or("missing file")?;
    let ds = Dataset::from_raw(&raw)?;

    let dir = cache_dir();
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    for f in FILES {
        std::fs::write(dir.join(format!("{f}.json")), raw.get(f)).map_err(|e| e.to_string())?;
    }
    let stamp = today();
    let _ = std::fs::write(dir.join("fetched.txt"), &stamp);
    Ok((ds, Source::Cached { fetched: stamp }))
}

/// Remove the downloaded cache so the bundled snapshot is used again.
pub fn clear_cache() {
    let _ = std::fs::remove_dir_all(cache_dir());
}

/// YYYY-MM-DD in UTC without pulling in a date crate.
fn today() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let days = (secs / 86_400) as i64;
    // Civil-from-days (Howard Hinnant).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + if m <= 2 { 1 } else { 0 };
    format!("{y:04}-{m:02}-{d:02}")
}

/// Seasons grouped by franchise, each group chronological.
pub fn by_version(ds: &Dataset) -> BTreeMap<usize, (String, Vec<usize>)> {
    let mut out = BTreeMap::new();
    for (rank, v) in ds.versions.iter().enumerate() {
        let idx: Vec<usize> =
            (0..ds.seasons.len()).filter(|&i| ds.seasons[i].version == *v).collect();
        out.insert(rank, (v.clone(), idx));
    }
    out
}
