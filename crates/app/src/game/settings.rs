//! 设置 (Settings): the player's persisted preferences, and the hotkeys that
//! flip them during a Run.
//!
//! Presentation-layer only. `GameState` is deliberately untouched (ADR-0004):
//! the settings screen is spawned and despawned imperatively rather than being
//! driven by a state transition.
//!
//! 暂停 (Pause) is NOT here, despite looking related. Pausing stops the wave
//! timer and the RNG draws, which makes it a simulation concern belonging to
//! `game-core` — the opposite side of the ADR-0004 boundary (CONTEXT.md).

use bevy::audio::{GlobalVolume, Volume};
use bevy::input::keyboard::KeyCode;
use bevy::input::ButtonInput;
use bevy::prelude::*;
use bevy_persistent::{Persistent, StorageFormat};
use serde::{Deserialize, Serialize};

use super::ui::ui_font;

/// Storage path. On wasm this MUST start with `local` or `session`: the crate
/// derives a localStorage key from it and panics otherwise (ADR 0011).
#[cfg(target_family = "wasm")]
const SETTINGS_PATH: &str = "/local/settings.json";

/// Native keeps it under `target/`, which is already gitignored, so the dev
/// loop never litters the repo.
#[cfg(not(target_family = "wasm"))]
const SETTINGS_PATH: &str = "target/settings.json";

/// How far the `−` / `+` buttons move the volume, in percent.
pub const VOLUME_STEP_PERCENT: u32 = 10;

/// How long a mute toast stays on screen, in seconds.
pub const TOAST_SECS: f32 = 1.5;

/// The persisted preferences (CONTEXT.md: 设置).
///
/// Fullscreen is deliberately absent: browsers require a user gesture to
/// enter it, so restoring a persisted `true` at startup would silently fail
/// and leave the panel displaying a state the screen is not in (ADR 0011).
#[derive(Resource, Serialize, Deserialize, Clone, Debug)]
pub struct Settings {
    /// Independent of `sfx_volume_percent` (CONTEXT.md: 静音). Muting
    /// silences without altering the stored volume, so unmuting restores it.
    pub sfx_muted: bool,
    /// 0–100, moved in steps of [`VOLUME_STEP_PERCENT`].
    pub sfx_volume_percent: u32,
    /// Whether the 诊断叠层 is shown.
    pub overlay: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            sfx_muted: false,
            // Not 100: three SFX at full volume are harsh on headphones.
            sfx_volume_percent: 80,
            overlay: false,
        }
    }
}

/// The SFX volume actually applied, in Bevy's linear 0–1 scale.
///
/// Muting silences without touching the stored value. This is the single
/// easiest rule in the module to break by accident, which is why it is a pure
/// function rather than something the buttons compute inline.
pub fn effective_volume(muted: bool, volume: f32) -> f32 {
    if muted {
        0.0
    } else {
        volume.clamp(0.0, 1.0)
    }
}

/// The settings, plus the handle used to persist them.
///
/// `persistent` is `None` when storage is unavailable — private browsing, a
/// read-only filesystem. Changes then stay in memory for the session instead
/// of taking the game down with them.
#[derive(Resource)]
pub struct SettingsStore {
    pub settings: Settings,
    persistent: Option<Persistent<Settings>>,
}

impl SettingsStore {
    /// Apply a change and save it immediately.
    ///
    /// Goes through `Persistent::set` rather than `DerefMut`, because mutating
    /// through `DerefMut` silently does not save (ADR 0011).
    pub fn change(&mut self, change: impl FnOnce(&mut Settings)) {
        change(&mut self.settings);
        let Some(persistent) = self.persistent.as_mut() else {
            return;
        };
        if let Err(error) = persistent.set(self.settings.clone()) {
            warn!("settings could not be saved, keeping the change in memory: {error}");
        }
    }

    /// The volume actually in effect, in Bevy's linear 0–1 scale.
    pub fn applied_volume(&self) -> f32 {
        effective_volume(
            self.settings.sfx_muted,
            self.settings.sfx_volume_percent as f32 / 100.0,
        )
    }

    /// Flip 静音 and report the new state.
    ///
    /// The stored volume is deliberately left alone: it is the value unmuting
    /// has to restore (CONTEXT.md). Both the M hotkey and the panel's button
    /// go through here so neither can diverge from that rule.
    pub fn toggle_mute(&mut self) -> bool {
        let mut muted = false;
        self.change(|settings| {
            settings.sfx_muted = !settings.sfx_muted;
            muted = settings.sfx_muted;
        });
        muted
    }

    /// Move the volume by a signed number of percentage points.
    ///
    /// The clamp lives here rather than in the view so no caller can push the
    /// value out of its 0–100 range.
    pub fn step_volume(&mut self, delta_percent: i32) {
        self.change(|settings| {
            let percent = settings.sfx_volume_percent as i32 + delta_percent;
            settings.sfx_volume_percent = percent.clamp(0, 100) as u32;
        });
    }
}

/// Plugin owning the settings resource, its hotkeys, and its side effects.
pub struct SettingsPlugin;

impl Plugin for SettingsPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(load_settings()).add_systems(
            Update,
            (mute_hotkey, overlay_hotkey, apply_sfx_volume, tick_toasts),
        );
    }
}

/// Load the settings, falling back to defaults when storage is unusable.
///
/// A corrupt file is handled inside the crate: `revert_to_default_on_
/// deserialization_errors` rewrites the defaults and returns normally. Only a
/// genuine IO failure reaches here, and it is not fatal — settings are not a
/// save file, so we warn and play on.
fn load_settings() -> SettingsStore {
    match Persistent::<Settings>::builder()
        .name("settings")
        .format(StorageFormat::Json)
        .path(SETTINGS_PATH)
        .default(Settings::default())
        // These two must be set together: the crate panics if
        // `revert_to_default_on_deserialization_errors` is set on a
        // non-revertible resource.
        .revertible(true)
        .revert_to_default_on_deserialization_errors(true)
        .build()
    {
        Ok(persistent) => SettingsStore {
            settings: persistent.get().clone(),
            persistent: Some(persistent),
        },
        Err(error) => {
            warn!("settings could not be loaded from {SETTINGS_PATH}, using defaults for this session: {error}");
            SettingsStore {
                settings: Settings::default(),
                persistent: None,
            }
        }
    }
}

fn apply_sfx_volume(store: Res<SettingsStore>, mut global: ResMut<GlobalVolume>) {
    let applied = Volume::Linear(store.applied_volume());
    if global.volume != applied {
        global.volume = applied;
    }
}

/// M mutes without opening the panel — the settings screen is only reachable
/// from the main menu, and a Run lasts twenty waves (CONTEXT.md).
fn mute_hotkey(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut store: ResMut<SettingsStore>,
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    toasts: Query<Entity, With<ToastRoot>>,
) {
    if !keyboard.just_pressed(KeyCode::KeyM) {
        return;
    }
    // Replace any toast still on screen rather than stacking them.
    for entity in &toasts {
        commands.entity(entity).despawn();
    }
    let muted = store.toggle_mute();
    spawn_toast(&mut commands, &asset_server, muted);
}

/// Backquote toggles the overlay. Not F3: browsers reserve it for find-again,
/// and it is not always preventable from a page.
fn overlay_hotkey(keyboard: Res<ButtonInput<KeyCode>>, mut store: ResMut<SettingsStore>) {
    if !keyboard.just_pressed(KeyCode::Backquote) {
        return;
    }
    store.change(|settings| settings.overlay = !settings.overlay);
}

/// Root of the mute toast. Not a `ScreenRoot`: it outlives any single screen
/// and must survive state transitions.
#[derive(Component)]
struct ToastRoot;

#[derive(Component)]
struct ToastTimer(Timer);

fn spawn_toast(commands: &mut Commands, asset_server: &AssetServer, muted: bool) {
    commands
        .spawn((
            ToastRoot,
            ToastTimer(Timer::from_seconds(TOAST_SECS, TimerMode::Once)),
            GlobalZIndex(2),
            // A plain `Node` carries neither `Button` nor `Interaction`, so it
            // defaults to FocusPolicy::Pass — this full-width strip cannot
            // swallow clicks meant for the screen underneath.
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(80.0),
                left: Val::Px(0.0),
                right: Val::Px(0.0),
                justify_content: JustifyContent::Center,
                ..default()
            },
        ))
        .with_child((
            Text::new(if muted {
                "已静音"
            } else {
                "已取消静音"
            }),
            ui_font(asset_server, 24.0),
            TextColor(Color::WHITE),
            Node {
                padding: UiRect::axes(Val::Px(20.0), Val::Px(10.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.7)),
        ));
}

fn tick_toasts(
    mut commands: Commands,
    time: Res<Time>,
    mut toasts: Query<(Entity, &mut ToastTimer)>,
) {
    for (entity, mut timer) in &mut toasts {
        if timer.0.tick(time.delta()).is_finished() {
            commands.entity(entity).despawn();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A store with no storage behind it, so the invariants can be exercised
    /// without touching the filesystem.
    fn unpersisted(settings: Settings) -> SettingsStore {
        SettingsStore {
            settings,
            persistent: None,
        }
    }

    #[test]
    fn muting_silences() {
        assert_eq!(effective_volume(true, 0.8), 0.0);
        assert_eq!(effective_volume(true, 0.0), 0.0);
    }

    #[test]
    fn volume_is_clamped_to_the_linear_range() {
        assert_eq!(effective_volume(false, 1.5), 1.0);
        assert_eq!(effective_volume(false, -0.2), 0.0);
        assert_eq!(effective_volume(false, 0.42), 0.42);
    }

    #[test]
    fn toggling_mute_preserves_the_volume_it_must_restore() {
        let mut store = unpersisted(Settings {
            sfx_volume_percent: 80,
            ..Default::default()
        });

        assert!(store.toggle_mute());
        assert_eq!(
            store.settings.sfx_volume_percent, 80,
            "muting must not zero the stored volume — there would be nothing to restore"
        );
        assert_eq!(store.applied_volume(), 0.0);

        assert!(!store.toggle_mute());
        assert_eq!(store.applied_volume(), 0.8);
    }

    #[test]
    fn a_step_moves_the_volume_by_ten_percent() {
        let mut store = unpersisted(Settings::default());
        store.step_volume(VOLUME_STEP_PERCENT as i32);
        assert_eq!(store.settings.sfx_volume_percent, 90);
        store.step_volume(-(VOLUME_STEP_PERCENT as i32));
        assert_eq!(store.settings.sfx_volume_percent, 80);
    }

    #[test]
    fn volume_steps_clamp_at_both_ends() {
        let mut store = unpersisted(Settings::default());
        store.step_volume(-1000);
        assert_eq!(store.settings.sfx_volume_percent, 0);
        store.step_volume(1000);
        assert_eq!(store.settings.sfx_volume_percent, 100);
    }

    #[test]
    fn defaults_are_playable_and_the_overlay_starts_hidden() {
        let settings = Settings::default();
        assert!(!settings.sfx_muted);
        assert_eq!(settings.sfx_volume_percent, 80);
        assert!(!settings.overlay);
    }
}
