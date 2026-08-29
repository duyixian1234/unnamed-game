//! Procedural background: a per-wave FBM noise texture plus a subtle
//! field-boundary line.
//!
//! The texture is generated at runtime in this presentation layer (purely
//! cosmetic, ADR-0004). Its variant is derived deterministically from
//! `hash(RunSeed, wave)` with a local integer hash — identical Seeds produce
//! identical backgrounds, and the sim's `GlobalRng` (ADR-0005) is untouched.

use bevy::asset::RenderAssetUsages;
use bevy::image::Image;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

use game_core::rng::Seed;
use game_core::waves::{WaveStarted, FIELD_HALF_HEIGHT, FIELD_HALF_WIDTH};

/// Noise texture resolution (tiled once across the viewport, so effectively
/// full-screen; generated once per wave).
const TEXTURE_SIZE: u32 = 512;
/// Lattice cells of the coarsest octave (wraps, so the tile is seamless).
const BASE_CELLS: u32 = 8;
const OCTAVES: u32 = 4;

/// Base background color: dark blue-grey `#1a2030`.
const BASE_RGB: [f32; 3] = [26.0 / 255.0, 32.0 / 255.0, 48.0 / 255.0];

/// Visible world extent at the fixed camera scale 0.7 and 1280x720 window
/// (1280/0.7 x 720/0.7, rounded up to overfill by half a unit).
const VIEW_WORLD_W: f32 = 1829.0;
const VIEW_WORLD_H: f32 = 1029.0;

/// Marker for the background sprite (the only entity whose image we swap).
#[derive(Component)]
struct BackgroundSprite;

/// Plugin for the procedural background and field-boundary line.
pub struct BackgroundPlugin;

impl Plugin for BackgroundPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_background)
            .add_systems(Update, regenerate_on_wave);
    }
}

fn spawn_background(mut commands: Commands, mut images: ResMut<Assets<Image>>, seed: Res<Seed>) {
    let handle = images.add(generate_background_image(wave_seed(seed.0, 0)));
    commands.spawn((
        BackgroundSprite,
        Sprite {
            image: handle,
            custom_size: Some(Vec2::new(VIEW_WORLD_W, VIEW_WORLD_H)),
            ..default()
        },
        Transform::from_xyz(0.0, 0.0, -100.0),
    ));

    // Subtle field-boundary outline (1px-ish at the fixed camera zoom).
    let line_color = Color::srgba(0.7, 0.8, 1.0, 0.14);
    let thickness = 2.0;
    let width = FIELD_HALF_WIDTH * 2.0;
    let height = FIELD_HALF_HEIGHT * 2.0;
    for (position, size) in [
        (
            Vec3::new(0.0, FIELD_HALF_HEIGHT, -99.0),
            Vec2::new(width, thickness),
        ),
        (
            Vec3::new(0.0, -FIELD_HALF_HEIGHT, -99.0),
            Vec2::new(width, thickness),
        ),
        (
            Vec3::new(FIELD_HALF_WIDTH, 0.0, -99.0),
            Vec2::new(thickness, height),
        ),
        (
            Vec3::new(-FIELD_HALF_WIDTH, 0.0, -99.0),
            Vec2::new(thickness, height),
        ),
    ] {
        commands.spawn((
            Sprite::from_color(line_color, size),
            Transform::from_translation(position),
        ));
    }
}

/// Swap in a fresh noise texture each time a wave starts.
fn regenerate_on_wave(
    mut images: ResMut<Assets<Image>>,
    seed: Res<Seed>,
    backgrounds: Query<&Sprite, With<BackgroundSprite>>,
    mut wave_starts: MessageReader<WaveStarted>,
) {
    for wave in wave_starts.read() {
        let Ok(sprite) = backgrounds.single() else {
            continue;
        };
        if let Some(image) = images.get_mut(&sprite.image) {
            *image = generate_background_image(wave_seed(seed.0, wave.number));
        }
    }
}

/// Deterministic per-wave variant seed: hash(RunSeed, wave).
fn wave_seed(run_seed: u64, wave: u32) -> u64 {
    splitmix64(run_seed ^ (wave as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15))
}

/// Generate the tiling FBM value-noise texture for one variant seed.
fn generate_background_image(seed: u64) -> Image {
    let size = TEXTURE_SIZE as usize;
    let mut pixels = Vec::with_capacity(size * size * 4);
    for y in 0..TEXTURE_SIZE {
        for x in 0..TEXTURE_SIZE {
            let noise = fbm(
                x as f32 / TEXTURE_SIZE as f32,
                y as f32 / TEXTURE_SIZE as f32,
                seed,
            );
            // Brightness swings ±10% around the base color: texture without
            // contrast that could fight the high-saturation sprites.
            let brightness = 0.9 + 0.2 * noise;
            pixels.push((BASE_RGB[0] * brightness * 255.0) as u8);
            pixels.push((BASE_RGB[1] * brightness * 255.0) as u8);
            pixels.push((BASE_RGB[2] * brightness * 255.0) as u8);
            pixels.push(255);
        }
    }

    Image::new(
        Extent3d {
            width: TEXTURE_SIZE,
            height: TEXTURE_SIZE,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        pixels,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    )
}

/// 4-octave value-noise FBM over [0,1)^2, tiling seamlessly (each octave's
/// lattice wraps at its period).
fn fbm(u: f32, v: f32, seed: u64) -> f32 {
    let mut sum = 0.0;
    let mut amplitude = 1.0;
    let mut norm = 0.0;
    for octave in 0..OCTAVES {
        let period = BASE_CELLS << octave;
        sum += amplitude * value_noise(u, v, period, splitmix64(seed ^ octave as u64));
        norm += amplitude;
        amplitude *= 0.5;
    }
    sum / norm
}

/// Smoothly interpolated lattice value noise; `period` wraps both axes so the
/// texture tiles without seams.
fn value_noise(u: f32, v: f32, period: u32, seed: u64) -> f32 {
    let period = period as i32;
    let x = u * period as f32;
    let y = v * period as f32;
    let ix = x.floor() as i32;
    let iy = y.floor() as i32;
    let wrap = |i: i32| i.rem_euclid(period) as u32;
    let tx = smoothstep(x - ix as f32);
    let ty = smoothstep(y - iy as f32);

    let a = cell_noise(wrap(ix), wrap(iy), seed);
    let b = cell_noise(wrap(ix + 1), wrap(iy), seed);
    let c = cell_noise(wrap(ix), wrap(iy + 1), seed);
    let d = cell_noise(wrap(ix + 1), wrap(iy + 1), seed);
    a + (b - a) * tx + (c - a) * ty + (a - b - c + d) * tx * ty
}

fn smoothstep(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}

/// Lattice hash: a bijective-looking u64 mix mapped to [0, 1).
fn cell_noise(x: u32, y: u32, seed: u64) -> f32 {
    let h = splitmix64(seed ^ ((x as u64) << 32) ^ y as u64);
    (h >> 40) as f32 / (1u64 << 24) as f32
}

fn splitmix64(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^ (x >> 31)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_seed_produces_identical_textures() {
        let a = generate_background_image(wave_seed(42, 3));
        let b = generate_background_image(wave_seed(42, 3));
        assert_eq!(a.data, b.data, "identical seed+wave must replay identically");

        let c = generate_background_image(wave_seed(42, 4));
        assert_ne!(a.data, c.data, "different waves should differ");
    }

    #[test]
    fn noise_tiles_seamlessly() {
        let seed = wave_seed(7, 1);
        for step in 0..8 {
            let t = step as f32 / 8.0;
            assert!((fbm(0.0, t, seed) - fbm(1.0, t, seed)).abs() < 1e-6);
            assert!((fbm(t, 0.0, seed) - fbm(t, 1.0, seed)).abs() < 1e-6);
        }
    }

    #[test]
    fn noise_stays_within_contrast_band() {
        let seed = wave_seed(123, 1);
        for y in [0.0, 0.25, 0.5, 0.75] {
            for x in [0.0, 0.2, 0.4, 0.6, 0.8] {
                let n = fbm(x, y, seed);
                assert!((0.0..=1.0).contains(&n));
            }
        }
    }
}
