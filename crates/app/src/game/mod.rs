//! App-layer plugin: presentation systems over the simulation.

pub mod assets;
pub mod audio;
pub mod background;
pub mod keyboard;
pub mod render;
pub mod ui;

use bevy::prelude::*;

use game_core::rng::Seed;
use game_core::CorePlugin;

/// App-layer plugins: sprite assets, SFX playback, sprite attachment,
/// keyboard intent, and UI screens.
pub struct AppPlugin;

impl Plugin for AppPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            assets::AssetsPlugin,
            audio::SfxPlugin,
            background::BackgroundPlugin,
            render::RenderPlugin,
            keyboard::KeyboardIntentPlugin,
            ui::UIPlugin,
        ));

        // Log the effective seed so a session can be replayed (ADR-0005).
        let seed = app.world().resource::<Seed>().0;
        info!(
            "Run seed: {} (replay with --seed {} or GAME_SEED={})",
            seed, seed, seed
        );

        // Simulation last, so app systems can order against core systems if
        // ever needed; core is otherwise self-contained.
        app.add_plugins(CorePlugin);
    }
}
