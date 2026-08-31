//! 诊断叠层 (Diagnostics Overlay): an app-layer performance readout, hidden by
//! default (see CONTEXT.md glossary).
//!
//! This module draws and refreshes the overlay but does not decide whether it
//! is shown: `OverlayVisible` is written by `settings`, which persists it and
//! owns the hotkey. Here it is read-only.
//!
//! Deliberately not built on `FrameTimeDiagnosticsPlugin`: it publishes only
//! smoothed averages for fps / frame_time / frame_count, and the point of this
//! overlay is the frame spikes those averages smooth away. Recovering the
//! worst frame from it would mean reading its raw history — the same rolling
//! window written below, minus the entity counts this overlay also reports.

use std::collections::VecDeque;

use bevy::ecs::entity::Entities;
use bevy::prelude::*;

use game_core::enemy::Enemy;

use super::settings::SettingsStore;
use super::ui::ui_font;

/// Length of the rolling statistics window, in seconds.
const WINDOW_SECS: f32 = 2.0;

/// How often the readout text is rebuilt, in seconds. Rebuilding text every
/// frame would make the overlay a source of the stutter it is measuring.
const REFRESH_SECS: f32 = 0.25;

/// Rolling frame timings plus the text refresh timer.
#[derive(Resource, Default)]
struct FrameTiming {
    window: FrameWindow,
    since_refresh: f32,
}

impl FrameTiming {
    /// Advance the refresh timer by one frame, reporting whether the readout
    /// is due for a rebuild (and resetting the timer when it is).
    fn tick(&mut self, delta: f32) -> bool {
        self.since_refresh += delta;
        if self.since_refresh < REFRESH_SECS {
            return false;
        }
        self.since_refresh = 0.0;
        true
    }

    /// Make the next [`Self::tick`] rebuild the readout immediately, so the
    /// numbers mean something the moment the overlay appears.
    fn refresh_now(&mut self) {
        self.since_refresh = REFRESH_SECS;
    }
}

/// Root node of the overlay. Deliberately not a `ScreenRoot`: it must survive
/// state transitions instead of being cleaned up with a screen, since the
/// wave-to-shop transition spikes are part of what it measures.
#[derive(Component)]
struct OverlayRoot;

/// The readout text.
#[derive(Component)]
struct OverlayText;

/// Plugin for the diagnostics overlay.
pub struct DiagnosticsOverlayPlugin;

impl Plugin for DiagnosticsOverlayPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<FrameTiming>()
            .add_systems(Startup, spawn_overlay)
            .add_systems(Update, (sync_overlay_visibility, sample_frames).chain())
            // The readout is rebuilt in `Last`, not `Update`, so the entity
            // counts reflect this frame's despawn commands — a wave
            // transition is exactly when hundreds of enemies are despawned at
            // once, and that is one of the spikes this overlay exists to see.
            .add_systems(Last, refresh_overlay_text.run_if(overlay_is_visible));
    }
}

fn spawn_overlay(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    settings: Res<SettingsStore>,
) {
    commands
        .spawn((
            OverlayRoot,
            // Above every other UI root: `ZIndex` only orders siblings, so a
            // separate root needs the global variant.
            GlobalZIndex(1),
            // Starts from the persisted setting, so a restored `true` is
            // visible immediately rather than after the first keypress.
            visibility_for(settings.settings.overlay),
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(16.0),
                right: Val::Px(16.0),
                padding: UiRect::all(Val::Px(8.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.55)),
        ))
        .with_child((
            OverlayText,
            Text::new(""),
            ui_font(&asset_server, 16.0),
            TextColor(Color::srgb(0.75, 0.95, 0.75)),
        ));
}

/// Mirror the persisted overlay setting onto the root node.
///
/// Compares against what is already applied rather than watching change
/// ticks. The setting is written from another plugin, and catching a
/// one-frame change tick by schedule order alone is fragile — if this system
/// ever ran first, the toggle would be dropped for good.
fn sync_overlay_visibility(
    settings: Res<SettingsStore>,
    mut timing: ResMut<FrameTiming>,
    mut roots: Query<&mut Visibility, With<OverlayRoot>>,
) {
    let wanted = visibility_for(settings.settings.overlay);
    for mut visibility in &mut roots {
        if *visibility == wanted {
            continue;
        }
        *visibility = wanted;
        // Refresh immediately rather than up to REFRESH_SECS later, so the
        // numbers mean something the moment the overlay appears.
        timing.refresh_now();
    }
}

fn visibility_for(visible: bool) -> Visibility {
    if visible {
        Visibility::Visible
    } else {
        Visibility::Hidden
    }
}

fn sample_frames(time: Res<Time<Real>>, mut timing: ResMut<FrameTiming>) {
    timing.window.push(time.elapsed_secs(), time.delta_secs());
}

fn refresh_overlay_text(
    time: Res<Time<Real>>,
    mut timing: ResMut<FrameTiming>,
    enemies: Query<&Enemy>,
    entities: &Entities,
    mut texts: Query<&mut Text, With<OverlayText>>,
) {
    if !timing.tick(time.delta_secs()) {
        return;
    }

    let text = readout_text(timing.window.stats(), enemies.iter().len(), entities.len());
    for mut target in &mut texts {
        target.0 = text.clone();
    }
}

/// Run condition: only refresh the readout while the overlay is meant to be
/// up. Reads the setting directly rather than a mirrored resource, so there
/// is no second copy to fall out of step.
fn overlay_is_visible(settings: Res<SettingsStore>) -> bool {
    settings.settings.overlay
}

/// Rolling window of real frame deltas.
#[derive(Default)]
struct FrameWindow {
    /// `(timestamp, delta)` pairs, oldest first. Timestamps come from
    /// `Time<Real>::elapsed_secs()`, so the window is measured in real time.
    samples: VecDeque<(f32, f32)>,
}

/// The numbers derived from a [`FrameWindow`].
#[derive(Debug, PartialEq)]
struct WindowStats {
    /// Frames per second: the inverse of [`Self::avg_ms`]. Reported because
    /// FPS is the unit people read fastest, not because it adds information.
    fps: f32,
    /// Mean frame time in milliseconds.
    avg_ms: f32,
    /// Worst (longest) frame time in milliseconds.
    worst_ms: f32,
}

impl FrameWindow {
    /// Record a frame that ended at `now`, dropping samples that have aged
    /// out of the window.
    fn push(&mut self, now: f32, delta: f32) {
        self.samples.push_back((now, delta));
        while self
            .samples
            .front()
            .is_some_and(|(timestamp, _)| now - timestamp > WINDOW_SECS)
        {
            self.samples.pop_front();
        }
    }

    /// Average FPS, average frame time, and worst frame time over the window,
    /// or `None` while it is still empty.
    fn stats(&self) -> Option<WindowStats> {
        let count = self.samples.len();
        if count == 0 {
            return None;
        }
        let sum: f32 = self.samples.iter().map(|(_, delta)| delta).sum();
        let worst = self
            .samples
            .iter()
            .fold(0.0_f32, |worst, (_, delta)| worst.max(*delta));
        Some(WindowStats {
            fps: count as f32 / sum,
            avg_ms: (sum / count as f32) * 1000.0,
            worst_ms: worst * 1000.0,
        })
    }
}

fn readout_text(stats: Option<WindowStats>, enemies: usize, entities: u32) -> String {
    let (fps, avg_ms, worst_ms) = match stats {
        Some(stats) => (stats.fps, stats.avg_ms, stats.worst_ms),
        None => (0.0, 0.0, 0.0),
    };
    format!(
        "FPS {fps:.0} · 帧时间 {avg_ms:.1} ms · 最差 {worst_ms:.1} ms\n\
         敌人 {enemies} · 实体 {entities}"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Fill a window from `(timestamp, delta)` pairs.
    fn window(samples: &[(f32, f32)]) -> FrameWindow {
        let mut window = FrameWindow::default();
        for (now, delta) in samples {
            window.push(*now, *delta);
        }
        window
    }

    #[test]
    fn empty_window_has_no_stats() {
        assert_eq!(FrameWindow::default().stats(), None);
    }

    #[test]
    fn single_frame_reports_its_own_rate() {
        let stats = window(&[(0.0, 0.016)]).stats().expect("one sample");
        assert!((stats.fps - 62.5).abs() < 0.1, "fps was {}", stats.fps);
        assert!(
            (stats.avg_ms - 16.0).abs() < 0.01,
            "avg was {}",
            stats.avg_ms
        );
        assert!(
            (stats.worst_ms - 16.0).abs() < 0.01,
            "worst was {}",
            stats.worst_ms
        );
    }

    #[test]
    fn worst_frame_is_the_longest_in_the_window() {
        // One 100 ms hitch among steady 16 ms frames. The mean smears the
        // hitch across all three frames (44 ms, a reading the game never
        // actually produced); the worst frame isolates it.
        let samples = [(1.0, 0.016), (1.1, 0.100), (1.2, 0.016)];
        let stats = window(&samples).stats().expect("three samples");
        assert!(
            (stats.worst_ms - 100.0).abs() < 0.01,
            "worst was {}",
            stats.worst_ms
        );
        assert!(
            (stats.avg_ms - 44.0).abs() < 0.1,
            "avg was {}",
            stats.avg_ms
        );
        assert!(
            (stats.fps - 3.0 / 0.132).abs() < 0.1,
            "fps was {}",
            stats.fps
        );
    }

    #[test]
    fn samples_older_than_the_window_are_dropped() {
        // The 500 ms stall is 2.5 s old by the third sample, so it must age
        // out — otherwise one startup hitch would poison the readout forever.
        let stats = window(&[(0.0, 0.500), (1.0, 0.010), (2.5, 0.020)])
            .stats()
            .expect("two live samples");
        assert!(
            (stats.worst_ms - 20.0).abs() < 0.01,
            "worst was {}",
            stats.worst_ms
        );
        assert!(
            (stats.fps - 2.0 / 0.030).abs() < 0.1,
            "fps was {}",
            stats.fps
        );
    }

    #[test]
    fn refresh_fires_on_the_first_frame_past_the_budget() {
        let mut timing = FrameTiming::default();
        // Steady 100 ms frames meet the 250 ms budget on the third. The
        // cadence is quantised to frame boundaries, so it can only ever be
        // coarser than 4 Hz — never finer.
        assert!(!timing.tick(0.1));
        assert!(!timing.tick(0.1));
        assert!(timing.tick(0.1));
        // The leftover is discarded rather than carried, so the next period
        // starts fresh instead of drifting with the frame rate.
        assert!(!timing.tick(0.1));
        assert!(!timing.tick(0.1));
        assert!(timing.tick(0.1));
    }

    #[test]
    fn a_single_long_frame_does_not_batch_up_refreshes() {
        let mut timing = FrameTiming::default();
        // A 1 s stall covers four refresh periods but must trigger one.
        assert!(timing.tick(1.0));
        assert!(!timing.tick(0.0));
    }

    #[test]
    fn refresh_now_forces_the_next_tick() {
        let mut timing = FrameTiming::default();
        timing.refresh_now();
        assert!(timing.tick(0.0), "must refresh on the very next frame");
    }

    #[test]
    fn text_reports_every_metric_and_both_counts() {
        let text = readout_text(
            Some(WindowStats {
                fps: 59.6,
                avg_ms: 16.8,
                worst_ms: 33.3,
            }),
            42,
            128,
        );
        assert_eq!(
            text,
            "FPS 60 · 帧时间 16.8 ms · 最差 33.3 ms\n敌人 42 · 实体 128"
        );
    }

    #[test]
    fn text_falls_back_to_zero_before_any_frame_is_sampled() {
        assert_eq!(
            readout_text(None, 0, 0),
            "FPS 0 · 帧时间 0.0 ms · 最差 0.0 ms\n敌人 0 · 实体 0"
        );
    }
}
