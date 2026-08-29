//! Attach presentation sprites to simulation entities.
//!
//! The simulation spawns entities with components + `Transform` (scale
//! included) only; these systems watch for newly added sim components and
//! insert the matching `Sprite` or procedural geometry (ADR-0004). Regular
//! visuals use atlas cells; evolution effects are drawn as geometry here.

use bevy::prelude::*;

use bevy::asset::RenderAssetUsages;
use bevy::render::render_resource::PrimitiveTopology;

use game_core::economy::Material;
use game_core::enemy::Enemy;
use game_core::player::Player;
use game_core::weapon::{
    BomberExplosion, MeleeHit, OrbitOrb, Projectile, Whirlwind, ORB_HIT_RADIUS,
};

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
                attach_whirlwind_visual,
                animate_whirlwinds,
                attach_bomber_explosion_visual,
                animate_bomber_explosions,
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
    // orb hitbox.
    for (entity, mut transform) in &mut orbs {
        transform.scale = Vec3::splat(ORB_HIT_RADIUS * 2.0 / ATLAS_CELL as f32);
        commands
            .entity(entity)
            .insert(atlas_sprite(&sprite_assets, atlas_index::ORB));
    }
}

/// Build a hollow ring from triangles so effect size is expressed directly in
/// gameplay units: its outer diameter is exactly `2 * radius`.
fn ring_mesh(radius: f32, thickness: f32) -> Mesh {
    let segments = 48;
    let inner_radius = (radius - thickness).max(1.0);
    let mut positions = Vec::with_capacity(segments * 6);
    for segment in 0..segments {
        let start = segment as f32 / segments as f32 * std::f32::consts::TAU;
        let end = (segment + 1) as f32 / segments as f32 * std::f32::consts::TAU;
        let point =
            |distance: f32, angle: f32| [distance * angle.cos(), distance * angle.sin(), 0.0];
        let outer_start = point(radius, start);
        let outer_end = point(radius, end);
        let inner_start = point(inner_radius, start);
        let inner_end = point(inner_radius, end);
        positions.extend([
            outer_start,
            outer_end,
            inner_end,
            outer_start,
            inner_end,
            inner_start,
        ]);
    }

    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh
}

fn attach_whirlwind_visual(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut whirlwinds: Query<(Entity, &Whirlwind, &mut Transform), Added<Whirlwind>>,
) {
    for (entity, whirlwind, mut transform) in &mut whirlwinds {
        transform.translation.z = 0.2;
        let mesh = meshes.add(ring_mesh(whirlwind.radius, 7.0));
        let material = materials.add(ColorMaterial::from(Color::srgba(1.0, 0.78, 0.22, 0.9)));
        commands
            .entity(entity)
            .insert((Mesh2d(mesh), MeshMaterial2d(material)));
    }
}

fn animate_whirlwinds(
    time: Res<Time>,
    mut whirlwinds: Query<(&mut Transform, &Whirlwind, &MeshMaterial2d<ColorMaterial>)>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    for (mut transform, whirlwind, material) in &mut whirlwinds {
        let duration = whirlwind.pulse.duration().as_secs_f32();
        let phase = if duration > 0.0 {
            whirlwind.pulse.elapsed_secs() / duration
        } else {
            0.0
        };
        let alpha = 0.68 + 0.22 * (phase * std::f32::consts::TAU).sin();
        transform.rotation = Quat::from_rotation_z(time.elapsed_secs() * 3.0);
        transform.scale = Vec3::ONE;
        if let Some(material) = materials.get_mut(&material.0) {
            material.color = Color::srgba(1.0, 0.78, 0.22, alpha);
        }
    }
}

fn attach_bomber_explosion_visual(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut explosions: Query<(Entity, &BomberExplosion, &mut Transform), Added<BomberExplosion>>,
) {
    for (entity, explosion, mut transform) in &mut explosions {
        transform.translation.z = 0.3;
        let mesh = meshes.add(ring_mesh(explosion.radius, 8.0));
        let material = materials.add(ColorMaterial::from(Color::srgba(1.0, 0.35, 0.12, 0.85)));
        commands
            .entity(entity)
            .insert((Mesh2d(mesh), MeshMaterial2d(material)));
    }
}

fn animate_bomber_explosions(
    mut explosions: Query<(
        &BomberExplosion,
        &mut Transform,
        &MeshMaterial2d<ColorMaterial>,
    )>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    for (explosion, mut transform, material) in &mut explosions {
        let duration = explosion.lifetime.duration().as_secs_f32();
        let progress = if duration > 0.0 {
            (explosion.lifetime.elapsed_secs() / duration).clamp(0.0, 1.0)
        } else {
            1.0
        };
        transform.scale = Vec3::ONE;
        if let Some(material) = materials.get_mut(&material.0) {
            material.color = Color::srgba(1.0, 0.35, 0.12, 0.85 * (1.0 - progress));
        }
    }
}
