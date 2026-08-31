//! 暂停 (Pause): freeze the simulation so the player can reach 设置 mid-Run.
//!
//! The freeze itself is a `game-core` concern: `Paused` makes `wave_running`
//! false, which stops the wave clock and the RNG draws (CONTEXT.md, ADR-0004).
//! This module only owns the hotkey and the overlay. The HUD, the weapon bar
//! and the frozen field stay on screen — only the simulation stops.

use bevy::ecs::hierarchy::ChildSpawnerCommands;
use bevy::input::keyboard::KeyCode;
use bevy::input::ButtonInput;
use bevy::prelude::*;

use game_core::{GameState, Paused};

use super::ui::settings_screen::{spawn_settings_screen, SettingsScreenRoot};
use super::ui::{apply_button_color, ui_font, SettingsOrigin, BUTTON_IDLE};

/// Plugin for the pause hotkey and overlay.
pub struct PausePlugin;

impl Plugin for PausePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (toggle_pause, pause_buttons).chain())
            // Leaving InGame for any reason drops the overlay and the freeze,
            // so a paused game can never leak into the next state.
            .add_systems(OnExit(GameState::InGame), end_pause);
    }
}

/// Root of the pause overlay. Deliberately not a `ScreenRoot`: the HUD and the
/// weapon bar are behind it and must stay visible, and `swap_screen` clears
/// every `ScreenRoot` — including both of those.
#[derive(Component)]
pub struct PauseOverlayRoot;

/// Which pause-overlay button an entity is. One enum-marked query instead of
/// two: separate queries over the same `&mut BackgroundColor` are rejected
/// (B0001) unless Bevy can prove the filters disjoint.
#[derive(Component)]
enum PauseButton {
    Resume,
    Settings,
}

/// ESC freezes and unfreezes the simulation.
fn toggle_pause(
    keyboard: Res<ButtonInput<KeyCode>>,
    state: Res<State<GameState>>,
    mut paused: ResMut<Paused>,
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    overlays: Query<Entity, With<PauseOverlayRoot>>,
    settings_open: Query<(), With<SettingsScreenRoot>>,
) {
    if !keyboard.just_pressed(KeyCode::Escape) {
        return;
    }
    if *state.get() != GameState::InGame {
        return;
    }
    // While the settings screen opened from here is up, 返回 owns the way
    // back. ESC would otherwise silently unfreeze the game behind an open
    // panel, and the player would resume into a wave they cannot see.
    if !settings_open.is_empty() {
        return;
    }

    paused.0 = !paused.0;
    if paused.0 {
        spawn_pause_overlay(&mut commands, &asset_server);
    } else {
        despawn_pause_overlay(&mut commands, &overlays);
    }
}

/// Drop the overlay and the freeze together.
fn end_pause(
    mut paused: ResMut<Paused>,
    mut commands: Commands,
    overlays: Query<Entity, With<PauseOverlayRoot>>,
) {
    paused.0 = false;
    despawn_pause_overlay(&mut commands, &overlays);
}

fn despawn_pause_overlay(
    commands: &mut Commands,
    overlays: &Query<Entity, With<PauseOverlayRoot>>,
) {
    for entity in overlays {
        commands.entity(entity).despawn();
    }
}

/// Spawn the pause overlay. Public so the settings screen can hand control
/// back to it instead of to the main menu.
pub fn spawn_pause_overlay(commands: &mut Commands, asset_server: &AssetServer) {
    commands
        .spawn((
            PauseOverlayRoot,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                row_gap: Val::Px(20.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.6)),
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new("暂停"),
                ui_font(asset_server, 48.0),
                TextColor(Color::WHITE),
            ));
            spawn_pause_button(parent, asset_server, PauseButton::Resume, "继续");
            spawn_pause_button(parent, asset_server, PauseButton::Settings, "设置");
        });
}

fn spawn_pause_button(
    parent: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    button: PauseButton,
    label: &'static str,
) {
    parent
        .spawn((
            button,
            Button,
            Node {
                padding: UiRect::axes(Val::Px(32.0), Val::Px(12.0)),
                ..default()
            },
            BackgroundColor(BUTTON_IDLE),
        ))
        .with_child((
            Text::new(label),
            ui_font(asset_server, 26.0),
            TextColor(Color::WHITE),
        ));
}

fn pause_buttons(
    mut paused: ResMut<Paused>,
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    overlays: Query<Entity, With<PauseOverlayRoot>>,
    mut buttons: Query<(&Interaction, &mut BackgroundColor, &PauseButton), Changed<Interaction>>,
) {
    for (interaction, mut color, button) in &mut buttons {
        apply_button_color(interaction, &mut color);
        if !matches!(*interaction, Interaction::Pressed) {
            continue;
        }
        match button {
            PauseButton::Resume => {
                paused.0 = false;
                despawn_pause_overlay(&mut commands, &overlays);
            }
            PauseButton::Settings => {
                // The game stays frozen while the settings screen is up.
                commands.insert_resource(SettingsOrigin::Pause);
                despawn_pause_overlay(&mut commands, &overlays);
                spawn_settings_screen(&mut commands, &asset_server);
            }
        }
    }
}
