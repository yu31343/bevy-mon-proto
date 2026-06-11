# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

```bash
# Fast compile check
cargo check

# Format code
cargo fmt --all

# Lint
cargo clippy --all-targets -- -D warnings

# Run all tests
cargo test

# Run a single test
cargo test <test_name>

# Run a test and keep println!/debug output visible
cargo test <test_name> -- --nocapture

# Run the game from the repository root
cargo run

# Run the relay server for PVP relay mode
python relay_server.py
# or choose bind address/port explicitly
python relay_server.py --host 0.0.0.0 --port 42043

# Build a release executable
cargo build --release
```

> `bevy` enables `dynamic_linking` by default for faster iteration. `cargo run` works in that mode, but the produced executable is not standalone. Before shipping a standalone release build, remove `dynamic_linking` from `Cargo.toml`, then run `cargo build --release`.
>
> Builds require at least one `.ttf`, `.otf`, or `.ttc` file under `assets/fonts/`. `build.rs` sorts that directory, embeds the first supported font it finds, and fails fast if none exist.
>
> Gameplay data is loaded from `assets/data/battle_data.ron` through a fixed relative path, so run the game and data-parsing tests from the repository root.

Debug logging is controlled by environment variables. `BEVY_MON_LOG_DEBUG=1` enables broad battle/formula/status/card/PVP/AI detail; use narrower flags when needed: `BEVY_MON_LOG_BATTLE_DEBUG=1`, `BEVY_MON_LOG_CARDS=1`, `BEVY_MON_LOG_PVP=1`, `BEVY_MON_LOG_AI=1`, or `BEVY_MON_LOG_SPINE=1`.

## Repository workflow

The README says active development happens on `develop`. New work is expected to happen on a personal branch based on `develop`, then merged back through a pull request.

## Architecture

`src/main.rs` is intentionally thin. It initializes the top-level `GameState` and nested `BattlePhase`, adds `EguiPlugin`, then registers gameplay plugins in this order: `DataPlugin`, `UiPlugin`, `LobbyPlugin`, `MapPlugin`, `PvpPlugin`, `TeamSelectionPlugin`, `BattlePlugin`, and `SpineAnimPlugin`.

### State flow

The app is driven by one top-level state machine plus one nested battle-phase state machine:

```text
GameState::Lobby
  -> GameState::Map, GameState::MonsterDex, GameState::PvpLobby, or GameState::TeamSelection
  -> GameState::Battle with BattlePhase::Init
  -> RoundStart -> PlayerTurn/EnemyTurn according to speed and turn order
  -> optional Discard when a side exceeds the retained hand limit
  -> CheckEnd
  -> DeathResolve or next RoundStart
  -> GameState::Result
  -> GameState::Map on restart
```

Important state details:

- `LobbyPlugin` owns the real entry screen. It routes into VS AI selection, the map, PVP lobby, monster dex, or debug battle setup.
- `MapPlugin` owns `GameState::Map`: it spawns the simple overworld, character, monster sprites, and a return-to-lobby UI. Clicking a map monster records `MapBattleContext.enemy_monster_index`, switches `SelectionEntryMode` to `VsAi`, and enters team selection.
- `TeamSelectionPlugin` has three modes via `SelectionEntryMode`: `VsAi` selects only the player team and auto-generates or uses the mapped enemy team, `Debug` is a two-step flow where the user selects both sides, and `Pvp` submits the local team then waits for the remote team.
- `PvpPlugin` owns `GameState::PvpLobby`, performs connection setup, then switches into `TeamSelection` once the protocol/data handshake succeeds.
- `restart_from_result_system` returns to `GameState::Map`, resets selection state, and restores `VsAi` mode when the player restarts.

### Data-driven battle setup

- `src/data/mod.rs` loads all battle content from `assets/data/battle_data.ron` at startup: battle rules, formula rules, element matrix, statuses, reactions, skills, monster prototypes, cards, deck order, and AI default difficulty/presets/weights.
- Startup inserts `BattleDbs`, `MonsterPool`, `CardDeck`, `BattleRules`, `BattleFormulaRules`, `BattleRulesBundle`, `EnemyAiPresets`, `EnemyAiConfig`, and `BattleDataStatus`.
- `BattleDbs` intentionally bundles skills, cards, elements, statuses, and reactions into one resource to stay under Bevy's system-parameter limit.
- If loading, parsing, or validation fails, startup still inserts fallback resources and records the failure in `BattleDataStatus.error`; downstream systems should treat that resource as the authoritative load-failure signal.
- `TeamSelectionPlugin` writes `TeamSelections`, and `init_battle_system` consumes that resource to despawn old `InBattle` entities, reset per-battle state, validate indices, initialize shuffled `CardPiles`, and spawn both teams.

### Enemy AI tuning and tests

- Vs AI difficulty is selected through `SelectedAiDifficulty`; confirming a Vs AI team pulls the matching preset from `EnemyAiPresets` and inserts the resulting `EnemyAiConfig` before entering battle.
- The runtime AI entry point is `src/battle/systems/enemy_turn.rs`: it reads the ECS battle state, builds AI context, applies pacing/logging, and executes the chosen real battle action.
- Pure scoring and lightweight planning live in `src/battle/ai/evaluation.rs`; most AI behavior tests are colocated there, so prefer changing weights/scoring with focused unit tests instead of duplicating combat resolution.
- `EnemyAiPresets`, `EnemyAiConfig`, and `EnemyAiWeights` control default AI difficulty, per-difficulty presets, search depth, top candidates, switch threshold, player-information visibility, and valuation of attacks, cards, statuses, reactions, threats, and defensive actions.

### Battle flow and authority boundaries

Battle logic is split under `src/battle/systems/` by phase and concern rather than by entity type:

- `init.rs`: battle bootstrap and invalid-setup aborts.
- `round.rs`: per-round resets, AP gain, redraws, speed/turn-order choice, and UI-control-side synchronization.
- `player_turn.rs`: player inputs and execution of queued intents.
- `enemy_turn.rs`: AI pacing and debug-control input for the enemy side.
- `cards.rs`: card draw/use/discard helpers, pending card boosts, and the forced discard phase.
- `end.rs`: KO handling, death resolution, result transitions, restart, and log export.
- `combat.rs`: shared effect resolution.
- `status.rs`: status ticking and side-end processing.
- `events.rs`: drains battle messages into logs and presentation-facing state.

Important runtime rules:

- `TurnContext`, `SelectedCards`, `BattleControlMode`, and `UiControlSide` are the main glue between UI intent and battle resolution.
- `round_start_system` redraws hands from per-battle `CardPiles`, adds `BattleRules.ap_per_round` on top of leftover AP, clears pending boosts, and chooses the first side by effective speed (ties alternate; PVP uses the negotiated host/client turn order).
- `BattlePhase::Discard` is entered when a side has more than `BattleRules.max_retained_hand`; once the hand is back under the limit, AP is clamped to `BattleRules.max_ap` and the saved next phase resumes.
- `BattlePlugin` uses Bevy messages as the contract between core battle logic and downstream consumers. New combat feedback should usually become a new `BattleEvent` or related message rather than direct UI mutation.
- Enemy turns stay in `Update` because AI pacing depends on local timers rather than a single `OnEnter` step.

### PVP networking flow

- `src/pvp/mod.rs` handles both direct TCP LAN play and relay-server play. Network IO runs on background threads that send `NetEvent`s back into Bevy resources; Bevy systems poll those events in `Update`. Relay mode depends on `relay_server.py`; keep its `RELAY_PROTOCOL_VERSION` in sync with `src/pvp/mod.rs`.
- PVP sessions start in `GameState::PvpLobby`. A `Hello`/`HelloAck` handshake checks both `PROTOCOL_VERSION` and a stable hash of battle data, rules, formulas, cards, monsters, and deck order before allowing team selection.
- When both players submit teams, `pvp_apply_remote_team_system` inserts `BattleControlMode::PlayerVsRemote`, `PvpTurnOrder`, and `TeamSelections`, then enters the normal battle state.
- The host is authoritative during PVP battles: clients send `BattleIntent`s, the host applies remote intents during its enemy turn, broadcasts battle feedback, and sends snapshots that the client mirrors into local ECS state.
- PVP uses the same battle UI/resources as local play, but `UiControlSide`, hand assignment, turn order, and result messages are mirrored depending on whether the local peer is host or client.

### Combat model

- Battle participants are normal ECS entities marked with `InBattle` and composed from components in `src/battle/components.rs`.
- Persistent per-battle flow is carried by resources such as `PlayerTeam`, `EnemyTeam`, `TurnContext`, `ActionPoints`, `Hand`, `CardPiles`, `PendingBoosts`, `BattleResult`, `PendingKoResolution`, `StructuredBattleLog`, `ReplayEventLog`, and `ActionTrace`.
- Formula tuning is data-driven: accuracy bounds, stage limits, and minimum damage come from the RON config and are exposed through `BattleFormulaRules`.
- Element handling is aura-based. Elemental attacks check the target's current aura first, fall back to base element when needed, and only replace aura when shield absorption did not occur.
- Statuses and reactions are also data-driven. When changing combat behavior, inspect both the Rust resolution code and the RON-defined status/reaction data.

### UI, map, and presentation structure

- `UiPlugin` initializes shared UI theme/scale state, then delegates battle UI registration to `src/ui/battle/plugin.rs`.
- `src/ui/battle/plugin.rs` is still a bridge into `crate::ui::register_legacy_battle_ui(app)`, so battle-UI ordering changes often need to be made in `src/ui/mod.rs` as well as under `src/ui/battle/`.
- `src/ui/battle/layout.rs` creates the battle UI tree once; systems under `src/ui/battle/systems/` update existing nodes via marker components instead of rebuilding the tree.
- `src/ui/mod.rs` owns battle UI lifecycle details that are easy to miss: it spawns the camera, loads the embedded font at `Startup`, creates battle UI on `OnEnter(GameState::Battle)`, and cleans it up when entering `TeamSelection`, `Lobby`, or `Map`.
- Team selection UI, map UI, and lobby/dex UI are separate from battle UI. They are set up in `Update` with resource and state guards, so missing prerequisites can prevent the screen from appearing even if the state changed correctly.
- `src/map/` owns the current overworld prototype. Map entities are tagged with marker components and cleaned up on `OnExit(GameState::Map)`; map battles hand off through `MapBattleContext` rather than spawning battle entities directly.

### Spine animation integration

- `src/spine_anim.rs` adds `bevy_spine::SpinePlugin`, loads skeleton assets at startup, spawns battle visuals only during `GameState::Battle`, and keeps some fade/cleanup behavior active during both battle and result states.
- Presentation changes may need coordinated updates in both the UI systems and spine animation systems, because battle messages feed both layers.
