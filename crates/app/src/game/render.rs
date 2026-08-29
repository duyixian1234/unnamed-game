//! Attach presentation sprites to simulation entities.
//!
//! The simulation spawns entities with components + `Transform` (scale
//! included) only; these systems watch for newly added sim components and
//! insert the matching `Sprite` (ADR-0004). All visuals use atlas cells; the
//! melee swing is tinted translucent here.

use bevy::prelude::*;

use game_core::economy::Material;
use game_core::enemy::Enemy;
use game_core::player::Player;
use game_core::weapon::{MeleeHit, OrbitOrb, Projectile};

use super::assets::{atlas_index, atlas_sprite, SpriteAssets, ATLAS_CELL};
/// Plugin attaching sprites to simulation entities.
pub struct RenderPlugin;

impl Plugin for RenderPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                attach_player_sprite,
                attach_enemy_sprite,
                attach_material_sprite,
                attach_projectile_sprite,
                attach_melee_sprite,
                attach_orb_sprite,
            ),
        );
    }
}

fn attach_player_sprite(
    mut commands: Commands,
    sprite_assets: Res<SpriteAssets>,
    players: Query<Entity, (Added<Player>, Without<Sprite>)>,
) {
    for entity in &players {
        commands
            .entity(entity)
            .insert(atlas_sprite(&sprite_assets, atlas_index::PLAYER));
    }
}

#[allow(clippy::type_complexity)] // Added<T> + Without<Sprite> disambiguation filters
fn attach_enemy_sprite(
    mut commands: Commands,
    sprite_assets: Res<SpriteAssets>,
    enemies: Query<(Entity, &Enemy), (Added<Enemy>, Without<Sprite>)>,
) {
    for (entity, enemy) in &enemies {
        let index = sprite_assets.enemy_index(enemy.kind);
        commands
            .entity(entity)
            .insert(atlas_sprite(&sprite_assets, index));
    }
}

/// Dropped materials render at a fixed small size (18px on the 128px atlas
/// cell); the sim spawns them unscaled, so the render layer sets the scale.
#[allow(clippy::type_complexity)] // Added<T> + Without<Sprite> disambiguation filters
fn attach_material_sprite(
    mut commands: Commands,
    sprite_assets: Res<SpriteAssets>,
    mut materials: Query<(Entity, &mut Transform), (Added<Material>, Without<Sprite>)>,
) {
    const MATERIAL_SIZE_PX: f32 = 18.0;
    for (entity, mut transform) in &mut materials {
        transform.scale = Vec3::splat(MATERIAL_SIZE_PX / ATLAS_CELL as f32);
        commands
            .entity(entity)
            .insert(atlas_sprite(&sprite_assets, atlas_index::MATERIAL));
    }
}

/// Projectiles render as the atlas energy-bolt sprite (cell 6), scaled to
/// ~26px and rotated along their flight direction (fixed at spawn — the sim
/// never changes `Projectile::direction`).
#[allow(clippy::type_complexity)] // Added<T> + Without<Sprite> disambiguation filters
fn attach_projectile_sprite(
    mut commands: Commands,
    sprite_assets: Res<SpriteAssets>,
    mut projectiles: Query<
        (Entity, &Projectile, &mut Transform),
        (Added<Projectile>, Without<Sprite>),
    >,
) {
    for (entity, projectile, mut transform) in &mut projectiles {
        transform.rotation =
            Quat::from_rotation_z(projectile.direction.y.atan2(projectile.direction.x));
        transform.scale = Vec3::splat(26.0 / ATLAS_CELL as f32);
        commands
            .entity(entity)
            .insert(atlas_sprite(&sprite_assets, atlas_index::PROJECTILE));
    }
}

/// The melee swing renders as the atlas ring (cell 5), scaled to the hitbox:
/// the ring sits at ~84% of the swing radius inside the 128px cell (ring
/// geometry generated in tools/gen_sprites.sh), so the drawn ring lands just
/// inside the actual hit radius. Tinted translucent so the 0.15 s flash reads
/// as a quick pulse, not a wall.
#[allow(clippy::type_complexity)] // Added<T> + Without<Sprite> disambiguation filters
fn attach_melee_sprite(
    mut commands: Commands,
    sprite_assets: Res<SpriteAssets>,
    mut melee_hits: Query<(Entity, &MeleeHit, &mut Transform), (Added<MeleeHit>, Without<Sprite>)>,
) {
    for (entity, melee, mut transform) in &mut melee_hits {
        transform.scale = Vec3::splat(melee.radius * 2.0 / ATLAS_CELL as f32);
        commands.entity(entity).insert(Sprite {
            color: Color::srgba(1.0, 1.0, 1.0, 0.6),
            ..atlas_sprite(&sprite_assets, atlas_index::MELEE_SWING)
        });
    }
}

#[allow(clippy::type_complexity)] // Added<T> + Without<Sprite> disambiguation filters
fn attach_orb_sprite(
    mut commands: Commands,
    sprite_assets: Res<SpriteAssets>,
    mut orbs: Query<(Entity, &mut Transform), (Added<OrbitOrb>, Without<Sprite>)>,
) {
    // The sim spawns orbs unscaled; render at 18px to exactly match the
    // orb hitbox (combat.rs: orb_radius = 9.0).
    for (entity, mut transform) in &mut orbs {
        transform.scale = Vec3::splat(18.0 / ATLAS_CELL as f32);
        commands
            .entity(entity)
            .insert(atlas_sprite(&sprite_assets, atlas_index::ORB));
    }
}
