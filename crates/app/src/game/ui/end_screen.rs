//! End screens: Victory (survived all waves) and Defeat (player died).

use bevy::prelude::*;

use game_core::damage::DamageStats;
use game_core::waves::WaveConfig;
use game_core::GameState;

use super::{damage_summary_text, ui_font, ScreenRoot};

/// A button that restarts the game from the starting weapon choice.
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
fn spawn_screen(
    commands: &mut Commands,
    asset_server: &AssetServer,
    title: &str,
    title_color: Color,
    subtitle: &str,
    damage_stats: &DamageStats,
    incomplete_label: bool,
) {
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
                ui_font(asset_server, 72.0),
                TextColor(title_color),
            ));
            parent.spawn((
                Text::new(subtitle),
                ui_font(asset_server, 28.0),
                TextColor(Color::WHITE),
            ));
            parent.spawn((
                Text::new(damage_summary_text(damage_stats, incomplete_label)),
                ui_font(asset_server, 18.0),
                TextColor(Color::srgb(0.78, 0.82, 0.90)),
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
                    Text::new("再来一局"),
                    ui_font(asset_server, 28.0),
                    TextColor(Color::WHITE),
                ));
        });
}

fn spawn_victory(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    config: Res<WaveConfig>,
    damage_stats: Res<DamageStats>,
) {
    spawn_screen(
        &mut commands,
        &asset_server,
        "胜利！",
        Color::srgb(0.9, 0.85, 0.2),
        &format!("你活过了全部 {} 波。", config.max_waves),
        &damage_stats,
        false,
    );
}

fn spawn_defeat(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    damage_stats: Res<DamageStats>,
) {
    spawn_screen(
        &mut commands,
        &asset_server,
        "失败",
        Color::srgb(0.9, 0.3, 0.3),
        "你被怪潮淹没了。",
        &damage_stats,
        !damage_stats.last_wave_completed,
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
        // Clear the current end screen. StartingWeaponChoice resets the
        // run-only app/core resources while preserving the previous weapon
        // selection for visual focus.
        for root in &roots {
            commands.entity(root).despawn();
        }
        next_state.set(GameState::StartingWeaponChoice);
    }
}
