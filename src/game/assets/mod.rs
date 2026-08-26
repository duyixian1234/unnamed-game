//! Runtime sprite assets loaded from the committed spritesheet atlas.
//!
//! Per ADR-0002, art is generated once via mmx and committed under
//! `assets/sprites/`. The individual sprites are baked into a single atlas
//! (`atlas.png`) with fixed 128x128 cells; this module maps entity kinds to
//! their atlas cell index.

use bevy::image::TextureAtlasLayout;
use bevy::prelude::*;

use crate::game::enemy::EnemyKind;

/// Atlas cell size in pixels (each generated sprite is scaled to this).
pub const ATLAS_CELL: u32 = 128;

/// Indices of each sprite within the 3x2 atlas grid (384x256).
pub mod atlas_index {
    pub const PLAYER: usize = 0;
    pub const MELEE_RUSHER: usize = 1;
    pub const SPEED_BURSTER: usize = 2;
    pub const SPLITTER: usize = 3;
    pub const MATERIAL: usize = 4;
}

/// Loaded sprite atlas handles.
#[derive(Resource)]
pub struct SpriteAssets {
    pub atlas: Handle<Image>,
    pub layout: Handle<TextureAtlasLayout>,
}

impl SpriteAssets {
    /// The atlas cell index for an enemy kind.
    pub fn enemy_index(&self, kind: EnemyKind) -> usize {
        match kind {
            EnemyKind::MeleeRusher => atlas_index::MELEE_RUSHER,
            EnemyKind::SpeedBurster => atlas_index::SPEED_BURSTER,
            EnemyKind::Splitter => atlas_index::SPLITTER,
        }
    }
}

/// Plugin that loads the sprite atlas at startup.
pub struct AssetsPlugin;

impl Plugin for AssetsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, load_sprites);
    }
}

fn load_sprites(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut layouts: ResMut<Assets<TextureAtlasLayout>>,
) {
    let image = asset_server.load("sprites/atlas.png");
    let layout = TextureAtlasLayout::from_grid(
        UVec2::splat(ATLAS_CELL),
        3,
        2,
        None,
        None,
    );
    let layout_handle = layouts.add(layout);
    commands.insert_resource(SpriteAssets {
        atlas: image,
        layout: layout_handle,
    });
}

/// Build a sprite from the atlas at the given cell index.
pub fn atlas_sprite(assets: &SpriteAssets, index: usize) -> Sprite {
    Sprite::from_atlas_image(
        assets.atlas.clone(),
        TextureAtlas {
            layout: assets.layout.clone(),
            index,
        },
    )
}
