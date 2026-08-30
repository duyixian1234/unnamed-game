//! Wave resource, progression, and edge spawning.

use bevy::ecs::message::Message;
use bevy::math::Vec2;
use bevy::prelude::*;
use rand::Rng;

use crate::damage::DamageStats;
use crate::economy::Material;
use crate::enemy::{Enemy, EnemyKind, EnemySpawned};
use crate::player::{Health, Player};
use crate::rng::{GlobalRng, RandomDraw};
use crate::weapon::{BomberExplosion, MeleeHit, OrbRespawn, OrbitOrb, Projectile, Whirlwind};
use crate::{GameState, RunStarted};

/// Half-extents of the play field; enemies spawn just outside these edges.
/// Sized to match the visible world at camera scale 0.7 (1280x720 viewport ->
/// ~896x504 world), so the player moves across a fixed, fully-visible screen.
pub const FIELD_HALF_WIDTH: f32 = 448.0;
pub const FIELD_HALF_HEIGHT: f32 = 252.0;

/// Base time between enemy spawns within a wave (scales down as waves rise).
const BASE_SPAWN_INTERVAL: f32 = 1.1;

/// How long each wave lasts (seconds); difficulty escalates via spawn rate and
/// enemy stats.
const WAVE_DURATION: f32 = 30.0;

/// Fraction of max HP recovered when a wave ends (CONTEXT.md: wave recovery).
const WAVE_END_HEAL_FRACTION: f32 = 0.5;

/// How many waves a Run lasts. Configurable so tests can run short
/// full-flow games (CONTEXT.md: wave count configurable). `spawning` can be
/// switched off so weapon/combat tests run in InGame without wave spawns.
#[derive(Resource, Debug, Clone, Copy)]
pub struct WaveConfig {
    pub max_waves: u32,
    pub spawning: bool,
}

impl Default for WaveConfig {
    fn default() -> Self {
        Self {
            max_waves: 20,
            spawning: true,
        }
    }
}

/// A wave began.
#[derive(Message, Debug, Clone, Copy)]
pub struct WaveStarted {
    pub number: u32,
}

/// A wave's timer elapsed and it completed.
#[derive(Message, Debug, Clone, Copy)]
pub struct WaveCompleted {
    pub number: u32,
}

/// The active wave state: which wave we're on and its timers.
#[derive(Resource)]
pub struct Wave {
    /// 1-based current wave number.
    pub number: u32,
    /// Counts down; at zero we spawn one enemy and reset (rate scales by wave).
    pub spawn_timer: Timer,
    /// Counts down to the end of the current wave.
    pub wave_timer: Timer,
}

impl Default for Wave {
    fn default() -> Self {
        Self {
            number: 1,
            spawn_timer: Timer::from_seconds(BASE_SPAWN_INTERVAL, TimerMode::Repeating),
            wave_timer: Timer::from_seconds(WAVE_DURATION, TimerMode::Once),
        }
    }
}

/// Plugin for wave lifecycle, progression, and edge spawning.
pub struct WavesPlugin;

impl Plugin for WavesPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Wave>()
            .init_resource::<WaveConfig>()
            .add_message::<WaveStarted>()
            .add_message::<WaveCompleted>()
            .add_systems(OnEnter(GameState::InGame), (start_wave, announce_wave))
            .add_systems(OnEnter(GameState::StartingWeaponChoice), reset_run)
            .add_systems(OnExit(GameState::InGame), clear_combat_entities)
            .add_systems(OnEnter(GameState::Victory), clear_player)
            .add_systems(OnEnter(GameState::Defeat), clear_player)
            .add_systems(
                Update,
                // `spawn_from_edges` consumes the GlobalRng: it must be
                // explicitly ordered (ADR-0005). `advance_wave` is chained
                // after it for a deterministic frame.
                (
                    spawn_from_edges.in_set(RandomDraw),
                    advance_wave.after(spawn_from_edges),
                )
                    .run_if(in_state(GameState::InGame)),
            );
    }
}

/// Leaving InGame (to UpgradeChoice, Victory, or Defeat) clears the field of
/// enemies, projectiles, orbs, melee hitboxes, whirlwinds, and dropped
/// materials. The player is kept across waves but despawned on Victory/Defeat
/// by `clear_player`.
#[allow(clippy::too_many_arguments)]
fn clear_combat_entities(
    mut commands: Commands,
    enemies: Query<Entity, With<Enemy>>,
    projectiles: Query<Entity, With<Projectile>>,
    orbs: Query<Entity, With<OrbitOrb>>,
    melee: Query<Entity, With<MeleeHit>>,
    whirlwinds: Query<Entity, With<Whirlwind>>,
    explosions: Query<Entity, With<BomberExplosion>>,
    respawns: Query<Entity, With<OrbRespawn>>,
    materials: Query<Entity, With<Material>>,
) {
    for entity in enemies
        .iter()
        .chain(projectiles.iter())
        .chain(orbs.iter())
        .chain(melee.iter())
        .chain(whirlwinds.iter())
        .chain(explosions.iter())
        .chain(respawns.iter())
        .chain(materials.iter())
    {
        commands.entity(entity).despawn();
    }
}

/// The player is removed on Victory/Defeat so a fresh run spawns a new one.
fn clear_player(mut commands: Commands, players: Query<Entity, With<Player>>) {
    for entity in &players {
        commands.entity(entity).despawn();
    }
}

/// Enter a wave: reset the timers (keeping the current wave number, which was
/// incremented by `advance_wave` or is 1 on a fresh run).
fn start_wave(mut wave: ResMut<Wave>) {
    wave.spawn_timer = Timer::from_seconds(
        (BASE_SPAWN_INTERVAL / (1.0 + wave.number as f32 * 0.05)).max(0.25),
        TimerMode::Repeating,
    );
    wave.wave_timer = Timer::from_seconds(WAVE_DURATION, TimerMode::Once);
}

/// Announce the wave (and a fresh Run on wave 1) via messages.
fn announce_wave(
    wave: Res<Wave>,
    mut wave_writer: MessageWriter<WaveStarted>,
    mut run_writer: MessageWriter<RunStarted>,
) {
    wave_writer.write(WaveStarted {
        number: wave.number,
    });
    if wave.number == 1 {
        run_writer.write(RunStarted);
    }
}

/// Fresh run: reset the wave counter back to 1.
fn reset_run(mut wave: ResMut<Wave>) {
    wave.number = 1;
}

/// When a wave's time elapses, recover 50% of max HP (wave recovery), then
/// advance to the UpgradeChoice (mandatory weapon upgrade pick) or to Victory.
fn advance_wave(
    time: Res<Time>,
    mut wave: ResMut<Wave>,
    config: Res<WaveConfig>,
    mut next_state: ResMut<NextState<GameState>>,
    mut completed_writer: MessageWriter<WaveCompleted>,
    mut damage_stats: ResMut<DamageStats>,
    mut players: Query<&mut Health, With<Player>>,
) {
    wave.wave_timer.tick(time.delta());
    if !wave.wave_timer.is_finished() {
        return;
    }
    if let Ok(mut health) = players.single_mut() {
        health.current = (health.current + health.max * WAVE_END_HEAL_FRACTION).min(health.max);
    }
    completed_writer.write(WaveCompleted {
        number: wave.number,
    });
    damage_stats.mark_wave_completed();
    if wave.number >= config.max_waves {
        next_state.set(GameState::Victory);
    } else {
        wave.number += 1;
        next_state.set(GameState::UpgradeChoice);
    }
}

/// Spawn an enemy from a random edge whenever the wave timer fires.
fn spawn_from_edges(
    time: Res<Time>,
    mut wave: ResMut<Wave>,
    config: Res<WaveConfig>,
    mut commands: Commands,
    mut rng: ResMut<GlobalRng>,
    mut spawn_writer: MessageWriter<EnemySpawned>,
) {
    if !config.spawning {
        return;
    }
    wave.spawn_timer.tick(time.delta());
    if !wave.spawn_timer.just_finished() {
        return;
    }

    let position = random_edge_position(&mut rng.0);
    let (kind, speed, health) = random_enemy_spec(&mut rng.0, wave.number);
    let size = match kind {
        EnemyKind::MeleeRusher => 62.0,
        EnemyKind::SpeedBurster => 48.0,
        EnemyKind::Splitter => 70.0,
    };

    spawn_writer.write(EnemySpawned { kind });
    commands.spawn((
        Enemy {
            kind,
            speed,
            health,
            split_depth: if kind == EnemyKind::Splitter { 2 } else { 0 },
        },
        Transform::from_translation(position.extend(0.0))
            .with_scale(Vec3::splat(size / crate::player::ATLAS_CELL_PX)),
    ));
}

/// Pick a random enemy type with wave-scaled stats.
fn random_enemy_spec(rng: &mut impl Rng, wave: u32) -> (EnemyKind, f32, f32) {
    let base = wave as f32;
    match rng.random_range(0..3) {
        0 => (
            EnemyKind::MeleeRusher,
            120.0 + base * 8.0,
            30.0 + base * 2.0,
        ),
        1 => (EnemyKind::SpeedBurster, 200.0 + base * 6.0, 18.0 + base),
        _ => (EnemyKind::Splitter, 140.0 + base * 6.0, 40.0 + base * 2.0),
    }
}

fn random_edge_position(rng: &mut impl Rng) -> Vec2 {
    // Pick an edge, then a point along it.
    match rng.random_range(0..4) {
        0 => Vec2::new(
            rng.random_range(-FIELD_HALF_WIDTH..=FIELD_HALF_WIDTH),
            FIELD_HALF_HEIGHT + 24.0,
        ),
        1 => Vec2::new(
            rng.random_range(-FIELD_HALF_WIDTH..=FIELD_HALF_WIDTH),
            -FIELD_HALF_HEIGHT - 24.0,
        ),
        2 => Vec2::new(
            FIELD_HALF_WIDTH + 24.0,
            rng.random_range(-FIELD_HALF_HEIGHT..=FIELD_HALF_HEIGHT),
        ),
        _ => Vec2::new(
            -FIELD_HALF_WIDTH - 24.0,
            rng.random_range(-FIELD_HALF_HEIGHT..=FIELD_HALF_HEIGHT),
        ),
    }
}
