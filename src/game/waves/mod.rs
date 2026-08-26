//! Wave resource and edge spawning.

use bevy::math::Vec2;
use bevy::prelude::*;

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

/// Spawn a melee-rusher from a random edge whenever the wave timer fires.
fn spawn_from_edges(
    time: Res<Time>,
    mut wave: ResMut<Wave>,
    mut commands: Commands,
) {
    wave.spawn_timer.tick(time.delta());
    if !wave.spawn_timer.just_finished() {
        return;
    }

    let position = random_edge_position();
    let (kind, speed, health, color, size) = random_enemy_spec(wave.number);

    commands.spawn((
        Enemy {
            kind,
            speed,
            health,
            split_depth: if kind == EnemyKind::Splitter { 2 } else { 0 },
        },
        Sprite {
            color,
            custom_size: Some(Vec2::splat(size)),
            ..default()
        },
        Transform::from_translation(position.extend(0.0)),
    ));
}

/// Pick a random enemy type with wave-scaled stats and a placeholder color.
fn random_enemy_spec(wave: u32) -> (EnemyKind, f32, f32, Color, f32) {
    use rand::Rng;
    let mut rng = rand::rng();
    let base = wave as f32;
    match rng.random_range(0..3) {
        0 => (
            EnemyKind::MeleeRusher,
            120.0 + base * 8.0,
            30.0 + base * 2.0,
            Color::srgb(0.8, 0.3, 0.3),
            32.0,
        ),
        1 => (
            EnemyKind::SpeedBurster,
            200.0 + base * 6.0,
            18.0 + base,
            Color::srgb(0.9, 0.6, 0.2),
            24.0,
        ),
        _ => (
            EnemyKind::Splitter,
            140.0 + base * 6.0,
            40.0 + base * 2.0,
            Color::srgb(0.85, 0.55, 0.25),
            36.0,
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
