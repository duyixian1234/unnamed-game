//! UI screens built on `bevy_ui` (HUD, menus, shop). Pure presentation:
//! state navigation is written directly as `NextState`, purchases go through
//! the core `PurchaseRequest` message.

pub mod end_screen;
pub mod hud;
pub mod main_menu;
pub mod shop;
pub mod upgrade;
pub mod weapon_bar;

use bevy::prelude::*;

/// Plugin for all UI screens.
pub struct UIPlugin;

impl Plugin for UIPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            end_screen::EndScreenPlugin,
            hud::HudPlugin,
            main_menu::MainMenuPlugin,
            shop::ShopPlugin,
            upgrade::UpgradeScreenPlugin,
            weapon_bar::WeaponBarPlugin,
        ));
    }
}

/// Marker for a UI screen root node, used to clean up screens on state exit.
#[derive(Component)]
pub struct ScreenRoot;

/// The subsetted Chinese UI font (ADR-0007). `AssetServer.load` dedups by
/// path, so calling this per screen is cheap and avoids load-order hazards.
pub fn ui_font(asset_server: &AssetServer, font_size: f32) -> TextFont {
    TextFont {
        font: asset_server.load("fonts/ui.ttf"),
        font_size,
        ..default()
    }
}
