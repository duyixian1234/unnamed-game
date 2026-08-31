//! The weapon system: auto-aiming loadout slots, projectiles, and orbit orbs.

use bevy::math::Vec2;
use bevy::prelude::*;

use crate::combat::{
    apply_damage, circle_hits_enemy, hit_cooldown_ready, restart_hit_cooldown, CombatSet,
};
use crate::damage::{DamageSource, DamageStats, WeaponSlot};
use crate::enemy::{Enemy, Knockback};
use crate::player::{Player, PlayerStats};
use crate::upgrade::Evolved;
use crate::GameState;

/// Maximum number of equipped weapons (per CONTEXT.md).
pub const MAX_WEAPON_SLOTS: usize = 6;

/// The MVP weapon archetypes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum WeaponKind {
    PiercingProjectile,
    MeleeSwing,
    OrbitingOrb,
}

impl WeaponKind {
    /// Stable order used by weapon-selection UI and deterministic tests.
    pub const ALL: [Self; 3] = [
        Self::PiercingProjectile,
        Self::MeleeSwing,
        Self::OrbitingOrb,
    ];

    /// Chinese display name (per ADR-0007, UI is Chinese).
    pub fn display_name(self) -> &'static str {
        match self {
            WeaponKind::PiercingProjectile => "穿透弹",
            WeaponKind::MeleeSwing => "近战弧光",
            WeaponKind::OrbitingOrb => "环绕球",
        }
    }

    /// Short description of the weapon's intended playstyle.
    pub fn playstyle(self) -> &'static str {
        match self {
            WeaponKind::PiercingProjectile => "远程安全、直线群伤",
            WeaponKind::MeleeSwing => "高风险高爆发、强击退",
            WeaponKind::OrbitingOrb => "贴身持续清群、依赖走位",
        }
    }

    /// Knockback impulse strength (initial velocity, units/s) applied to
    /// enemies on each damaging hit. Melee is strongest (self-defense), the
    /// orb weakest (it re-hits every frame while touching).
    pub fn knockback(self) -> f32 {
        match self {
            WeaponKind::PiercingProjectile => 80.0,
            WeaponKind::MeleeSwing => 150.0,
            WeaponKind::OrbitingOrb => 180.0,
        }
    }
}

/// The Weapon chosen for the current Run. The selection remains available on
/// end screens so "Play Again" can focus the previous choice.
#[derive(Resource, Debug, Default)]
pub struct StartingWeapon {
    pub selected: Option<WeaponKind>,
}

/// Confirms the Weapon used for a new Run.
#[derive(Message, Debug, Clone, Copy)]
pub struct StartingWeaponSelected {
    pub kind: WeaponKind,
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
    /// Multiplier on `WeaponKind::knockback` (upgrade paths scale it).
    pub knockback_mult: f32,
    /// Orbit angular speed (radians/s); used when kind == OrbitingOrb.
    pub orbit_speed: f32,
    /// Orbit radius around the player; used when kind == OrbitingOrb.
    pub orbit_radius: f32,
    /// Number of independent orbs produced by this weapon slot.
    pub orb_count: u8,
    /// Orb collision + visual size multiplier (upgrade paths scale it,
    /// damage unchanged); used when kind == OrbitingOrb.
    pub orb_size: f32,
}

impl Weapon {
    pub fn new(kind: WeaponKind) -> Self {
        let (interval, damage, projectile_speed, range) = match kind {
            WeaponKind::PiercingProjectile => (0.6, 24.0, 420.0, 900.0),
            WeaponKind::MeleeSwing => (0.7, 40.0, 0.0, 105.0),
            WeaponKind::OrbitingOrb => (0.0, 70.0, 0.0, 0.0),
        };
        let (orbit_speed, orbit_radius) = match kind {
            WeaponKind::OrbitingOrb => (10.0, 100.0),
            _ => (0.0, 0.0),
        };
        Self {
            kind,
            cooldown: Timer::from_seconds(interval, TimerMode::Repeating),
            damage,
            projectile_speed,
            range,
            knockback_mult: 1.0,
            orbit_speed,
            orbit_radius,
            orb_count: 1,
            orb_size: 1.0,
        }
    }

    /// Final knockback impulse for this weapon's hits.
    pub fn knockback_impulse(&self) -> f32 {
        self.kind.knockback() * self.knockback_mult
    }

    pub(crate) fn clone_for_new_slot(&self) -> Self {
        Self {
            kind: self.kind,
            cooldown: Timer::from_seconds(
                self.cooldown.duration().as_secs_f32(),
                TimerMode::Repeating,
            ),
            damage: self.damage,
            projectile_speed: self.projectile_speed,
            range: self.range,
            knockback_mult: self.knockback_mult,
            orbit_speed: self.orbit_speed,
            orbit_radius: self.orbit_radius,
            orb_count: self.orb_count,
            orb_size: self.orb_size,
        }
    }

    pub(crate) fn threat_radius(&self, ranged_default: f32) -> f32 {
        match self.kind {
            WeaponKind::PiercingProjectile => ranged_default,
            WeaponKind::MeleeSwing => 55.0,
            WeaponKind::OrbitingOrb => self.orbit_radius * 0.75,
        }
    }

    pub(crate) fn engagement_range(&self) -> Option<f32> {
        match self.kind {
            WeaponKind::PiercingProjectile => None,
            WeaponKind::MeleeSwing => Some(self.range * 0.8),
            // Orbs are passive area coverage; chasing enemies to the maximum
            // pulse radius leaves the player exposed while the orb returns.
            WeaponKind::OrbitingOrb => None,
        }
    }
}

/// A projectile fired by a weapon; pierces through enemies until it expires.
#[derive(Component)]
pub struct Projectile {
    pub source: DamageSource,
    pub damage: f32,
    /// Knockback impulse applied to enemies on hit.
    pub knockback: f32,
    pub speed: f32,
    pub direction: Vec2,
    pub lifetime: Timer,
    /// Enemies this projectile has already struck, so a piercing shot only
    /// hits each enemy once.
    pub hit_enemies: Vec<Entity>,
}

/// Marker on projectiles of a Splitshot-evolved weapon: on the first enemy
/// hit they fan out into 3 short-range shards (then the marker is removed so
/// they split only once). Shards themselves carry no marker.
#[derive(Component)]
pub struct SplitsOnHit;

/// Marker on orbs of a Bomber-Orb-evolved weapon: on contact they explode in
/// a small AOE and despawn (`update_orbs` respawns them after a delay).
#[derive(Component)]
pub struct BomberOrb;

/// Short-lived presentation hook for the Bomber Orb's real AOE.
#[derive(Component)]
pub struct BomberExplosion {
    pub radius: f32,
    pub lifetime: Timer,
}

/// The Whirlwind evolution's persistent blade: follows the player and damages
/// every enemy in reach continuously (per-enemy re-hit cooldown), replacing
/// the melee swing's discrete rhythm entirely.
#[derive(Component)]
pub struct Whirlwind {
    pub slot: WeaponSlot,
    pub source: DamageSource,
    pub damage: f32,
    pub knockback: f32,
    /// Radius of the blade reach around the player.
    pub radius: f32,
    /// Per-enemy re-hit cooldowns, so "continuous" does not mean per-frame.
    pub hit_cooldowns: Vec<(Entity, Timer)>,
    /// Presentation pulse synchronized to the 0.4 second damage rhythm.
    pub pulse: Timer,
}

/// Seconds before the whirlwind can strike the same enemy again.
pub const WHIRLWIND_REHIT: f32 = 0.4;
/// Extra knockback on whirlwind strikes (Evolution: knockback boost).
const WHIRLWIND_KNOCKBACK_BONUS: f32 = 1.25;
/// Radius of the Bomber Orb's contact explosion.
pub const BOMBER_AOE_RADIUS: f32 = 90.0;
/// Seconds before an orb may damage the same enemy again.
pub const ORB_REHIT: f32 = 0.25;
/// Full period of the orbiting orb's 0 -> max -> 0 radius pulse.
pub const ORB_RADIUS_CYCLE: f32 = 1.2;
/// Seconds before an exploded Bomber Orb returns.
pub const BOMBER_RESPAWN: f32 = 0.6;
/// Bomber Orb AOE damage multiplier.
const BOMBER_AOE_DAMAGE_MULTIPLIER: f32 = 2.0;
/// Collision radius of an orbiting orb.
pub const ORB_HIT_RADIUS: f32 = 18.0;
/// Damage fraction each Splitshot shard inherits.
const SHARD_DAMAGE_FRACTION: f32 = 0.5;
/// Shard lifetime (short range); at base speed that is ~150 units.
const SHARD_LIFETIME: f32 = 0.35;
/// Half-angle of the 3-shard fan (radians).
const SHARD_FAN_ANGLE: f32 = 0.35;
/// Base melee swing arc: a 120-degree fan.
pub const MELEE_FAN_HALF_ANGLE: f32 = std::f32::consts::PI / 3.0;

/// A melee swing hitbox spawned briefly at the player; damages enemies it
/// overlaps. Lives long enough for combat resolution to run before it expires.
#[derive(Component)]
pub struct MeleeHit {
    pub source: DamageSource,
    pub damage: f32,
    /// Knockback impulse applied to enemies on hit.
    pub knockback: f32,
    /// Radius of the swing around the player.
    pub radius: f32,
    /// Center direction and half-angle of the melee fan.
    pub direction: Vec2,
    pub half_angle: f32,
    pub hit_enemies: Vec<Entity>,
    /// How long the swing stays active; combat must resolve within this window.
    pub lifetime: Timer,
}

/// An orb orbiting the player; damages enemies it touches.
#[derive(Component)]
pub struct OrbitOrb {
    pub source: DamageSource,
    pub damage: f32,
    /// Knockback impulse applied to enemies on hit (each frame while touching).
    pub knockback: f32,
    /// Angular position around the player (radians).
    pub angle: f32,
    /// Angular speed (radians/sec).
    pub angular_speed: f32,
    /// Orbit radius around the player.
    pub radius: f32,
    /// Maximum radius used by the pulse envelope.
    pub max_radius: f32,
    /// Radial pulse phase in the 1.2 second cycle.
    pub radial_phase: f32,
    /// Weapon slot that owns this independent orb.
    pub slot: WeaponSlot,
    /// Stable ordinal within its owning weapon's orb set.
    pub index: u8,
    /// Global ordinal across every live orb (spawn order): drives the
    /// alternating spin direction and the per-orb tint.
    pub ordinal: u8,
    /// Spin direction: +1 counter-clockwise, -1 clockwise. Alternates by
    /// global ordinal (ADR-0009 amendment).
    pub spin: f32,
    /// Collision + visual size multiplier, synced from the owning weapon's
    /// `orb_size` every frame.
    pub size: f32,
    /// Enemies hit this frame; Bomber Orb uses this to detect contact.
    pub hit_enemies: Vec<Entity>,
    /// Per-enemy cooldowns make contact damage independent of frame rate.
    pub hit_cooldowns: Vec<(Entity, Timer)>,
}

/// A Bomber Orb waiting to respawn. This state is per entity, so one orb
/// exploding never pauses or respawns its siblings.
#[derive(Component, Clone)]
pub struct OrbRespawn {
    pub source: DamageSource,
    pub damage: f32,
    pub knockback: f32,
    pub angular_speed: f32,
    pub max_radius: f32,
    pub angle: f32,
    pub radial_phase: f32,
    pub slot: WeaponSlot,
    pub index: u8,
    pub ordinal: u8,
    pub spin: f32,
    pub size: f32,
    pub bomber: bool,
    pub timer: Timer,
}

/// Marker added to the player once its weapon loadout has been spawned, so we
/// don't duplicate weapons on every re-entry to InGame (e.g. between waves).
#[derive(Component)]
pub struct WeaponLoadout;

/// Plugin for the weapon system.
pub struct WeaponPlugin;

impl Plugin for WeaponPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<StartingWeapon>()
            .add_message::<StartingWeaponSelected>()
            .add_systems(
                Update,
                select_starting_weapon.run_if(in_state(GameState::StartingWeaponChoice)),
            )
            .add_systems(
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
                    // The whirlwind blade must be positioned/synced before damage
                    // resolution; its hits resolve after combat like the others.
                    update_whirlwind.before(CombatSet::ResolveDamage),
                    // Expire hitboxes only after combat damage has resolved, so
                    // a just-spawned melee swing connects this frame.
                    expire_melee_hits.after(CombatSet::ResolveDamage),
                    // Evolution behaviors resolve after core combat so their
                    // triggers (first hit / orb contact) are already recorded.
                    (
                        splitshot_on_first_hit,
                        bomber_orb_explosions,
                        resolve_whirlwind_hits,
                    )
                        .after(CombatSet::ResolveDamage),
                    expire_bomber_explosions,
                )
                    .run_if(in_state(GameState::InGame)),
            );
    }
}

fn select_starting_weapon(
    mut selections: MessageReader<StartingWeaponSelected>,
    mut starting_weapon: ResMut<StartingWeapon>,
    mut next_state: ResMut<NextState<GameState>>,
) {
    if let Some(selection) = selections.read().next() {
        starting_weapon.selected = Some(selection.kind);
        next_state.set(GameState::InGame);
    }
}

/// Hand the player the selected starting Weapon when a Run begins.
#[allow(clippy::type_complexity)]
fn give_starting_weapons(
    mut commands: Commands,
    starting_weapon: Res<StartingWeapon>,
    players: Query<(Entity, &Transform), (With<Player>, Without<WeaponLoadout>)>,
) {
    let Some(kind) = starting_weapon.selected else {
        return;
    };
    for (player, player_transform) in &players {
        let slot = commands
            .spawn((
                Weapon::new(kind),
                WeaponSlot(0),
                Transform::from_translation(player_transform.translation),
            ))
            .id();
        commands.entity(player).add_child(slot);
        commands.entity(player).insert(WeaponLoadout);
    }
}

/// Drive every weapon: fire projectiles, swing melee hitboxes, or manage orbs.
#[allow(clippy::type_complexity)] // Evolved disambiguation on the slot query
fn auto_attack(
    time: Res<Time>,
    mut commands: Commands,
    players: Query<(&Transform, &PlayerStats), With<Player>>,
    mut weapons: Query<(&mut Weapon, &WeaponSlot, Option<&Evolved>), Without<Player>>,
    enemies: Query<(&Transform, &Enemy), Without<Player>>,
) {
    let Ok((player_transform, stats)) = players.single() else {
        return;
    };
    let player_pos = player_transform.translation.truncate();

    for (mut weapon, slot, evolved) in &mut weapons {
        let evolved = evolved.is_some();
        let source = DamageSource::Weapon {
            slot: *slot,
            kind: weapon.kind,
        };
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
                let mut projectile = commands.spawn((
                    Projectile {
                        source,
                        damage: weapon.damage * stats.damage_mult,
                        knockback: weapon.knockback_impulse(),
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
                if evolved {
                    projectile.insert(SplitsOnHit);
                }
            }
            WeaponKind::MeleeSwing => {
                // The Whirlwind evolution replaces the discrete swing: the
                // blade never fires on the swing rhythm again.
                if evolved {
                    continue;
                }
                // Gate on reach (the same formula as the hit test via
                // combat::circle_hits_enemy), so the swing only fires — and
                // its visual only flashes — when it can actually connect.
                let Some(target) = nearest_enemy(player_pos, &enemies) else {
                    continue;
                };
                let direction = (target - player_pos).normalize_or_zero();
                if direction == Vec2::ZERO {
                    continue;
                }
                let in_reach = enemies.iter().any(|(transform, enemy)| {
                    circle_hits_enemy(
                        player_pos,
                        weapon.range,
                        transform.translation.truncate(),
                        enemy,
                    )
                });
                if !in_reach {
                    continue;
                }
                let radius = weapon.range;
                commands.spawn((
                    MeleeHit {
                        source,
                        damage: weapon.damage * stats.damage_mult,
                        knockback: weapon.knockback_impulse(),
                        radius,
                        direction,
                        half_angle: MELEE_FAN_HALF_ANGLE,
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
    enemies: &Query<(&Transform, &Enemy), Without<Player>>,
) -> Option<Vec2> {
    let mut nearest: Option<(f32, Vec2)> = None;
    for (transform, _) in enemies {
        let pos = transform.translation.truncate();
        let dist_sq = pos.distance_squared(player_pos);
        if nearest.is_none_or(|(best, _)| dist_sq < best) {
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
/// Orb stats live on the weapon (upgradeable) and are re-synced every frame;
/// an evolved weapon spawns Bomber Orb variants.
#[allow(clippy::type_complexity)] // slot vs. live-orb queries need mutual exclusion
fn update_orbs(
    mut commands: Commands,
    time: Res<Time>,
    players: Query<(&Transform, &PlayerStats), With<Player>>,
    weapons: Query<(&Weapon, &WeaponSlot, Option<&Evolved>), Without<Player>>,
    mut orbs: Query<(Entity, &mut OrbitOrb, &mut Transform), (Without<Player>, Without<Weapon>)>,
    mut respawns: Query<
        (Entity, &mut OrbRespawn),
        (Without<Player>, Without<Weapon>, Without<OrbitOrb>),
    >,
) {
    let Ok((player_transform, stats)) = players.single() else {
        return;
    };
    let player_pos = player_transform.translation;

    let mut specs = Vec::new();
    for (weapon, slot, evolved) in &weapons {
        if weapon.kind == WeaponKind::OrbitingOrb {
            specs.push((
                *slot,
                weapon.damage * stats.damage_mult,
                weapon.knockback_impulse(),
                weapon.orbit_speed,
                weapon.orbit_radius,
                DamageSource::Weapon {
                    slot: *slot,
                    kind: weapon.kind,
                },
                evolved.is_some(),
                weapon.orb_size,
                weapon.orb_count.max(1),
            ));
        }
    }

    // Respawn timers are attached to the individual orb entity.
    for (_, mut respawn) in &mut respawns {
        respawn.timer.tick(time.delta());
    }
    let finished: Vec<_> = respawns
        .iter()
        .filter(|(_, respawn)| respawn.timer.is_finished())
        .map(|(entity, respawn)| (entity, respawn.clone()))
        .collect();
    for (entity, respawn) in finished {
        if !respawn.timer.is_finished() {
            continue;
        }
        let orb = OrbitOrb {
            source: respawn.source,
            damage: respawn.damage,
            knockback: respawn.knockback,
            angle: respawn.angle,
            angular_speed: respawn.angular_speed,
            radius: pulse_radius(respawn.max_radius, respawn.radial_phase),
            max_radius: respawn.max_radius,
            radial_phase: respawn.radial_phase,
            slot: respawn.slot,
            index: respawn.index,
            ordinal: respawn.ordinal,
            spin: respawn.spin,
            size: respawn.size,
            hit_enemies: Vec::new(),
            hit_cooldowns: Vec::new(),
        };
        let mut entity_commands = commands.entity(entity);
        entity_commands.remove::<OrbRespawn>();
        entity_commands.insert((orb, Transform::from_translation(player_pos)));
        if respawn.bomber {
            entity_commands.insert(BomberOrb);
        }
    }

    // Rotate existing orbs around the player and re-sync upgradeable stats.
    let total: u32 = specs.iter().map(|(.., count)| *count as u32).sum();
    let rephase = respawns.iter().next().is_none() && orbs.iter().count() as u32 != total;
    for (_, mut orb, mut transform) in &mut orbs {
        if rephase {
            let ordinal = specs
                .iter()
                .take_while(|(slot, ..)| *slot != orb.slot)
                .map(|(.., count)| *count as u32)
                .sum::<u32>()
                + orb.index as u32;
            let angular_phase = ordinal as f32 / total.max(1) as f32;
            orb.angle = std::f32::consts::TAU * angular_phase;
            orb.radial_phase = ordinal as f32 / total.max(1) as f32;
            orb.ordinal = ordinal as u8;
            orb.spin = spin_sign(ordinal);
        }
        if let Some((_, damage, knockback, angular_speed, max_radius, source, _, size, _)) =
            specs.iter().find(|(slot, ..)| *slot == orb.slot)
        {
            orb.damage = *damage;
            orb.knockback = *knockback;
            orb.angular_speed = *angular_speed;
            orb.max_radius = *max_radius;
            orb.source = *source;
            orb.size = *size;
        }
        orb.angle += orb.spin * orb.angular_speed * time.delta_secs();
        orb.radial_phase = (orb.radial_phase + time.delta_secs() / ORB_RADIUS_CYCLE).fract();
        orb.radius = pulse_radius(orb.max_radius, orb.radial_phase);
        let offset = Vec2::from_angle(orb.angle) * orb.radius;
        transform.translation = player_pos.truncate().extend(0.0) + offset.extend(0.0);
        for (_, timer) in &mut orb.hit_cooldowns {
            timer.tick(time.delta());
        }
        orb.hit_cooldowns
            .retain(|(entity, _)| commands.get_entity(*entity).is_ok());
        // Contact markers last one frame; cooldowns persist.
        orb.hit_enemies.clear();
    }

    // Ensure each weapon has its requested number of independent orbs.
    let mut ordinal = 0u32;
    for (slot, damage, knockback, angular_speed, max_radius, source, evolved, size, count) in specs
    {
        for index in 0..count {
            let exists = orbs
                .iter()
                .any(|(_, orb, _)| orb.slot == slot && orb.index == index);
            let pending = respawns
                .iter()
                .any(|(_, respawn)| respawn.slot == slot && respawn.index == index);
            if exists || pending {
                ordinal += 1;
                continue;
            }
            let angular_phase = ordinal as f32 / total.max(1) as f32;
            let radial_phase = ordinal as f32 / total.max(1) as f32;
            let mut orb = commands.spawn((
                OrbitOrb {
                    source,
                    damage,
                    knockback,
                    angle: std::f32::consts::TAU * angular_phase,
                    angular_speed,
                    radius: pulse_radius(max_radius, radial_phase),
                    max_radius,
                    radial_phase,
                    slot,
                    index,
                    ordinal: ordinal as u8,
                    spin: spin_sign(ordinal),
                    size,
                    hit_enemies: Vec::new(),
                    hit_cooldowns: Vec::new(),
                },
                Transform::from_translation(player_pos),
            ));
            if evolved {
                orb.insert(BomberOrb);
            }
            ordinal += 1;
        }
    }
}

fn pulse_radius(max_radius: f32, phase: f32) -> f32 {
    let phase = phase.fract();
    let progress = if phase <= 0.5 {
        phase * 2.0
    } else {
        (1.0 - phase) * 2.0
    };
    let envelope = progress * progress * (3.0 - 2.0 * progress);
    max_radius * envelope
}

/// Even ordinals spin counter-clockwise, odd ones clockwise, so consecutive
/// orbs visibly counter-rotate (ADR-0009 amendment).
fn spin_sign(ordinal: u32) -> f32 {
    if ordinal.is_multiple_of(2) {
        1.0
    } else {
        -1.0
    }
}

/// Whirlwind evolution: keep one persistent blade per evolved MeleeSwing,
/// glued to the player (movement never interrupts it), with stats mirrored
/// from the weapon each frame.
fn update_whirlwind(
    mut commands: Commands,
    players: Query<(&Transform, &PlayerStats), With<Player>>,
    melee: Query<(&Weapon, &WeaponSlot, Option<&Evolved>), Without<Player>>,
    mut whirlwinds: Query<(Entity, &mut Whirlwind, &mut Transform), Without<Player>>,
) {
    let Ok((player_transform, stats)) = players.single() else {
        return;
    };
    let player_pos = player_transform.translation;

    let specs: Vec<_> = melee
        .iter()
        .filter(|(weapon, _, evolved)| weapon.kind == WeaponKind::MeleeSwing && evolved.is_some())
        .map(|(weapon, slot, _)| {
            (
                *slot,
                DamageSource::Weapon {
                    slot: *slot,
                    kind: weapon.kind,
                },
                weapon.damage * stats.damage_mult,
                weapon.knockback_impulse() * WHIRLWIND_KNOCKBACK_BONUS,
                weapon.range,
            )
        })
        .collect();
    if specs.is_empty() {
        return;
    }
    for (_, mut whirlwind, mut transform) in &mut whirlwinds {
        let Some((_, source, damage, knockback, radius)) =
            specs.iter().find(|(slot, ..)| *slot == whirlwind.slot)
        else {
            continue;
        };
        whirlwind.damage = *damage;
        whirlwind.source = *source;
        whirlwind.knockback = *knockback;
        whirlwind.radius = *radius;
        transform.translation = player_pos;
    }
    for (slot, source, damage, knockback, radius) in specs {
        if whirlwinds
            .iter()
            .any(|(_, whirlwind, _)| whirlwind.slot == slot)
        {
            continue;
        }
        commands.spawn((
            Whirlwind {
                slot,
                source,
                damage,
                knockback,
                radius,
                hit_cooldowns: Vec::new(),
                pulse: Timer::from_seconds(WHIRLWIND_REHIT, TimerMode::Repeating),
            },
            Transform::from_translation(player_pos),
        ));
    }
}

/// The whirlwind damages every enemy in reach continuously: a per-enemy
/// re-hit cooldown spreads strikes out instead of hitting every frame.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn resolve_whirlwind_hits(
    time: Res<Time>,
    mut commands: Commands,
    mut death_writer: MessageWriter<crate::combat::EnemyDied>,
    mut spawn_writer: MessageWriter<crate::enemy::EnemySpawned>,
    mut hit_writer: MessageWriter<crate::combat::HitSfx>,
    mut damage_stats: ResMut<DamageStats>,
    mut whirlwinds: Query<(&mut Whirlwind, &Transform)>,
    mut enemies: Query<
        (Entity, &mut Enemy, &Transform, &mut Knockback),
        (Without<Whirlwind>, Without<Player>),
    >,
) {
    for (mut whirlwind, whirlwind_transform) in &mut whirlwinds {
        let whirlwind_pos = whirlwind_transform.translation.truncate();
        whirlwind.pulse.tick(time.delta());

        for (_, timer) in whirlwind.hit_cooldowns.iter_mut() {
            timer.tick(time.delta());
        }
        // Drop cooldown entries for enemies that no longer exist.
        whirlwind
            .hit_cooldowns
            .retain(|(entity, _)| enemies.contains(*entity));

        for (enemy_entity, mut enemy, enemy_transform, mut knockback) in &mut enemies {
            if !hit_cooldown_ready(&whirlwind.hit_cooldowns, enemy_entity) {
                continue;
            }
            let enemy_pos = enemy_transform.translation.truncate();
            if !circle_hits_enemy(whirlwind_pos, whirlwind.radius, enemy_pos, &enemy) {
                continue;
            }
            hit_writer.write(crate::combat::HitSfx);
            apply_damage(
                &mut commands,
                &mut death_writer,
                &mut spawn_writer,
                enemy_entity,
                &mut enemy,
                enemy_pos,
                whirlwind.damage,
                whirlwind.source,
                &mut damage_stats,
            );
            let dir = (enemy_pos - whirlwind_pos).normalize_or_zero();
            knockback.0 += dir * whirlwind.knockback;
            restart_hit_cooldown(&mut whirlwind.hit_cooldowns, enemy_entity, WHIRLWIND_REHIT);
        }
    }
}

/// Splitshot evolution: a marked projectile that just struck its first enemy
/// fans out into 3 short-range shards inheriting part of its damage. The
/// marker is removed so the parent splits exactly once (shards are unmarked).
fn splitshot_on_first_hit(
    mut commands: Commands,
    splitters: Query<(Entity, &Projectile, &Transform), With<SplitsOnHit>>,
) {
    for (entity, projectile, transform) in &splitters {
        if projectile.hit_enemies.is_empty() {
            continue;
        }
        commands.entity(entity).remove::<SplitsOnHit>();
        for i in [-1, 0, 1] {
            let direction =
                Vec2::from_angle(SHARD_FAN_ANGLE * i as f32).rotate(projectile.direction);
            commands.spawn((
                Projectile {
                    source: projectile.source,
                    damage: projectile.damage * SHARD_DAMAGE_FRACTION,
                    knockback: projectile.knockback,
                    speed: projectile.speed,
                    direction,
                    lifetime: Timer::from_seconds(SHARD_LIFETIME, TimerMode::Once),
                    hit_enemies: Vec::new(),
                },
                Transform::from_translation(transform.translation),
            ));
        }
    }
}

/// Bomber Orb evolution: an orb that touched an enemy this tick explodes in a
/// small AOE (skipping enemies the contact hit already damaged) and despawns;
/// `update_orbs` respawns it after a delay.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn bomber_orb_explosions(
    mut commands: Commands,
    mut death_writer: MessageWriter<crate::combat::EnemyDied>,
    mut spawn_writer: MessageWriter<crate::enemy::EnemySpawned>,
    mut hit_writer: MessageWriter<crate::combat::HitSfx>,
    mut damage_stats: ResMut<DamageStats>,
    orbs: Query<(Entity, &OrbitOrb, &Transform), With<BomberOrb>>,
    mut enemies: Query<
        (Entity, &mut Enemy, &Transform, &mut Knockback),
        (Without<OrbitOrb>, Without<Player>),
    >,
) {
    for (orb_entity, orb, orb_transform) in &orbs {
        if orb.hit_enemies.is_empty() {
            continue;
        }
        let orb_pos = orb_transform.translation.truncate();

        // Collect AOE victims first so the mutable enemy query is not held
        // across damage application. The contact target receives both the
        // direct hit and the explosion.
        let mut victims: Vec<(Entity, Vec2)> = Vec::new();
        for (enemy_entity, enemy, enemy_transform, _) in &enemies {
            let enemy_pos = enemy_transform.translation.truncate();
            if enemy.health > 0.0 && circle_hits_enemy(orb_pos, BOMBER_AOE_RADIUS, enemy_pos, enemy)
            {
                victims.push((enemy_entity, enemy_pos));
            }
        }
        for (victim, victim_pos) in victims {
            let Ok((_, mut enemy, _, mut knockback)) = enemies.get_mut(victim) else {
                continue;
            };
            hit_writer.write(crate::combat::HitSfx);
            apply_damage(
                &mut commands,
                &mut death_writer,
                &mut spawn_writer,
                victim,
                &mut enemy,
                victim_pos,
                orb.damage * BOMBER_AOE_DAMAGE_MULTIPLIER,
                orb.source,
                &mut damage_stats,
            );
            let dir = (victim_pos - orb_pos).normalize_or_zero();
            knockback.0 += dir * orb.knockback;
        }
        commands.spawn((
            BomberExplosion {
                radius: BOMBER_AOE_RADIUS,
                lifetime: Timer::from_seconds(0.22, TimerMode::Once),
            },
            Transform::from_translation(orb_transform.translation),
        ));
        commands
            .entity(orb_entity)
            .remove::<OrbitOrb>()
            .insert(OrbRespawn {
                source: orb.source,
                damage: orb.damage,
                knockback: orb.knockback,
                angular_speed: orb.angular_speed,
                max_radius: orb.max_radius,
                angle: orb.angle,
                radial_phase: orb.radial_phase,
                slot: orb.slot,
                index: orb.index,
                ordinal: orb.ordinal,
                spin: orb.spin,
                size: orb.size,
                bomber: true,
                timer: Timer::from_seconds(BOMBER_RESPAWN, TimerMode::Once),
            });
    }
}

fn expire_bomber_explosions(
    time: Res<Time>,
    mut commands: Commands,
    mut explosions: Query<(Entity, &mut BomberExplosion)>,
) {
    for (entity, mut explosion) in &mut explosions {
        explosion.lifetime.tick(time.delta());
        if explosion.lifetime.is_finished() {
            commands.entity(entity).despawn();
        }
    }
}
