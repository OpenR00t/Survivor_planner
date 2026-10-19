# Survivor Planner

A desktop app (Rust + egui) for tracking which Survivor seasons you've watched and planning
what to watch so returning players make sense. Pick a season like *Heroes vs. Villains* and it
lists every earlier season its cast came from, in air-date order, with who links each one.

Data: [survivoR](https://github.com/doehm/survivoR) by Daniel Oehm and contributors, sourced from
Wikipedia and the Survivor Wiki. The JSON exports are read from the
[daviseford/survivoR](https://github.com/daviseford/survivoR) fork (`dev/json`).

## Run

```powershell
cargo run --release
```

## What it does

- **Seasons**: every season from the US, Australia, New Zealand, South Africa and UK versions,
  grouped by franchise. Tick to mark watched. Unwatched returnee seasons show a badge:
  `ready` (all source seasons seen) or `missing/total`.
- **Context you need**: for the selected season, a numbered watch order of the seasons its
  returnees came from, each tagged `direct` or `chain ×n`, with the players who link it
  (hover for the full list and which season needs it), plus a returning-castaway grid.
- **Watch plan**: add several targets (e.g. HvV + Winners at War) and get one merged,
  chronological order with remaining episodes and hours.
- **Players**: search castaways and see every season they played.

### Plan options
| Option | Effect |
|---|---|
| Every previous appearance / Most recent only / Debut only | Which earlier seasons count for each returnee |
| Follow the chain | Also pull in context for the context seasons (All-Stars → Pearl Islands …) |
| Stop at seasons I've seen | Don't chase the backstory of a season you've already watched |
| min. players | Skip source seasons contributing fewer returnees than this |

Seasons are ordered by first-episode air date, so cross-franchise returnees (e.g. *Australia
vs. The World*) work. Players are linked by survivoR's `castaway_id`; returnees from versions not
in the dataset (Québec, Finland) have no earlier season to point at.

## Data

A snapshot of the four JSON files is in `data/` and compiled into the binary. The data is
MIT-licensed, © 2021 Daniel Oehm; see [`data/LICENSE-survivoR.md`](data/LICENSE-survivoR.md). **⟳ Update data**
downloads the current files from GitHub into the app folder; **use bundled** discards that copy.

Watched seasons, plan targets and options are saved to
`%APPDATA%\SurvivorPlanner\state.json`; downloaded data to `%APPDATA%\SurvivorPlanner\data\`.

## Look

Gruvbox dark with a green accent. If CaskaydiaCove Nerd Font is installed (system or per-user
fonts) it's used automatically: the Propo variant for UI text, Mono for monospace.

## Layout

| File | |
|---|---|
| `src/data.rs` | JSON loading, cache/refresh, season + player index |
| `src/planner.rs` | context/watch-order logic + tests |
| `src/state.rs` | saved progress |
| `src/theme.rs` | Gruvbox visuals, font discovery |
| `src/app.rs` | UI |

`cargo test` checks the planner against real seasons (HvV, Winners at War, Australia vs. The World).
