//! The materials economy: enemy drops, player pickup, and the wallet.

use bevy::ecs::message::{Message, MessageReader, MessageWriter};
use bevy::prelude::*;

use crate::combat::EnemyDied;
use crate::player::{Player, PlayerStats};
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

/// Marker: the material entered the player's attraction radius and flies
/// toward the player until collected.
#[derive(Component)]
pub struct Attracted;

/// The player's material wallet — the shop currency.
#[derive(Resource, Default)]
pub struct Materials {
    pub count: u32,
}

/// How much a material drop is worth.
const MATERIAL_VALUE: u32 = 1;
/// Speed at which an attracted material flies toward the player. Faster than
/// the player's top speed so the magnet always catches up.
const FLIGHT_SPEED: f32 = 480.0;
/// Contact distance at which a material is collected into the wallet.
const COLLECT_RADIUS: f32 = 16.0;

/// Plugin for the materials economy.
pub struct EconomyPlugin;

impl Plugin for EconomyPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Materials>()
            .add_message::<PickupSfx>()
            .add_message::<MaterialPickedUp>()
            .add_systems(OnEnter(GameState::StartingWeaponChoice), reset_materials)
            .add_systems(OnExit(GameState::InGame), vacuum_remaining_materials)
            .add_systems(
                Update,
                (
                    drop_on_enemy_death,
                    (
                        attract_materials,
                        fly_attracted_materials,
                        pick_up_materials,
                    )
                        .chain(),
                )
                    .run_if(in_state(GameState::InGame)),
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

/// Materials entering the player's attraction radius start flying to them.
/// The boostable radius lives on `PlayerStats`.
fn attract_materials(
    mut commands: Commands,
    players: Query<(&Transform, &PlayerStats), With<Player>>,
    materials: Query<(Entity, &Transform), (With<Material>, Without<Attracted>, Without<Player>)>,
) {
    let Ok((player_pos, radius)) = players
        .single()
        .map(|(t, s)| (t.translation.truncate(), s.attraction_radius))
    else {
        return;
    };

    for (entity, transform) in &materials {
        let dist = player_pos.distance(transform.translation.truncate());
        if dist <= radius {
            commands.entity(entity).insert(Attracted);
        }
    }
}

/// An attracted material flies toward the player until collected.
fn fly_attracted_materials(
    time: Res<Time>,
    players: Query<&Transform, With<Player>>,
    mut materials: Query<&mut Transform, (With<Material>, With<Attracted>, Without<Player>)>,
) {
    let Ok(player_pos) = players.single().map(|t| t.translation.truncate()) else {
        return;
    };

    for mut transform in &mut materials {
        let to_player = player_pos - transform.translation.truncate();
        let dist = to_player.length();
        // Close enough for the pickup system; never step past the player.
        if dist <= COLLECT_RADIUS {
            continue;
        }
        let step = (FLIGHT_SPEED * time.delta_secs()).min(dist);
        let movement = to_player.normalize_or_zero() * step;
        transform.translation += movement.extend(0.0);
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
        if dist > COLLECT_RADIUS {
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

/// Wave end: every material still on the field is collected automatically so
/// nothing is lost when the wave closes (runs on any InGame exit).
fn vacuum_remaining_materials(
    mut commands: Commands,
    mut wallet: ResMut<Materials>,
    mut pickup_writer: MessageWriter<PickupSfx>,
    mut collected_writer: MessageWriter<MaterialPickedUp>,
    materials: Query<(Entity, &Material)>,
) {
    for (entity, material) in &materials {
        wallet.count += material.value;
        pickup_writer.write(PickupSfx);
        collected_writer.write(MaterialPickedUp {
            amount: material.value,
        });
        commands.entity(entity).despawn();
    }
}
