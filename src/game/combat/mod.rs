//! Combat resolution: projectiles/weapons deal damage and enemies die.

use bevy::ecs::message::Message;
use bevy::prelude::*;

use crate::game::enemy::{Enemy, EnemyKind};
use crate::game::player::{Health, HitCooldown, Player, PLAYER_RADIUS};
use crate::game::weapon::Projectile;
use crate::game::GameState;

/// Fired when an enemy dies, so other systems (e.g. economy) can react.
#[derive(Message)]
pub struct EnemyDied {
    pub entity: Entity,
    pub position: Vec2,
    pub kind: EnemyKind,
}

/// Plugin for combat resolution: hit detection, damage, and death.
pub struct CombatPlugin;

impl Plugin for CombatPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<EnemyDied>().add_systems(
            Update,
            (resolve_projectile_hits, contact_damage).run_if(in_state(GameState::InGame)),
        );
    }
}

/// Collide piercing projectiles with enemies: apply damage and kill them.
///
/// A projectile may pass through (pierce) multiple enemies but only strikes a
/// given enemy once. Enemies at zero health are despawned and an `EnemyDied`
/// message is written for downstream systems.
fn resolve_projectile_hits(
    mut commands: Commands,
    mut death_writer: MessageWriter<EnemyDied>,
    mut projectiles: Query<(&mut Projectile, &Transform)>,
    mut enemies: Query<(Entity, &mut Enemy, &Transform), Without<Projectile>>,
) {
    for (mut projectile, proj_transform) in &mut projectiles {
        let proj_pos = proj_transform.translation.truncate();
        let proj_radius = 8.0;

        for (enemy_entity, mut enemy, enemy_transform) in &mut enemies {
            // Skip enemies this projectile already hit (pierce avoids re-hit).
            if projectile.hit_enemies.contains(&enemy_entity) {
                continue;
            }
            let enemy_pos = enemy_transform.translation.truncate();
            let combined = proj_radius + enemy.radius();
            if proj_pos.distance_squared(enemy_pos) > combined * combined {
                continue;
            }

            // Register the hit and apply damage.
            projectile.hit_enemies.push(enemy_entity);
            enemy.health -= projectile.damage;

            if enemy.health <= 0.0 {
                let position = enemy_pos;
                let kind = enemy.kind;
                death_writer.write(EnemyDied {
                    entity: enemy_entity,
                    position,
                    kind,
                });
                commands.entity(enemy_entity).despawn();
            }
        }
    }
}

/// Enemy contact damages the player, gated by a short invulnerability window.
///
/// Once the player's HP hits zero we transition to the Defeat state (one-life
/// roguelike). Contact damage is `EnemyKind`-agnostic for now; per-kind contact
/// stats can be tuned later.
fn contact_damage(
    time: Res<Time>,
    mut next_state: ResMut<NextState<GameState>>,
    mut players: Query<(&Transform, &mut Health, &mut HitCooldown), With<Player>>,
    enemies: Query<(&Transform, &Enemy), Without<Player>>,
) {
    let Ok((player_transform, mut health, mut hit_cooldown)) = players.single_mut() else {
        return;
    };

    // Advance invulnerability even when not hit so it recovers over time.
    hit_cooldown.0.tick(time.delta());

    let player_pos = player_transform.translation.truncate();
    for (enemy_transform, enemy) in &enemies {
        let enemy_pos = enemy_transform.translation.truncate();
        let combined = PLAYER_RADIUS + enemy.radius();
        if player_pos.distance_squared(enemy_pos) > combined * combined {
            continue;
        }
        // Already hurt this invulnerability window — skip.
        if !hit_cooldown.0.is_finished() {
            continue;
        }
        health.current -= 10.0;
        hit_cooldown.0.reset();

        if health.current <= 0.0 {
            next_state.set(GameState::Defeat);
            return;
        }
    }
}
