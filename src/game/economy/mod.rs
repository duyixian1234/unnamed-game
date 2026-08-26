//! The materials economy: enemy drops, player pickup, and the wallet.

use bevy::ecs::message::{MessageReader, MessageWriter};
use bevy::prelude::*;

use crate::game::assets::{atlas_index, atlas_sprite, SpriteAssets, ATLAS_CELL};
use crate::game::audio::PickupSfx;
use crate::game::combat::EnemyDied;
use crate::game::player::Player;
use crate::game::GameState;

/// A dropped material lying on the field; the player picks it up by collision.
#[derive(Component)]
pub struct Material {
    pub value: u32,
}

/// The player's material wallet — the shop currency.
#[derive(Resource, Default)]
pub struct Materials {
    pub count: u32,
}

/// How much a material drop is worth.
const MATERIAL_VALUE: u32 = 1;
/// Pickup radius (materials are pulled in when the player gets close).
const PICKUP_RADIUS: f32 = 26.0;

/// Plugin for the materials economy.
pub struct EconomyPlugin;

impl Plugin for EconomyPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Materials>()
            .add_systems(OnEnter(GameState::MainMenu), reset_materials)
            .add_systems(Update, (drop_on_enemy_death, pick_up_materials)
                .run_if(in_state(GameState::InGame)));
    }
}

/// A fresh run starts with an empty wallet.
fn reset_materials(mut materials: ResMut<Materials>) {
    materials.count = 0;
}

/// Spawn a material where each enemy died (EnemyDied message).
fn drop_on_enemy_death(
    mut commands: Commands,
    sprite_assets: Res<SpriteAssets>,
    mut deaths: MessageReader<EnemyDied>,
) {
    for death in deaths.read() {
        commands.spawn((
            Material { value: MATERIAL_VALUE },
            atlas_sprite(&sprite_assets, atlas_index::MATERIAL),
            Transform::from_translation(death.position.extend(0.0))
                .with_scale(Vec3::splat(18.0 / ATLAS_CELL as f32)),
        ));
    }
}

/// When the player touches a material, collect it into the wallet.
fn pick_up_materials(
    mut commands: Commands,
    mut wallet: ResMut<Materials>,
    mut pickup_writer: MessageWriter<PickupSfx>,
    players: Query<&Transform, With<Player>>,
    mut materials: Query<(Entity, &Transform, &Material), Without<Player>>,
) {
    let Ok(player_pos) = players.single().map(|t| t.translation.truncate()) else {
        return;
    };

    for (entity, transform, material) in &mut materials {
        let dist = player_pos.distance(transform.translation.truncate());
        if dist > PICKUP_RADIUS {
            continue;
        }
        wallet.count += material.value;
        pickup_writer.write(PickupSfx);
        commands.entity(entity).despawn();
    }
}
