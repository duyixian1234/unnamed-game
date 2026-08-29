//! Enemies: hostile units spawned in waves that pursue the Player.

use bevy::ecs::message::Message;
use bevy::prelude::*;

use crate::player::Player;

/// The MVP enemy archetypes (per CONTEXT.md).
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnemyKind {
    /// Straight-line pursuer that deals contact damage.
    MeleeRusher,
    /// Accelerates toward the player the closer it gets.
    SpeedBurster,
    /// Splits into smaller enemies on death.
    Splitter,
}

/// Fired whenever an enemy entity is spawned (edge spawns and Splitter
/// splits). The assertion interface for spawn counts/kinds (deterministic
/// under a fixed seed).
#[derive(Message, Debug, Clone, Copy)]
pub struct EnemySpawned {
    pub kind: EnemyKind,
}

/// External impulse velocity applied by weapon hits (units/s). Decays
/// exponentially in `enemy_pursuit`; auto-inserted with every `Enemy`.
#[derive(Component, Debug, Default)]
pub struct Knockback(pub Vec2);

/// The enemy's stats and identity.
#[derive(Component)]
#[require(Knockback)]
pub struct Enemy {
    pub kind: EnemyKind,
    /// Current movement speed (may change, e.g. a burster accelerating).
    pub speed: f32,
    pub health: f32,
    /// How many more times a Splitter may split (0 = splitter minion that
    /// can no longer split; avoids infinite recursion).
    pub split_depth: u8,
}

impl Enemy {
    /// Collision hitbox radius. Smaller than the rendered sprite: the art
    /// covers 43-69% of the atlas cell (measured), so these are the visual
    /// radii plus a small grace margin — contact must not start while there
    /// is still a visible gap.
    pub fn radius(&self) -> f32 {
        match self.kind {
            EnemyKind::MeleeRusher => 22.0,
            EnemyKind::SpeedBurster => 14.0,
            EnemyKind::Splitter => 22.0,
        }
    }

    /// Contact damage dealt to the player per kind.
    pub fn contact_damage(&self) -> f32 {
        match self.kind {
            EnemyKind::MeleeRusher => 10.0,
            EnemyKind::SpeedBurster => 6.0,
            EnemyKind::Splitter => 8.0,
        }
    }
}

/// Plugin for enemies: spawning, pursuit, and despawning off-world.
pub struct EnemyPlugin;

impl Plugin for EnemyPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<EnemySpawned>()
            .add_systems(Update, (enemy_pursuit, despawn_off_world));
    }
}

/// Move every enemy toward the player, applying per-kind movement rules and
/// integrating any knockback impulse (which decays exponentially).
fn enemy_pursuit(
    time: Res<Time>,
    players: Query<&Transform, (With<Player>, Without<Enemy>)>,
    mut enemies: Query<(&mut Transform, &Enemy, &mut Knockback), Without<Player>>,
) {
    const KNOCKBACK_HALF_LIFE: f32 = 0.12;

    let Ok(player) = players.single() else {
        return;
    };
    let player_pos = player.translation.truncate();
    let dt = time.delta_secs();
    for (mut transform, enemy, mut knockback) in &mut enemies {
        let pos = transform.translation.truncate();
        let dir = player_pos - pos;
        let dist = dir.length();
        if dist >= 0.001 {
            let dir = dir / dist;

            // Per-kind speed: bursters accelerate as they close in on the player.
            let mut speed = enemy.speed;
            if enemy.kind == EnemyKind::SpeedBurster {
                speed += (800.0 / (dist + 40.0)).min(320.0);
            }

            let delta = dir * speed * dt;
            transform.translation.x += delta.x;
            transform.translation.y += delta.y;
        }

        // Knockback: integrate the impulse velocity and decay it
        // exponentially (half-life KNOCKBACK_HALF_LIFE).
        transform.translation.x += knockback.0.x * dt;
        transform.translation.y += knockback.0.y * dt;
        knockback.0 *= f32::exp2(-dt / KNOCKBACK_HALF_LIFE);
        if knockback.0.length_squared() < 0.01 {
            knockback.0 = Vec2::ZERO;
        }
    }
}

/// Free the renderer from enemies that wander beyond the play field.
fn despawn_off_world(mut commands: Commands, enemies: Query<(Entity, &Transform), With<Enemy>>) {
    const LIMIT: f32 = 6000.0;
    for (entity, transform) in &enemies {
        let pos = transform.translation;
        if pos.x.abs() > LIMIT || pos.y.abs() > LIMIT {
            commands.entity(entity).despawn();
        }
    }
}
