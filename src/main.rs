//! unnamed-game — a Brotato-like horde-survival roguelike in Bevy 0.17.
//!
//! T1: project skeleton. A plain Bevy app that boots and opens an empty window
//! on both native (`cargo run`) and wasm (`trunk serve`) targets.

use bevy::prelude::*;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_systems(Startup, setup)
        .run();
}

fn setup(mut commands: Commands) {
    // A 2D camera is required for any sprite rendering later; add it now so the
    // window has a render target and later milestones can drop sprites in.
    commands.spawn(Camera2d);
}
