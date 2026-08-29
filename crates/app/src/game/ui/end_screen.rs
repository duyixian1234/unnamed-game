//! End screens: Victory (survived all waves) and Defeat (player died).

use bevy::prelude::*;

use game_core::GameState;

use super::ScreenRoot;

/// A button that restarts the game from the main menu.
#[derive(Component)]
struct PlayAgainButton;

/// Plugin for the Victory / Defeat screens.
pub struct EndScreenPlugin;

impl Plugin for EndScreenPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::Victory), spawn_victory)
            .add_systems(OnEnter(GameState::Defeat), spawn_defeat)
            .add_systems(
                Update,
                play_again.run_if(in_state(GameState::Victory).or(in_state(GameState::Defeat))),
            );
    }
}

/// Shared end-screen layout: a title, subtitle, and a Play Again button.
fn spawn_screen(commands: &mut Commands, title: &str, title_color: Color, subtitle: &str) {
    commands
        .spawn((
            ScreenRoot,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                row_gap: Val::Px(20.0),
                ..default()
            },
            BackgroundColor(Color::srgb(0.06, 0.06, 0.08)),
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new(title),
                TextFont { font_size: 72.0, ..default() },
                TextColor(title_color),
            ));
            parent.spawn((
                Text::new(subtitle),
                TextFont { font_size: 28.0, ..default() },
                TextColor(Color::WHITE),
            ));
            parent
                .spawn((
                    PlayAgainButton,
                    Button,
                    Node {
                        padding: UiRect::axes(Val::Px(48.0), Val::Px(14.0)),
                        margin: UiRect::top(Val::Px(16.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.2, 0.4, 0.8)),
                ))
                .with_child((
                    Text::new("Play Again"),
                    TextFont { font_size: 28.0, ..default() },
                    TextColor(Color::WHITE),
                ));
        });
}

fn spawn_victory(mut commands: Commands) {
    spawn_screen(
        &mut commands,
        "Victory!",
        Color::srgb(0.9, 0.85, 0.2),
        "You survived all 20 waves.",
    );
}

fn spawn_defeat(mut commands: Commands) {
    spawn_screen(
        &mut commands,
        "Defeat",
        Color::srgb(0.9, 0.3, 0.3),
        "The horde overwhelmed you.",
    );
}

fn play_again(
    mut commands: Commands,
    mut interactions: Query<&Interaction, (Changed<Interaction>, With<PlayAgainButton>)>,
    mut next_state: ResMut<NextState<GameState>>,
    roots: Query<Entity, With<ScreenRoot>>,
) {
    for interaction in &mut interactions {
        if *interaction != Interaction::Pressed {
            continue;
        }
        // Clear the current end screen; returning to MainMenu resets the run.
        for root in &roots {
            commands.entity(root).despawn();
        }
        next_state.set(GameState::MainMenu);
    }
}
