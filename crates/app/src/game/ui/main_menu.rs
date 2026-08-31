//! Main menu screen: a title and a Start button that opens weapon selection.

use bevy::prelude::*;

use game_core::GameState;

use super::settings_screen::spawn_settings_screen;
use super::{apply_button_color, swap_screen, ui_font, ScreenRoot, SettingsOrigin, BUTTON_IDLE};

/// Plugin that owns the main menu screen and its interactions.
pub struct MainMenuPlugin;

impl Plugin for MainMenuPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::MainMenu), on_enter_main_menu)
            .add_systems(OnExit(GameState::MainMenu), despawn_main_menu)
            .add_systems(Update, (start_button, settings_button));
    }
}

#[derive(Component)]
struct StartButton;

/// Opens the settings screen by swapping this screen out (see
/// `settings_screen`: `GameState` is left untouched, per ADR-0004).
#[derive(Component)]
struct OpenSettingsButton;

fn on_enter_main_menu(mut commands: Commands, asset_server: Res<AssetServer>) {
    spawn_main_menu(&mut commands, &asset_server);
}

/// Spawn the main menu. Public so the settings screen can hand control back.
pub fn spawn_main_menu(commands: &mut Commands, asset_server: &AssetServer) {
    commands
        .spawn((
            ScreenRoot,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                row_gap: Val::Px(24.0),
                ..default()
            },
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new("unnamed-game"),
                ui_font(asset_server, 64.0),
                TextColor(Color::WHITE),
            ));
            parent
                .spawn((
                    StartButton,
                    Button,
                    Node {
                        padding: UiRect::axes(Val::Px(40.0), Val::Px(16.0)),
                        ..default()
                    },
                    BackgroundColor(BUTTON_IDLE),
                ))
                .with_child((
                    Text::new("开始"),
                    ui_font(asset_server, 32.0),
                    TextColor(Color::WHITE),
                ));
            parent
                .spawn((
                    OpenSettingsButton,
                    Button,
                    Node {
                        padding: UiRect::axes(Val::Px(32.0), Val::Px(12.0)),
                        ..default()
                    },
                    BackgroundColor(BUTTON_IDLE),
                ))
                .with_child((
                    Text::new("设置"),
                    ui_font(asset_server, 26.0),
                    TextColor(Color::WHITE),
                ));
        });
}

fn despawn_main_menu(mut commands: Commands, roots: Query<Entity, With<ScreenRoot>>) {
    for root in &roots {
        // `despawn` recursively despawns all descendants in Bevy 0.17.
        commands.entity(root).despawn();
    }
}

#[allow(clippy::type_complexity)] // Changed<Interaction> + With<StartButton> filter
fn start_button(
    mut interaction: Query<
        (&Interaction, &mut BackgroundColor),
        (Changed<Interaction>, With<StartButton>),
    >,
    mut next_state: ResMut<NextState<GameState>>,
) {
    for (interaction, mut color) in &mut interaction {
        apply_button_color(interaction, &mut color);
        if matches!(*interaction, Interaction::Pressed) {
            next_state.set(GameState::StartingWeaponChoice);
        }
    }
}

#[allow(clippy::type_complexity)] // Changed<Interaction> + With<OpenSettingsButton> filter
fn settings_button(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    roots: Query<Entity, With<ScreenRoot>>,
    mut interaction: Query<
        (&Interaction, &mut BackgroundColor),
        (Changed<Interaction>, With<OpenSettingsButton>),
    >,
) {
    for (interaction, mut color) in &mut interaction {
        apply_button_color(interaction, &mut color);
        if matches!(*interaction, Interaction::Pressed) {
            commands.insert_resource(SettingsOrigin::MainMenu);
            swap_screen(&mut commands, &roots, |commands| {
                spawn_settings_screen(commands, &asset_server);
            });
        }
    }
}
