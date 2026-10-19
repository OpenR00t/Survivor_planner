//! Working out which earlier seasons give context for returning players.

use crate::data::Dataset;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashSet, VecDeque};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Coverage {
    /// Every earlier season each returnee played.
    #[default]
    AllPrior,
    /// Only each returnee's most recent earlier season.
    MostRecent,
    /// Only each returnee's debut season.
    Debut,
}

impl Coverage {
    pub const ALL: [Coverage; 3] = [Coverage::AllPrior, Coverage::MostRecent, Coverage::Debut];

    pub fn label(self) -> &'static str {
        match self {
            Coverage::AllPrior => "Every previous appearance",
            Coverage::MostRecent => "Most recent appearance only",
            Coverage::Debut => "Debut season only",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PlanOptions {
    pub coverage: Coverage,
    /// Follow the chain: also pull in context for the prerequisite seasons themselves.
    pub recursive: bool,
    /// When recursing, treat a season you've already seen as understood and don't expand it.
    pub stop_at_watched: bool,
    /// Ignore prerequisite seasons that contribute fewer than this many returnees.
    pub min_players: usize,
}

impl Default for PlanOptions {
    fn default() -> Self {
        Self { coverage: Coverage::AllPrior, recursive: true, stop_at_watched: true, min_players: 1 }
    }
}

/// One season in a watch plan.
#[derive(Clone, Debug)]
pub struct Step {
    pub season: usize,
    /// 0 = a target you chose, 1 = direct context, 2+ = context for context.
    pub depth: u32,
    /// (player id, season they return in) pairs that make this season relevant.
    pub links: Vec<(String, usize)>,
}

impl Step {
    pub fn is_target(&self) -> bool {
        self.depth == 0
    }

    pub fn player_ids(&self) -> Vec<&str> {
        let mut seen = BTreeSet::new();
        self.links.iter().map(|(p, _)| p.as_str()).filter(|p| seen.insert(*p)).collect()
    }

    pub fn needed_by(&self) -> BTreeSet<usize> {
        self.links.iter().map(|(_, s)| *s).collect()
    }
}

/// Earlier seasons of `player` (aired before season `before`), filtered by coverage.
pub fn prior_seasons(ds: &Dataset, player: &str, before: usize, cov: Coverage) -> Vec<usize> {
    let Some(p) = ds.players.get(player) else { return vec![] };
    let prior: Vec<usize> = p.seasons.iter().copied().filter(|&s| s < before).collect();
    match cov {
        Coverage::AllPrior => prior,
        Coverage::MostRecent => prior.last().copied().into_iter().collect(),
        Coverage::Debut => prior.first().copied().into_iter().collect(),
    }
}

/// Build a chronological watch plan for one or more target seasons.
/// The result includes the targets themselves (depth 0) and watched seasons (so the UI can show them).
pub fn plan(ds: &Dataset, targets: &[usize], opts: &PlanOptions, watched: &HashSet<String>) -> Vec<Step> {
    let mut steps: BTreeMap<usize, Step> = BTreeMap::new();
    let mut queue: VecDeque<usize> = VecDeque::new();

    for &t in targets {
        steps.insert(t, Step { season: t, depth: 0, links: vec![] });
        queue.push_back(t);
    }

    let mut expanded: HashSet<usize> = HashSet::new();
    while let Some(s) = queue.pop_front() {
        if !expanded.insert(s) {
            continue;
        }
        let depth = steps[&s].depth;

        // Group this season's returnees by the earlier season they link to.
        let mut found: BTreeMap<usize, Vec<String>> = BTreeMap::new();
        for pid in &ds.seasons[s].cast {
            for q in prior_seasons(ds, pid, s, opts.coverage) {
                found.entry(q).or_default().push(pid.clone());
            }
        }

        for (q, players) in found {
            if players.len() < opts.min_players.max(1) {
                continue;
            }
            let step = steps.entry(q).or_insert(Step { season: q, depth: depth + 1, links: vec![] });
            if step.depth != 0 {
                step.depth = step.depth.min(depth + 1);
            }
            for p in players {
                step.links.push((p, s));
            }

            let seen = watched.contains(&ds.seasons[q].id);
            if opts.recursive && !(seen && opts.stop_at_watched) {
                queue.push_back(q);
            }
        }
    }

    steps.into_values().collect()
}

/// Direct, non-recursive context for one season: how many earlier seasons are relevant and how many are unwatched.
pub fn readiness(ds: &Dataset, season: usize, watched: &HashSet<String>) -> (usize, usize) {
    let opts = PlanOptions { recursive: false, ..PlanOptions::default() };
    let steps = plan(ds, &[season], &opts, watched);
    let pre: Vec<&Step> = steps.iter().filter(|s| !s.is_target()).collect();
    let missing = pre.iter().filter(|s| !watched.contains(&ds.seasons[s.season].id)).count();
    (pre.len(), missing)
}

pub struct Totals {
    pub seasons: usize,
    pub episodes: u32,
    pub minutes: u32,
}

/// What's left to watch in a plan.
pub fn remaining(ds: &Dataset, steps: &[Step], watched: &HashSet<String>) -> Totals {
    let mut t = Totals { seasons: 0, episodes: 0, minutes: 0 };
    for st in steps {
        let s = &ds.seasons[st.season];
        if !watched.contains(&s.id) {
            t.seasons += 1;
            t.episodes += s.episodes;
            t.minutes += s.minutes;
        }
    }
    t
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::{Dataset, RawFiles};

    fn ds() -> Dataset {
        Dataset::from_raw(&RawFiles::bundled()).unwrap()
    }

    fn ids(ds: &Dataset, steps: &[Step]) -> Vec<String> {
        steps.iter().map(|s| ds.seasons[s.season].id.clone()).collect()
    }

    #[test]
    fn print_hvv() {
        let ds = ds();
        let hvv = ds.index["US20"];
        for (label, recursive) in [("DIRECT", false), ("FULL", true)] {
            let steps = plan(&ds, &[hvv], &PlanOptions { recursive, ..Default::default() }, &HashSet::new());
            println!("{label}: {} seasons", steps.len());
            for st in &steps {
                let names: Vec<&str> = st.player_ids().iter().map(|p| ds.player_name(p)).collect();
                println!("  d{} {} {:<28} {}", st.depth, ds.seasons[st.season].id, ds.seasons[st.season].short_name(), names.join(", "));
            }
        }
    }

    #[test]
    fn hvv_direct_context() {
        let ds = ds();
        let hvv = ds.index["US20"];
        let opts = PlanOptions { recursive: false, ..Default::default() };
        let steps = plan(&ds, &[hvv], &opts, &HashSet::new());
        let got = ids(&ds, &steps);
        // Every HvV castaway is a returnee; Pearl Islands (Rupert, Sandra), All-Stars,
        // Cook Islands and Micronesia must all be direct context.
        for must in ["US07", "US08", "US13", "US16", "US20"] {
            assert!(got.contains(&must.to_string()), "missing {must} in {got:?}");
        }
        assert!(got.iter().all(|id| ds.index[id] <= hvv));
        assert_eq!(ds.returnee_count(hvv), 20);
    }

    #[test]
    fn recursion_and_watched_cutoff() {
        let ds = ds();
        let hvv = ds.index["US20"];
        let full = plan(&ds, &[hvv], &PlanOptions::default(), &HashSet::new());
        let direct =
            plan(&ds, &[hvv], &PlanOptions { recursive: false, ..Default::default() }, &HashSet::new());
        assert!(full.len() >= direct.len());

        // Having watched All-Stars means we don't chase All-Stars' own context.
        let watched: HashSet<String> = ["US08".to_string()].into();
        let cut = plan(&ds, &[hvv], &PlanOptions::default(), &watched);
        assert!(cut.len() <= full.len());
    }

    #[test]
    fn debut_mode_is_subset() {
        let ds = ds();
        let wat = ds.index["US40"];
        let all = plan(&ds, &[wat], &PlanOptions { recursive: false, ..Default::default() }, &HashSet::new());
        let debut = plan(
            &ds,
            &[wat],
            &PlanOptions { recursive: false, coverage: Coverage::Debut, ..Default::default() },
            &HashSet::new(),
        );
        let a: HashSet<_> = ids(&ds, &all).into_iter().collect();
        assert!(ids(&ds, &debut).iter().all(|i| a.contains(i)));
    }

    #[test]
    fn cross_franchise_returnees() {
        let ds = ds();
        let avw = ds.index["AU11"];
        let steps = plan(&ds, &[avw], &PlanOptions { recursive: false, ..Default::default() }, &HashSet::new());
        assert!(ids(&ds, &steps).iter().any(|i| i.starts_with("US")), "AvW should pull in US seasons");
    }
}
