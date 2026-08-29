//! The weapon system: auto-aiming loadout slots, projectiles, and orbit orbs.

use bevy::math::Vec2;
use bevy::prelude::*;

use crate::combat::CombatSet;
use crate::enemy::Enemy;
use crate::player::{Player, PlayerStats};
use crate::GameState;

/// Maximum number of equipped weapons (per CONTEXT.md).
pub const MAX_WEAPON_SLOTS: usize = 6;

/// The MVP weapon archetypes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WeaponKind {
    PiercingProjectile,
    MeleeSwing,
    OrbitingOrb,
}

/// A single equipped weapon. Lives on a child entity (a "slot") of the player,
/// so the player can carry up to `MAX_WEAPON_SLOTS` weapons.
#[derive(Component)]
pub struct Weapon {
    pub kind: WeaponKind,
    /// Seconds between attacks.
    pub cooldown: Timer,
    pub damage: f32,
    pub projectile_speed: f32,
    /// Effective range for melee swing (and projectile lifetime).
    pub range: f32,
}

impl Weapon {
    pub fn new(kind: WeaponKind) -> Self {
        let (interval, damage, projectile_speed, range) = match kind {
            WeaponKind::PiercingProjectile => (0.8, 10.0, 420.0, 900.0),
            WeaponKind::MeleeSwing => (0.9, 25.0, 0.0, 90.0),
            WeaponKind::OrbitingOrb => (0.0, 8.0, 0.0, 0.0),
        };
        Self {
            kind,
            cooldown: Timer::from_seconds(interval, TimerMode::Repeating),
            damage,
            projectile_speed,
            range,
        }
    }
}

/// A projectile fired by a weapon; pierces through enemies until it expires.
#[derive(Component)]
pub struct Projectile {
    pub damage: f32,
    pub speed: f32,
    pub direction: Vec2,
    pub lifetime: Timer,
    /// Enemies this projectile has already struck, so a piercing shot only
    /// hits each enemy once.
    pub hit_enemies: Vec<Entity>,
}

/// A melee swing hitbox spawned briefly at the player; damages enemies it
/// overlaps. Lives long enough for combat resolution to run before it expires.
#[derive(Component)]
pub struct MeleeHit {
    pub damage: f32,
    /// Radius of the swing around the player.
    pub radius: f32,
    pub hit_enemies: Vec<Entity>,
    /// How long the swing stays active; combat must resolve within this window.
    pub lifetime: Timer,
}

/// An orb orbiting the player; damages enemies it touches.
#[derive(Component)]
pub struct OrbitOrb {
    pub damage: f32,
    /// Angular position around the player (radians).
    pub angle: f32,
    /// Angular speed (radians/sec).
    pub angular_speed: f32,
    /// Orbit radius around the player.
    pub radius: f32,
    /// Enemies hit this tick, to avoid multi-hitting the same enemy per frame.
    pub hit_enemies: Vec<Entity>,
}

/// Marker added to the player once its weapon loadout has been spawned, so we
/// don't duplicate weapons on every re-entry to InGame (e.g. between waves).
#[derive(Component)]
pub struct WeaponLoadout;

/// Plugin for the weapon system.
pub struct WeaponPlugin;

impl Plugin for WeaponPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            OnEnter(GameState::InGame),
            give_starting_weapons
                // The player must exist before we attach weapon slots, otherwise
                // a fresh run could give the loadout before the player spawns.
                .after(crate::player::spawn_player_if_absent),
        )
        .add_systems(
            Update,
            (
                auto_attack,
                move_projectiles,
                // Orbs must move and clear their per-frame hit list before
                // combat resolves orb contact damage this frame.
                update_orbs.before(CombatSet::ResolveDamage),
                // Expire hitboxes only after combat damage has resolved, so
                // a just-spawned melee swing connects this frame.
                expire_melee_hits.after(CombatSet::ResolveDamage),
            )
                .run_if(in_state(GameState::InGame)),
        );
    }
}

/// Hand the player a starting loadout (one of each weapon kind) when a run
/// begins. Slots are child entities so up to `MAX_WEAPON_SLOTS` can be held.
fn give_starting_weapons(
    mut commands: Commands,
    players: Query<(Entity, &Transform), (With<Player>, Without<WeaponLoadout>)>,
) {
    for (player, player_transform) in &players {
        let kinds = [
            WeaponKind::PiercingProjectile,
            WeaponKind::MeleeSwing,
            WeaponKind::OrbitingOrb,
        ];
        debug_assert!(
            kinds.len() <= MAX_WEAPON_SLOTS,
            "starting loadout exceeds MAX_WEAPON_SLOTS"
        );
        for kind in kinds {
            let slot = commands
                .spawn((Weapon::new(kind), Transform::from_translation(player_transform.translation)))
                .id();
            commands.entity(player).add_child(slot);
        }
        commands.entity(player).insert(WeaponLoadout);
    }
}

/// Drive every weapon: fire projectiles, swing melee hitboxes, or manage orbs.
fn auto_attack(
    time: Res<Time>,
    mut commands: Commands,
    players: Query<(&Transform, &PlayerStats), With<Player>>,
    mut weapons: Query<&mut Weapon, Without<Player>>,
    enemies: Query<&Transform, (With<Enemy>, Without<Player>)>,
) {
    let Ok((player_transform, stats)) = players.single() else {
        return;
    };
    let player_pos = player_transform.translation.truncate();

    for mut weapon in &mut weapons {
        // Orbiting orbs are persistent; handled by update_orbs. Advance their
        // cooldown too so freshly added orbs spawn immediately.
        if weapon.kind == WeaponKind::OrbitingOrb {
            continue;
        }
        weapon.cooldown.tick(time.delta());
        if !weapon.cooldown.just_finished() {
            continue;
        }

        match weapon.kind {
            WeaponKind::PiercingProjectile => {
                let Some(target) = nearest_enemy(player_pos, &enemies) else {
                    continue;
                };
                let direction = (target - player_pos).normalize_or_zero();
                if direction == Vec2::ZERO {
                    continue;
                }
                commands.spawn((
                    Projectile {
                        damage: weapon.damage * stats.damage_mult,
                        speed: weapon.projectile_speed,
                        direction,
                        lifetime: Timer::from_seconds(
                            weapon.range / weapon.projectile_speed,
                            TimerMode::Once,
                        ),
                        hit_enemies: Vec::new(),
                    },
                    Transform::from_translation(player_pos.extend(0.0)),
                ));
            }
            WeaponKind::MeleeSwing => {
                let radius = weapon.range;
                commands.spawn((
                    MeleeHit {
                        damage: weapon.damage * stats.damage_mult,
                        radius,
                        hit_enemies: Vec::new(),
                        lifetime: Timer::from_seconds(0.15, TimerMode::Once),
                    },
                    Transform::from_translation(player_pos.extend(0.0)),
                ));
            }
            WeaponKind::OrbitingOrb => {}
        }
    }
}

fn nearest_enemy(
    player_pos: Vec2,
    enemies: &Query<&Transform, (With<Enemy>, Without<Player>)>,
) -> Option<Vec2> {
    let mut nearest: Option<(f32, Vec2)> = None;
    for transform in enemies {
        let pos = transform.translation.truncate();
        let dist_sq = pos.distance_squared(player_pos);
        if nearest.map_or(true, |(best, _)| dist_sq < best) {
            nearest = Some((dist_sq, pos));
        }
    }
    nearest.map(|(_, pos)| pos)
}

/// Advance piercing projectiles along their direction and despawn on expiry.
fn move_projectiles(
    time: Res<Time>,
    mut commands: Commands,
    mut projectiles: Query<(Entity, &mut Transform, &mut Projectile), Without<MeleeHit>>,
) {
    for (entity, mut transform, mut projectile) in &mut projectiles {
        projectile.lifetime.tick(time.delta());
        if projectile.lifetime.is_finished() {
            commands.entity(entity).despawn();
            continue;
        }
        let step = projectile.direction * projectile.speed * time.delta_secs();
        transform.translation.x += step.x;
        transform.translation.y += step.y;
    }
}

/// Melee hits last a short window (so combat can resolve), then despawn.
fn expire_melee_hits(
    time: Res<Time>,
    mut commands: Commands,
    mut melee_hits: Query<(Entity, &mut MeleeHit), Without<Projectile>>,
) {
    for (entity, mut melee) in &mut melee_hits {
        melee.lifetime.tick(time.delta());
        if melee.lifetime.is_finished() {
            commands.entity(entity).despawn();
        }
    }
}

/// Spawn and orbit orbs around the player, one per OrbitingOrb weapon slot.
fn update_orbs(
    mut commands: Commands,
    time: Res<Time>,
    players: Query<(&Transform, &PlayerStats), With<Player>>,
    mut weapons: Query<(&mut Weapon, &Transform), (Without<Player>, Without<OrbitOrb>)>,
    mut orbs: Query<(Entity, &mut OrbitOrb, &mut Transform), (Without<Player>, Without<Weapon>)>,
) {
    let Ok((player_transform, stats)) = players.single() else {
        return;
    };
    let player_pos = player_transform.translation;

    // Rotate existing orbs around the player.
    for (_, mut orb, mut transform) in &mut orbs {
        orb.angle += orb.angular_speed * time.delta_secs();
        let offset = Vec2::from_angle(orb.angle) * orb.radius;
        transform.translation = player_pos.truncate().extend(0.0) + offset.extend(0.0);
        // Clear per-frame hit list so an orb can re-hit enemies each rotation.
        orb.hit_enemies.clear();
    }

    // Ensure each OrbitingOrb weapon has an active orb; spawn if missing.
    let existing = orbs.iter().count() as i32;
    let mut needed = 0;
    let mut orb_damage = 8.0;
    for (weapon, _) in &mut weapons {
        if weapon.kind == WeaponKind::OrbitingOrb {
            needed += 1;
            orb_damage = weapon.damage;
        }
    }
    for _ in existing..needed {
        commands.spawn((
            OrbitOrb {
                damage: orb_damage * stats.damage_mult,
                angle: 0.0,
                angular_speed: 2.5,
                radius: 70.0,
                hit_enemies: Vec::new(),
            },
            Transform::from_translation(player_pos),
        ));
    }
}
