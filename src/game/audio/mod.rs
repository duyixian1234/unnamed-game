//! Audio: plays the mmx-generated SFX (hit / pickup / hurt) on game events.
//!
//! Per CONTEXT.md and ADR-0002, SFX are generated once via mmx TTS, converted
//! to ogg, and committed under `assets/audio/sfx/`. BGM is deferred.

use bevy::audio::{AudioPlayer, PlaybackSettings};
use bevy::ecs::message::Message;
use bevy::prelude::*;

use crate::game::GameState;

/// Fired when a weapon lands a hit on an enemy.
#[derive(Message)]
pub struct HitSfx;

/// Fired when the player picks up a material.
#[derive(Message)]
pub struct PickupSfx;

/// Fired when the player takes damage.
#[derive(Message)]
pub struct HurtSfx;

/// Holds the loaded SFX handles so they stay alive and can be played on demand.
#[derive(Resource)]
pub struct Sfx {
    pub hit: Handle<AudioSource>,
    pub pickup: Handle<AudioSource>,
    pub hurt: Handle<AudioSource>,
}

/// Plugin for playing SFX on the relevant game events.
pub struct SfxPlugin;

impl Plugin for SfxPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<HitSfx>()
            .add_message::<PickupSfx>()
            .add_message::<HurtSfx>()
            .add_systems(Startup, load_sfx)
            .add_systems(
                Update,
                (
                    play_hit,
                    play_pickup,
                    play_hurt,
                )
                    .run_if(in_state(GameState::InGame)),
            );
    }
}

fn load_sfx(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.insert_resource(Sfx {
        hit: asset_server.load("audio/sfx/hit.ogg"),
        pickup: asset_server.load("audio/sfx/pickup.ogg"),
        hurt: asset_server.load("audio/sfx/hurt.ogg"),
    });
}

fn play_hit(mut commands: Commands, sfx: Res<Sfx>, mut messages: MessageReader<HitSfx>) {
    if !messages.read().next().is_some() {
        return;
    }
    commands.spawn((AudioPlayer(sfx.hit.clone()), PlaybackSettings::ONCE));
}

fn play_pickup(mut commands: Commands, sfx: Res<Sfx>, mut messages: MessageReader<PickupSfx>) {
    if !messages.read().next().is_some() {
        return;
    }
    commands.spawn((AudioPlayer(sfx.pickup.clone()), PlaybackSettings::ONCE));
}

fn play_hurt(mut commands: Commands, sfx: Res<Sfx>, mut messages: MessageReader<HurtSfx>) {
    if !messages.read().next().is_some() {
        return;
    }
    commands.spawn((AudioPlayer(sfx.hurt.clone()), PlaybackSettings::ONCE));
}
