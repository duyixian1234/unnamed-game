//! The materials economy: enemy drops, player pickup, and the wallet.

use bevy::ecs::message::{Message, MessageReader, MessageWriter};
use bevy::prelude::*;

use crate::combat::EnemyDied;
use crate::player::Player;
use crate::GameState;

/// Fired when the player picks up a material (the app plays a sound).
#[derive(Message)]
pub struct PickupSfx;

/// A material was collected into the wallet; the assertion interface for the
/// economy loop.
#[derive(Message, Debug, Clone, Copy)]
pub struct MaterialPickedUp {
    pub amount: u32,
}

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
            .add_message::<PickupSfx>()
            .add_message::<MaterialPickedUp>()
            .add_systems(OnEnter(GameState::MainMenu), reset_materials)
            .add_systems(
                Update,
                (drop_on_enemy_death, pick_up_materials).run_if(in_state(GameState::InGame)),
            );
    }
}

/// A fresh run starts with an empty wallet.
fn reset_materials(mut materials: ResMut<Materials>) {
    materials.count = 0;
}

/// Spawn a material where each enemy died (EnemyDied message).
fn drop_on_enemy_death(mut commands: Commands, mut deaths: MessageReader<EnemyDied>) {
    for death in deaths.read() {
        commands.spawn((
            Material {
                value: MATERIAL_VALUE,
            },
            Transform::from_translation(death.position.extend(0.0)),
        ));
    }
}

/// When the player touches a material, collect it into the wallet.
fn pick_up_materials(
    mut commands: Commands,
    mut wallet: ResMut<Materials>,
    mut pickup_writer: MessageWriter<PickupSfx>,
    mut collected_writer: MessageWriter<MaterialPickedUp>,
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
        collected_writer.write(MaterialPickedUp {
            amount: material.value,
        });
        commands.entity(entity).despawn();
    }
}
