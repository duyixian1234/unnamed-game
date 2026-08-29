//! The Player: manual movement via `PlayerMoveIntent`, auto-firing weapons.

use bevy::prelude::*;

use crate::intent::PlayerMoveIntent;
use crate::waves::{FIELD_HALF_HEIGHT, FIELD_HALF_WIDTH};
use crate::GameState;

pub const PLAYER_SPEED: f32 = 320.0;
/// Collision radius for contact damage. The sprite art covers only ~43% of
/// its atlas cell (measured), so the hitbox is much smaller than the sprite
/// plus a small grace margin — damage must not start while there is still a
/// visible gap between the characters.
pub const PLAYER_RADIUS: f32 = 20.0;
/// Visual half-size of the player sprite (kept separate from the collision
/// radius so shrinking the hitbox does not shrink the character).
pub const PLAYER_SPRITE_RADIUS: f32 = 34.0;

/// Atlas cell size in px; the app crate's spritesheet uses the same cell.
/// The simulation keeps Transform.scale as the entity's visual size so the
/// render layer can attach sprites without knowing gameplay sizes.
pub const ATLAS_CELL_PX: f32 = 128.0;

/// Marker for the player entity.
#[derive(Component)]
pub struct Player;

/// The player's hit points.
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
pub fn spawn_player_if_absent(mut commands: Commands, players: Query<(), With<Player>>) {
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
        // Atlas cells are 128px; scale so the visual matches the sprite size
        // (2 * PLAYER_SPRITE_RADIUS). The app crate attaches the sprite.
        Transform::from_xyz(0.0, 0.0, 0.0)
            .with_scale(Vec3::splat((PLAYER_SPRITE_RADIUS * 2.0) / ATLAS_CELL_PX)),
    ));
}

/// Move the player along the current `PlayerMoveIntent`, applying any
/// shop-bought speed multiplier.
fn player_movement(
    time: Res<Time>,
    intent: Res<PlayerMoveIntent>,
    mut players: Query<(&mut Transform, &PlayerStats), With<Player>>,
) {
    let Ok((mut transform, stats)) = players.single_mut() else {
        return;
    };

    if intent.dir == Vec2::ZERO {
        return;
    }
    let dir = intent.dir.normalize();
    let delta = dir * PLAYER_SPEED * stats.speed_mult * time.delta_secs();
    transform.translation.x += delta.x;
    transform.translation.y += delta.y;
}

/// Keep the player inside the play field so they don't wander off into the
/// void where enemies can't reach them. Clamps by the sprite radius so the
/// character never visually leaves the arena.
fn clamp_to_field(mut players: Query<&mut Transform, With<Player>>) {
    for mut transform in &mut players {
        transform.translation.x = transform.translation.x.clamp(
            -(FIELD_HALF_WIDTH - PLAYER_SPRITE_RADIUS),
            FIELD_HALF_WIDTH - PLAYER_SPRITE_RADIUS,
        );
        transform.translation.y = transform.translation.y.clamp(
            -(FIELD_HALF_HEIGHT - PLAYER_SPRITE_RADIUS),
            FIELD_HALF_HEIGHT - PLAYER_SPRITE_RADIUS,
        );
    }
}
