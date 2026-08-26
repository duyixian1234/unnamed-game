//! UI screens built on `bevy_ui` (HUD, menus, shop).

pub mod main_menu;

use bevy::prelude::*;

/// Plugin for all UI screens. Only the MainMenu exists for now (T2); later
/// milestones add the HUD and Shop.
pub struct UIPlugin;

impl Plugin for UIPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(main_menu::MainMenuPlugin);
    }
}

/// Marker for a UI screen root node, used to clean up screens on state exit.
#[derive(Component)]
pub struct ScreenRoot;
