//! Wave resource and edge spawning.

use bevy::math::Vec2;
use bevy::prelude::*;

use crate::game::assets::{atlas_sprite, SpriteAssets, ATLAS_CELL};
use crate::game::economy::Material;
use crate::game::enemy::{Enemy, EnemyKind};
use crate::game::player::Player;
use crate::game::weapon::{MeleeHit, OrbitOrb, Projectile};
use crate::game::GameState;

/// Half-extents of the play field; enemies spawn just outside these edges.
pub const FIELD_HALF_WIDTH: f32 = 900.0;
pub const FIELD_HALF_HEIGHT: f32 = 560.0;

/// Base time between enemy spawns within a wave (scales down as waves rise).
const BASE_SPAWN_INTERVAL: f32 = 1.1;

/// Number of waves to survive for victory (per CONTEXT.md).
pub const MAX_WAVES: u32 = 20;

/// How long each wave lasts (seconds); difficulty escalates via spawn rate and
/// enemy stats.
const WAVE_DURATION: f32 = 30.0;

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
            .add_systems(OnEnter(GameState::InGame), start_wave)
            .add_systems(OnEnter(GameState::MainMenu), reset_run)
            .add_systems(
                OnExit(GameState::InGame),
                clear_combat_entities,
            )
            .add_systems(OnEnter(GameState::Victory), clear_player)
            .add_systems(OnEnter(GameState::Defeat), clear_player)
            .add_systems(
                Update,
                (spawn_from_edges, advance_wave).run_if(in_state(GameState::InGame)),
            );
    }
}

/// Leaving InGame (to Shop, Victory, or Defeat) clears the field of enemies,
/// projectiles, orbs, melee hitboxes, and dropped materials. The player is kept
/// across waves but despawned on Victory/Defeat by `clear_player`.
fn clear_combat_entities(
    mut commands: Commands,
    enemies: Query<Entity, With<Enemy>>,
    projectiles: Query<Entity, With<Projectile>>,
    orbs: Query<Entity, With<OrbitOrb>>,
    melee: Query<Entity, With<MeleeHit>>,
    materials: Query<Entity, With<Material>>,
) {
    for entity in enemies
        .iter()
        .chain(projectiles.iter())
        .chain(orbs.iter())
        .chain(melee.iter())
        .chain(materials.iter())
    {
        commands.entity(entity).despawn();
    }
}

/// The player is removed on Victory/Defeat so a fresh run spawns a new one.
fn clear_player(
    mut commands: Commands,
    players: Query<Entity, With<Player>>,
) {
    for entity in &players {
        commands.entity(entity).despawn();
    }
}

/// Enter a wave: reset the timers (keeping the current wave number, which was
/// incremented by `advance_wave` or is 1 on a fresh run).
fn start_wave(mut wave: ResMut<Wave>) {
    wave.spawn_timer =
        Timer::from_seconds((BASE_SPAWN_INTERVAL / (1.0 + wave.number as f32 * 0.05)).max(0.25), TimerMode::Repeating);
    wave.wave_timer = Timer::from_seconds(WAVE_DURATION, TimerMode::Once);
}

/// Fresh run: reset the wave counter back to 1.
fn reset_run(mut wave: ResMut<Wave>) {
    wave.number = 1;
}

/// When a wave's time elapses, advance to the Shop or to Victory.
fn advance_wave(
    time: Res<Time>,
    mut wave: ResMut<Wave>,
    mut next_state: ResMut<NextState<GameState>>,
) {
    wave.wave_timer.tick(time.delta());
    if !wave.wave_timer.is_finished() {
        return;
    }
    if wave.number >= MAX_WAVES {
        next_state.set(GameState::Victory);
    } else {
        wave.number += 1;
        next_state.set(GameState::Shop);
    }
}

/// Spawn an enemy from a random edge whenever the wave timer fires.
fn spawn_from_edges(
    time: Res<Time>,
    mut wave: ResMut<Wave>,
    mut commands: Commands,
    sprite_assets: Res<SpriteAssets>,
) {
    wave.spawn_timer.tick(time.delta());
    if !wave.spawn_timer.just_finished() {
        return;
    }

    let position = random_edge_position();
    let (kind, speed, health) = random_enemy_spec(wave.number);
    let index = sprite_assets.enemy_index(kind);
    let size = match kind {
        EnemyKind::MeleeRusher => 32.0,
        EnemyKind::SpeedBurster => 24.0,
        EnemyKind::Splitter => 36.0,
    };

    commands.spawn((
        Enemy {
            kind,
            speed,
            health,
            split_depth: if kind == EnemyKind::Splitter { 2 } else { 0 },
        },
        atlas_sprite(&sprite_assets, index),
        Transform::from_translation(position.extend(0.0))
            .with_scale(Vec3::splat(size / ATLAS_CELL as f32)),
    ));
}

/// Pick a random enemy type with wave-scaled stats.
fn random_enemy_spec(wave: u32) -> (EnemyKind, f32, f32) {
    use rand::Rng;
    let mut rng = rand::rng();
    let base = wave as f32;
    match rng.random_range(0..3) {
        0 => (
            EnemyKind::MeleeRusher,
            120.0 + base * 8.0,
            30.0 + base * 2.0,
        ),
        1 => (
            EnemyKind::SpeedBurster,
            200.0 + base * 6.0,
            18.0 + base,
        ),
        _ => (
            EnemyKind::Splitter,
            140.0 + base * 6.0,
            40.0 + base * 2.0,
        ),
    }
}

fn random_edge_position() -> Vec2 {
    use rand::Rng;
    let mut rng = rand::rng();
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
