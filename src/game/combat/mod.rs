//! Combat resolution: projectiles/weapons deal damage and enemies die.

use bevy::ecs::message::Message;
use bevy::prelude::*;

use crate::game::assets::{atlas_sprite, SpriteAssets, ATLAS_CELL};
use crate::game::audio::{HitSfx, HurtSfx};
use crate::game::enemy::{Enemy, EnemyKind};
use crate::game::player::{Health, HitCooldown, Player, PLAYER_RADIUS};
use crate::game::weapon::{MeleeHit, OrbitOrb, Projectile};
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
            (resolve_projectile_hits, resolve_melee_hits, resolve_orb_hits, contact_damage)
                .run_if(in_state(GameState::InGame)),
        );
    }
}

/// Apply damage to an enemy; despawn and emit `EnemyDied` (with splitter
/// children) if it drops to zero health.
fn apply_damage(
    commands: &mut Commands,
    death_writer: &mut MessageWriter<EnemyDied>,
    sprite_assets: &SpriteAssets,
    enemy_entity: Entity,
    enemy: &mut Enemy,
    enemy_pos: Vec2,
    amount: f32,
) {
    enemy.health -= amount;
    if enemy.health > 0.0 {
        return;
    }
    let kind = enemy.kind;
    let split_depth = enemy.split_depth;
    death_writer.write(EnemyDied {
        entity: enemy_entity,
        position: enemy_pos,
        kind,
    });
    spawn_splitter_children(commands, sprite_assets, enemy_pos, split_depth);
    commands.entity(enemy_entity).despawn();
}

/// Collide piercing projectiles with enemies: apply damage and kill them.
///
/// A projectile may pass through (pierce) multiple enemies but only strikes a
/// given enemy once.
fn resolve_projectile_hits(
    mut commands: Commands,
    mut death_writer: MessageWriter<EnemyDied>,
    mut hit_writer: MessageWriter<HitSfx>,
    sprite_assets: Res<SpriteAssets>,
    mut projectiles: Query<(&mut Projectile, &Transform)>,
    mut enemies: Query<(Entity, &mut Enemy, &Transform), Without<Projectile>>,
) {
    for (mut projectile, proj_transform) in &mut projectiles {
        let proj_pos = proj_transform.translation.truncate();
        let proj_radius = 8.0;

        for (enemy_entity, mut enemy, enemy_transform) in &mut enemies {
            if projectile.hit_enemies.contains(&enemy_entity) {
                continue;
            }
            let enemy_pos = enemy_transform.translation.truncate();
            let combined = proj_radius + enemy.radius();
            if proj_pos.distance_squared(enemy_pos) > combined * combined {
                continue;
            }
            projectile.hit_enemies.push(enemy_entity);
            hit_writer.write(HitSfx);
            apply_damage(
                &mut commands,
                &mut death_writer,
                &sprite_assets,
                enemy_entity,
                &mut enemy,
                enemy_pos,
                projectile.damage,
            );
        }
    }
}

/// Melee swing hitboxes damage every enemy they overlap once.
fn resolve_melee_hits(
    mut commands: Commands,
    mut death_writer: MessageWriter<EnemyDied>,
    mut hit_writer: MessageWriter<HitSfx>,
    sprite_assets: Res<SpriteAssets>,
    mut melee_hits: Query<(&mut MeleeHit, &Transform)>,
    mut enemies: Query<(Entity, &mut Enemy, &Transform), Without<MeleeHit>>,
) {
    for (mut melee, melee_transform) in &mut melee_hits {
        let melee_pos = melee_transform.translation.truncate();
        let melee_radius = melee.radius;

        for (enemy_entity, mut enemy, enemy_transform) in &mut enemies {
            if melee.hit_enemies.contains(&enemy_entity) {
                continue;
            }
            let enemy_pos = enemy_transform.translation.truncate();
            let combined = melee_radius + enemy.radius();
            if melee_pos.distance_squared(enemy_pos) > combined * combined {
                continue;
            }
            melee.hit_enemies.push(enemy_entity);
            hit_writer.write(HitSfx);
            apply_damage(
                &mut commands,
                &mut death_writer,
                &sprite_assets,
                enemy_entity,
                &mut enemy,
                enemy_pos,
                melee.damage,
            );
        }
    }
}

/// Orbiting orbs damage enemies they touch each frame.
fn resolve_orb_hits(
    mut commands: Commands,
    mut death_writer: MessageWriter<EnemyDied>,
    mut hit_writer: MessageWriter<HitSfx>,
    sprite_assets: Res<SpriteAssets>,
    mut orbs: Query<(&mut OrbitOrb, &Transform)>,
    mut enemies: Query<(Entity, &mut Enemy, &Transform), Without<OrbitOrb>>,
) {
    for (mut orb, orb_transform) in &mut orbs {
        let orb_pos = orb_transform.translation.truncate();
        let orb_radius = 9.0;

        for (enemy_entity, mut enemy, enemy_transform) in &mut enemies {
            if orb.hit_enemies.contains(&enemy_entity) {
                continue;
            }
            let enemy_pos = enemy_transform.translation.truncate();
            let combined = orb_radius + enemy.radius();
            if orb_pos.distance_squared(enemy_pos) > combined * combined {
                continue;
            }
            orb.hit_enemies.push(enemy_entity);
            hit_writer.write(HitSfx);
            apply_damage(
                &mut commands,
                &mut death_writer,
                &sprite_assets,
                enemy_entity,
                &mut enemy,
                enemy_pos,
                orb.damage,
            );
        }
    }
}

/// A dying Splitter breaks into smaller Splitters (up to its split depth).
fn spawn_splitter_children(
    commands: &mut Commands,
    sprite_assets: &SpriteAssets,
    position: Vec2,
    split_depth: u8,
) {
    if split_depth == 0 {
        return;
    }
    let child_depth = split_depth - 1;
    for i in 0..2 {
        let angle = std::f32::consts::TAU * (i as f32) / 2.0;
        let offset = Vec2::new(18.0, 0.0).rotate(Vec2::from_angle(angle));
        let index = sprite_assets.enemy_index(EnemyKind::Splitter);
        commands.spawn((
            Enemy {
                kind: EnemyKind::Splitter,
                speed: 150.0,
                health: 15.0,
                split_depth: child_depth,
            },
            atlas_sprite(sprite_assets, index),
            Transform::from_translation((position + offset).extend(0.0))
                .with_scale(Vec3::splat(20.0 / ATLAS_CELL as f32)),
        ));
    }
}

/// Enemy contact damages the player, gated by a short invulnerability window.
///
/// Once the player's HP hits zero we transition to the Defeat state (one-life
/// roguelike). Contact damage varies by `EnemyKind`.
fn contact_damage(
    time: Res<Time>,
    mut next_state: ResMut<NextState<GameState>>,
    mut hurt_writer: MessageWriter<HurtSfx>,
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
        health.current -= enemy.contact_damage();
        hit_cooldown.0.reset();
        hurt_writer.write(HurtSfx);

        if health.current <= 0.0 {
            next_state.set(GameState::Defeat);
            return;
        }
    }
}
