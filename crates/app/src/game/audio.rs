//! Audio: plays the mmx-generated SFX (hit / pickup / hurt) on game messages.
//!
//! Per CONTEXT.md and ADR-0002, SFX are generated once via mmx TTS, converted
//! to ogg, and committed under `assets/audio/sfx/`. The messages are defined
//! by the simulation (game-core) and merely consumed here — presentation
//! reacting to sim events (ADR-0004).

use bevy::audio::{AudioPlayer, PlaybackSettings};
use bevy::prelude::*;

use super::settings::SettingsStore;

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

fn play_hit(
    mut commands: Commands,
    sfx: Res<Sfx>,
    store: Res<SettingsStore>,
    mut messages: MessageReader<HitSfx>,
) {
    if !messages.read().next().is_some() {
        return;
    }
    // 静音: no sound work at all — don't even spawn the player. Muting must
    // mean zero audio objects, not silent ones (settings.rs).
    if store.settings.sfx_muted {
        return;
    }
    // DESPAWN (not ONCE): `AudioPlayer` entities are otherwise never removed,
    // so one entity would leak per hit/pickup/hurt event. Under constant
    // combat that unbounded growth tanks the frame rate late game.
    commands.spawn((AudioPlayer(sfx.hit.clone()), PlaybackSettings::DESPAWN));
}

fn play_pickup(
    mut commands: Commands,
    sfx: Res<Sfx>,
    store: Res<SettingsStore>,
    mut messages: MessageReader<PickupSfx>,
) {
    if !messages.read().next().is_some() {
        return;
    }
    if store.settings.sfx_muted {
        return;
    }
    commands.spawn((AudioPlayer(sfx.pickup.clone()), PlaybackSettings::DESPAWN));
}

fn play_hurt(
    mut commands: Commands,
    sfx: Res<Sfx>,
    store: Res<SettingsStore>,
    mut messages: MessageReader<HurtSfx>,
) {
    if !messages.read().next().is_some() {
        return;
    }
    if store.settings.sfx_muted {
        return;
    }
    commands.spawn((AudioPlayer(sfx.hurt.clone()), PlaybackSettings::DESPAWN));
}

#[cfg(test)]
mod tests {
    use bevy::audio::{AudioPlayer, PlaybackMode, PlaybackSettings};
    use bevy::ecs::message::Messages;
    use bevy::prelude::*;

    use game_core::combat::HitSfx;

    use super::{play_hit, Sfx};
    use crate::game::settings::{Settings, SettingsStore};

    /// Regression guard for the unbounded-entity leak: every SFX event spawned
    /// an `AudioPlayer` with `PlaybackSettings::ONCE`, which Bevy never
    /// despawns, so one entity leaked per hit/pickup/hurt and the entity count
    /// grew without bound (late-game FPS fell below 40). The spawned entity must
    /// instead use `PlaybackMode::Despawn`, which removes it when playback ends.
    #[test]
    fn sfx_entity_is_marked_for_despawn_not_leaked() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_message::<HitSfx>();
        app.insert_resource(Sfx {
            hit: Handle::default(),
            pickup: Handle::default(),
            hurt: Handle::default(),
        });
        app.insert_resource(SettingsStore::unpersisted(Settings {
            sfx_muted: false,
            ..Default::default()
        }));
        // Mount just the hit player (the other two are identical in shape).
        app.add_systems(Update, play_hit);

        // A single hit event must spawn exactly one AudioPlayer entity...
        app.world_mut()
            .resource_mut::<Messages<HitSfx>>()
            .write(HitSfx);
        app.update();

        let spawned_count = app
            .world_mut()
            .query_filtered::<&PlaybackSettings, ()>()
            .iter(app.world())
            .count();
        assert_eq!(spawned_count, 1, "one hit should spawn one AudioPlayer");

        // ...and that entity must be configured to DESPAWN on playback finish,
        // otherwise it leaks forever.
        let uses_despawn = app
            .world_mut()
            .query_filtered::<&PlaybackSettings, ()>()
            .iter(app.world())
            .any(|s| matches!(s.mode, PlaybackMode::Despawn));
        assert!(
            uses_despawn,
            "AudioPlayer must use PlaybackMode::Despawn or it leaks one entity per SFX event"
        );

        // No new AudioPlayer should spawn on a frame where no SFX fired, and the
        // one we spawned must still be present (its playback simply hasn't
        // finished in this headless harness).
        app.update();
        assert_eq!(
            app.world_mut()
                .query_filtered::<&AudioPlayer, ()>()
                .iter(app.world())
                .count(),
            1,
            "no new AudioPlayer should spawn without a fresh SFX event"
        );
    }

    /// 静音 must suppress audio objects entirely — not spawn silent ones. A
    /// muted run spawns zero `AudioPlayer` entities per SFX event, so a long
    /// muted run creates no entity churn at all.
    #[test]
    fn muted_run_spawns_no_audio_players() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_message::<HitSfx>();
        app.insert_resource(Sfx {
            hit: Handle::default(),
            pickup: Handle::default(),
            hurt: Handle::default(),
        });
        app.insert_resource(SettingsStore::unpersisted(Settings {
            sfx_muted: true,
            ..Default::default()
        }));
        app.add_systems(Update, play_hit);

        // Fire a hit while muted: no AudioPlayer may be spawned.
        app.world_mut()
            .resource_mut::<Messages<HitSfx>>()
            .write(HitSfx);
        app.update();
        assert_eq!(
            app.world_mut()
                .query_filtered::<&AudioPlayer, ()>()
                .iter(app.world())
                .count(),
            0,
            "muted: a hit must spawn no AudioPlayer at all"
        );

        // And a second frame without a fresh event still leaves the count at 0.
        app.update();
        assert_eq!(
            app.world_mut()
                .query_filtered::<&AudioPlayer, ()>()
                .iter(app.world())
                .count(),
            0,
            "muted: no AudioPlayer should linger across frames"
        );
    }
}
