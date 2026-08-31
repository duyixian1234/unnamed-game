//! unnamed-game — a Brotato-like horde-survival roguelike in Bevy 0.17.
//!
//! Presentation crate: boots DefaultPlugins, parses the run seed, and adds
//! the simulation (`game_core::CorePlugin`) plus the app-layer plugins
//! (sprites, audio, UI, camera, keyboard intent).

mod game;

use bevy::prelude::*;

use game_core::rng::Seed;
use game_core::waves::WaveConfig;

/// Marks the single 2D camera. It stays fixed so the player moves across the
/// screen (no camera-follow); the field is sized to match the visible area.
#[derive(Component)]
pub struct MainCamera;

fn main() {
    let seed = parse_seed();
    let max_waves = parse_max_waves();

    App::new()
        .add_plugins(DefaultPlugins.set(bevy::asset::AssetPlugin {
            // We commit plain assets with no .meta files. In wasm, the dev
            // server returns the SPA index.html for missing .meta paths,
            // which Bevy can't parse as meta. Disabling the meta check lets
            // assets load straight from their bytes so sprites render.
            meta_check: bevy::asset::AssetMetaCheck::Never,
            ..default()
        }))
        // Must be inserted before CorePlugin: both are picked up from the
        // world rather than defaulted (ADR-0005 for the seed).
        .insert_resource(Seed(seed))
        .insert_resource(WaveConfig {
            max_waves,
            spawning: true,
        })
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

/// Read a `--<name>=value` / `--<name> value` CLI argument, if present.
fn cli_value(name: &str) -> Option<String> {
    let args: Vec<String> = std::env::args().collect();
    let flag = format!("--{name}");
    for (i, arg) in args.iter().enumerate() {
        if let Some(rest) = arg.strip_prefix(&format!("{flag}=")) {
            return Some(rest.to_string());
        }
        if arg == &flag {
            return args.get(i + 1).cloned();
        }
    }
    None
}

/// The CLI flag, else the environment variable, else nothing.
fn configured_value(name: &str, env_var: &str) -> Option<String> {
    cli_value(name).or_else(|| std::env::var(env_var).ok())
}

/// Parse the run seed: `--seed=N` / `--seed N` CLI arg, else the `GAME_SEED`
/// env var, else a random value. The effective seed is logged at startup so
/// a session can be replayed (ADR-0005).
fn parse_seed() -> u64 {
    configured_value("seed", "GAME_SEED")
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or_else(|| {
            web_time::SystemTime::now()
                .duration_since(web_time::SystemTime::UNIX_EPOCH)
                .expect("system clock before UNIX_EPOCH")
                .as_nanos() as u64
        })
}

/// Parse the wave count: `--waves=N` / `--waves N` CLI arg, else the
/// `GAME_WAVES` env var, else `WaveConfig`'s default (CONTEXT.md: wave count
/// configurable).
///
/// Zero and unparseable values fall back to the default rather than starting a
/// Run that ends immediately. This is a launch-time knob, not a player-facing
/// setting: the wave count is a gameplay parameter, and CONTEXT.md keeps those
/// as 配置 in `game-core`, distinct from the player's 设置.
fn parse_max_waves() -> u32 {
    let default = WaveConfig::default().max_waves;
    match configured_value("waves", "GAME_WAVES").map(|value| value.parse::<u32>()) {
        None => default,
        Some(Ok(waves)) if waves > 0 => waves,
        Some(Ok(_)) => {
            warn!("a wave count of 0 is not a Run, falling back to {default}");
            default
        }
        Some(Err(error)) => {
            warn!("could not parse the wave count ({error}), falling back to {default}");
            default
        }
    }
}
