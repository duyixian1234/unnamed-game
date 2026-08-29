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

use bevy::ecs::message::{MessageReader, Messages};
use bevy::ecs::schedule::ExecutorKind;
use bevy::prelude::*;
use bevy::state::state::{NextState, State, StateTransitionEvent};
use bevy::time::TimeUpdateStrategy;

use game_core::ai::AiPlugin;
use game_core::combat::{EnemyDied, HitSfx, PlayerHurt};
use game_core::economy::{MaterialPickedUp, Materials};
use game_core::enemy::{Enemy, EnemyKind, EnemySpawned};
use game_core::intent::{PlayerMoveIntent, PurchaseRequest};
use game_core::player::{Health, Player, PlayerStats};
use game_core::rng::Seed;
use game_core::shop::ItemPurchased;
use game_core::waves::{Wave, WaveCompleted, WaveConfig, WaveStarted};
use game_core::weapon::{MeleeHit, OrbitOrb, Projectile, Weapon, WeaponKind};
use game_core::{CorePlugin, GameState, RunEnded, RunOutcome, RunStarted};

/// Fixed timestep of the test harness.
const STEP: f32 = 1.0 / 60.0;

/// Everything the tests assert on, accumulated across frames.
#[derive(Resource, Default)]
struct Recording {
    states: Vec<GameState>,
    spawns: Vec<EnemyKind>,
    deaths: Vec<EnemyKind>,
    hurts: Vec<f32>,
    hits: u32,
    pickups: Vec<u32>,
    purchases: Vec<ItemPurchased>,
    wave_starts: Vec<u32>,
    wave_completes: Vec<u32>,
    run_started: u32,
    run_ends: Vec<RunOutcome>,
}

#[allow(clippy::too_many_arguments)] // one reader per recorded message kind
fn record(
    mut transitions: MessageReader<StateTransitionEvent<GameState>>,
    mut spawns: MessageReader<EnemySpawned>,
    mut deaths: MessageReader<EnemyDied>,
    mut hurts: MessageReader<PlayerHurt>,
    mut hits: MessageReader<HitSfx>,
    mut pickups: MessageReader<MaterialPickedUp>,
    mut purchases: MessageReader<ItemPurchased>,
    mut wave_starts: MessageReader<WaveStarted>,
    mut wave_completes: MessageReader<WaveCompleted>,
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
    for death in deaths.read() {
        rec.deaths.push(death.kind);
    }
    for hurt in hurts.read() {
        rec.hurts.push(hurt.hp_after);
    }
    rec.hits += hits.read().count() as u32;
    for pickup in pickups.read() {
        rec.pickups.push(pickup.amount);
    }
    for purchase in purchases.read() {
        rec.purchases.push(*purchase);
    }
    for start in wave_starts.read() {
        rec.wave_starts.push(start.number);
    }
    for complete in wave_completes.read() {
        rec.wave_completes.push(complete.number);
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
    app.insert_resource(WaveConfig {
        max_waves,
        spawning: true,
    });
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

/// Send a purchase request through the same message path the UI and AI use.
fn send_purchase(app: &mut App, item_index: usize) {
    app.world_mut()
        .resource_mut::<Messages<PurchaseRequest>>()
        .write(PurchaseRequest { item_index });
}

/// Catalog index of the item whose name contains `needle`.
fn catalog_index(needle: &str) -> usize {
    game_core::shop::SHOP_ITEMS
        .iter()
        .position(|item| item.name.contains(needle))
        .expect("item in shop catalog")
}

/// Despawn every starting weapon slot except `keep`, plus any live hitbox
/// entities, so the weapon under test is the only attacker. Scenario setup:
/// `update_orbs` re-spawns orbs only while an OrbitingOrb slot exists, so
/// removing the slot removes the orb permanently.
fn keep_only_weapon(app: &mut App, keep: WeaponKind) {
    let mut weapons = app.world_mut().query::<(Entity, &Weapon)>();
    let doomed: Vec<Entity> = weapons
        .iter(app.world())
        .filter(|(_, weapon)| weapon.kind != keep)
        .map(|(entity, _)| entity)
        .collect();
    for entity in doomed {
        app.world_mut().despawn(entity);
    }
    let mut strays = app.world_mut().query_filtered::<Entity, Or<(With<Projectile>, With<MeleeHit>, With<OrbitOrb>)>>();
    let stray_ids: Vec<Entity> = strays.iter(app.world()).collect();
    for entity in stray_ids {
        app.world_mut().despawn(entity);
    }
}

/// Spawn a static one-hit enemy inside the orbiting orb's kill ring (the orb
/// circles at radius 70 around the player, so x=70 is struck within a frame
/// or two), letting the weapon system kill it without wave interference.
fn spawn_static_enemy_at_orb_ring(app: &mut App, kind: EnemyKind, split_depth: u8) {
    app.world_mut().spawn((
        Enemy {
            kind,
            speed: 0.0,
            health: 1.0,
            split_depth,
        },
        Transform::from_xyz(70.0, 0.0, 0.0),
    ));
}

#[test]
fn shop_purchase_deducts_applies_and_rejects() {
    let mut app = headless(42, 5, false);
    app.update();
    start_run(&mut app);
    set_next_state(&mut app, GameState::Shop);
    step(&mut app);
    assert_eq!(current_state(&app), GameState::Shop);

    // Affordable +HP item: deducts cost, boosts stats, announces purchase.
    app.world_mut().resource_mut::<Materials>().count = 30;
    let titan = catalog_index("Titan's Heart");
    let titan_cost = game_core::shop::SHOP_ITEMS[titan].cost;
    send_purchase(&mut app, titan);
    step(&mut app);
    assert_eq!(
        app.world().resource::<Recording>().purchases.last().map(|p| (p.item_index, p.cost)),
        Some((titan, titan_cost)),
        "ItemPurchased announced with catalog index and cost"
    );
    assert_eq!(app.world().resource::<Materials>().count, 10, "cost deducted");
    let mut players =
        app.world_mut().query_filtered::<(&PlayerStats, &Health), With<Player>>();
    let (stats, health) = players.single(app.world()).expect("player in shop");
    assert_eq!(stats.max_hp_bonus, 25.0, "stat boost applied");
    assert_eq!(health.max, 125.0, "max HP raised by the boost");
    assert_eq!(health.current, 100.0, "current HP clamped, not healed");

    // Unaffordable request: rejected, wallet and stats untouched.
    app.world_mut().resource_mut::<Materials>().count = 5;
    let purchases_before = app.world().resource::<Recording>().purchases.len();
    let sharpened = catalog_index("Sharpened Edge");
    send_purchase(&mut app, sharpened); // cost 15 > wallet 5
    step(&mut app);
    assert_eq!(
        app.world().resource::<Materials>().count, 5,
        "rejected purchase must not deduct"
    );
    assert_eq!(
        app.world().resource::<Recording>().purchases.len(),
        purchases_before,
        "no ItemPurchased for a rejected request"
    );
    let mut stats_only = app.world_mut().query_filtered::<&PlayerStats, With<Player>>();
    let stats = stats_only.single(app.world()).unwrap();
    assert_eq!(stats.damage_mult, 1.0, "rejected boost not applied");
    assert_eq!(stats.max_hp_bonus, 25.0, "earlier purchase intact");
    assert_eq!(stats.speed_mult, 1.0, "no cross-item leakage");

    // Invalid catalog index: ignored.
    send_purchase(&mut app, 99);
    step(&mut app);
    assert_eq!(app.world().resource::<Materials>().count, 5);
    assert_eq!(app.world().resource::<Recording>().purchases.len(), purchases_before);
}

#[test]
fn economy_loop_kill_drop_pickup() {
    let mut app = headless(42, 5, false);
    app.update();
    start_run(&mut app);

    // Scenario setup: a one-hit enemy inside the orb's kill ring (orb fires
    // on the frame after the run starts; no wave spawn happens for ~63
    // frames, so the loop below stays isolated).
    spawn_static_enemy_at_orb_ring(&mut app, EnemyKind::MeleeRusher, 0);

    // Walk toward the drop via the intent layer until the wallet grows.
    let mut steps = 0;
    while steps < 60 {
        if app.world().resource::<Materials>().count > 0 {
            break;
        }
        app.world_mut().resource_mut::<PlayerMoveIntent>().dir = Vec2::X;
        step(&mut app);
        steps += 1;
    }

    assert_eq!(app.world().resource::<Materials>().count, 1, "pickup credited the wallet");
    let rec = app.world().resource::<Recording>();
    assert_eq!(rec.deaths, vec![EnemyKind::MeleeRusher], "enemy died exactly once");
    assert_eq!(rec.pickups, vec![1], "one material of value 1 was picked up");
}

#[test]
fn splitter_death_splits_twice_then_stops() {
    let mut app = headless(42, 5, false);
    app.update();
    start_run(&mut app);
    // Grandchildren get into contact range during the chain; buff the player
    // so the split test doesn't turn into a defeat test (scenario setup).
    buff_player(&mut app);

    // One-shot orb so the whole chain resolves in a few frames, well before
    // the first wave spawn (~63 frames).
    let mut orbs = app.world_mut().query::<&mut OrbitOrb>();
    for mut orb in orbs.iter_mut(app.world_mut()) {
        orb.damage = 100.0;
    }

    spawn_static_enemy_at_orb_ring(&mut app, EnemyKind::Splitter, 2);

    let mut first_gen_checked = false;
    for _ in 0..50 {
        step(&mut app);

        let splits = {
            let rec = app.world().resource::<Recording>();
            rec.spawns.iter().filter(|k| **k == EnemyKind::Splitter).count()
        };
        if !first_gen_checked && splits == 2 {
            let mut enemies = app.world_mut().query::<&Enemy>();
            let children: Vec<&Enemy> = enemies.iter(app.world()).collect();
            assert_eq!(children.len(), 2, "first split yields exactly 2 children");
            assert!(
                children.iter().all(|e| e.split_depth == 1),
                "children are one split-depth lower"
            );
            first_gen_checked = true;
        }
    }

    let rec = app.world().resource::<Recording>();
    let splits = rec.spawns.iter().filter(|k| **k == EnemyKind::Splitter).count();
    assert_eq!(splits, 6, "2 children + 4 grandchildren, then the chain stops");
    assert_eq!(rec.deaths.len(), 7, "parent + 6 descendants die");
    assert!(rec.deaths.iter().all(|k| *k == EnemyKind::Splitter));
    assert!(first_gen_checked, "first-generation split was observed");
    let mut remaining = app.world_mut().query_filtered::<Entity, With<Enemy>>();
    assert_eq!(
        remaining.iter(app.world()).count(),
        0,
        "field is empty after the chain"
    );
    assert_eq!(current_state(&app), GameState::InGame, "still mid-wave (no shop/victory)");
}

#[test]
fn contact_damage_is_gated_by_invulnerability_and_scales_by_kind() {
    let mut app = headless(42, 5, false);
    // Isolate from wave spawns: only the enemies placed below touch the player.
    app.insert_resource(WaveConfig {
        max_waves: 5,
        spawning: false,
    });
    app.update();
    start_run(&mut app);

    // Phase 1: a MeleeRusher glued to the player (10 contact damage). The
    // 0.6 s invulnerability window gates the first hit and every repeat.
    let rusher = app
        .world_mut()
        .spawn((
            Enemy {
                kind: EnemyKind::MeleeRusher,
                speed: 0.0,
                health: f32::MAX,
                split_depth: 0,
            },
            Transform::from_xyz(0.0, 0.0, 0.0),
        ))
        .id();
    for _ in 0..138 {
        step(&mut app); // 2.3 s
    }
    let phase1 = app.world().resource::<Recording>().hurts.clone();
    assert!(
        (2..=4).contains(&phase1.len()),
        "one hit per ~0.6 s window, got {}: {:?}",
        phase1.len(),
        phase1
    );
    assert!(
        phase1.iter().all(|hp_after| *hp_after <= 100.0 - 10.0),
        "MeleeRusher deals 10 per hit: {:?}",
        phase1
    );

    // Phase 2: swap the enemy for a SpeedBurster (6 contact damage).
    app.world_mut().despawn(rusher);
    app.world_mut().spawn((
        Enemy {
            kind: EnemyKind::SpeedBurster,
            speed: 0.0,
            health: f32::MAX,
            split_depth: 0,
        },
        Transform::from_xyz(0.0, 0.0, 0.0),
    ));
    for _ in 0..78 {
        step(&mut app); // 1.3 s
    }
    let rec = app.world().resource::<Recording>();
    let phase2 = &rec.hurts[phase1.len()..];
    assert!(
        (1..=3).contains(&phase2.len()),
        "burster also gated by the window, got {}: {:?}",
        phase2.len(),
        phase2
    );
    assert!(
        phase2.iter().all(|hp_after| *hp_after <= 100.0 - 30.0 - 6.0),
        "SpeedBurster deals 6 per hit: {:?}",
        phase2
    );
    // hp_after decreases by exactly the per-kind amount between hits.
    let mut all = phase1.clone();
    all.extend_from_slice(phase2);
    for pair in all.windows(2) {
        let drop = pair[0] - pair[1];
        assert!(
            [10.0, 6.0].contains(&drop),
            "each hit drops HP by a per-kind amount, got {drop}: {all:?}"
        );
    }
}

#[test]
fn piercing_projectile_hits_each_enemy_once() {
    let mut app = headless(42, 5, false);
    // Isolate from wave spawns so no stray enemy crosses the shot's path.
    app.insert_resource(WaveConfig {
        max_waves: 5,
        spawning: false,
    });
    app.update();
    start_run(&mut app);
    keep_only_weapon(&mut app, WeaponKind::PiercingProjectile);

    // Two tanky enemies aligned on +X: one shot pierces both.
    for x in [100.0, 300.0] {
        app.world_mut().spawn((
            Enemy {
                kind: EnemyKind::MeleeRusher,
                speed: 0.0,
                health: 100.0,
                split_depth: 0,
            },
            Transform::from_xyz(x, 0.0, 0.0),
        ));
    }

    // First volley fires at ~0.8 s and reaches x=300 by ~1.5 s; break before
    // the second volley (~1.6 s).
    let mut steps = 0;
    while steps < 150 {
        step(&mut app);
        steps += 1;
        if app.world().resource::<Recording>().hits >= 2 {
            break;
        }
    }

    let rec = app.world().resource::<Recording>();
    assert_eq!(rec.hits, 2, "each enemy struck exactly once (pierce, no re-hit)");
    assert!(rec.deaths.is_empty(), "tanky enemies survive the single hit");
    let mut enemies = app.world_mut().query::<&Enemy>();
    let healths: Vec<f32> = enemies.iter(app.world()).map(|e| e.health).collect();
    assert_eq!(healths.len(), 2);
    for health in healths {
        assert_eq!(health, 90.0, "exactly one 10-damage hit per enemy");
    }
}

#[test]
fn melee_swing_hits_every_enemy_in_radius_once() {
    let mut app = headless(42, 5, false);
    // Isolate from wave spawns so hit counts come only from the swing.
    app.insert_resource(WaveConfig {
        max_waves: 5,
        spawning: false,
    });
    app.update();
    start_run(&mut app);
    keep_only_weapon(&mut app, WeaponKind::MeleeSwing);

    // Three tanky enemies inside the swing ring (radius 90 + body 31) but
    // outside contact range (65), so only the swing can touch them.
    for (x, y) in [(70.0, 0.0), (0.0, 70.0), (-70.0, 0.0)] {
        app.world_mut().spawn((
            Enemy {
                kind: EnemyKind::MeleeRusher,
                speed: 0.0,
                health: 100.0,
                split_depth: 0,
            },
            Transform::from_xyz(x, y, 0.0),
        ));
    }

    // First swing fires at ~0.9 s; break before the second (~1.8 s).
    let mut steps = 0;
    while steps < 120 {
        step(&mut app);
        steps += 1;
        if app.world().resource::<Recording>().hits >= 3 {
            break;
        }
    }

    let rec = app.world().resource::<Recording>();
    assert_eq!(rec.hits, 3, "one swing hit all three enemies, once each");
    assert!(rec.deaths.is_empty(), "no enemy died to the first swing");
    let mut enemies = app.world_mut().query::<&Enemy>();
    let healths: Vec<f32> = enemies.iter(app.world()).map(|e| e.health).collect();
    assert_eq!(healths.len(), 3);
    for health in healths {
        assert_eq!(health, 75.0, "exactly one 25-damage hit per enemy");
    }
}

#[test]
fn wave_lifecycle_events_and_loadout_persistence() {
    let mut app = headless(42, 3, false);
    app.update();
    start_run(&mut app);

    let mut buffed = false;
    let mut shop_visits = 0u32;
    let mut steps = 0;
    while steps < 10_000 {
        if !buffed {
            buff_player(&mut app);
            buffed = true;
        }
        if current_state(&app) == GameState::Shop {
            shop_visits += 1;
            // Both before and after a Shop→InGame re-entry the starting
            // loadout must still be exactly three slots (no re-grant).
            let mut weapons = app.world_mut().query::<&Weapon>();
            assert_eq!(
                weapons.iter(app.world()).count(),
                3,
                "starting loadout granted exactly once (shop visit {})",
                shop_visits
            );
            set_next_state(&mut app, GameState::InGame);
        }
        step(&mut app);
        steps += 1;
        if current_state(&app) == GameState::Victory {
            break;
        }
    }

    assert_eq!(current_state(&app), GameState::Victory);
    assert!(shop_visits >= 2, "should have re-entered the shop at least once");
    let rec = app.world().resource::<Recording>();
    assert_eq!(rec.wave_starts, vec![1, 2, 3], "waves ascend from 1");
    assert_eq!(rec.wave_completes, vec![1, 2, 3], "each wave completes (the last, then Victory)");
    assert_eq!(rec.run_started, 1, "exactly one RunStarted for the run");
    // Every wave start (after the first) is immediately preceded by the
    // previous wave's completion.
    for (i, start) in rec.wave_starts.iter().enumerate().skip(1) {
        let prev = rec.wave_completes[i - 1];
        assert_eq!(
            *start,
            prev + 1,
            "WaveStarted({start}) must follow WaveCompleted({prev})"
        );
    }
}
