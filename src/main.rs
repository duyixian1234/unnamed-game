//! unnamed-game — a Brotato-like horde-survival roguelike in Bevy 0.17.

mod game;

use bevy::asset::AssetMetaCheck;
use bevy::prelude::*;

use crate::game::GamePlugin;

/// Marks the single 2D camera that follows the player during play.
#[derive(Component)]
pub struct MainCamera;

fn main() {
    App::new()
        .add_plugins(
            DefaultPlugins.set(AssetPlugin {
                // We commit plain assets with no .meta files. In wasm, the dev
                // server returns the SPA index.html for missing .meta paths,
                // which Bevy can't parse as meta. Disabling the meta check lets
                // assets load straight from their bytes so sprites render.
                meta_check: AssetMetaCheck::Never,
                ..default()
            }),
        )
        .add_plugins(GamePlugin)
        .add_systems(Startup, (setup, zoom_camera.after(setup)))
        .run();
}

fn setup(mut commands: Commands) {
    commands.spawn((MainCamera, Camera2d));
}

/// Zoom the main camera so the player and enemies render at a good size.
/// Smaller `scale` = more zoomed in (0.62 keeps characters readable while still
/// showing enough of the field). Runs after `setup` so the camera exists.
fn zoom_camera(mut cameras: Query<&mut Projection, With<MainCamera>>) {
    for mut projection in &mut cameras {
        if let Projection::Orthographic(ortho) = &mut *projection {
            ortho.scale = 0.62;
        }
    }
}
