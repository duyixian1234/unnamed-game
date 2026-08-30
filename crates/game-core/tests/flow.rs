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

use game_core::ai::{AiBuild, AiPlugin};
use game_core::combat::{EnemyDied, HitSfx, PlayerHurt};
use game_core::damage::DamageStats;
use game_core::damage::WeaponSlot;
use game_core::economy::{Material, MaterialPickedUp, Materials};
use game_core::enemy::{Enemy, EnemyKind, EnemySpawned};
use game_core::intent::{PlayerMoveIntent, PurchaseRequest};
use game_core::player::{Health, Player, PlayerStats};
use game_core::rng::Seed;
use game_core::shop::ItemPurchased;
use game_core::upgrade::{Evolved, UpgradeSelected, WeaponLevels};
use game_core::waves::{Wave, WaveCompleted, WaveConfig, WaveStarted};
use game_core::weapon::{
    BomberOrb, MeleeHit, OrbitOrb, Projectile, StartingWeapon, StartingWeaponSelected, Weapon,
    WeaponKind, Whirlwind,
};
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
    // Record in `Last` so a frame's messages are recorded in the same frame
    // they are written, regardless of how ambiguous Update systems order
    // against the recording system.
    app.add_systems(Last, record);
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
    app.world_mut()
        .resource_mut::<NextState<GameState>>()
        .set(state);
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

/// Start a run through the public starting-weapon selection seam.
fn start_run_with_weapon(app: &mut App, kind: WeaponKind) {
    set_next_state(app, GameState::StartingWeaponChoice);
    step(app);
    app.world_mut()
        .resource_mut::<Messages<StartingWeaponSelected>>()
        .write(StartingWeaponSelected { kind });
    step(app);
    step(app);
}

fn start_run(app: &mut App) {
    start_run_with_weapon(app, WeaponKind::PiercingProjectile);
}

#[test]
fn starting_weapon_choice_spawns_only_the_selected_slot() {
    let mut app = headless(42, 5, false);
    app.update();

    start_run_with_weapon(&mut app, WeaponKind::MeleeSwing);

    assert_eq!(current_state(&app), GameState::InGame);
    let mut weapons = app.world_mut().query::<&Weapon>();
    let kinds: Vec<_> = weapons
        .iter(app.world())
        .map(|weapon| weapon.kind)
        .collect();
    assert_eq!(kinds, vec![WeaponKind::MeleeSwing]);
    let stats = app.world().resource::<DamageStats>();
    let slot = stats
        .current_wave
        .slot(WeaponSlot(0))
        .expect("zero-damage slot registered");
    assert_eq!(slot.kind, WeaponKind::MeleeSwing);
    assert_eq!(slot.effective_damage, 0.0);
    assert_eq!(stats.current_wave.percentage(0.0), 0.0);
}

#[test]
fn upgrade_choice_accepts_only_the_equipped_weapon() {
    let mut app = headless(42, 5, false);
    app.update();
    start_run_with_weapon(&mut app, WeaponKind::MeleeSwing);
    set_next_state(&mut app, GameState::UpgradeChoice);
    step(&mut app);

    send_upgrade(&mut app, WeaponKind::PiercingProjectile, 0);
    step(&mut app);
    step(&mut app);

    assert_eq!(current_state(&app), GameState::UpgradeChoice);
    assert_eq!(
        app.world()
            .resource::<WeaponLevels>()
            .level(WeaponKind::PiercingProjectile),
        1
    );

    send_upgrade(&mut app, WeaponKind::MeleeSwing, 0);
    step(&mut app);
    step(&mut app);
    assert_eq!(current_state(&app), GameState::Shop);
}

#[test]
fn weapon_damage_contribution_counts_effective_damage_by_slot() {
    let mut app = headless(42, 5, false);
    app.insert_resource(WaveConfig {
        max_waves: 5,
        spawning: false,
    });
    app.update();
    start_run_with_weapon(&mut app, WeaponKind::PiercingProjectile);
    app.world_mut().spawn((
        Enemy {
            kind: EnemyKind::MeleeRusher,
            speed: 0.0,
            health: 3.0,
            split_depth: 0,
        },
        Transform::from_xyz(50.0, 0.0, 0.0),
    ));

    for _ in 0..90 {
        step(&mut app);
        if !app.world().resource::<Recording>().deaths.is_empty() {
            break;
        }
    }

    let stats = app.world().resource::<DamageStats>();
    let slot = stats
        .run
        .slot(WeaponSlot(0))
        .expect("selected weapon slot recorded");
    assert_eq!(slot.kind, WeaponKind::PiercingProjectile);
    assert_eq!(slot.effective_damage, 3.0, "overkill is excluded");
    assert_eq!(stats.run.total(), 3.0);
}

#[test]
fn duplicate_weapon_kinds_keep_separate_slot_contributions() {
    let mut app = headless(42, 5, false);
    app.insert_resource(WaveConfig {
        max_waves: 5,
        spawning: false,
    });
    app.update();
    start_run(&mut app);
    app.world_mut().spawn((
        Weapon::new(WeaponKind::PiercingProjectile),
        WeaponSlot(1),
        Transform::default(),
    ));
    app.world_mut().spawn((
        Enemy {
            kind: EnemyKind::MeleeRusher,
            speed: 0.0,
            health: 100.0,
            split_depth: 0,
        },
        Transform::from_xyz(50.0, 0.0, 0.0),
    ));

    for _ in 0..90 {
        step(&mut app);
        let stats = app.world().resource::<DamageStats>();
        if stats
            .run
            .slot(WeaponSlot(0))
            .is_some_and(|slot| slot.effective_damage > 0.0)
            && stats
                .run
                .slot(WeaponSlot(1))
                .is_some_and(|slot| slot.effective_damage > 0.0)
        {
            break;
        }
    }

    let stats = app.world().resource::<DamageStats>();
    assert_eq!(
        stats.run.slot(WeaponSlot(0)).unwrap().effective_damage,
        24.0
    );
    assert_eq!(
        stats.run.slot(WeaponSlot(1)).unwrap().effective_damage,
        24.0
    );
}

#[test]
fn damage_stats_snapshot_each_wave_and_preserve_run_total() {
    let mut app = headless(42, 5, false);
    app.insert_resource(WaveConfig {
        max_waves: 5,
        spawning: false,
    });
    app.update();
    start_run(&mut app);
    app.world_mut().spawn((
        Enemy {
            kind: EnemyKind::MeleeRusher,
            speed: 0.0,
            health: 3.0,
            split_depth: 0,
        },
        Transform::from_xyz(50.0, 0.0, 0.0),
    ));
    for _ in 0..90 {
        step(&mut app);
        if !app.world().resource::<Recording>().deaths.is_empty() {
            break;
        }
    }

    rush_wave_end(&mut app);
    step(&mut app);
    step(&mut app);
    assert_eq!(current_state(&app), GameState::UpgradeChoice);
    {
        let stats = app.world().resource::<DamageStats>();
        assert_eq!(stats.last_wave.total(), 3.0);
        assert!(stats.last_wave_completed);
        assert_eq!(stats.run.total(), 3.0);
    }

    send_upgrade(&mut app, WeaponKind::PiercingProjectile, 0);
    step(&mut app);
    step(&mut app);
    set_next_state(&mut app, GameState::InGame);
    step(&mut app);

    let stats = app.world().resource::<DamageStats>();
    assert_eq!(stats.current_wave.total(), 0.0);
    assert_eq!(stats.run.total(), 3.0);
}

#[test]
fn orbiting_orb_rehit_cooldown_is_time_based() {
    let mut app = headless(42, 5, false);
    app.insert_resource(WaveConfig {
        max_waves: 5,
        spawning: false,
    });
    app.update();
    start_run_with_weapon(&mut app, WeaponKind::OrbitingOrb);
    for mut weapon in app
        .world_mut()
        .query::<&mut Weapon>()
        .iter_mut(app.world_mut())
    {
        weapon.knockback_mult = 0.0;
    }
    for mut weapon in app
        .world_mut()
        .query::<&mut Weapon>()
        .iter_mut(app.world_mut())
    {
        weapon.orbit_speed = 0.0;
    }
    app.world_mut().spawn((
        Enemy {
            kind: EnemyKind::MeleeRusher,
            speed: 0.0,
            health: 1.0e6,
            split_depth: 0,
        },
        Transform::from_xyz(70.0, 0.0, 0.0),
    ));

    for _ in 0..60 {
        step(&mut app);
        if app.world().resource::<DamageStats>().run.total() > 0.0 {
            break;
        }
    }
    let first_hit = app.world().resource::<DamageStats>().run.total();
    assert!(first_hit > 0.0, "stationary orb lands its first hit");

    for _ in 0..10 {
        step(&mut app);
    }
    assert_eq!(
        app.world().resource::<DamageStats>().run.total(),
        first_hit,
        "same enemy cannot be hit again within 0.25 seconds"
    );
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
        if current_state(&app) == GameState::UpgradeChoice {
            // The wave-end pick is mandatory; make it like the UI/AI do.
            pick_any_upgrade(&mut app);
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
    // State sequence: MainMenu (init) → InGame → UpgradeChoice → Shop →
    // InGame → UpgradeChoice → Shop → InGame → Victory.
    assert_eq!(
        rec.states,
        vec![
            GameState::MainMenu,
            GameState::StartingWeaponChoice,
            GameState::InGame,
            GameState::UpgradeChoice,
            GameState::Shop,
            GameState::InGame,
            GameState::UpgradeChoice,
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
    start_run_with_weapon(&mut app, WeaponKind::OrbitingOrb);
    for mut weapon in app
        .world_mut()
        .query::<&mut Weapon>()
        .iter_mut(app.world_mut())
    {
        weapon.knockback_mult = 0.0;
    }

    let target = app
        .world_mut()
        .spawn((
            Enemy {
                kind: EnemyKind::MeleeRusher,
                speed: 0.0,
                health: 1000.0,
                split_depth: 0,
            },
            Transform::from_xyz(100.0, 0.0, 0.0),
        ))
        .id();
    for _ in 0..90 {
        step(&mut app);
        if app.world().resource::<DamageStats>().run.total() > 0.0 {
            break;
        }
    }
    assert!(
        app.world().resource::<DamageStats>().run.total() > 0.0,
        "first run must contain damage before reset"
    );
    app.world_mut().despawn(target);

    // Scenario setup: a lethal-immune enemy is placed on top of the player.
    let mut players = app.world_mut().query_filtered::<&Transform, With<Player>>();
    let player_pos = *players
        .single(app.world())
        .expect("player spawned on wave 1");
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
        app.world()
            .resource::<Recording>()
            .run_ends
            .contains(&RunOutcome::Defeat),
        "RunEnded(Defeat) must be announced"
    );

    // Play Again: back to StartingWeaponChoice (resets run resources).
    set_next_state(&mut app, GameState::StartingWeaponChoice);
    step(&mut app);
    assert_eq!(current_state(&app), GameState::StartingWeaponChoice);
    assert_eq!(app.world().resource::<Materials>().count, 0, "wallet reset");
    assert_eq!(
        app.world().resource::<Wave>().number,
        1,
        "wave counter reset"
    );
    let damage = app.world().resource::<DamageStats>();
    assert_eq!(damage.current_wave.total(), 0.0, "current wave reset");
    assert_eq!(damage.last_wave.total(), 0.0, "wave snapshot reset");
    assert_eq!(damage.run.total(), 0.0, "run damage reset");

    // A fresh run spawns a fresh player at full health.
    start_run_with_weapon(&mut app, WeaponKind::OrbitingOrb);
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
            if current_state(&app) == GameState::UpgradeChoice {
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

#[test]
fn every_single_weapon_upgrade_path_meets_balance_checkpoints() {
    for kind in [
        WeaponKind::PiercingProjectile,
        WeaponKind::MeleeSwing,
        WeaponKind::OrbitingOrb,
    ] {
        for choices in 0u8..16 {
            let upgrade_options = std::array::from_fn(|level| ((choices >> level) & 1) as usize);
            let mut app = headless(42, 5, true);
            app.insert_resource(AiBuild {
                weapon: kind,
                upgrade_options,
                buy_items: false,
            });
            app.update();

            for _ in 0..10_000 {
                step(&mut app);
                if matches!(current_state(&app), GameState::Victory | GameState::Defeat) {
                    break;
                }
            }

            assert_eq!(
                current_state(&app),
                GameState::Victory,
                "{kind:?} choices {choices:04b} must survive 5 waves; \
                 reached wave {}, dealt {:.0} damage",
                app.world().resource::<Wave>().number,
                app.world().resource::<DamageStats>().run.total(),
            );
        }
    }

    for (kind, choices) in [
        (WeaponKind::PiercingProjectile, 0b0000u8),
        (WeaponKind::MeleeSwing, 0b0000u8),
        (WeaponKind::OrbitingOrb, 0b1010u8),
    ] {
        let mut app = headless(42, 10, true);
        app.insert_resource(AiBuild {
            weapon: kind,
            upgrade_options: std::array::from_fn(|level| ((choices >> level) & 1) as usize),
            buy_items: false,
        });
        app.update();
        for _ in 0..20_000 {
            step(&mut app);
            if matches!(current_state(&app), GameState::Victory | GameState::Defeat) {
                break;
            }
        }
        assert_eq!(
            current_state(&app),
            GameState::Victory,
            "{kind:?} reasonable route must survive 10 waves"
        );
    }
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
    let titan = catalog_index("泰坦之心");
    let titan_cost = game_core::shop::SHOP_ITEMS[titan].cost;
    send_purchase(&mut app, titan);
    step(&mut app);
    assert_eq!(
        app.world()
            .resource::<Recording>()
            .purchases
            .last()
            .map(|p| (p.item_index, p.cost)),
        Some((titan, titan_cost)),
        "ItemPurchased announced with catalog index and cost"
    );
    assert_eq!(
        app.world().resource::<Materials>().count,
        10,
        "cost deducted"
    );
    let mut players = app
        .world_mut()
        .query_filtered::<(&PlayerStats, &Health), With<Player>>();
    let (stats, health) = players.single(app.world()).expect("player in shop");
    assert_eq!(stats.max_hp_bonus, 25.0, "stat boost applied");
    assert_eq!(health.max, 125.0, "max HP raised by the boost");
    assert_eq!(
        health.current, 125.0,
        "current HP rises by the same amount as max"
    );

    // Unaffordable request: rejected, wallet and stats untouched.
    app.world_mut().resource_mut::<Materials>().count = 5;
    let purchases_before = app.world().resource::<Recording>().purchases.len();
    let sharpened = catalog_index("磨砺之刃");
    send_purchase(&mut app, sharpened); // cost 15 > wallet 5
    step(&mut app);
    assert_eq!(
        app.world().resource::<Materials>().count,
        5,
        "rejected purchase must not deduct"
    );
    assert_eq!(
        app.world().resource::<Recording>().purchases.len(),
        purchases_before,
        "no ItemPurchased for a rejected request"
    );
    let mut stats_only = app
        .world_mut()
        .query_filtered::<&PlayerStats, With<Player>>();
    let stats = stats_only.single(app.world()).unwrap();
    assert_eq!(stats.damage_mult, 1.0, "rejected boost not applied");
    assert_eq!(stats.max_hp_bonus, 25.0, "earlier purchase intact");
    assert_eq!(stats.speed_mult, 1.0, "no cross-item leakage");

    // Invalid catalog index: ignored.
    send_purchase(&mut app, 99);
    step(&mut app);
    assert_eq!(app.world().resource::<Materials>().count, 5);
    assert_eq!(
        app.world().resource::<Recording>().purchases.len(),
        purchases_before
    );
}

#[test]
fn economy_loop_kill_drop_pickup() {
    let mut app = headless(42, 5, false);
    app.update();
    start_run_with_weapon(&mut app, WeaponKind::OrbitingOrb);

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

    assert_eq!(
        app.world().resource::<Materials>().count,
        1,
        "pickup credited the wallet"
    );
    let rec = app.world().resource::<Recording>();
    assert_eq!(
        rec.deaths,
        vec![EnemyKind::MeleeRusher],
        "enemy died exactly once"
    );
    assert_eq!(
        rec.pickups,
        vec![1],
        "one material of value 1 was picked up"
    );
}

#[test]
fn splitter_death_splits_twice_then_stops() {
    let mut app = headless(42, 5, false);
    app.insert_resource(WaveConfig {
        max_waves: 5,
        spawning: false,
    });
    app.update();
    start_run_with_weapon(&mut app, WeaponKind::MeleeSwing);
    // Grandchildren get into contact range during the chain; buff the player
    // so the split test doesn't turn into a defeat test (scenario setup).
    buff_player(&mut app);

    // A fast, wide one-shot swing isolates the splitter lifecycle from normal
    // weapon pacing.
    let mut weapons = app.world_mut().query::<&mut Weapon>();
    for mut weapon in weapons.iter_mut(app.world_mut()) {
        weapon.damage = 100.0;
        weapon.range = 500.0;
    }

    spawn_static_enemy_at_orb_ring(&mut app, EnemyKind::Splitter, 2);

    let mut first_gen_checked = false;
    for _ in 0..240 {
        step(&mut app);

        let splits = {
            let rec = app.world().resource::<Recording>();
            rec.spawns
                .iter()
                .filter(|k| **k == EnemyKind::Splitter)
                .count()
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
    let splits = rec
        .spawns
        .iter()
        .filter(|k| **k == EnemyKind::Splitter)
        .count();
    assert_eq!(
        splits, 6,
        "2 children + 4 grandchildren, then the chain stops"
    );
    assert_eq!(rec.deaths.len(), 7, "parent + 6 descendants die");
    assert!(rec.deaths.iter().all(|k| *k == EnemyKind::Splitter));
    assert!(first_gen_checked, "first-generation split was observed");
    let mut remaining = app.world_mut().query_filtered::<&Enemy, With<Enemy>>();
    let splitters_left = remaining
        .iter(app.world())
        .filter(|e| e.kind == EnemyKind::Splitter)
        .count();
    assert_eq!(
        splitters_left, 0,
        "no Splitter remains after the chain (a wave-spawned enemy may roam)"
    );
    assert_eq!(
        current_state(&app),
        GameState::InGame,
        "still mid-wave (no shop/victory)"
    );
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
        phase2
            .iter()
            .all(|hp_after| *hp_after <= 100.0 - 30.0 - 6.0),
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
    start_run_with_weapon(&mut app, WeaponKind::PiercingProjectile);

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
    assert_eq!(
        rec.hits, 2,
        "each enemy struck exactly once (pierce, no re-hit)"
    );
    assert!(
        rec.deaths.is_empty(),
        "tanky enemies survive the single hit"
    );
    let mut enemies = app.world_mut().query::<&Enemy>();
    let healths: Vec<f32> = enemies.iter(app.world()).map(|e| e.health).collect();
    assert_eq!(healths.len(), 2);
    for health in healths {
        assert_eq!(health, 76.0, "exactly one 24-damage hit per enemy");
    }
}

#[test]
fn melee_swing_hits_only_enemies_in_120_degree_fan() {
    let mut app = headless(42, 5, false);
    // Isolate from wave spawns so hit counts come only from the swing.
    app.insert_resource(WaveConfig {
        max_waves: 5,
        spawning: false,
    });
    app.update();
    start_run_with_weapon(&mut app, WeaponKind::MeleeSwing);

    // Three tanky enemies inside the 120-degree fan (radius 105 + body 22)
    // but outside contact range (42), so only the swing can touch them.
    for (x, y) in [(70.0, 0.0), (60.0, 30.0), (60.0, -30.0)] {
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
    app.world_mut().spawn((
        Enemy {
            kind: EnemyKind::MeleeRusher,
            speed: 0.0,
            health: 100.0,
            split_depth: 0,
        },
        Transform::from_xyz(0.0, 70.0, 0.0),
    ));

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
    assert_eq!(healths, vec![60.0, 60.0, 60.0, 100.0]);
}

#[test]
fn melee_upgrade_adds_independent_weapon_slot_with_shared_stats() {
    let mut app = headless(42, 5, false);
    app.insert_resource(WaveConfig {
        max_waves: 5,
        spawning: false,
    });
    app.update();
    start_run_with_weapon(&mut app, WeaponKind::MeleeSwing);
    set_next_state(&mut app, GameState::UpgradeChoice);
    step(&mut app);

    send_upgrade(&mut app, WeaponKind::MeleeSwing, 1);
    step(&mut app);
    step(&mut app);

    let mut weapons = app.world_mut().query::<(&Weapon, &WeaponSlot)>();
    let slots: Vec<_> = weapons
        .iter(app.world())
        .filter(|(weapon, _)| weapon.kind == WeaponKind::MeleeSwing)
        .map(|(_, slot)| *slot)
        .collect();
    assert_eq!(slots, vec![WeaponSlot(0), WeaponSlot(1)]);
    let mut weapons = app.world_mut().query_filtered::<&Weapon, Without<Player>>();
    let damages: Vec<_> = weapons
        .iter(app.world())
        .filter(|weapon| weapon.kind == WeaponKind::MeleeSwing)
        .map(|weapon| weapon.damage)
        .collect();
    assert_eq!(damages, vec![40.0, 40.0]);
}

#[test]
fn added_melee_weapons_attack_with_staggered_phases() {
    let mut app = headless(42, 5, false);
    app.insert_resource(WaveConfig {
        max_waves: 5,
        spawning: false,
    });
    app.update();
    start_run_with_weapon(&mut app, WeaponKind::MeleeSwing);
    set_next_state(&mut app, GameState::UpgradeChoice);
    step(&mut app);

    send_upgrade(&mut app, WeaponKind::MeleeSwing, 1);
    step(&mut app);
    step(&mut app);

    let mut weapons = app
        .world_mut()
        .query_filtered::<&Weapon, Without<Player>>();
    let mut phases: Vec<f32> = weapons
        .iter(app.world())
        .filter(|weapon| weapon.kind == WeaponKind::MeleeSwing)
        .map(|weapon| {
            weapon.cooldown.elapsed_secs() / weapon.cooldown.duration().as_secs_f32()
        })
        .collect();
    phases.sort_by(|a, b| a.partial_cmp(b).unwrap());
    assert_eq!(phases.len(), 2);
    assert!(
        (phases[1] - phases[0] - 0.5).abs() < 1e-4,
        "two melee slots must be half a cooldown apart, got {phases:?}"
    );
}

#[test]
fn melee_evolution_folds_merged_slots_run_stats_into_surviving_slot() {
    let mut app = headless(42, 5, false);
    app.insert_resource(WaveConfig {
        max_waves: 5,
        spawning: false,
    });
    app.update();
    start_run_with_weapon(&mut app, WeaponKind::MeleeSwing);
    buff_player(&mut app);
    // Zero knockback so the tanky test enemy stays inside the fan reach and
    // both melee slots can land damage on it.
    for mut weapon in app
        .world_mut()
        .query::<&mut Weapon>()
        .iter_mut(app.world_mut())
    {
        weapon.knockback_mult = 0.0;
    }

    // A second melee weapon through the real flow.
    set_next_state(&mut app, GameState::UpgradeChoice);
    step(&mut app);
    send_upgrade(&mut app, WeaponKind::MeleeSwing, 1);
    step(&mut app);
    step(&mut app);
    set_next_state(&mut app, GameState::InGame);
    step(&mut app);

    // A tanky enemy in reach of the fan so both slots land damage.
    app.world_mut().spawn((
        Enemy {
            kind: EnemyKind::MeleeRusher,
            speed: 0.0,
            health: 10_000.0,
            split_depth: 0,
        },
        Transform::from_xyz(60.0, 0.0, 0.0),
    ));
    for _ in 0..240 {
        step(&mut app);
        let stats = app.world().resource::<DamageStats>();
        let dmg = |slot: WeaponSlot| {
            stats
                .run
                .slot(slot)
                .map(|s| s.effective_damage)
                .unwrap_or(0.0)
        };
        if dmg(WeaponSlot(0)) > 0.0 && dmg(WeaponSlot(1)) > 0.0 {
            break;
        }
    }
    let stats = app.world().resource::<DamageStats>();
    let slot0 = stats.run.slot(WeaponSlot(0)).unwrap().effective_damage;
    let slot1 = stats.run.slot(WeaponSlot(1)).unwrap().effective_damage;
    assert!(slot0 > 0.0 && slot1 > 0.0, "both melee slots dealt damage");

    // Evolve: the merged-away slot's history folds into the survivor.
    app.world_mut()
        .resource_mut::<WeaponLevels>()
        .set_level(WeaponKind::MeleeSwing, 5);
    set_next_state(&mut app, GameState::UpgradeChoice);
    step(&mut app);
    send_upgrade(&mut app, WeaponKind::MeleeSwing, 0);
    step(&mut app);
    step(&mut app);

    let stats = app.world().resource::<DamageStats>();
    assert!(
        stats.run.slot(WeaponSlot(1)).is_none(),
        "merged-away slot must not leave a ghost summary row"
    );
    let pooled = stats.run.slot(WeaponSlot(0)).unwrap();
    assert!(
        (pooled.effective_damage - (slot0 + slot1)).abs() < 0.1,
        "survivor slot holds the pooled history: {} vs {} + {}",
        pooled.effective_damage,
        slot0,
        slot1
    );
}

#[test]
fn melee_lv6_evolution_merges_all_melee_slots_into_one_whirlwind() {
    let mut app = headless(42, 5, false);
    app.insert_resource(WaveConfig {
        max_waves: 5,
        spawning: false,
    });
    app.update();
    start_run_with_weapon(&mut app, WeaponKind::MeleeSwing);

    // Acquire a second melee weapon (额外近战武器 +1) through the real flow.
    set_next_state(&mut app, GameState::UpgradeChoice);
    step(&mut app);
    send_upgrade(&mut app, WeaponKind::MeleeSwing, 1);
    step(&mut app);
    step(&mut app);
    assert_eq!(current_state(&app), GameState::Shop);

    // Level the path to 5, then make the Lv6 evolution pick.
    app.world_mut()
        .resource_mut::<WeaponLevels>()
        .set_level(WeaponKind::MeleeSwing, 5);
    set_next_state(&mut app, GameState::UpgradeChoice);
    step(&mut app);
    send_upgrade(&mut app, WeaponKind::MeleeSwing, 0);
    step(&mut app);
    step(&mut app);
    assert_eq!(current_state(&app), GameState::Shop);

    // Exactly one melee weapon survives, holding the pooled damage.
    let mut weapons = app
        .world_mut()
        .query_filtered::<(Entity, &Weapon), Without<Player>>();
    let melee: Vec<_> = weapons
        .iter(app.world())
        .filter(|(_, weapon)| weapon.kind == WeaponKind::MeleeSwing)
        .collect();
    assert_eq!(melee.len(), 1, "all melee instances merge into one");
    let (kept, weapon) = melee[0];
    assert_eq!(weapon.damage, 80.0, "merged blade pools both weapons' damage");
    assert!(
        app.world().get::<Evolved>(kept).is_some(),
        "the merged weapon is the evolved one"
    );
    assert_eq!(weapon.range, 105.0, "range comes from the max-level template");

    // Back in the wave, exactly one whirlwind blade fights for that slot.
    set_next_state(&mut app, GameState::InGame);
    step(&mut app);
    let mut whirlwinds = app.world_mut().query::<&Whirlwind>();
    let blades: Vec<_> = whirlwinds.iter(app.world()).collect();
    assert_eq!(blades.len(), 1, "one merged blade, not one per slot");
    assert_eq!(blades[0].damage, 80.0);
}

#[test]
fn orbit_orbs_alternate_spin_direction_and_ordinal() {
    let mut app = headless(42, 5, false);
    app.insert_resource(WaveConfig {
        max_waves: 5,
        spawning: false,
    });
    app.update();
    start_run_with_weapon(&mut app, WeaponKind::OrbitingOrb);
    for mut weapon in app
        .world_mut()
        .query::<&mut Weapon>()
        .iter_mut(app.world_mut())
    {
        weapon.orb_count = 3;
    }
    step(&mut app);

    let mut orbs = app.world_mut().query::<&OrbitOrb>();
    let mut orbs: Vec<&OrbitOrb> = orbs.iter(app.world()).collect();
    orbs.sort_by_key(|orb| orb.ordinal);
    assert_eq!(orbs.len(), 3);
    assert_eq!(
        orbs.iter().map(|orb| orb.spin).collect::<Vec<_>>(),
        vec![1.0, -1.0, 1.0],
        "even ordinals spin counter-clockwise, odd ones clockwise"
    );

    // A few frames later the counter-rotating orbs must have moved apart.
    let angles_before: Vec<f32> = orbs.iter().map(|orb| orb.angle).collect();
    for _ in 0..12 {
        step(&mut app);
    }
    let mut orbs = app.world_mut().query::<&OrbitOrb>();
    let mut orbs: Vec<&OrbitOrb> = orbs.iter(app.world()).collect();
    orbs.sort_by_key(|orb| orb.ordinal);
    let deltas: Vec<f32> = orbs
        .iter()
        .zip(angles_before)
        .map(|(orb, before)| orb.angle - before)
        .collect();
    assert!(
        deltas[0] * deltas[1] < 0.0 && deltas[1] * deltas[2] < 0.0,
        "adjacent orbs must rotate in opposite directions, got {deltas:?}"
    );
}

#[test]
fn orbiting_orbs_pulse_and_distribute_added_orbs() {
    let mut app = headless(42, 5, false);
    app.insert_resource(WaveConfig {
        max_waves: 5,
        spawning: false,
    });
    app.update();
    start_run_with_weapon(&mut app, WeaponKind::OrbitingOrb);
    for mut weapon in app
        .world_mut()
        .query::<&mut Weapon>()
        .iter_mut(app.world_mut())
    {
        weapon.orbit_speed = 0.0;
        weapon.orb_count = 3;
    }
    step(&mut app);

    let mut orbs = app.world_mut().query::<&OrbitOrb>();
    let phases: Vec<_> = orbs.iter(app.world()).map(|orb| orb.radial_phase).collect();
    assert_eq!(phases.len(), 3);
    assert!((phases[1] - phases[0] - 1.0 / 3.0).abs() < 0.03);
    assert!((phases[2] - phases[1] - 1.0 / 3.0).abs() < 0.03);

    let before = orbs.iter(app.world()).next().unwrap().radius;
    for _ in 0..30 {
        step(&mut app);
    }
    let after = orbs.iter(app.world()).next().unwrap().radius;
    assert_ne!(before, after, "orb radius should pulse over time");
}

/// Count the live `MeleeHit` entities this frame (spawns are visible for the
/// 0.15 s they stay alive, so per-frame sampling over a full swing window
/// cannot miss one).
fn count_live_melee_hits(app: &mut App) -> usize {
    let mut melee = app.world_mut().query_filtered::<Entity, With<MeleeHit>>();
    melee.iter(app.world()).count()
}

#[test]
fn melee_swing_only_fires_when_an_enemy_is_in_range() {
    let mut app = headless(42, 5, false);
    // Isolate from wave spawns so swing triggers come only from the enemies
    // placed below.
    app.insert_resource(WaveConfig {
        max_waves: 5,
        spawning: false,
    });
    app.update();
    start_run_with_weapon(&mut app, WeaponKind::MeleeSwing);

    // No enemies at all: across two swing cooldown cycles (~1.8 s) not a
    // single MeleeHit may spawn — the swing must not flash over empty ground.
    let mut melee_seen = 0;
    for _ in 0..132 {
        step(&mut app);
        melee_seen += count_live_melee_hits(&mut app);
    }
    assert_eq!(melee_seen, 0, "no swing with no enemies on the field");

    // A tanky enemy just past the swing reach (range 90 + body 22 < 200):
    // still no swing, the gate is a reach check, not a "has any enemy" check.
    let far = app
        .world_mut()
        .spawn((
            Enemy {
                kind: EnemyKind::MeleeRusher,
                speed: 0.0,
                health: f32::MAX,
                split_depth: 0,
            },
            Transform::from_xyz(200.0, 0.0, 0.0),
        ))
        .id();
    for _ in 0..66 {
        step(&mut app);
        melee_seen += count_live_melee_hits(&mut app);
    }
    assert_eq!(melee_seen, 0, "no swing while the enemy is out of reach");

    // The enemy steps into reach (x=70 < 90 + 31): swings begin. The cooldown
    // has been ticking the whole time, so the first in-range boundary fires
    // within one cycle (~0.9 s); assert on landed hits to prove the swing
    // both spawned and connected.
    app.world_mut().despawn(far);
    app.world_mut().spawn((
        Enemy {
            kind: EnemyKind::MeleeRusher,
            speed: 0.0,
            health: 100.0,
            split_depth: 0,
        },
        Transform::from_xyz(70.0, 0.0, 0.0),
    ));
    let mut steps = 0;
    while steps < 120 {
        step(&mut app);
        steps += 1;
        if app.world().resource::<Recording>().hits >= 1 {
            break;
        }
    }
    assert!(
        app.world().resource::<Recording>().hits >= 1,
        "an in-range enemy must trigger the swing"
    );
}

#[test]
fn wave_end_heals_half_of_max_hp() {
    let mut app = headless(42, 3, false);
    // No wave spawns: the only HP change must come from wave recovery.
    app.insert_resource(WaveConfig {
        max_waves: 3,
        spawning: false,
    });
    app.update();
    start_run(&mut app);

    // Scenario setup: damage the player directly to 40/100.
    let mut players = app.world_mut().query_filtered::<Entity, With<Player>>();
    let player = players
        .single(app.world())
        .expect("player spawned on wave 1");
    app.world_mut().entity_mut(player).insert(Health {
        max: 100.0,
        current: 40.0,
    });

    // Wait for wave 1's timer (30 s) to elapse and the upgrade pick to open.
    let mut steps = 0;
    while current_state(&app) != GameState::UpgradeChoice && steps < 2_400 {
        step(&mut app);
        steps += 1;
    }
    assert_eq!(
        current_state(&app),
        GameState::UpgradeChoice,
        "wave 1 completed"
    );
    let mut players = app.world_mut().query_filtered::<&Health, With<Player>>();
    let health = players
        .single(app.world())
        .expect("player persists across waves");
    assert_eq!(health.max, 100.0);
    assert_eq!(health.current, 90.0, "wave end recovers 50% of max HP");

    // The mandatory pick confirms into the Shop (apply frame + transition).
    pick_any_upgrade(&mut app);
    step(&mut app);
    step(&mut app);
    assert_eq!(current_state(&app), GameState::Shop);
}

/// Spawn a material drop directly on the field (scenario setup).
fn spawn_material_at(app: &mut App, x: f32, y: f32) -> Entity {
    app.world_mut()
        .spawn((Material { value: 1 }, Transform::from_xyz(x, y, 0.0)))
        .id()
}

/// Count the live `Material` entities on the field.
fn count_materials(app: &mut App) -> usize {
    let mut q = app.world_mut().query_filtered::<Entity, With<Material>>();
    q.iter(app.world()).count()
}

#[test]
fn material_within_attraction_radius_flies_to_player_and_is_collected() {
    let mut app = headless(42, 5, false);
    // Isolate from wave spawns; no enemies means no stray drops.
    app.insert_resource(WaveConfig {
        max_waves: 5,
        spawning: false,
    });
    app.update();
    start_run(&mut app);

    // One material inside the base attraction radius (26), one far outside.
    spawn_material_at(&mut app, 24.0, 0.0);
    let far = spawn_material_at(&mut app, 200.0, 0.0);

    let mut steps = 0;
    while app.world().resource::<Materials>().count == 0 && steps < 60 {
        step(&mut app);
        steps += 1;
    }
    assert_eq!(
        app.world().resource::<Materials>().count,
        1,
        "material inside the attraction radius was collected"
    );
    assert_eq!(
        count_materials(&mut app),
        1,
        "the far material still lies on the field"
    );
    let mut far_q = app.world_mut().query::<(&Transform, &Material)>();
    let (transform, _) = far_q.get(app.world(), far).expect("far material alive");
    assert!(
        (transform.translation.x - 200.0).abs() < 0.01,
        "material outside the attraction radius must not move"
    );

    // Boost the radius past the far material's distance (scenario setup):
    // it must now fly — its position changes toward the player — then land.
    let mut players = app
        .world_mut()
        .query_filtered::<&mut PlayerStats, With<Player>>();
    players
        .single_mut(app.world_mut())
        .unwrap()
        .attraction_radius = 220.0;
    step(&mut app);
    let (transform, _) = far_q.get(app.world(), far).unwrap();
    assert!(
        transform.translation.x < 200.0,
        "attracted material flies toward the player"
    );
    let mut steps = 0;
    while count_materials(&mut app) > 0 && steps < 120 {
        step(&mut app);
        steps += 1;
    }
    assert_eq!(count_materials(&mut app), 0, "attracted material landed");
    assert_eq!(app.world().resource::<Materials>().count, 2);
    let pickups = &app.world().resource::<Recording>().pickups;
    assert_eq!(pickups, &vec![1, 1], "two pickups of value 1 each");
}

#[test]
fn shop_item_extends_the_attraction_radius() {
    let mut app = headless(42, 5, false);
    app.insert_resource(WaveConfig {
        max_waves: 5,
        spawning: false,
    });
    app.update();
    start_run(&mut app);

    // Buy the magnet item in the shop (vacuum is a no-op: no materials yet).
    set_next_state(&mut app, GameState::Shop);
    step(&mut app);
    app.world_mut().resource_mut::<Materials>().count = 30;
    let magnet = catalog_index("Magnet Arm");
    let magnet_cost = game_core::shop::SHOP_ITEMS[magnet].cost;
    send_purchase(&mut app, magnet);
    step(&mut app);
    let mut players = app
        .world_mut()
        .query_filtered::<&PlayerStats, With<Player>>();
    let stats = players.single(app.world()).expect("player in shop");
    assert_eq!(
        stats.attraction_radius, 50.0,
        "Magnet Arm raised the attraction radius from 26 to 50"
    );

    // Back in the wave: a drop between the old and new radius now collects.
    set_next_state(&mut app, GameState::InGame);
    step(&mut app);
    spawn_material_at(&mut app, 40.0, 0.0); // 26 < 40 <= 50
    let mut steps = 0;
    let expected = 30 - magnet_cost + 1;
    while app.world().resource::<Materials>().count < expected && steps < 120 {
        step(&mut app);
        steps += 1;
    }
    assert_eq!(
        app.world().resource::<Materials>().count,
        expected,
        "boosted attraction radius pulled in the drop"
    );
}

#[test]
fn wave_end_vacuums_all_remaining_materials_into_the_wallet() {
    let mut app = headless(42, 3, false);
    app.insert_resource(WaveConfig {
        max_waves: 3,
        spawning: false,
    });
    app.update();
    start_run(&mut app);

    // Three drops far outside the attraction radius: they survive the wave
    // and must all be credited when the wave closes.
    spawn_material_at(&mut app, 300.0, 0.0);
    spawn_material_at(&mut app, -300.0, 0.0);
    spawn_material_at(&mut app, 0.0, 300.0);

    // Nothing collected mid-wave (invariant holds while still in the wave).
    let mut steps = 0;
    while steps < 2_400 {
        step(&mut app);
        steps += 1;
        if current_state(&app) != GameState::InGame {
            break;
        }
        assert_eq!(
            app.world().resource::<Materials>().count,
            0,
            "unattracted materials must stay uncollected mid-wave"
        );
    }
    assert_eq!(
        current_state(&app),
        GameState::UpgradeChoice,
        "wave 1 completed"
    );
    assert_eq!(
        app.world().resource::<Materials>().count,
        3,
        "wave end vacuum credited every remaining material"
    );
    assert_eq!(count_materials(&mut app), 0, "no material entities remain");
    let pickups = app.world().resource::<Recording>().pickups.to_vec();
    assert_eq!(pickups, vec![1, 1, 1], "vacuum announced each pickup");

    // The mandatory pick confirms into the Shop (apply frame + transition).
    pick_any_upgrade(&mut app);
    step(&mut app);
    step(&mut app);
    assert_eq!(current_state(&app), GameState::Shop);
}

#[test]
fn projectile_knockback_pushes_enemy_then_decays() {
    let mut app = headless(42, 5, false);
    // Isolate from wave spawns so only the enemy below is involved.
    app.insert_resource(WaveConfig {
        max_waves: 5,
        spawning: false,
    });
    app.update();
    start_run_with_weapon(&mut app, WeaponKind::PiercingProjectile);

    // A tanky, motionless enemy downrange on +X: the projectile hits it and
    // the knockback must shove it further along +X, then decay to rest.
    app.world_mut().spawn((
        Enemy {
            kind: EnemyKind::MeleeRusher,
            speed: 0.0,
            health: 1.0e6,
            split_depth: 0,
        },
        Transform::from_xyz(100.0, 0.0, 0.0),
    ));

    let mut steps = 0;
    while app.world().resource::<Recording>().hits < 1 && steps < 180 {
        step(&mut app);
        steps += 1;
    }
    assert!(
        app.world().resource::<Recording>().hits >= 1,
        "projectile must land"
    );
    // Freeze the weapon so no second volley interferes with the decay window.
    let mut weapons = app.world_mut().query::<&mut Weapon>();
    for mut weapon in weapons.iter_mut(app.world_mut()) {
        if weapon.kind == WeaponKind::PiercingProjectile {
            weapon.cooldown = Timer::from_seconds(1.0e9, TimerMode::Repeating);
        }
    }
    let mut enemies = app.world_mut().query::<(&Transform, &Enemy)>();
    let (transform, _) = enemies.single(app.world()).expect("enemy alive after hit");
    let x_at_hit = transform.translation.x;
    assert!(x_at_hit > 100.0, "knockback pushed the enemy along +X");

    // 0.5 s later the drift is nearly spent; 1 s later it has stopped.
    for _ in 0..30 {
        step(&mut app);
    }
    let (transform, _) = enemies.single(app.world()).unwrap();
    let x_05s = transform.translation.x;
    assert!(
        x_05s - x_at_hit > 2.0,
        "still drifting shortly after the hit"
    );
    for _ in 0..30 {
        step(&mut app);
    }
    let (transform, _) = enemies.single(app.world()).unwrap();
    let x_1s = transform.translation.x;
    assert!(
        x_1s - x_05s < 2.0,
        "knockback decays: moved {} in the second half-second",
        x_1s - x_05s
    );
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
        if current_state(&app) == GameState::UpgradeChoice {
            pick_any_upgrade(&mut app);
        }
        if current_state(&app) == GameState::Shop {
            shop_visits += 1;
            // Both before and after a Shop→InGame re-entry the starting
            // loadout must still be exactly one slot (no re-grant).
            let mut weapons = app.world_mut().query::<&Weapon>();
            assert_eq!(
                weapons.iter(app.world()).count(),
                1,
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
    assert!(
        shop_visits >= 2,
        "should have re-entered the shop at least once"
    );
    let rec = app.world().resource::<Recording>();
    assert_eq!(rec.wave_starts, vec![1, 2, 3], "waves ascend from 1");
    assert_eq!(
        rec.wave_completes,
        vec![1, 2, 3],
        "each wave completes (the last, then Victory)"
    );
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

/// Write an upgrade pick through the same message path the UI and AI use.
fn send_upgrade(app: &mut App, kind: WeaponKind, option: usize) {
    app.world_mut()
        .resource_mut::<Messages<UpgradeSelected>>()
        .write(UpgradeSelected { kind, option });
}

/// Pick option A for the equipped path (what the test AI does).
fn pick_any_upgrade(app: &mut App) {
    let kind = app
        .world()
        .resource::<StartingWeapon>()
        .selected
        .expect("starting weapon selected");
    send_upgrade(app, kind, 0);
}

/// Scenario setup: shrink the current wave timer so the next step ends it.
fn rush_wave_end(app: &mut App) {
    app.world_mut().resource_mut::<Wave>().wave_timer = Timer::from_seconds(0.01, TimerMode::Once);
}

/// Scenario setup + player agency: put `kind` at Lv5, then make the Lv6
/// evolution pick through the real UpgradeChoice flow (levels resource write
/// is scenario setup; the pick itself goes through the message path).
const DERIVED_DAMAGE_SLOT: WeaponSlot = WeaponSlot(3);

fn evolve_via_choice(app: &mut App, kind: WeaponKind) {
    app.world_mut()
        .resource_mut::<WeaponLevels>()
        .set_level(kind, 5);
    set_next_state(app, GameState::UpgradeChoice);
    step(app);
    assert_eq!(current_state(app), GameState::UpgradeChoice);
    send_upgrade(app, kind, 0);
    // Two frames: the core applies the pick in UpgradeChoice, the transition
    // to Shop lands at the start of the next frame.
    step(app);
    step(app);
    assert_eq!(current_state(app), GameState::Shop);
    let mut slots = app.world_mut().query::<&mut WeaponSlot>();
    let mut slot = slots
        .single_mut(app.world_mut())
        .expect("single equipped weapon");
    *slot = DERIVED_DAMAGE_SLOT;
    set_next_state(app, GameState::InGame);
    step(app);
}

#[test]
fn upgrade_choice_flow_and_stat_application() {
    let mut app = headless(42, 5, false);
    app.insert_resource(WaveConfig {
        max_waves: 5,
        spawning: false,
    });
    app.update();
    start_run(&mut app);

    // Wave 1 ends into UpgradeChoice, not Shop.
    rush_wave_end(&mut app);
    let mut steps = 0;
    while current_state(&app) != GameState::UpgradeChoice && steps < 120 {
        step(&mut app);
        steps += 1;
    }
    assert_eq!(
        current_state(&app),
        GameState::UpgradeChoice,
        "wave end must open UpgradeChoice, not Shop"
    );
    assert!(
        !app.world()
            .resource::<Recording>()
            .states
            .contains(&GameState::Shop),
        "Shop must not be reached before the pick"
    );

    // Pick Piercing L2 B (Cooldown -15%): stats change, level +1, then Shop.
    send_upgrade(&mut app, WeaponKind::PiercingProjectile, 1);
    step(&mut app);
    step(&mut app);
    assert_eq!(
        current_state(&app),
        GameState::Shop,
        "the pick confirms into the Shop"
    );
    assert_eq!(
        app.world()
            .resource::<WeaponLevels>()
            .level(WeaponKind::PiercingProjectile),
        2,
        "level incremented"
    );
    assert_eq!(
        app.world()
            .resource::<WeaponLevels>()
            .level(WeaponKind::MeleeSwing),
        1,
        "other paths untouched"
    );
    let mut weapons = app.world_mut().query::<&Weapon>();
    for weapon in weapons.iter(app.world()) {
        match weapon.kind {
            WeaponKind::PiercingProjectile => {
                let cooldown = weapon.cooldown.duration().as_secs_f32();
                assert!(
                    (cooldown - 0.51).abs() < 1e-6,
                    "cooldown -15% applied, got {cooldown}"
                );
                assert_eq!(weapon.damage, 24.0, "unpicked stat untouched");
            }
            WeaponKind::MeleeSwing => {
                assert_eq!(weapon.damage, 25.0, "other weapons untouched");
            }
            WeaponKind::OrbitingOrb => {}
        }
    }

    // Wave 2: continue the equipped Piercing path with L3 A (Speed +20%).
    set_next_state(&mut app, GameState::InGame);
    step(&mut app);
    rush_wave_end(&mut app);
    steps = 0;
    while current_state(&app) != GameState::UpgradeChoice && steps < 120 {
        step(&mut app);
        steps += 1;
    }
    send_upgrade(&mut app, WeaponKind::PiercingProjectile, 0);
    step(&mut app);
    step(&mut app);
    assert_eq!(current_state(&app), GameState::Shop);
    assert_eq!(
        app.world()
            .resource::<WeaponLevels>()
            .level(WeaponKind::PiercingProjectile),
        3
    );
    assert_eq!(
        app.world()
            .resource::<WeaponLevels>()
            .level(WeaponKind::MeleeSwing),
        1,
        "unowned path remains untouched"
    );
    let mut weapons = app.world_mut().query::<&Weapon>();
    for weapon in weapons.iter(app.world()) {
        if weapon.kind == WeaponKind::PiercingProjectile {
            assert!(
                (weapon.projectile_speed - 504.0).abs() < 1e-4,
                "projectile speed +20% applied, got {}",
                weapon.projectile_speed
            );
        }
    }
}

#[test]
fn melee_lv6_whirlwind_hits_continuously_without_swing_rhythm() {
    let mut app = headless(42, 5, false);
    app.insert_resource(WaveConfig {
        max_waves: 5,
        spawning: false,
    });
    app.update();
    start_run_with_weapon(&mut app, WeaponKind::MeleeSwing);
    buff_player(&mut app);
    evolve_via_choice(&mut app, WeaponKind::MeleeSwing);

    // A tanky enemy well inside the blade reach (radius 90 + body 22).
    let enemy_entity = app
        .world_mut()
        .spawn((
            Enemy {
                kind: EnemyKind::MeleeRusher,
                speed: 0.0,
                health: 100_000.0,
                split_depth: 0,
            },
            Transform::from_xyz(40.0, 0.0, 0.0),
        ))
        .id();

    let mut first_hit_frame = None;
    let mut melee_seen = 0;
    let mut steps = 0;
    while steps < 90 {
        step(&mut app);
        steps += 1;
        melee_seen += count_live_melee_hits(&mut app);
        if first_hit_frame.is_none() && app.world().resource::<Recording>().hits >= 1 {
            first_hit_frame = Some(steps);
        }
    }
    let rec = app.world().resource::<Recording>();
    let first = first_hit_frame.expect("the whirlwind must strike");
    // A plain swing would not land before its 0.9 s cooldown (~54 frames);
    // the whirlwind strikes immediately and re-hits every 0.4 s.
    assert!(
        first <= 12,
        "first strike must be immediate (frame {first}), not swing-rhythm gated"
    );
    assert!(
        rec.hits >= 2,
        "continuous re-hits within 1.5 s, got {}",
        rec.hits
    );
    assert_eq!(
        melee_seen, 0,
        "no discrete MeleeHit hitboxes after the evolution"
    );
    let enemy = app
        .world()
        .get::<Enemy>(enemy_entity)
        .expect("tanky enemy survives");
    let attributed = app
        .world()
        .resource::<DamageStats>()
        .run
        .slot(DERIVED_DAMAGE_SLOT)
        .expect("whirlwind damage attributed to source slot")
        .effective_damage;
    assert!(
        (attributed - (100_000.0 - enemy.health)).abs() < 0.1,
        "all whirlwind damage stays on the originating slot"
    );
    assert_eq!(
        app.world()
            .resource::<DamageStats>()
            .run
            .slot(WeaponSlot(0))
            .expect("starting slot remains registered")
            .effective_damage,
        0.0,
        "whirlwind must not fall back to slot zero"
    );
}

#[test]
fn piercing_lv6_splitshot_spawns_fan_shards_on_first_hit() {
    let mut app = headless(42, 5, false);
    app.insert_resource(WaveConfig {
        max_waves: 5,
        spawning: false,
    });
    app.update();
    start_run_with_weapon(&mut app, WeaponKind::PiercingProjectile);
    evolve_via_choice(&mut app, WeaponKind::PiercingProjectile);

    // A tanky enemy downrange on +X.
    let enemy_entity = app
        .world_mut()
        .spawn((
            Enemy {
                kind: EnemyKind::MeleeRusher,
                speed: 0.0,
                health: 100_000.0,
                split_depth: 0,
            },
            Transform::from_xyz(300.0, 0.0, 0.0),
        ))
        .id();

    let mut saw_first_hit = false;
    let mut saw_shards = false;
    let mut steps = 0;
    while steps < 180 {
        step(&mut app);
        steps += 1;
        let hits = app.world().resource::<Recording>().hits;
        if hits >= 1 {
            saw_first_hit = true;
        }
        if saw_first_hit {
            let mut projectiles = app.world_mut().query_filtered::<Entity, With<Projectile>>();
            saw_shards |= projectiles.iter(app.world()).count() >= 4;
        }
        if saw_shards && hits >= 4 {
            break; // parent + all 3 shards connected
        }
    }
    assert!(saw_first_hit, "the parent projectile must land");
    assert!(
        saw_shards,
        "first hit must fan out into 3 shards (parent + 3)"
    );

    // Shards inherit 50% damage: parent (10) + at least one shard (5).
    let health = app
        .world()
        .get::<Enemy>(enemy_entity)
        .expect("tanky enemy alive")
        .health;
    assert!(
        100_000.0 - health >= 15.0 - 1e-3,
        "shard damage must land beyond the parent's hit, total {}",
        100_000.0 - health
    );
    let attributed = app
        .world()
        .resource::<DamageStats>()
        .run
        .slot(DERIVED_DAMAGE_SLOT)
        .expect("splitshot damage attributed to source slot")
        .effective_damage;
    assert!(
        (attributed - (100_000.0 - health)).abs() < 0.1,
        "parent and shard damage stay on the originating slot"
    );
    assert_eq!(
        app.world()
            .resource::<DamageStats>()
            .run
            .slot(WeaponSlot(0))
            .expect("starting slot remains registered")
            .effective_damage,
        0.0,
        "splitshot must not fall back to slot zero"
    );
}

#[test]
fn orb_lv6_bomber_orb_explodes_on_contact_and_respawns() {
    let mut app = headless(42, 5, false);
    app.insert_resource(WaveConfig {
        max_waves: 5,
        spawning: false,
    });
    app.update();
    start_run_with_weapon(&mut app, WeaponKind::OrbitingOrb);
    evolve_via_choice(&mut app, WeaponKind::OrbitingOrb);

    // A contact enemy in the orb's path, plus a witness far beyond the orb's
    // contact reach (31) but inside the explosion radius (90 around the orb).
    let contact = app
        .world_mut()
        .spawn((
            Enemy {
                kind: EnemyKind::MeleeRusher,
                speed: 0.0,
                health: 100_000.0,
                split_depth: 0,
            },
            Transform::from_xyz(70.0, 0.0, 0.0),
        ))
        .id();
    let witness = app
        .world_mut()
        .spawn((
            Enemy {
                kind: EnemyKind::MeleeRusher,
                speed: 0.0,
                health: 1000.0,
                split_depth: 0,
            },
            Transform::from_xyz(140.0, 0.0, 0.0),
        ))
        .id();

    let mut exploded = false;
    let mut steps = 0;
    while steps < 90 {
        step(&mut app);
        steps += 1;
        let mut enemies = app.world_mut().query::<&Enemy>();
        if let Ok(enemy) = enemies.get(app.world(), witness) {
            if enemy.health < 1000.0 {
                exploded = true;
                break;
            }
        }
    }
    assert!(
        exploded,
        "the AOE explosion must reach the witness at x=140 (orb contact cannot)"
    );
    let contact_health = app
        .world()
        .get::<Enemy>(contact)
        .expect("contact enemy survives")
        .health;
    let witness_health = app
        .world()
        .get::<Enemy>(witness)
        .expect("witness survives")
        .health;
    let attributed = app
        .world()
        .resource::<DamageStats>()
        .run
        .slot(DERIVED_DAMAGE_SLOT)
        .expect("bomber damage attributed to source slot")
        .effective_damage;
    let removed_health = 100_000.0 - contact_health + 1000.0 - witness_health;
    assert!(
        (attributed - removed_health).abs() < 0.1,
        "contact and explosion damage stay on the originating slot"
    );
    assert_eq!(
        app.world()
            .resource::<DamageStats>()
            .run
            .slot(WeaponSlot(0))
            .expect("starting slot remains registered")
            .effective_damage,
        0.0,
        "bomber damage must not fall back to slot zero"
    );

    // The orb stays absent for its 0.6 second respawn window.
    app.world_mut().despawn(contact);
    for _ in 0..30 {
        step(&mut app);
    }
    let mut orbs = app
        .world_mut()
        .query_filtered::<Entity, (With<OrbitOrb>, With<BomberOrb>)>();
    assert_eq!(
        orbs.iter(app.world()).count(),
        0,
        "BomberOrb must remain absent before 0.6 seconds"
    );
    for _ in 0..10 {
        step(&mut app);
    }
    assert!(
        orbs.iter(app.world()).count() >= 1,
        "a BomberOrb returns after the 0.6 second delay"
    );
}
