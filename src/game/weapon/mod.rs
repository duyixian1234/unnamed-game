//! The weapon system: auto-aiming loadout slots and projectile entities.

use bevy::math::Vec2;
use bevy::prelude::*;

use crate::game::enemy::Enemy;
use crate::game::player::Player;
use crate::game::GameState;

/// The MVP weapon archetypes. Only the piercing projectile exists yet; melee
/// swing and orbiting orb arrive in a later milestone (T12).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WeaponKind {
    PiercingProjectile,
}

/// A weapon attached to the player. Auto-aims and auto-fires at enemies.
#[derive(Component)]
pub struct Weapon {
    pub kind: WeaponKind,
    /// Seconds between shots.
    pub fire_cooldown: Timer,
    pub damage: f32,
    pub projectile_speed: f32,
}

impl Weapon {
    pub fn new(kind: WeaponKind) -> Self {
        let (fire_interval, damage, projectile_speed) = match kind {
            WeaponKind::PiercingProjectile => (0.8, 10.0, 420.0),
        };
        Self {
            kind,
            fire_cooldown: Timer::from_seconds(fire_interval, TimerMode::Repeating),
            damage,
            projectile_speed,
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
}

/// Plugin for the weapon system.
pub struct WeaponPlugin;

impl Plugin for WeaponPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::InGame), give_starting_weapon)
            .add_systems(
                Update,
                (auto_fire, move_projectiles).run_if(in_state(GameState::InGame)),
            );
    }
}

/// Hand the player a starting piercing-projectile weapon when a run begins.
fn give_starting_weapon(
    mut commands: Commands,
    players: Query<Entity, (With<Player>, Without<Weapon>)>,
) {
    for entity in &players {
        commands
            .entity(entity)
            .insert(Weapon::new(WeaponKind::PiercingProjectile));
    }
}

/// Auto-aim at the nearest enemy and fire on cooldown.
fn auto_fire(
    time: Res<Time>,
    mut commands: Commands,
    mut players: Query<(&Transform, &mut Weapon), With<Player>>,
    enemies: Query<&Transform, (With<Enemy>, Without<Player>)>,
) {
    let Ok((player_transform, mut weapon)) = players.single_mut() else {
        return;
    };
    weapon.fire_cooldown.tick(time.delta());
    if !weapon.fire_cooldown.just_finished() {
        return;
    }

    let player_pos = player_transform.translation.truncate();

    // Auto-aim at the nearest enemy.
    let mut nearest: Option<(f32, Vec2)> = None;
    for transform in &enemies {
        let pos = transform.translation.truncate();
        let dist_sq = pos.distance_squared(player_pos);
        if nearest.map_or(true, |(best, _)| dist_sq < best) {
            nearest = Some((dist_sq, pos));
        }
    }
    let Some(target) = nearest.map(|(_, pos)| pos) else {
        return;
    };

    let direction = (target - player_pos).normalize_or_zero();
    if direction == Vec2::ZERO {
        return;
    }

    commands.spawn((
        Projectile {
            damage: weapon.damage,
            speed: weapon.projectile_speed,
            direction,
            lifetime: Timer::from_seconds(3.0, TimerMode::Once),
        },
        Sprite {
            color: Color::srgb(0.95, 0.85, 0.3),
            custom_size: Some(Vec2::new(24.0, 8.0)),
            ..default()
        },
        Transform::from_translation(player_pos.extend(0.0)),
    ));
}

fn move_projectiles(
    time: Res<Time>,
    mut commands: Commands,
    mut projectiles: Query<(Entity, &mut Transform, &mut Projectile)>,
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
