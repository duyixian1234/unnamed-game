//! Audio: plays the mmx-generated SFX (hit / pickup / hurt) on game messages.
//!
//! Per CONTEXT.md and ADR-0002, SFX are generated once via mmx TTS, converted
//! to ogg, and committed under `assets/audio/sfx/`. The messages are defined
//! by the simulation (game-core) and merely consumed here — presentation
//! reacting to sim events (ADR-0004).

use bevy::audio::{AudioPlayer, PlaybackSettings};
use bevy::prelude::*;

use game_core::combat::{HitSfx, HurtSfx};
use game_core::economy::PickupSfx;

/// Holds the loaded SFX handles so they stay alive and can be played on demand.
#[derive(Resource)]
pub struct Sfx {
    pub hit: Handle<AudioSource>,
    pub pickup: Handle<AudioSource>,
    pub hurt: Handle<AudioSource>,
}

/// Plugin for playing SFX on the relevant game messages.
pub struct SfxPlugin;

impl Plugin for SfxPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, load_sfx).add_systems(
            Update,
            (play_hit, play_pickup, play_hurt).run_if(in_state(game_core::GameState::InGame)),
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
