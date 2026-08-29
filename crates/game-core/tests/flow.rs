//! Headless integration tests for the full game flow (issue #15).
//!
//! The harness runs `game-core` in a headless `App` with a single-threaded
//! scheduler and a fixed 60 Hz timestep (ADR-0005), injecting a `Seed` for
//! determinism. Assertions are invariants over the state machine and the
//! sim message stream — no golden snapshots.
//!
//! Test boundary: player actions go through the AI / intent layer; scenario
//! setup (buffs, forced damage) writes state directly.

use std::time::Duration;

use bevy::ecs::message::MessageReader;
use bevy::ecs::schedule::ExecutorKind;
use bevy::prelude::*;
use bevy::state::state::{NextState, State, StateTransitionEvent};
use bevy::time::TimeUpdateStrategy;

use game_core::ai::AiPlugin;
use game_core::combat::PlayerHurt;
use game_core::economy::Materials;
use game_core::enemy::{Enemy, EnemyKind, EnemySpawned};
use game_core::player::{Health, Player};
use game_core::rng::Seed;
use game_core::shop::ItemPurchased;
use game_core::waves::{Wave, WaveConfig};
use game_core::{CorePlugin, GameState, RunEnded, RunOutcome, RunStarted};

/// Fixed timestep of the test harness.
const STEP: f32 = 1.0 / 60.0;

/// Everything the tests assert on, accumulated across frames.
#[derive(Resource, Default)]
struct Recording {
    states: Vec<GameState>,
    spawns: Vec<EnemyKind>,
    hurts: Vec<f32>,
    purchases: Vec<ItemPurchased>,
    run_started: u32,
    run_ends: Vec<RunOutcome>,
}

fn record(
    mut transitions: MessageReader<StateTransitionEvent<GameState>>,
    mut spawns: MessageReader<EnemySpawned>,
    mut hurts: MessageReader<PlayerHurt>,
    mut purchases: MessageReader<ItemPurchased>,
    mut run_starts: MessageReader<RunStarted>,
    mut run_ends: MessageReader<RunEnded>,
    mut rec: ResMut<Recording>,
) {
    for event in transitions.read() {
        if let Some(entered) = event.entered {
            rec.states.push(entered);
        }
    }
    for spawn in spawns.read() {
        rec.spawns.push(spawn.kind);
    }
    for hurt in hurts.read() {
        rec.hurts.push(hurt.hp_after);
    }
    for purchase in purchases.read() {
        rec.purchases.push(*purchase);
    }
    rec.run_started += run_starts.read().count() as u32;
    for end in run_ends.read() {
        rec.run_ends.push(end.outcome);
    }
}

/// A headless game-core app: single-threaded scheduler, fixed timestep,
/// seeded RNG. `with_ai` mounts the test AI as the player.
fn headless(seed: u64, max_waves: u32, with_ai: bool) -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    // MinimalPlugins lacks the state machinery (StateTransition schedule).
    app.add_plugins(bevy::state::app::StatesPlugin);
    // Deterministic fixed timestep: TimePlugin advances the clock from this
    // manual instant instead of the system clock; `step` moves it by STEP.
    app.insert_resource(TimeUpdateStrategy::ManualInstant(std::time::Instant::now()));
    app.insert_resource(Seed(seed));
    app.insert_resource(WaveConfig { max_waves });
    app.insert_resource(Recording::default());
    app.add_plugins(CorePlugin);
    if with_ai {
        app.add_plugins(AiPlugin);
    }
    // Determinism belt-and-suspenders (ADR-0005): the spawn system is the
    // only RNG consumer and is explicitly ordered, and the scheduler runs
    // single-threaded so message order is stable too.
    app.edit_schedule(Update, |schedule| {
        schedule.set_executor_kind(ExecutorKind::SingleThreaded);
    });
    app.add_systems(Update, record);
    app
}

/// Advance the simulation by one fixed step.
fn step(app: &mut App) {
    {
        let mut strategy = app.world_mut().resource_mut::<TimeUpdateStrategy>();
        let TimeUpdateStrategy::ManualInstant(instant) = &mut *strategy else {
            panic!("harness must use TimeUpdateStrategy::ManualInstant");
        };
        *instant += Duration::from_secs_f32(STEP);
    }
    app.update();
}

fn current_state(app: &App) -> GameState {
    **app.world().resource::<State<GameState>>()
}

fn set_next_state(app: &mut App, state: GameState) {
    app.world_mut().resource_mut::<NextState<GameState>>().set(state);
}

/// Set the player's health to a huge value so flow tests cannot die
/// (scenario setup, not player agency).
fn buff_player(app: &mut App) {
    let mut players = app.world_mut().query_filtered::<Entity, With<Player>>();
    if let Ok(entity) = players.single(app.world()) {
        app.world_mut().entity_mut(entity).insert(Health {
            max: 1_000_000.0,
            current: 1_000_000.0,
        });
    }
}

/// Start a run from the main menu (the same direct path the Start button / AI use).
fn start_run(app: &mut App) {
    set_next_state(app, GameState::InGame);
    step(app);
}

#[test]
fn full_flow_reaches_victory_with_expected_state_sequence() {
    let mut app = headless(42, 3, false);
    app.update(); // initialize state (records the initial MainMenu)
    start_run(&mut app);

    // Scenario setup: keep the player alive; the flow (not AI skill) is
    // what's under test.
    let mut buffed = false;
    let mut steps = 0;
    while steps < 10_000 {
        if !buffed {
            buff_player(&mut app);
            buffed = true;
        }
        if current_state(&app) == GameState::Shop {
            // Continue to the next wave (the Continue button's direct path).
            set_next_state(&mut app, GameState::InGame);
        }
        step(&mut app);
        steps += 1;
        if current_state(&app) == GameState::Victory {
            break;
        }
    }

    assert_eq!(
        current_state(&app),
        GameState::Victory,
        "3-wave run should end in Victory"
    );
    let rec = app.world().resource::<Recording>();
    // State sequence: MainMenu (init) → InGame → Shop → InGame → Shop → InGame → Victory.
    assert_eq!(
        rec.states,
        vec![
            GameState::MainMenu,
            GameState::InGame,
            GameState::Shop,
            GameState::InGame,
            GameState::Shop,
            GameState::InGame,
            GameState::Victory,
        ],
        "unexpected state transition sequence: {:?}",
        rec.states
    );
    // Run lifecycle: a fresh run started exactly once.
    assert_eq!(rec.run_started, 1);
    // Player took contact damage along the way (combat is wired); surviving
    // to Victory is proven by the state sequence (a death would have routed
    // through Defeat). The player entity is despawned on Victory by design.
    assert!(
        !rec.hurts.is_empty(),
        "enemies should have reached the stationary player at least once"
    );
}

#[test]
fn defeat_returns_to_menu_and_resets_the_run() {
    let mut app = headless(7, 20, false);
    app.update();
    start_run(&mut app);

    // Scenario setup: a lethal-immune enemy is placed on top of the player.
    let mut players = app.world_mut().query_filtered::<&Transform, With<Player>>();
    let player_pos = *players.single(app.world()).expect("player spawned on wave 1");
    app.world_mut().spawn((
        Enemy {
            kind: EnemyKind::MeleeRusher,
            speed: 0.0,
            health: f32::MAX,
            split_depth: 0,
        },
        Transform::from_translation(player_pos.translation),
    ));

    // Contact damage is gated by a 0.6s invulnerability window; step past it.
    let mut steps = 0;
    while current_state(&app) != GameState::Defeat && steps < 600 {
        step(&mut app);
        steps += 1;
    }
    assert_eq!(current_state(&app), GameState::Defeat, "player should die");
    assert!(
        app.world().resource::<Recording>().run_ends.contains(&RunOutcome::Defeat),
        "RunEnded(Defeat) must be announced"
    );

    // Play Again: back to MainMenu (resets wave counter and wallet).
    set_next_state(&mut app, GameState::MainMenu);
    step(&mut app);
    assert_eq!(current_state(&app), GameState::MainMenu);
    assert_eq!(app.world().resource::<Materials>().count, 0, "wallet reset");
    assert_eq!(app.world().resource::<Wave>().number, 1, "wave counter reset");

    // A fresh run spawns a fresh player at full health.
    start_run(&mut app);
    assert_eq!(current_state(&app), GameState::InGame);
    let mut players = app.world_mut().query_filtered::<&Health, With<Player>>();
    let health = players.single(app.world()).expect("fresh player spawned");
    assert_eq!(health.current, 100.0);
}

#[test]
fn same_seed_produces_identical_spawn_streams() {
    let run = |seed: u64| -> (Vec<EnemyKind>, Vec<GameState>) {
        let mut app = headless(seed, 5, false);
        app.update();
        start_run(&mut app);
        let mut buffed = false;
        let mut steps = 0;
        while steps < 3_600 {
            if !buffed {
                buff_player(&mut app);
                buffed = true;
            }
            if current_state(&app) == GameState::Shop {
                break; // wave 1 completed; stop here
            }
            step(&mut app);
            steps += 1;
        }
        let rec = app.world().resource::<Recording>();
        (rec.spawns.clone(), rec.states.clone())
    };

    let (a_spawns, a_states) = run(42);
    let (b_spawns, b_states) = run(42);
    let (c_spawns, _) = run(43);

    assert!(!a_spawns.is_empty(), "wave 1 should spawn enemies");
    assert_eq!(a_spawns, b_spawns, "same seed must replay identically");
    assert_eq!(a_states, b_states);
    assert_ne!(
        a_spawns, c_spawns,
        "different seeds should (over ~30 spawns) diverge"
    );
}

#[test]
fn ai_completes_a_short_run_to_victory() {
    let mut app = headless(42, 2, true);
    app.update();

    let mut steps = 0;
    let mut last_seen_pos = Vec3::ZERO;
    while steps < 8_000 {
        // Keep the player buffed whenever it exists (it respawns across
        // runs); AI survival comes from the buff, not AI skill.
        buff_player(&mut app);
        step(&mut app);
        steps += 1;
        if let Ok(transform) = app
            .world_mut()
            .query_filtered::<&Transform, With<Player>>()
            .single(app.world())
        {
            last_seen_pos = transform.translation;
        }
        if current_state(&app) == GameState::Victory {
            break;
        }
    }

    assert_eq!(
        current_state(&app),
        GameState::Victory,
        "AI-driven 2-wave run should reach Victory"
    );
    let rec = app.world().resource::<Recording>();
    assert!(rec.run_started > 0, "RunStarted must be announced");
    assert!(
        rec.run_ends.contains(&RunOutcome::Victory),
        "RunEnded(Victory) must be announced"
    );
    // The AI moved the player via the intent layer (the player entity is
    // despawned on Victory, hence the last-seen position).
    assert!(
        last_seen_pos.distance(Vec3::ZERO) > 0.5,
        "AI should have moved the player off spawn"
    );
}
