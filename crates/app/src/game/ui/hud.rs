//! In-game HUD: HP, wave, materials, and controls hint.

use bevy::prelude::*;

use game_core::economy::Materials;
use game_core::player::{Health, Player};
use game_core::waves::{Wave, WaveConfig};
use game_core::GameState;

/// Root marker for the HUD (separate from generic ScreenRoot cleanup on exit).
#[derive(Component)]
struct HudRoot;

/// The HP bar fill node (its width reflects current HP).
#[derive(Component)]
struct HpBarFill;

/// The HP "current/max" number next to the bar.
#[derive(Component)]
struct HpText;

/// The wave + materials status label.
#[derive(Component)]
struct StatusText;

/// Plugin for the in-game HUD.
pub struct HudPlugin;

impl Plugin for HudPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::InGame), spawn_hud)
            .add_systems(OnExit(GameState::InGame), despawn_hud)
            .add_systems(Update, update_hud.run_if(in_state(GameState::InGame)));
    }
}

fn spawn_hud(mut commands: Commands) {
    commands
        .spawn((
            HudRoot,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                position_type: PositionType::Absolute,
                padding: UiRect::all(Val::Px(16.0)),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::SpaceBetween,
                ..default()
            },
        ))
        .with_children(|parent| {
            // Top-left: HP bar + HP number.
            parent
                .spawn(Node {
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(12.0),
                    ..default()
                })
                .with_children(|top| {
                    top.spawn((
                        Text::new("HP"),
                        TextFont { font_size: 20.0, ..default() },
                        TextColor(Color::WHITE),
                    ));
                    top.spawn((
                        Node {
                            width: Val::Px(200.0),
                            height: Val::Px(18.0),
                            border: UiRect::all(Val::Px(2.0)),
                            ..default()
                        },
                        BorderColor::all(Color::WHITE),
                        BackgroundColor(Color::srgb(0.15, 0.15, 0.15)),
                    ))
                    .with_child((
                        HpBarFill,
                        Node {
                            width: Val::Percent(100.0),
                            height: Val::Percent(100.0),
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.8, 0.2, 0.2)),
                    ));
                    top.spawn((
                        HpText,
                        TextFont { font_size: 20.0, ..default() },
                        TextColor(Color::WHITE),
                    ));
                });

            // Bottom-left: wave + materials status.
            parent.spawn((
                StatusText,
                TextFont { font_size: 22.0, ..default() },
                TextColor(Color::srgb(0.9, 0.9, 0.9)),
            ));

            // Bottom-right: controls hint.
            parent.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    bottom: Val::Px(16.0),
                    right: Val::Px(16.0),
                    ..default()
                },
            ))
            .with_child((
                Text::new("WASD to move — weapons auto-fire"),
                TextFont { font_size: 18.0, ..default() },
                TextColor(Color::srgb(0.8, 0.8, 0.8)),
            ));
        });
}

fn despawn_hud(mut commands: Commands, roots: Query<Entity, With<HudRoot>>) {
    for root in &roots {
        commands.entity(root).despawn();
    }
}

/// Update the HP bar, HP number, and the wave/materials status each frame.
fn update_hud(
    mut hp_fill: Query<&mut Node, With<HpBarFill>>,
    mut hp_text: Query<&mut Text, With<HpText>>,
    mut status_text: Query<&mut Text, (With<StatusText>, Without<HpText>)>,
    players: Query<&Health, With<Player>>,
    wave: Res<Wave>,
    config: Res<WaveConfig>,
    materials: Res<Materials>,
) {
    let Ok(health) = players.single() else {
        return;
    };
    let hp_pct = if health.max > 0.0 {
        (health.current / health.max).clamp(0.0, 1.0) * 100.0
    } else {
        0.0
    };

    for mut node in &mut hp_fill {
        node.width = Val::Percent(hp_pct);
    }
    for mut text in &mut hp_text {
        text.0 = format!("{:.0}/{:.0}", health.current, health.max);
    }
    for mut text in &mut status_text {
        text.0 = format!(
            "Wave {}/{}    Materials: {}",
            wave.number, config.max_waves, materials.count
        );
    }
}
