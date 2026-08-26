//! Enemies: hostile units spawned in waves that pursue the Player.

use bevy::prelude::*;

use crate::game::player::Player;

/// The enemy archetypes for the MVP. Only `MeleeRusher` exists yet; the rest
/// are added in a later milestone (T11).
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnemyKind {
    /// Straight-line pursuer that deals contact damage. Fast enough to reach
    /// the player but not as fast as a SpeedBurster.
    MeleeRusher,
}

/// The enemy's stats and identity.
#[derive(Component)]
pub struct Enemy {
    pub kind: EnemyKind,
    pub speed: f32,
    pub health: f32,
}

impl Enemy {
    pub fn radius(&self) -> f32 {
        match self.kind {
            EnemyKind::MeleeRusher => 16.0,
        }
    }
}

/// Plugin for enemies: spawning, pursuit, and despawning off-world.
pub struct EnemyPlugin;

impl Plugin for EnemyPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (enemy_pursuit, despawn_off_world));
    }
}

/// Move every enemy in a straight line toward the player.
fn enemy_pursuit(
    time: Res<Time>,
    players: Query<&Transform, (With<Player>, Without<Enemy>)>,
    mut enemies: Query<(&mut Transform, &Enemy), Without<Player>>,
) {
    let Ok(player) = players.single() else {
        return;
    };
    let player_pos = player.translation.truncate();
    for (mut transform, enemy) in &mut enemies {
        let dir = player_pos - transform.translation.truncate();
        let dist = dir.length();
        if dist < 0.001 {
            continue;
        }
        let dir = dir / dist;
        let delta = dir * enemy.speed * time.delta_secs();
        transform.translation.x += delta.x;
        transform.translation.y += delta.y;
    }
}

/// Free the renderer from enemies that wander beyond the play field.
fn despawn_off_world(
    mut commands: Commands,
    enemies: Query<(Entity, &Transform), With<Enemy>>,
) {
    const LIMIT: f32 = 6000.0;
    for (entity, transform) in &enemies {
        let pos = transform.translation;
        if pos.x.abs() > LIMIT || pos.y.abs() > LIMIT {
            commands.entity(entity).despawn();
        }
    }
}
