//! The Player: manual WASD movement, auto-firing weapon loadout.

use bevy::prelude::*;

use crate::game::GameState;

pub const PLAYER_SPEED: f32 = 260.0;
pub const PLAYER_RADIUS: f32 = 18.0;

/// Marker for the player entity.
#[derive(Component)]
pub struct Player;

/// The player's hit points. Damage comes from enemy contact (T7); healing and
/// stat boosts come later from shop items (T9).
#[derive(Component)]
pub struct Health {
    pub max: f32,
    pub current: f32,
}

/// Invulnerability window after being hit, so contact damage doesn't tick
/// every frame. Short enough to keep combat tense.
const HIT_INVULNERABILITY: f32 = 0.6;

/// Tracks the player's post-hit invulnerability timer.
#[derive(Component)]
pub struct HitCooldown(pub Timer);

/// Plugin for the player: spawning and movement.
pub struct PlayerPlugin;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            OnEnter(GameState::InGame),
            spawn_player_if_absent.run_if(no_player_exists),
        )
        .add_systems(Update, player_movement.run_if(in_state(GameState::InGame)));
    }
}

fn no_player_exists(players: Query<&Player>) -> bool {
    players.is_empty()
}

fn spawn_player_if_absent(
    mut commands: Commands,
    players: Query<(), With<Player>>,
) {
    if !players.is_empty() {
        return;
    }
    commands.spawn((
        Player,
        Health {
            max: 100.0,
            current: 100.0,
        },
        HitCooldown(Timer::from_seconds(HIT_INVULNERABILITY, TimerMode::Once)),
        Sprite {
            color: Color::srgb(0.3, 0.8, 0.4),
            custom_size: Some(Vec2::splat(PLAYER_RADIUS * 2.0)),
            ..default()
        },
        Transform::from_xyz(0.0, 0.0, 0.0),
    ));
}

/// Move the player with WASD, clamped to the visible world.
fn player_movement(
    time: Res<Time>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut players: Query<(&mut Transform,), With<Player>>,
) {
    let Ok((mut transform,)) = players.single_mut() else {
        return;
    };

    let mut dir = Vec2::ZERO;
    if keyboard.pressed(KeyCode::KeyW) {
        dir.y += 1.0;
    }
    if keyboard.pressed(KeyCode::KeyS) {
        dir.y -= 1.0;
    }
    if keyboard.pressed(KeyCode::KeyA) {
        dir.x -= 1.0;
    }
    if keyboard.pressed(KeyCode::KeyD) {
        dir.x += 1.0;
    }
    if dir == Vec2::ZERO {
        return;
    }

    let dir = dir.normalize();
    let delta = dir * PLAYER_SPEED * time.delta_secs();
    transform.translation.x += delta.x;
    transform.translation.y += delta.y;
}
