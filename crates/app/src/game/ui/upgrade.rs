//! Between-wave weapon upgrade screen: full route preview (including the
//! Lv6 evolution) plus clickable options. Choice logic lives in game-core;
//! buttons send an `UpgradeSelected` message (same path as the test AI).
//! The screen is rebuilt on every entry, so the offered pair always matches
//! the current level. Text is Chinese (ADR-0007) — add new copy to
//! `tools/ui_text.txt` and re-run `tools/gen_font.sh`.

use bevy::prelude::*;

use game_core::damage::DamageStats;
use game_core::upgrade::{UpgradeSelected, WeaponLevels, UPGRADE_PATHS};
use game_core::weapon::{StartingWeapon, WeaponKind};
use game_core::GameState;

use super::{damage_summary_text, ui_font, ScreenRoot};

/// A clickable upgrade option button, tagged with its target path + option.
#[derive(Component)]
struct UpgradeButton {
    kind: WeaponKind,
    option: usize,
}

/// Any button on the upgrade screen (shared hover feedback).
#[derive(Component)]
struct UpgradeAnyButton(Color);

#[derive(Component)]
struct DamageSummaryText;

/// Plugin for the upgrade-choice screen.
pub struct UpgradeScreenPlugin;

impl Plugin for UpgradeScreenPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::UpgradeChoice), spawn_upgrade_screen)
            .add_systems(OnExit(GameState::UpgradeChoice), despawn_upgrade_screen)
            .add_systems(
                Update,
                (request_upgrades, update_damage_summary, button_hover)
                    .run_if(in_state(GameState::UpgradeChoice)),
            );
    }
}

fn spawn_upgrade_screen(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    levels: Res<WeaponLevels>,
    starting_weapon: Res<StartingWeapon>,
    damage_stats: Res<DamageStats>,
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
                row_gap: Val::Px(6.0),
                padding: UiRect::horizontal(Val::Px(80.0)),
                overflow: Overflow::clip_y(),
                ..default()
            },
            BackgroundColor(Color::srgb(0.07, 0.07, 0.12)),
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new("武器升级 · 每波选择一次"),
                ui_font(&asset_server, 36.0),
                TextColor(Color::WHITE),
            ));
            parent
                .spawn(Node {
                    width: Val::Percent(100.0),
                    max_width: Val::Px(1120.0),
                    padding: UiRect::all(Val::Px(10.0)),
                    margin: UiRect::vertical(Val::Px(4.0)),
                    ..default()
                })
                .with_child((
                    DamageSummaryText,
                    Text::new(damage_summary_text(&damage_stats, false)),
                    ui_font(&asset_server, 16.0),
                    TextColor(Color::srgb(0.78, 0.82, 0.90)),
                ));

            if let Some(kind) = starting_weapon.selected {
                if let Some(path) = UPGRADE_PATHS.iter().find(|path| path.kind == kind) {
                    spawn_path_ui(parent, &asset_server, path, levels.level(path.kind));
                }
            } else {
                parent.spawn((
                    Text::new("尚未选择武器"),
                    ui_font(&asset_server, 24.0),
                    TextColor(Color::srgb(0.8, 0.4, 0.4)),
                ));
            }
        });
}

fn update_damage_summary(
    damage_stats: Res<DamageStats>,
    mut summary: Query<&mut Text, With<DamageSummaryText>>,
) {
    if !damage_stats.is_changed() {
        return;
    }
    let text = damage_summary_text(&damage_stats, false);
    for mut summary in &mut summary {
        summary.0 = text.clone();
    }
}

/// One weapon's column: header (with current level), the fixed full route
/// (transparent, including the Lv6 evolution preview), and the picks for its
/// next level-up.
fn spawn_path_ui(
    parent: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    path: &game_core::upgrade::WeaponUpgradePath,
    level: u8,
) {
    parent
        .spawn(Node {
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            row_gap: Val::Px(2.0),
            margin: UiRect::top(Val::Px(8.0)),
            ..default()
        })
        .with_children(|weapon_node| {
            weapon_node.spawn((
                Text::new(format!("{}  Lv {}/6", path.kind.display_name(), level)),
                ui_font(asset_server, 26.0),
                TextColor(Color::srgb(0.9, 0.8, 0.4)),
            ));

            // Full route preview, always visible (issue #19 transparency).
            for (i, pair) in path.levels.iter().enumerate() {
                let target = i + 2; // rows are level-ups 1->2 .. 4->5
                let mark = |taken: bool| if taken { "x" } else { " " };
                weapon_node.spawn((
                    Text::new(format!(
                        "Lv{}:  [{}] A) {}    [{}] B) {}",
                        target,
                        mark(level >= target as u8),
                        pair[0].label,
                        mark(level >= target as u8),
                        pair[1].label
                    )),
                    ui_font(asset_server, 18.0),
                    TextColor(Color::srgb(0.55, 0.55, 0.6)),
                ));
            }
            weapon_node.spawn((
                Text::new(format!(
                    "Lv6 质变：{} · {}",
                    path.evolution.name(),
                    path.evolution.description()
                )),
                ui_font(asset_server, 18.0),
                TextColor(Color::srgb(0.85, 0.5, 0.85)),
            ));

            // Clickable picks for the next level-up.
            match level {
                1..=4 => {
                    let row = &path.levels[(level - 1) as usize];
                    weapon_node
                        .spawn(Node {
                            flex_direction: FlexDirection::Row,
                            column_gap: Val::Px(16.0),
                            margin: UiRect::vertical(Val::Px(4.0)),
                            ..default()
                        })
                        .with_children(|buttons| {
                            for (option, opt) in row.iter().enumerate() {
                                buttons
                                    .spawn((
                                        UpgradeButton {
                                            kind: path.kind,
                                            option,
                                        },
                                        UpgradeAnyButton(Color::srgb(0.2, 0.5, 0.3)),
                                        Button,
                                        Node {
                                            padding: UiRect::axes(Val::Px(20.0), Val::Px(6.0)),
                                            ..default()
                                        },
                                        BackgroundColor(Color::srgb(0.2, 0.5, 0.3)),
                                    ))
                                    .with_child((
                                        Text::new(format!("Lv{} {}", level + 1, opt.label)),
                                        ui_font(asset_server, 20.0),
                                        TextColor(Color::WHITE),
                                    ));
                            }
                        });
                }
                5 => {
                    // The evolution pick: a single mandatory button.
                    weapon_node
                        .spawn(Node {
                            margin: UiRect::vertical(Val::Px(4.0)),
                            ..default()
                        })
                        .with_children(|buttons| {
                            buttons
                                .spawn((
                                    UpgradeButton {
                                        kind: path.kind,
                                        option: 0,
                                    },
                                    UpgradeAnyButton(Color::srgb(0.6, 0.2, 0.6)),
                                    Button,
                                    Node {
                                        padding: UiRect::axes(Val::Px(24.0), Val::Px(6.0)),
                                        ..default()
                                    },
                                    BackgroundColor(Color::srgb(0.6, 0.2, 0.6)),
                                ))
                                .with_child((
                                    Text::new(format!("Lv6 进化：{}", path.evolution.name())),
                                    ui_font(asset_server, 20.0),
                                    TextColor(Color::WHITE),
                                ));
                        });
                }
                _ => {
                    weapon_node.spawn((
                        Text::new("已满级"),
                        ui_font(asset_server, 20.0),
                        TextColor(Color::srgb(0.7, 0.7, 0.2)),
                    ));
                }
            }
        });
}

fn despawn_upgrade_screen(mut commands: Commands, roots: Query<Entity, With<ScreenRoot>>) {
    for root in &roots {
        commands.entity(root).despawn();
    }
}

/// Clicking an option sends `UpgradeSelected`; the core validates the choice,
/// applies it, and advances to the Shop.
fn request_upgrades(
    mut requests: MessageWriter<UpgradeSelected>,
    interactions: Query<(&Interaction, &UpgradeButton), Changed<Interaction>>,
) {
    for (interaction, button) in &interactions {
        if *interaction == Interaction::Pressed {
            requests.write(UpgradeSelected {
                kind: button.kind,
                option: button.option,
            });
        }
    }
}

/// Hover highlight on all upgrade buttons.
fn button_hover(
    mut buttons: Query<
        (&Interaction, &UpgradeAnyButton, &mut BackgroundColor),
        Changed<Interaction>,
    >,
) {
    for (interaction, base, mut color) in &mut buttons {
        let c = base.0.to_srgba();
        let lighten = |delta: f32| {
            Color::srgb(
                (c.red + delta).min(1.0),
                (c.green + delta).min(1.0),
                (c.blue + delta).min(1.0),
            )
        };
        *color = BackgroundColor(match *interaction {
            Interaction::Pressed => lighten(0.2),
            Interaction::Hovered => lighten(0.1),
            Interaction::None => base.0,
        });
    }
}
