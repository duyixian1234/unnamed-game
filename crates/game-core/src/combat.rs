//! Combat resolution: projectiles/weapons deal damage and enemies die.

use bevy::ecs::message::Message;
use bevy::prelude::*;

use crate::damage::{DamageSource, DamageStats};
use crate::enemy::{Enemy, EnemyKind, EnemySpawned, Knockback};
use crate::player::{Health, HitCooldown, Player, ATLAS_CELL_PX, PLAYER_RADIUS};
use crate::weapon::{MeleeHit, OrbitOrb, Projectile, ORB_REHIT};
use crate::GameState;

/// Fired when a weapon lands a hit on an enemy (the app plays a sound).
#[derive(Message)]
pub struct HitSfx;

/// Fired when the player takes damage (the app plays a sound).
#[derive(Message)]
pub struct HurtSfx;

/// The player was hurt by enemy contact; `hp_after` is post-damage HP.
#[derive(Message, Debug, Clone, Copy)]
pub struct PlayerHurt {
    pub amount: f32,
    pub hp_after: f32,
}

/// Fired when an enemy dies, so other systems (e.g. economy) can react.
#[derive(Message, Debug, Clone, Copy)]
pub struct EnemyDied {
    pub entity: Entity,
    pub position: Vec2,
    pub kind: EnemyKind,
}

/// Runs combat damage resolution before weapon hitboxes are expired, so a
/// freshly spawned melee swing / orb can connect before it despawns.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum CombatSet {
    /// Resolve projectile/melee/orb damage against enemies.
    ResolveDamage,
}

/// Plugin for combat resolution: hit detection, damage, and death.
pub struct CombatPlugin;

impl Plugin for CombatPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<EnemyDied>()
            .add_message::<HitSfx>()
            .add_message::<HurtSfx>()
            .add_message::<PlayerHurt>()
            .add_systems(
                Update,
                (
                    resolve_projectile_hits,
                    resolve_melee_hits,
                    resolve_orb_hits,
                    contact_damage,
                )
                    .in_set(CombatSet::ResolveDamage)
                    .run_if(in_state(GameState::InGame)),
            );
    }
}

/// Apply damage to an enemy; despawn and emit `EnemyDied` (with splitter
/// children) if it drops to zero health. Shared by the core hit resolution
/// and the evolution behaviors (whirlwind strikes, bomber AOE) in weapon.rs.
#[allow(clippy::too_many_arguments)]
pub(crate) fn apply_damage(
    commands: &mut Commands,
    death_writer: &mut MessageWriter<EnemyDied>,
    spawn_writer: &mut MessageWriter<EnemySpawned>,
    enemy_entity: Entity,
    enemy: &mut Enemy,
    enemy_pos: Vec2,
    amount: f32,
    source: DamageSource,
    damage_stats: &mut DamageStats,
) {
    if enemy.health <= 0.0 {
        return;
    }
    let effective_damage = amount.min(enemy.health.max(0.0));
    enemy.health -= amount;
    damage_stats.record(source, effective_damage);
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
    spawn_splitter_children(commands, spawn_writer, enemy_pos, split_depth);
    commands.entity(enemy_entity).despawn();
}

/// True when a circle of `radius` centered at `pos` overlaps an enemy's body.
/// Single source of the "attacker radius + enemy body" reach shape, shared by
/// the projectile / melee / orb hit tests and the melee fire gate in weapon.rs.
pub(crate) fn circle_hits_enemy(pos: Vec2, radius: f32, enemy_pos: Vec2, enemy: &Enemy) -> bool {
    let combined = radius + enemy.radius();
    pos.distance_squared(enemy_pos) <= combined * combined
}

/// Collide piercing projectiles with enemies: apply damage and kill them.
///
/// A projectile may pass through (pierce) multiple enemies but only strikes a
/// given enemy once.
fn resolve_projectile_hits(
    mut commands: Commands,
    mut death_writer: MessageWriter<EnemyDied>,
    mut spawn_writer: MessageWriter<EnemySpawned>,
    mut hit_writer: MessageWriter<HitSfx>,
    mut damage_stats: ResMut<DamageStats>,
    mut projectiles: Query<(&mut Projectile, &Transform)>,
    mut enemies: Query<(Entity, &mut Enemy, &Transform, &mut Knockback), Without<Projectile>>,
) {
    for (mut projectile, proj_transform) in &mut projectiles {
        let proj_pos = proj_transform.translation.truncate();
        let proj_radius = 8.0;

        for (enemy_entity, mut enemy, enemy_transform, mut knockback) in &mut enemies {
            if projectile.hit_enemies.contains(&enemy_entity) {
                continue;
            }
            let enemy_pos = enemy_transform.translation.truncate();
            if !circle_hits_enemy(proj_pos, proj_radius, enemy_pos, &enemy) {
                continue;
            }
            projectile.hit_enemies.push(enemy_entity);
            hit_writer.write(HitSfx);
            apply_damage(
                &mut commands,
                &mut death_writer,
                &mut spawn_writer,
                enemy_entity,
                &mut enemy,
                enemy_pos,
                projectile.damage,
                projectile.source,
                &mut damage_stats,
            );
            knockback.0 += projectile.direction * projectile.knockback;
        }
    }
}

/// Melee swing hitboxes damage every enemy they overlap once.
fn resolve_melee_hits(
    mut commands: Commands,
    mut death_writer: MessageWriter<EnemyDied>,
    mut spawn_writer: MessageWriter<EnemySpawned>,
    mut hit_writer: MessageWriter<HitSfx>,
    mut damage_stats: ResMut<DamageStats>,
    mut melee_hits: Query<(&mut MeleeHit, &Transform)>,
    mut enemies: Query<(Entity, &mut Enemy, &Transform, &mut Knockback), Without<MeleeHit>>,
) {
    for (mut melee, melee_transform) in &mut melee_hits {
        let melee_pos = melee_transform.translation.truncate();
        let melee_radius = melee.radius;

        for (enemy_entity, mut enemy, enemy_transform, mut knockback) in &mut enemies {
            if melee.hit_enemies.contains(&enemy_entity) {
                continue;
            }
            let enemy_pos = enemy_transform.translation.truncate();
            if !circle_hits_enemy(melee_pos, melee_radius, enemy_pos, &enemy) {
                continue;
            }
            melee.hit_enemies.push(enemy_entity);
            hit_writer.write(HitSfx);
            apply_damage(
                &mut commands,
                &mut death_writer,
                &mut spawn_writer,
                enemy_entity,
                &mut enemy,
                enemy_pos,
                melee.damage,
                melee.source,
                &mut damage_stats,
            );
            let dir = (enemy_pos - melee_pos).normalize_or_zero();
            knockback.0 += dir * melee.knockback;
        }
    }
}

/// Orbiting orbs damage enemies they touch each frame.
fn resolve_orb_hits(
    mut commands: Commands,
    mut death_writer: MessageWriter<EnemyDied>,
    mut spawn_writer: MessageWriter<EnemySpawned>,
    mut hit_writer: MessageWriter<HitSfx>,
    mut damage_stats: ResMut<DamageStats>,
    mut orbs: Query<(&mut OrbitOrb, &Transform)>,
    mut enemies: Query<(Entity, &mut Enemy, &Transform, &mut Knockback), Without<OrbitOrb>>,
) {
    for (mut orb, orb_transform) in &mut orbs {
        let orb_pos = orb_transform.translation.truncate();
        let orb_radius = crate::weapon::ORB_HIT_RADIUS;

        for (enemy_entity, mut enemy, enemy_transform, mut knockback) in &mut enemies {
            if !hit_cooldown_ready(&orb.hit_cooldowns, enemy_entity) {
                continue;
            }
            let enemy_pos = enemy_transform.translation.truncate();
            if !circle_hits_enemy(orb_pos, orb_radius, enemy_pos, &enemy) {
                continue;
            }
            orb.hit_enemies.push(enemy_entity);
            hit_writer.write(HitSfx);
            apply_damage(
                &mut commands,
                &mut death_writer,
                &mut spawn_writer,
                enemy_entity,
                &mut enemy,
                enemy_pos,
                orb.damage,
                orb.source,
                &mut damage_stats,
            );
            let dir = (enemy_pos - orb_pos).normalize_or_zero();
            knockback.0 += dir * orb.knockback;
            restart_hit_cooldown(&mut orb.hit_cooldowns, enemy_entity, ORB_REHIT);
        }
    }
}

pub(crate) fn hit_cooldown_ready(cooldowns: &[(Entity, Timer)], target: Entity) -> bool {
    cooldowns
        .iter()
        .find(|(entity, _)| *entity == target)
        .is_none_or(|(_, timer)| timer.is_finished())
}

pub(crate) fn restart_hit_cooldown(
    cooldowns: &mut Vec<(Entity, Timer)>,
    target: Entity,
    duration: f32,
) {
    match cooldowns.iter_mut().find(|(entity, _)| *entity == target) {
        Some((_, timer)) => timer.reset(),
        None => cooldowns.push((target, Timer::from_seconds(duration, TimerMode::Once))),
    }
}

/// A dying Splitter breaks into smaller Splitters (up to its split depth).
fn spawn_splitter_children(
    commands: &mut Commands,
    spawn_writer: &mut MessageWriter<EnemySpawned>,
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
        spawn_writer.write(EnemySpawned {
            kind: EnemyKind::Splitter,
        });
        commands.spawn((
            Enemy {
                kind: EnemyKind::Splitter,
                speed: 150.0,
                health: 15.0,
                split_depth: child_depth,
            },
            Transform::from_translation((position + offset).extend(0.0))
                .with_scale(Vec3::splat(40.0 / ATLAS_CELL_PX)),
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
    mut player_hurt_writer: MessageWriter<PlayerHurt>,
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
        let amount = enemy.contact_damage();
        health.current -= amount;
        hit_cooldown.0.reset();
        hurt_writer.write(HurtSfx);
        player_hurt_writer.write(PlayerHurt {
            amount,
            hp_after: health.current,
        });

        if health.current <= 0.0 {
            next_state.set(GameState::Defeat);
            return;
        }
    }
}
