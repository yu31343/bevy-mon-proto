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

# Run tests
cargo test

# Run a single test
cargo test <test_name>

# Run the game in dev mode
cargo run

# Build a standalone release executable
cargo build --release
```

> `bevy` uses the `dynamic_linking` feature by default for faster iteration. `cargo run` works in that mode, but the built executable is not standalone. Before shipping a standalone release, remove `dynamic_linking` from `Cargo.toml`, then run `cargo build --release`.
>
> UI text now depends on the build script finding at least one `.ttf`, `.otf`, or `.ttc` file under `assets/fonts/`. Builds fail fast if that directory has no usable font, and the build log prints which font file was embedded.

## Architecture

`src/main.rs` only assembles the app: it initializes `GameState` and `BattlePhase`, then registers four plugins in order: `DataPlugin`, `UiPlugin`, `TeamSelectionPlugin`, and `BattlePlugin`.

### State flow

The game loop is driven by one top-level state and one nested battle state:

```text
GameState::TeamSelection
  -> GameState::Battle with BattlePhase::Init
  -> RoundStart -> PlayerTurn -> EnemyTurn -> CheckEnd -> RoundStart ...
  -> GameState::Result
```

`R` on the result screen clears the previous team selection and returns to `GameState::TeamSelection`.

### Data and setup flow

- `src/data/mod.rs` loads and validates `assets/data/battle_data.ron` at startup.
- That RON file is the gameplay source of truth: it defines battle rules, the element matrix, skills, monster prototypes, card definitions, and deck composition.
- Startup inserts `BattleDbs`, `MonsterPool`, `CardDeck`, `BattleRules`, and `BattleDataStatus` resources. `BattleDbs` intentionally bundles skills/cards/elements into one resource to stay under Bevy's system-parameter limit.
- `TeamSelectionPlugin` uses `MonsterPool` plus `BattleRules.max_team_size` to build the selection UI. Once confirmed, it writes a `TeamSelections` resource and transitions into battle.
- `init_battle_system` waits for `TeamSelections`, despawns all `InBattle` entities from any prior run, resets per-battle resources, and spawns the selected monsters into `PlayerTeam` and `EnemyTeam`.

### Battle flow

Battle systems are split across `src/battle/systems/` by phase rather than living in one file:

- `init.rs`: battle bootstrap and failure handling.
- `round.rs`: round-start bookkeeping.
- `player_turn.rs`: keyboard-driven player actions plus execution of UI-written intents.
- `enemy_turn.rs`: AI decisions and pacing.
- `end.rs`: fainting, auto-switching, result transitions.
- `events.rs`: converts `BattleEvent` messages into log lines.
- `combat.rs`: shared combat resolution helpers.

Important runtime rules:

- `round_start_system` redraws both hands every round from `CardDeck`, adds `BattleRules.ap_per_round` on top of any leftover AP, clears `PendingBoosts`, and returns control to the player first.
- `TurnContext` and `SelectedCard` are the glue between UI interactions and battle resolution. UI button systems usually write intent first; `player_turn_input_system` performs the actual skill/card effects.
- Skill AP costs are hard-coded by slot in `src/battle/systems/combat.rs`: slot 0 costs 2 AP, slot 1 costs 3 AP, slots 2-3 cost 1 AP. Team switch costs 1 AP, and discarding a card grants 1 AP.
- `PendingBoosts` stores card-granted bonuses for the next matching attack/heal/shield action and is consumed on use.
- `check_end_system` does not immediately declare defeat when the active combatant faints; it first auto-switches to the next living member, then only moves to `GameState::Result` if a side has no living members left.
- Enemy AI uses a local timer inside `enemy_turn_ai_system` to create 1.5-2.5 second pauses between actions, so that system must keep running in `Update` rather than `OnEnter`.

### Combat model

- Battle entities are ordinary ECS entities marked with `InBattle` and composed from `Combatant`, `Stats`, `SkillList`, `SkillCount`, `Shield`, and `ElementAura` in `src/battle/components.rs`.
- `BattleEvent` is the contract between battle logic and presentation. New combat feedback should generally become a new event rather than direct UI mutation from battle systems.
- Element attacks first evaluate effectiveness against the target's current aura (`ElementAura`) or, if none is attached, the target's base element.
- If a shield absorbs any amount of an elemental hit, the game recomputes damage against the target's base element only and does not apply a new aura. If the shield absorbs nothing, the incoming element replaces the target aura.
- Some combatants start battle with an initial aura matching their element; that is assigned during `init_battle_system`, not in the data file.

### UI structure

- `src/ui/battle/layout.rs` spawns the full battle UI tree once. Update systems under `src/ui/battle/systems/` mutate specific nodes via marker components rather than rebuilding the tree.
- `src/ui/battle/fx.rs` owns timed visual feedback such as flashes and other short-lived battle effects.
- `src/ui/battle/plugin.rs` is currently a bridge: it still delegates registration to `crate::ui::register_legacy_battle_ui(app)`, so system ordering changes often need to be made in `src/ui/mod.rs` as well.
- Team selection UI is separate from battle UI and lives under `src/team_selection/`.
- Battle UI font selection is no longer environment-dependent: `build.rs` scans `assets/fonts/`, picks the first supported font file in sorted order, copies it into `OUT_DIR`, and `src/ui/battle/layout.rs` loads that embedded byte blob with `include_bytes!`. If you need to change the distributed font, replace the files in `assets/fonts/` rather than editing runtime fallback logic.
