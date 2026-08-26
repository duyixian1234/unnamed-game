//! Wave resource and edge spawning.

use bevy::math::Vec2;
use bevy::prelude::*;

use crate::game::assets::{atlas_sprite, SpriteAssets, ATLAS_CELL};
use crate::game::enemy::{Enemy, EnemyKind};
use crate::game::GameState;

/// Half-extents of the play field; enemies spawn just outside these edges.
pub const FIELD_HALF_WIDTH: f32 = 900.0;
pub const FIELD_HALF_HEIGHT: f32 = 560.0;

/// Time between enemy spawns within a wave (seconds).
const SPAWN_INTERVAL: f32 = 1.1;

/// The active wave state: which wave we're on and when to spawn the next enemy.
#[derive(Resource)]
pub struct Wave {
    /// 1-based current wave number.
    pub number: u32,
    /// Counts down; when it hits zero we spawn one enemy and reset.
    pub spawn_timer: Timer,
}

impl Default for Wave {
    fn default() -> Self {
        Self {
            number: 1,
            spawn_timer: Timer::from_seconds(SPAWN_INTERVAL, TimerMode::Repeating),
        }
    }
}

/// Plugin for wave lifecycle and edge spawning.
pub struct WavesPlugin;

impl Plugin for WavesPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Wave>()
            .add_systems(OnEnter(GameState::InGame), reset_wave)
            .add_systems(Update, spawn_from_edges.run_if(in_state(GameState::InGame)));
    }
}

fn reset_wave(mut wave: ResMut<Wave>) {
    *wave = Wave::default();
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
