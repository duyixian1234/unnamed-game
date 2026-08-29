//! Main menu screen: a title and a Start button that begins the game.

use bevy::prelude::*;

use game_core::GameState;

use super::ScreenRoot;

/// Plugin that owns the main menu screen and its interactions.
pub struct MainMenuPlugin;

impl Plugin for MainMenuPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::MainMenu), spawn_main_menu)
            .add_systems(OnExit(GameState::MainMenu), despawn_main_menu)
            .add_systems(Update, start_button);
    }
}

#[derive(Component)]
struct StartButton;

fn spawn_main_menu(mut commands: Commands) {
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
                TextFont {
                    font_size: 64.0,
                    ..default()
                },
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
                    BackgroundColor(Color::srgb(0.2, 0.4, 0.8)),
                ))
                .with_child((
                    Text::new("Start"),
                    TextFont {
                        font_size: 32.0,
                        ..default()
                    },
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

fn start_button(
    mut interaction: Query<
        (&Interaction, &mut BackgroundColor),
        (Changed<Interaction>, With<StartButton>),
    >,
    mut next_state: ResMut<NextState<GameState>>,
) {
    for (interaction, mut color) in &mut interaction {
        match *interaction {
            Interaction::Pressed => {
                next_state.set(GameState::InGame);
            }
            Interaction::Hovered => {
                *color = BackgroundColor(Color::srgb(0.3, 0.5, 0.9));
            }
            Interaction::None => {
                *color = BackgroundColor(Color::srgb(0.2, 0.4, 0.8));
            }
        }
    }
}
