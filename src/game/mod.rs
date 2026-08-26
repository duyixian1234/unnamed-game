//! Game-wide definitions: state machine and top-level plugin.

pub mod player;
pub mod ui;

use bevy::prelude::*;

use self::ui::UIPlugin;

/// The high-level game state machine. Drives the whole roguelike loop:
/// MainMenu → InGame(wave) → Shop → InGame … → Victory / Defeat.
#[derive(States, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum GameState {
    /// Title screen with a Start button.
    #[default]
    MainMenu,
    /// An active wave of combat (movement + auto-attack + spawning).
    InGame,
    /// Between-wave shop where the player spends Materials on items.
    Shop,
    /// Survived all 20 waves.
    Victory,
    /// Player died (one-life roguelike).
    Defeat,
}

/// Top-level plugin for the game systems (everything above the Bevy boot).
pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<GameState>()
            .add_plugins((player::PlayerPlugin, UIPlugin));
    }
}
