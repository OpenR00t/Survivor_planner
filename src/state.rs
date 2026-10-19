//! Watched seasons and plan targets, saved as JSON in the app data folder.

use crate::planner::PlanOptions;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashSet};
use std::path::PathBuf;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct State {
    /// version_season ids, e.g. "US20"
    pub watched: BTreeSet<String>,
    /// Seasons you want to watch with full context, in the order added.
    pub targets: Vec<String>,
    pub options: PlanOptions,
    pub selected: Option<String>,
    pub collapsed_versions: BTreeSet<String>,
    pub hide_watched_in_plan: bool,
    pub returnees_only: bool,
}

impl State {
    fn path() -> PathBuf {
        crate::data::app_dir().join("state.json")
    }

    pub fn load() -> Self {
        std::fs::read_to_string(Self::path())
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_else(|| State { returnees_only: true, ..Default::default() })
    }

    pub fn save(&self) -> Result<(), String> {
        let p = Self::path();
        if let Some(dir) = p.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        let text = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        // Write-then-rename so a crash mid-save can't wipe the watch list.
        let tmp = p.with_extension("json.tmp");
        std::fs::write(&tmp, text).map_err(|e| e.to_string())?;
        std::fs::rename(&tmp, &p).map_err(|e| e.to_string())
    }

    pub fn watched_set(&self) -> HashSet<String> {
        self.watched.iter().cloned().collect()
    }

    pub fn toggle_watched(&mut self, id: &str) {
        if !self.watched.remove(id) {
            self.watched.insert(id.to_owned());
        }
    }

    pub fn is_target(&self, id: &str) -> bool {
        self.targets.iter().any(|t| t == id)
    }
}
