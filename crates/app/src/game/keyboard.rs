//! Keyboard → `PlayerMoveIntent`: the human input path into the intent layer.

use bevy::input::keyboard::KeyCode;
use bevy::input::ButtonInput;
use bevy::math::Vec2;
use bevy::prelude::{App, Plugin, Res, ResMut, Update};

use game_core::intent::PlayerMoveIntent;

/// Plugin translating WASD keys into the player's move intent.
pub struct KeyboardIntentPlugin;

impl Plugin for KeyboardIntentPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, keyboard_intent);
    }
}

fn keyboard_intent(keyboard: Res<ButtonInput<KeyCode>>, mut intent: ResMut<PlayerMoveIntent>) {
    let mut dir = Vec2::ZERO;
    if keyboard.pressed(KeyCode::KeyW) {
        dir.y += 1.0;
    }
    if keyboard.pressed(KeyCode::KeyS) {
        dir.y -= 1.0;
    }
    if keyboard.pressed(KeyCode::KeyA) {
        dir.x -= 1.0;
    }
    if keyboard.pressed(KeyCode::KeyD) {
        dir.x += 1.0;
    }
    intent.dir = dir;
}
