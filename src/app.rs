use crate::data::{self, Dataset, Source};
use crate::planner::{self, Coverage, PlanOptions, Step};
use crate::state::State;
use crate::theme::*;
use eframe::egui::{self, Align, Layout, RichText, ScrollArea, Ui};
use std::collections::HashSet;
use std::sync::mpsc::{self, Receiver};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Seasons,
    Plan,
    Players,
}

/// UI events collected during a frame and applied afterwards, which keeps borrows simple.
enum Action {
    ToggleWatched(String),
    SetWatched(Vec<String>, bool),
    Select(usize),
    AddTarget(String),
    RemoveTarget(String),
    ClearTargets,
    Tab(Tab),
    Refresh,
    UseBundled,
}

pub struct App {
    ds: Dataset,
    source: Source,
    state: State,
    tab: Tab,
    search: String,
    player_search: String,
    refresh_rx: Option<Receiver<Result<(Dataset, Source), String>>>,
    status: Option<(String, bool)>,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        apply(&cc.egui_ctx);
        let (ds, source) = data::load();
        let mut state = State::load();
        // Drop ids that no longer exist upstream.
        state.targets.retain(|t| ds.index.contains_key(t));
        if state.selected.as_ref().is_none_or(|s| !ds.index.contains_key(s)) {
            state.selected = ds.index.get("US20").map(|_| "US20".to_owned());
        }
        Self {
            ds,
            source,
            state,
            tab: Tab::Seasons,
            search: String::new(),
            player_search: String::new(),
            refresh_rx: None,
            status: None,
        }
    }

    fn apply(&mut self, actions: Vec<Action>, ctx: &egui::Context) {
        let mut dirty = false;
        for a in actions {
            match a {
                Action::ToggleWatched(id) => {
                    self.state.toggle_watched(&id);
                    dirty = true;
                }
                Action::SetWatched(ids, on) => {
                    for id in ids {
                        if on {
                            self.state.watched.insert(id);
                        } else {
                            self.state.watched.remove(&id);
                        }
                    }
                    dirty = true;
                }
                Action::Select(i) => {
                    self.state.selected = Some(self.ds.seasons[i].id.clone());
                    self.tab = Tab::Seasons;
                    dirty = true;
                }
                Action::AddTarget(id) => {
                    if !self.state.is_target(&id) {
                        self.state.targets.push(id);
                        dirty = true;
                    }
                }
                Action::RemoveTarget(id) => {
                    self.state.targets.retain(|t| *t != id);
                    dirty = true;
                }
                Action::ClearTargets => {
                    self.state.targets.clear();
                    dirty = true;
                }
                Action::Tab(t) => self.tab = t,
                Action::Refresh => self.start_refresh(ctx),
                Action::UseBundled => {
                    data::clear_cache();
                    let (ds, src) = data::load();
                    self.ds = ds;
                    self.source = src;
                    self.status = Some(("Switched back to the bundled dataset.".into(), false));
                }
            }
        }
        if dirty {
            self.save();
        }
    }

    fn save(&mut self) {
        if let Err(e) = self.state.save() {
            self.status = Some((format!("Couldn't save progress: {e}"), true));
        }
    }

    fn start_refresh(&mut self, ctx: &egui::Context) {
        if self.refresh_rx.is_some() {
            return;
        }
        let (tx, rx) = mpsc::channel();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let _ = tx.send(data::refresh());
            ctx.request_repaint();
        });
        self.refresh_rx = Some(rx);
        self.status = Some(("Downloading the latest survivoR data…".into(), false));
    }

    fn poll_refresh(&mut self) {
        let Some(rx) = &self.refresh_rx else { return };
        let Ok(result) = rx.try_recv() else { return };
        self.refresh_rx = None;
        match result {
            Ok((ds, src)) => {
                let n = ds.seasons.len();
                self.ds = ds;
                self.source = src;
                self.status = Some((format!("Updated — {n} seasons loaded."), false));
            }
            Err(e) => self.status = Some((format!("Update failed: {e}"), true)),
        }
    }

    fn selected(&self) -> Option<usize> {
        self.state.selected.as_ref().and_then(|s| self.ds.index.get(s).copied())
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_refresh();
        let mut actions = Vec::new();
        let watched = self.state.watched_set();

        top_bar(ctx, self, &mut actions);

        if self.tab == Tab::Seasons {
            egui::SidePanel::left("seasons")
                .resizable(true)
                .default_width(340.0)
                .min_width(260.0)
                .frame(egui::Frame::new().fill(BG0_H).inner_margin(10.0))
                .show(ctx, |ui| season_list(ui, self, &watched, &mut actions));
        }

        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(BG0).inner_margin(16.0))
            .show(ctx, |ui| match self.tab {
                Tab::Seasons => match self.selected() {
                    Some(i) => season_detail(ui, &self.ds, &mut self.state, i, &watched, &mut actions),
                    None => {
                        ui.label("Pick a season on the left.");
                    }
                },
                Tab::Plan => plan_tab(ui, &self.ds, &mut self.state, &watched, &mut actions),
                Tab::Players => players_tab(ui, &self.ds, &mut self.state, &mut self.player_search, &watched, &mut actions),
            });

        if !actions.is_empty() {
            self.apply(actions, ctx);
        }
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        let _ = self.state.save();
    }
}

// ---------------------------------------------------------------- top bar

fn top_bar(ctx: &egui::Context, app: &mut App, actions: &mut Vec<Action>) {
    egui::TopBottomPanel::top("top")
        .frame(egui::Frame::new().fill(BG0_H).inner_margin(egui::Margin::symmetric(12, 8)))
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("🔥 Survivor Planner").size(18.0).strong().color(GREEN));
                ui.add_space(16.0);
                for (t, label) in [(Tab::Seasons, "Seasons"), (Tab::Plan, "Watch plan"), (Tab::Players, "Players")] {
                    let text = if t == Tab::Plan && !app.state.targets.is_empty() {
                        format!("{label} ({})", app.state.targets.len())
                    } else {
                        label.to_owned()
                    };
                    if ui.selectable_label(app.tab == t, RichText::new(text).size(15.0)).clicked() {
                        actions.push(Action::Tab(t));
                    }
                }

                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let busy = app.refresh_rx.is_some();
                    if busy {
                        ui.spinner();
                    } else if ui
                        .button("⟳ Update data")
                        .on_hover_text("Download the latest survivoR JSON from GitHub")
                        .clicked()
                    {
                        actions.push(Action::Refresh);
                    }
                    if matches!(app.source, Source::Cached { .. })
                        && ui.small_button("use bundled").on_hover_text("Forget the downloaded copy").clicked()
                    {
                        actions.push(Action::UseBundled);
                    }
                    ui.label(
                        RichText::new(format!("{} seasons · {}", app.ds.seasons.len(), app.source.describe()))
                            .small()
                            .color(GRAY),
                    );
                    if let Some((msg, err)) = &app.status {
                        ui.label(RichText::new(msg).small().color(if *err { RED } else { AQUA }));
                    }
                });
            });
        });
}

// ---------------------------------------------------------------- left: season list

fn season_list(ui: &mut Ui, app: &mut App, watched: &HashSet<String>, actions: &mut Vec<Action>) {
    let ds = &app.ds;
    let total = ds.seasons.len();
    let seen = ds.seasons.iter().filter(|s| watched.contains(&s.id)).count();

    ui.add(
        egui::ProgressBar::new(seen as f32 / total.max(1) as f32)
            .text(RichText::new(format!("{seen} / {total} seasons watched")).color(FG))
            .fill(GREEN_DIM),
    );
    ui.add_space(4.0);
    ui.add(egui::TextEdit::singleline(&mut app.search).hint_text("🔍 Filter seasons…").desired_width(f32::INFINITY));
    ui.add_space(4.0);

    let q = app.search.to_lowercase();
    let selected = app.state.selected.clone();

    ScrollArea::vertical().auto_shrink([false; 2]).show(ui, |ui| {
        for (_, (version, idxs)) in data::by_version(ds) {
            let shown: Vec<usize> = idxs
                .iter()
                .copied()
                .filter(|&i| {
                    let s = &ds.seasons[i];
                    q.is_empty()
                        || s.name.to_lowercase().contains(&q)
                        || s.id.to_lowercase().contains(&q)
                        || s.location.to_lowercase().contains(&q)
                })
                .collect();
            if shown.is_empty() {
                continue;
            }
            let w = idxs.iter().filter(|&&i| watched.contains(&ds.seasons[i].id)).count();
            let open = !app.state.collapsed_versions.contains(&version) || !q.is_empty();
            let header = RichText::new(format!("{}   {w}/{}", data::version_label(&version), idxs.len()))
                .strong()
                .color(YELLOW);
            let resp = egui::CollapsingHeader::new(header)
                .id_salt(&version)
                .open(Some(open))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        let ids: Vec<String> = idxs.iter().map(|&i| ds.seasons[i].id.clone()).collect();
                        if ui.small_button("all watched").clicked() {
                            actions.push(Action::SetWatched(ids.clone(), true));
                        }
                        if ui.small_button("none").clicked() {
                            actions.push(Action::SetWatched(ids, false));
                        }
                    });
                    for i in shown {
                        season_row(ui, ds, i, watched, selected.as_deref(), actions);
                    }
                });
            if resp.header_response.clicked() {
                if open {
                    app.state.collapsed_versions.insert(version.clone());
                } else {
                    app.state.collapsed_versions.remove(&version);
                }
                let _ = app.state.save();
            }
        }
    });
}

fn season_row(
    ui: &mut Ui,
    ds: &Dataset,
    i: usize,
    watched: &HashSet<String>,
    selected: Option<&str>,
    actions: &mut Vec<Action>,
) {
    let s = &ds.seasons[i];
    let is_w = watched.contains(&s.id);
    let size = egui::vec2(ui.available_width(), 24.0);
    ui.allocate_ui_with_layout(size, Layout::left_to_right(Align::Center), |ui| {
        let mut w = is_w;
        if ui.checkbox(&mut w, "").on_hover_text("Watched").changed() {
            actions.push(Action::ToggleWatched(s.id.clone()));
        }
        let color = if is_w { FG4 } else { FG };
        let label = RichText::new(format!("{:>2}  {}", s.number, s.short_name())).color(color);
        if ui.selectable_label(selected == Some(s.id.as_str()), label).clicked() {
            actions.push(Action::Select(i));
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if is_w {
                return;
            }
            let (n, missing) = planner::readiness(ds, i, watched);
            if n == 0 {
                return;
            }
            if missing == 0 {
                ui.label(RichText::new("ready").small().color(GREEN))
                    .on_hover_text("You've seen every season its returnees came from");
            } else {
                ui.label(RichText::new(format!("{missing}/{n}")).small().color(ORANGE))
                    .on_hover_text(format!("{missing} of {n} returnee source seasons unwatched"));
            }
        });
    });
}

// ---------------------------------------------------------------- centre: one season

fn season_detail(
    ui: &mut Ui,
    ds: &Dataset,
    state: &mut State,
    i: usize,
    watched: &HashSet<String>,
    actions: &mut Vec<Action>,
) {
    let s = &ds.seasons[i];
    ScrollArea::vertical().auto_shrink([false; 2]).show(ui, |ui| {
        ui.label(RichText::new(format!("{} · Season {}", data::version_label(&s.version), s.number)).color(GRAY));
        ui.heading(RichText::new(s.short_name()).color(GREEN).strong());

        let mut meta = vec![];
        if !s.location.is_empty() {
            meta.push(s.location.clone());
        }
        if let Some(p) = &s.premiere {
            meta.push(format!("premiered {p}"));
        }
        if s.episodes > 0 {
            meta.push(format!("{} episodes · {}", s.episodes, hours(s.minutes)));
        }
        let ret = ds.returnee_count(i);
        meta.push(format!("{} castaways, {} returning", s.cast.len().max(s.n_cast as usize), ret));
        ui.label(RichText::new(meta.join("   ·   ")).color(FG4));
        ui.add_space(6.0);

        ui.horizontal(|ui| {
            let is_w = watched.contains(&s.id);
            let wtxt = if is_w { "✔ Watched" } else { "Mark watched" };
            if ui.add(egui::Button::new(RichText::new(wtxt).color(if is_w { GREEN } else { FG }))).clicked() {
                actions.push(Action::ToggleWatched(s.id.clone()));
            }
            if state.is_target(&s.id) {
                if ui.button("✖ Remove from watch plan").clicked() {
                    actions.push(Action::RemoveTarget(s.id.clone()));
                }
            } else if ret > 0 && ui.button("➕ Add to watch plan").clicked() {
                actions.push(Action::AddTarget(s.id.clone()));
            }
        });

        if !s.description.is_empty() {
            ui.add_space(8.0);
            ui.label(RichText::new(&s.description).color(FG));
        }
        if !s.tribe_setup.is_empty() {
            ui.label(RichText::new(&s.tribe_setup).italics().color(FG4));
        }

        ui.add_space(12.0);
        ui.separator();

        if ret == 0 {
            ui.add_space(8.0);
            ui.label(RichText::new("All-new cast — no homework needed. Watch it whenever.").color(AQUA));
            return;
        }

        section(ui, "Context you need");
        if options_ui(ui, &mut state.options) {
            let _ = state.save();
        }
        let opts = state.options.clone();
        let steps = planner::plan(ds, &[i], &opts, watched);
        plan_summary(ui, ds, &steps, watched);
        ui.add_space(6.0);
        watch_order(ui, ds, &steps, watched, false, actions);

        ui.add_space(14.0);
        section(ui, "Returning castaways");
        returnee_grid(ui, ds, i, watched, actions);
    });
}

fn section(ui: &mut Ui, title: &str) {
    ui.add_space(6.0);
    ui.label(RichText::new(title).size(17.0).strong().color(YELLOW));
    ui.add_space(2.0);
}

/// Returns true if anything changed.
fn options_ui(ui: &mut Ui, opts: &mut PlanOptions) -> bool {
    let before = opts.clone();
    ui.horizontal_wrapped(|ui| {
        egui::ComboBox::from_id_salt("coverage")
            .selected_text(opts.coverage.label())
            .width(230.0)
            .show_ui(ui, |ui| {
                for c in Coverage::ALL {
                    ui.selectable_value(&mut opts.coverage, c, c.label());
                }
            });
        ui.checkbox(&mut opts.recursive, "Follow the chain")
            .on_hover_text("Also pull in context for the context seasons (e.g. All-Stars → Pearl Islands)");
        ui.add_enabled_ui(opts.recursive, |ui| {
            ui.checkbox(&mut opts.stop_at_watched, "Stop at seasons I've seen")
                .on_hover_text("Don't chase the backstory of a season you've already watched");
        });
        ui.label(RichText::new("min. players").color(FG4));
        ui.add(egui::DragValue::new(&mut opts.min_players).range(1..=10))
            .on_hover_text("Skip source seasons that only contribute fewer returnees than this");
    });
    *opts != before
}

fn plan_summary(ui: &mut Ui, ds: &Dataset, steps: &[Step], watched: &HashSet<String>) {
    let pre: Vec<&Step> = steps.iter().filter(|s| !s.is_target()).collect();
    let pre_owned: Vec<Step> = pre.iter().map(|s| (*s).clone()).collect();
    let t = planner::remaining(ds, &pre_owned, watched);
    let seen = pre.len() - t.seasons;
    let text = if t.seasons == 0 {
        RichText::new(format!("✔ You're ready — all {} context seasons watched.", pre.len())).color(GREEN)
    } else {
        RichText::new(format!(
            "{} context seasons · {} watched · {} to go ({} episodes, about {})",
            pre.len(),
            seen,
            t.seasons,
            t.episodes,
            hours(t.minutes)
        ))
        .color(FG)
    };
    ui.label(text.strong());
}

/// Numbered chronological list. In plan mode, targets are highlighted and watched rows can be hidden.
fn watch_order(
    ui: &mut Ui,
    ds: &Dataset,
    steps: &[Step],
    watched: &HashSet<String>,
    hide_watched: bool,
    actions: &mut Vec<Action>,
) {
    let mut n = 0;
    egui::Grid::new(ui.next_auto_id()).num_columns(4).striped(true).spacing([12.0, 6.0]).show(ui, |ui| {
        for st in steps {
            let s = &ds.seasons[st.season];
            let is_w = watched.contains(&s.id);
            if hide_watched && is_w {
                continue;
            }
            n += 1;
            ui.label(RichText::new(format!("{n:>2}.")).color(GRAY).monospace());

            ui.horizontal(|ui| {
                let mut w = is_w;
                if ui.checkbox(&mut w, "").changed() {
                    actions.push(Action::ToggleWatched(s.id.clone()));
                }
                let color = if st.is_target() {
                    GREEN
                } else if is_w {
                    FG4
                } else {
                    FG
                };
                let mut txt = RichText::new(format!("{} {}", s.id, s.short_name())).color(color);
                if st.is_target() {
                    txt = txt.strong();
                }
                if ui.link(txt).on_hover_text(s.location.as_str()).clicked() {
                    actions.push(Action::Select(st.season));
                }
            });

            let (tag, tc) = match st.depth {
                0 => ("▶ target".to_owned(), GREEN),
                1 => ("direct".to_owned(), AQUA),
                d => (format!("chain ×{}", d - 1), BLUE),
            };
            ui.label(RichText::new(tag).small().color(tc));

            if st.is_target() {
                ui.label(RichText::new(format!("{} eps · {}", s.episodes, hours(s.minutes))).small().color(GRAY));
            } else {
                let players = display_names(ds, &st.player_ids());
                let players: Vec<&str> = players.iter().map(String::as_str).collect();
                let needed: Vec<String> =
                    st.needed_by().iter().map(|&q| ds.seasons[q].short_name().to_owned()).collect();
                let summary = if players.len() > 6 {
                    format!("{} +{} more", players[..6].join(", "), players.len() - 6)
                } else {
                    players.join(", ")
                };
                ui.label(RichText::new(summary).color(FG4))
                    .on_hover_text(format!("{}\n\nfor: {}", players.join(", "), needed.join(", ")));
            }
            ui.end_row();
        }
    });
    if n == 0 {
        ui.label(RichText::new("Nothing left — everything in this plan is watched.").color(GREEN));
    }
}

fn returnee_grid(ui: &mut Ui, ds: &Dataset, i: usize, watched: &HashSet<String>, actions: &mut Vec<Action>) {
    egui::Grid::new("returnees").num_columns(2).striped(true).spacing([16.0, 6.0]).show(ui, |ui| {
        for pid in &ds.seasons[i].cast {
            let p = &ds.players[pid];
            let prior: Vec<usize> = p.seasons.iter().copied().filter(|&s| s < i).collect();
            if prior.is_empty() {
                continue;
            }
            ui.label(RichText::new(&p.full_name).color(FG));
            ui.horizontal_wrapped(|ui| {
                for q in prior {
                    let s = &ds.seasons[q];
                    let c = if watched.contains(&s.id) { GREEN } else { ORANGE };
                    if ui
                        .link(RichText::new(s.short_name()).color(c))
                        .on_hover_text(format!("{} · {}", s.id, if c == GREEN { "watched" } else { "not watched" }))
                        .clicked()
                    {
                        actions.push(Action::Select(q));
                    }
                }
            });
            ui.end_row();
        }
    });
}

// ---------------------------------------------------------------- plan tab

fn plan_tab(ui: &mut Ui, ds: &Dataset, state: &mut State, watched: &HashSet<String>, actions: &mut Vec<Action>) {
    ScrollArea::vertical().auto_shrink([false; 2]).show(ui, |ui| {
        ui.heading(RichText::new("Watch plan").color(GREEN).strong());
        ui.label(
            RichText::new(
                "Seasons you want to watch with full context. The order below merges everything they need, oldest first.",
            )
            .color(FG4),
        );
        ui.add_space(8.0);

        if state.targets.is_empty() {
            ui.label(
                RichText::new("No targets yet — open a returnee season (like Heroes vs. Villains) and hit “Add to watch plan”.")
                    .color(ORANGE),
            );
            ui.add_space(8.0);
            quick_targets(ui, ds, actions);
            return;
        }

        ui.horizontal_wrapped(|ui| {
            for id in &state.targets {
                let Some(&i) = ds.index.get(id) else { continue };
                egui::Frame::new().fill(BG1).corner_radius(6).inner_margin(egui::Margin::symmetric(8, 4)).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        if ui.link(RichText::new(ds.seasons[i].short_name()).color(GREEN)).clicked() {
                            actions.push(Action::Select(i));
                        }
                        if ui.small_button("✖").on_hover_text("Remove").clicked() {
                            actions.push(Action::RemoveTarget(id.clone()));
                        }
                    });
                });
            }
            if ui.small_button("clear all").clicked() {
                actions.push(Action::ClearTargets);
            }
        });
        ui.add_space(6.0);
        quick_targets(ui, ds, actions);

        section(ui, "Order");
        if options_ui(ui, &mut state.options) {
            let _ = state.save();
        }
        if ui.checkbox(&mut state.hide_watched_in_plan, "Hide watched").changed() {
            let _ = state.save();
        }

        let targets: Vec<usize> = state.targets.iter().filter_map(|t| ds.index.get(t).copied()).collect();
        let steps = planner::plan(ds, &targets, &state.options, watched);
        let t = planner::remaining(ds, &steps, watched);
        ui.label(
            RichText::new(format!(
                "{} seasons in plan · {} to go · {} episodes · about {}",
                steps.len(),
                t.seasons,
                t.episodes,
                hours(t.minutes)
            ))
            .strong(),
        );
        ui.add_space(6.0);
        watch_order(ui, ds, &steps, watched, state.hide_watched_in_plan, actions);
    });
}

fn quick_targets(ui: &mut Ui, ds: &Dataset, actions: &mut Vec<Action>) {
    // Seasons that are mostly or entirely returnees.
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new("Returnee seasons:").small().color(GRAY));
        for i in (0..ds.seasons.len()).rev() {
            let s = &ds.seasons[i];
            let r = ds.returnee_count(i);
            if r * 2 >= s.cast.len().max(1) {
                if ui
                    .small_button(s.short_name())
                    .on_hover_text(format!("{} · {r} returnees — click to add", s.id))
                    .clicked()
                {
                    actions.push(Action::AddTarget(s.id.clone()));
                }
            }
        }
    });
}

// ---------------------------------------------------------------- players tab

fn players_tab(
    ui: &mut Ui,
    ds: &Dataset,
    state: &mut State,
    search: &mut String,
    watched: &HashSet<String>,
    actions: &mut Vec<Action>,
) {
    ui.heading(RichText::new("Players").color(GREEN).strong());
    ui.horizontal(|ui| {
        ui.add(egui::TextEdit::singleline(search).hint_text("🔍 Search castaways…").desired_width(280.0));
        if ui.checkbox(&mut state.returnees_only, "Played more than once").changed() {
            let _ = state.save();
        }
    });
    ui.add_space(6.0);

    let q = search.to_lowercase();
    let mut list: Vec<_> = ds
        .players
        .values()
        .filter(|p| !state.returnees_only || p.seasons.len() > 1)
        .filter(|p| q.is_empty() || p.full_name.to_lowercase().contains(&q) || p.name.to_lowercase().contains(&q))
        .collect();
    list.sort_by(|a, b| b.seasons.len().cmp(&a.seasons.len()).then(a.full_name.cmp(&b.full_name)));
    let count = list.len();
    list.truncate(400);

    ui.label(RichText::new(format!("{count} castaways")).small().color(GRAY));
    ScrollArea::vertical().auto_shrink([false; 2]).show(ui, |ui| {
        egui::Grid::new("players").num_columns(3).striped(true).spacing([16.0, 6.0]).show(ui, |ui| {
            for p in list {
                ui.label(RichText::new(&p.full_name).color(FG));
                ui.label(RichText::new(format!("×{}", p.seasons.len())).color(YELLOW));
                ui.horizontal_wrapped(|ui| {
                    for &s in &p.seasons {
                        let season = &ds.seasons[s];
                        let c = if watched.contains(&season.id) { GREEN } else { FG4 };
                        if ui
                            .link(RichText::new(season.short_name()).color(c))
                            .on_hover_text(season.id.as_str())
                            .clicked()
                        {
                            actions.push(Action::Select(s));
                        }
                    }
                });
                ui.end_row();
            }
        });
    });
}

/// Short names, falling back to full names where two players would read the same (e.g. two Russells).
fn display_names(ds: &Dataset, ids: &[&str]) -> Vec<String> {
    let short: Vec<&str> = ids.iter().map(|p| ds.player_name(p)).collect();
    ids.iter()
        .zip(&short)
        .map(|(id, name)| {
            if short.iter().filter(|n| *n == name).count() > 1 {
                ds.players.get(*id).map(|p| p.full_name.clone()).unwrap_or_else(|| name.to_string())
            } else {
                name.to_string()
            }
        })
        .collect()
}

fn hours(minutes: u32) -> String {
    if minutes == 0 {
        return "? h".into();
    }
    let h = minutes as f32 / 60.0;
    if h >= 10.0 {
        format!("{h:.0} h")
    } else {
        format!("{h:.1} h")
    }
}
