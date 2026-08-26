//! unnamed-game — a Brotato-like horde-survival roguelike in Bevy 0.17.

mod game;

use bevy::prelude::*;

use crate::game::GamePlugin;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(GamePlugin)
        .add_systems(Startup, setup)
        .run();
}

fn setup(mut commands: Commands) {
    // A 2D camera is required for sprite rendering; add it once at boot.
    commands.spawn(Camera2d);
}
