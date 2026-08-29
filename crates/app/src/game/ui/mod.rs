//! UI screens built on `bevy_ui` (HUD, menus, shop). Pure presentation:
//! state navigation is written directly as `NextState`, purchases go through
//! the core `PurchaseRequest` message.

pub mod end_screen;
pub mod hud;
pub mod main_menu;
pub mod shop;

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
        ));
    }
}

/// Marker for a UI screen root node, used to clean up screens on state exit.
#[derive(Component)]
pub struct ScreenRoot;
