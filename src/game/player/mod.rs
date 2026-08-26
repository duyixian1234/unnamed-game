//! The Player: manual WASD movement, auto-firing weapon loadout.

use bevy::prelude::*;

use crate::game::assets::{atlas_index, atlas_sprite, SpriteAssets, ATLAS_CELL};
use crate::game::waves::{FIELD_HALF_HEIGHT, FIELD_HALF_WIDTH};
use crate::game::GameState;

pub const PLAYER_SPEED: f32 = 320.0;
/// Collision radius. The sprite is scaled up from the 128px atlas cell so the
/// character reads clearly on screen.
pub const PLAYER_RADIUS: f32 = 34.0;

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

/// Player stats that shop items can boost (pure stat-gain items per CONTEXT.md).
#[derive(Component)]
pub struct PlayerStats {
    /// Damage multiplier applied to all weapons.
    pub damage_mult: f32,
    /// Movement speed multiplier.
    pub speed_mult: f32,
    /// Bonus max HP added at spawn (used by +HP items).
    pub max_hp_bonus: f32,
}

impl Default for PlayerStats {
    fn default() -> Self {
        Self {
            damage_mult: 1.0,
            speed_mult: 1.0,
            max_hp_bonus: 0.0,
        }
    }
}

/// Plugin for the player: spawning and movement.
pub struct PlayerPlugin;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            OnEnter(GameState::InGame),
            spawn_player_if_absent.run_if(no_player_exists),
        )
        .add_systems(
            Update,
            (player_movement, clamp_to_field).run_if(in_state(GameState::InGame)),
        );
    }
}

fn no_player_exists(players: Query<&Player>) -> bool {
    players.is_empty()
}

/// Spawn the player once per run. Public so the weapon plugin can order its
/// starting-loadout system to run after the player exists.
pub fn spawn_player_if_absent(
    mut commands: Commands,
    sprite_assets: Res<SpriteAssets>,
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
        PlayerStats::default(),
        atlas_sprite(&sprite_assets, atlas_index::PLAYER),
        // Atlas cells are 128px; scale down so the visual matches the collision
        // radius (2 * PLAYER_RADIUS).
        Transform::from_xyz(0.0, 0.0, 0.0).with_scale(Vec3::splat(
            (PLAYER_RADIUS * 2.0) / ATLAS_CELL as f32,
        )),
    ));
}

/// Move the player with WASD, applying any shop-bought speed multiplier.
fn player_movement(
    time: Res<Time>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut players: Query<(&mut Transform, &PlayerStats), With<Player>>,
) {
    let Ok((mut transform, stats)) = players.single_mut() else {
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
    let delta = dir * PLAYER_SPEED * stats.speed_mult * time.delta_secs();
    transform.translation.x += delta.x;
    transform.translation.y += delta.y;
}

/// Keep the player inside the play field so they don't wander off into the
/// void where enemies can't reach them.
fn clamp_to_field(mut players: Query<&mut Transform, With<Player>>) {
    for mut transform in &mut players {
        transform.translation.x = transform.translation.x
            .clamp(-(FIELD_HALF_WIDTH - PLAYER_RADIUS), FIELD_HALF_WIDTH - PLAYER_RADIUS);
        transform.translation.y = transform.translation.y
            .clamp(-(FIELD_HALF_HEIGHT - PLAYER_RADIUS), FIELD_HALF_HEIGHT - PLAYER_RADIUS);
    }
}
