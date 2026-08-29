//! unnamed-game — a Brotato-like horde-survival roguelike in Bevy 0.17.
//!
//! Presentation crate: boots DefaultPlugins, parses the run seed, and adds
//! the simulation (`game_core::CorePlugin`) plus the app-layer plugins
//! (sprites, audio, UI, camera, keyboard intent).

mod game;

use bevy::prelude::*;

use game_core::rng::Seed;

/// Marks the single 2D camera. It stays fixed so the player moves across the
/// screen (no camera-follow); the field is sized to match the visible area.
#[derive(Component)]
pub struct MainCamera;

fn main() {
    let seed = parse_seed();

    App::new()
        .add_plugins(
            DefaultPlugins.set(bevy::asset::AssetPlugin {
                // We commit plain assets with no .meta files. In wasm, the dev
                // server returns the SPA index.html for missing .meta paths,
                // which Bevy can't parse as meta. Disabling the meta check lets
                // assets load straight from their bytes so sprites render.
                meta_check: bevy::asset::AssetMetaCheck::Never,
                ..default()
            }),
        )
        // Must be inserted before CorePlugin: init_rng picks up an existing
        // Seed instead of generating a random one (ADR-0005).
        .insert_resource(Seed(seed))
        .add_plugins(game::AppPlugin)
        .add_systems(Startup, (setup, zoom_camera.after(setup)))
        .run();
}

fn setup(mut commands: Commands) {
    commands.spawn((MainCamera, Camera2d));
}

/// Zoom the camera so characters read at a good size. The field
/// (see FIELD_HALF_WIDTH/HEIGHT) is sized to match the visible area, so the
/// player moves across a fixed screen without scrolling. Smaller `scale` =
/// more zoomed in.
fn zoom_camera(mut cameras: Query<&mut Projection, With<MainCamera>>) {
    for mut projection in &mut cameras {
        if let Projection::Orthographic(ortho) = &mut *projection {
            ortho.scale = 0.7;
        }
    }
}

/// Parse the run seed: `--seed=N` / `--seed N` CLI arg, else the `GAME_SEED`
/// env var, else a random value. The effective seed is logged at startup so
/// a session can be replayed (ADR-0005).
fn parse_seed() -> u64 {
    let args: Vec<String> = std::env::args().collect();
    let mut parsed: Option<u64> = None;
    for (i, arg) in args.iter().enumerate() {
        let value: Option<&str> = if let Some(rest) = arg.strip_prefix("--seed=") {
            Some(rest)
        } else if arg == "--seed" {
            args.get(i + 1).map(|s| s.as_str())
        } else {
            None
        };
        if let Some(value) = value {
            parsed = value.parse::<u64>().ok();
            if parsed.is_some() {
                break;
            }
        }
    }
    if parsed.is_none() {
        if let Ok(value) = std::env::var("GAME_SEED") {
            parsed = value.parse::<u64>().ok();
        }
    }
    parsed.unwrap_or_else(|| {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock before UNIX_EPOCH")
            .as_nanos() as u64
    })
}
