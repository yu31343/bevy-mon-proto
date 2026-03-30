# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

```bash
# Run (dev, with dynamic linking — exe is NOT standalone)
cargo run

# Run tests
cargo test

# Run a single test
cargo test <test_name>

# Build standalone release executable (remove dynamic_linking from Cargo.toml first)
cargo build --release
```

> `dynamic_linking` is enabled by default for fast iteration. The compiled exe depends on Bevy's shared library and cannot run standalone. Remove the feature from `Cargo.toml` before shipping.

## Architecture

The app is assembled in `src/main.rs` from three plugins, with no logic in `main()` itself:

| Plugin | Location | Role |
|---|---|---|
| `DataPlugin` | `src/data/mod.rs` | Loads `assets/data/battle_data.ron` at startup, inserts `SkillDb`, `CardDb`, `CardDeck`, `TeamSetup` resources |
| `BattlePlugin` | `src/battle/` | All battle logic, state transitions, AI |
| `UiPlugin` | `src/ui/` | Rendering, visual effects |

### State Machine

Two nested Bevy `States` drive the entire game loop:

```
GameState::Battle → BattlePhase::{Init → RoundStart → PlayerTurn → EnemyTurn → CheckEnd → RoundStart…}
GameState::Result  (shows outcome, R key restarts)
```

Each `BattlePhase` variant has a dedicated system in `src/battle/systems.rs` gated with `run_if(in_state(...))`.

### Battle Module (`src/battle/`)

- **`components.rs`**: All ECS types — `Combatant`, `Stats` (hp/atk/def/spd), `SkillList` (4 fixed slots), `Shield`, `ElementAura`, `Side` (Player/Enemy), plus resources `TurnContext`, `ActionPoints`, `Hand`, `PendingBoosts`, `BattleLog`, `BattleResult`, `TurnCount`, `PlayerTeam`/`EnemyTeam`.
- **`events.rs`**: `BattleEvent` message enum — emitted by logic systems, consumed by `consume_battle_events_system` for logging and UI.
- **`systems.rs`**: One system per phase. `init_battle_system` despawns all `InBattle`-marked entities and rebuilds them from `TeamSetup`. Use the `InBattle` marker component on any entity that should be cleaned up on restart.

### Data Layer (`src/data/mod.rs`)

- Game data is defined in `assets/data/battle_data.ron` (skills, player/enemy teams, card definitions).
- `ElementMatrix::get_effectiveness(attacker, defender) -> f32` encodes the 7-element type chart (2.0/1.0/0.5). The matrix is asymmetric — check both directions when adding new relations.
- Card system: players draw from `CardDeck` each round, spend `ActionPoints` (AP) to play `CardEffect`s (AP gain, next-attack/shield/heal boost).
- `BattleDataStatus` tracks load errors; `init_battle_system` aborts to `GameState::Result` if data is invalid.

### Key Conventions

- All game data goes in `assets/data/battle_data.ron`, not hardcoded in Rust (except the card definitions, which are currently inline in `DataPlugin`).
- Battle events (`BattleEvent`) are the contract between logic and UI — add new event variants there rather than coupling systems directly.
- `PendingBoosts` accumulates card-granted bonuses that apply to the next action; it resets each time a boost is consumed.
