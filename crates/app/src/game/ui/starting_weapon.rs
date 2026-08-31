//! Starting weapon choice screen.
//!
//! Cards are previews only; the run starts only after the explicit confirm
//! button sends `StartingWeaponSelected` to game-core.

use bevy::prelude::*;
use bevy::ui::InteractionDisabled;

use game_core::upgrade::path_for;
use game_core::weapon::{StartingWeapon, StartingWeaponSelected, Weapon, WeaponKind};
use game_core::GameState;

use super::{ui_font, ScreenRoot};
use crate::game::assets::{atlas_image, SpriteAssets};

const CARD_BG: Color = Color::srgb(0.10, 0.11, 0.16);
const CARD_HOVER: Color = Color::srgb(0.16, 0.19, 0.28);
const CARD_SELECTED: Color = Color::srgb(0.18, 0.27, 0.40);

#[derive(Resource, Debug, Clone, Copy)]
struct StartingWeaponUiState {
    focused: WeaponKind,
    selected: Option<WeaponKind>,
    details_expanded: bool,
}

impl Default for StartingWeaponUiState {
    fn default() -> Self {
        Self {
            focused: WeaponKind::PiercingProjectile,
            selected: None,
            details_expanded: false,
        }
    }
}

#[derive(Component)]
struct WeaponCard {
    kind: WeaponKind,
}

#[derive(Component)]
struct SelectionText;

#[derive(Component)]
struct RouteDetailsText;

#[derive(Component)]
struct ConfirmButton;

#[derive(Component)]
struct ChoiceButton(Color);

/// Plugin for the mandatory pre-run weapon choice.
pub struct StartingWeaponPlugin;

impl Plugin for StartingWeaponPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<StartingWeaponUiState>()
            .add_systems(
                OnEnter(GameState::StartingWeaponChoice),
                spawn_starting_weapon_screen,
            )
            .add_systems(
                OnExit(GameState::StartingWeaponChoice),
                despawn_starting_weapon_screen,
            )
            .add_systems(
                Update,
                (
                    select_card,
                    keyboard_choice,
                    confirm_selection,
                    update_choice_visuals,
                    sync_confirm_button,
                    button_hover,
                )
                    .run_if(in_state(GameState::StartingWeaponChoice)),
            );
    }
}

fn spawn_starting_weapon_screen(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    starting_weapon: Res<StartingWeapon>,
    sprite_assets: Res<SpriteAssets>,
    mut ui_state: ResMut<StartingWeaponUiState>,
) {
    ui_state.focused = starting_weapon
        .selected
        .unwrap_or(WeaponKind::PiercingProjectile);
    ui_state.selected = None;
    ui_state.details_expanded = false;

    commands
        .spawn((
            ScreenRoot,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                row_gap: Val::Px(12.0),
                padding: UiRect::axes(Val::Px(34.0), Val::Px(18.0)),
                overflow: Overflow::clip_y(),
                ..default()
            },
            BackgroundColor(Color::srgb(0.055, 0.065, 0.10)),
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new("选择初始武器"),
                ui_font(&asset_server, 42.0),
                TextColor(Color::WHITE),
            ));
            parent.spawn((
                Text::new("点击卡片预览路线，确认后开始本局"),
                ui_font(&asset_server, 20.0),
                TextColor(Color::srgb(0.70, 0.74, 0.84)),
            ));

            parent
                .spawn(Node {
                    width: Val::Percent(100.0),
                    max_width: Val::Px(1120.0),
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(14.0),
                    align_items: AlignItems::Stretch,
                    ..default()
                })
                .with_children(|cards| {
                    for kind in WeaponKind::ALL {
                        spawn_weapon_card(cards, &asset_server, &sprite_assets, kind);
                    }
                });

            parent.spawn((
                SelectionText,
                Text::new("当前预览："),
                ui_font(&asset_server, 22.0),
                TextColor(Color::srgb(0.62, 0.78, 0.96)),
            ));

            parent
                .spawn((
                    Node {
                        width: Val::Percent(100.0),
                        max_width: Val::Px(1120.0),
                        min_height: Val::Px(156.0),
                        padding: UiRect::all(Val::Px(16.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.08, 0.09, 0.14)),
                    BorderColor::all(Color::srgb(0.28, 0.34, 0.48)),
                ))
                .with_child((
                    RouteDetailsText,
                    Text::new(""),
                    ui_font(&asset_server, 18.0),
                    TextColor(Color::srgb(0.84, 0.87, 0.94)),
                ));

            parent
                .spawn((
                    ConfirmButton,
                    ChoiceButton(Color::srgb(0.20, 0.52, 0.32)),
                    Button,
                    InteractionDisabled,
                    Node {
                        padding: UiRect::axes(Val::Px(48.0), Val::Px(12.0)),
                        margin: UiRect::top(Val::Px(2.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.20, 0.52, 0.32)),
                ))
                .with_child((
                    Text::new("确认选择，开始游戏"),
                    ui_font(&asset_server, 24.0),
                    TextColor(Color::WHITE),
                ));
        });
}

fn spawn_weapon_card(
    parent: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    sprite_assets: &SpriteAssets,
    kind: WeaponKind,
) {
    let weapon = Weapon::new(kind);
    let path = path_for(kind);
    let stats = base_stats_copy(&weapon);
    parent
        .spawn((
            WeaponCard { kind },
            ChoiceButton(CARD_BG),
            Button,
            Node {
                width: Val::Percent(33.333),
                min_height: Val::Px(176.0),
                padding: UiRect::all(Val::Px(14.0)),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(7.0),
                border: UiRect::all(Val::Px(2.0)),
                ..default()
            },
            BackgroundColor(CARD_BG),
            BorderColor::all(Color::srgb(0.24, 0.28, 0.38)),
        ))
        .with_children(|card| {
            card.spawn((
                Node {
                    width: Val::Px(48.0),
                    height: Val::Px(48.0),
                    ..default()
                },
                atlas_image(sprite_assets, SpriteAssets::weapon_icon_index(kind)),
            ));
            card.spawn((
                Text::new(kind.display_name()),
                ui_font(asset_server, 26.0),
                TextColor(Color::WHITE),
            ));
            card.spawn((
                Text::new(format!("定位：{}", kind.playstyle())),
                ui_font(asset_server, 17.0),
                TextColor(Color::srgb(0.62, 0.78, 0.96)),
            ));
            card.spawn((
                Text::new(format!("一级关键属性：{}", stats)),
                ui_font(asset_server, 17.0),
                TextColor(Color::srgb(0.88, 0.88, 0.92)),
            ));
            card.spawn((
                Text::new(format!(
                    "八级质变：{}：{}",
                    path.evolution.name(),
                    path.evolution.description()
                )),
                ui_font(asset_server, 17.0),
                TextColor(Color::srgb(0.93, 0.70, 0.86)),
            ));
        });
}

fn base_stats_copy(weapon: &Weapon) -> String {
    match weapon.kind {
        WeaponKind::PiercingProjectile => format!(
            "伤害 {:.0} · 冷却 {:.1} 秒 · 射程 {:.0}",
            weapon.damage,
            weapon.cooldown.duration().as_secs_f32(),
            weapon.range
        ),
        WeaponKind::MeleeSwing => format!(
            "伤害 {:.0} · 冷却 {:.1} 秒 · 范围 {:.0} · 击退 {:.0}",
            weapon.damage,
            weapon.cooldown.duration().as_secs_f32(),
            weapon.range,
            weapon.knockback_impulse()
        ),
        WeaponKind::OrbitingOrb => format!(
            "伤害 {:.0} · 最大半径 {:.0} · 环绕速度 {:.1} · 击退 {:.0}",
            weapon.damage,
            weapon.orbit_radius,
            weapon.orbit_speed,
            weapon.knockback_impulse()
        ),
    }
}

fn route_details(kind: WeaponKind) -> String {
    let path = path_for(kind);
    let mut details = format!("{} · 完整升级路线\n", kind.display_name());
    details.push_str(&format!("一级：{}\n", base_stats_copy(&Weapon::new(kind))));
    for (index, pair) in path.levels.iter().enumerate() {
        details.push_str(&format!(
            "{}级：甲：{}　乙：{}\n",
            index + 2,
            pair[0].label,
            pair[1].label
        ));
    }
    details.push_str(&format!(
        "八级质变：{} · {}",
        path.evolution.name(),
        path.evolution.description()
    ));
    details
}

fn select_card(
    interactions: Query<(&Interaction, &WeaponCard), Changed<Interaction>>,
    mut ui_state: ResMut<StartingWeaponUiState>,
) {
    for (interaction, card) in &interactions {
        if *interaction == Interaction::Pressed {
            ui_state.focused = card.kind;
            ui_state.selected = Some(card.kind);
            ui_state.details_expanded = true;
        }
    }
}

fn confirm_selection(
    interactions: Query<&Interaction, (Changed<Interaction>, With<ConfirmButton>)>,
    ui_state: Res<StartingWeaponUiState>,
    mut selections: MessageWriter<StartingWeaponSelected>,
) {
    for interaction in &interactions {
        if *interaction != Interaction::Pressed {
            continue;
        }
        if let Some(kind) = ui_state.selected {
            selections.write(StartingWeaponSelected { kind });
        }
    }
}

fn update_choice_visuals(
    ui_state: Res<StartingWeaponUiState>,
    cards: Query<(
        &WeaponCard,
        &Interaction,
        &mut BackgroundColor,
        &mut BorderColor,
    )>,
    mut selection_text: Query<&mut Text, (With<SelectionText>, Without<RouteDetailsText>)>,
    mut details_text: Query<&mut Text, (With<RouteDetailsText>, Without<SelectionText>)>,
) {
    for (card, interaction, mut background, mut border) in cards {
        let selected = ui_state.selected == Some(card.kind);
        let focused = card.kind == ui_state.focused;
        let hovered = *interaction == Interaction::Hovered;
        *background = BackgroundColor(if selected {
            CARD_SELECTED
        } else if hovered {
            CARD_HOVER
        } else if focused {
            Color::srgb(0.13, 0.17, 0.25)
        } else {
            CARD_BG
        });
        *border = BorderColor::all(if selected {
            Color::srgb(0.95, 0.76, 0.28)
        } else if focused {
            Color::srgb(0.36, 0.64, 0.95)
        } else {
            Color::srgb(0.24, 0.28, 0.38)
        });
    }

    for mut text in &mut selection_text {
        text.0 = match ui_state.selected {
            Some(kind) => format!(
                "已选择：{}　·　再次按回车键或空格键确认",
                kind.display_name()
            ),
            None => format!(
                "当前预览：{}　·　点击卡片或按回车键或空格键选择",
                ui_state.focused.display_name()
            ),
        };
    }
    for mut text in &mut details_text {
        text.0 = if ui_state.details_expanded {
            route_details(ui_state.focused)
        } else {
            "选择武器卡片以展开完整升级路线".to_string()
        };
    }
}

fn keyboard_choice(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut ui_state: ResMut<StartingWeaponUiState>,
    mut selections: MessageWriter<StartingWeaponSelected>,
) {
    let kinds = WeaponKind::ALL;
    let current = kinds
        .iter()
        .position(|kind| *kind == ui_state.focused)
        .unwrap_or(0);

    if keyboard.just_pressed(KeyCode::ArrowLeft) || keyboard.just_pressed(KeyCode::KeyA) {
        ui_state.focused = kinds[(current + kinds.len() - 1) % kinds.len()];
        ui_state.selected = None;
        ui_state.details_expanded = false;
    } else if keyboard.just_pressed(KeyCode::ArrowRight) || keyboard.just_pressed(KeyCode::KeyD) {
        ui_state.focused = kinds[(current + 1) % kinds.len()];
        ui_state.selected = None;
        ui_state.details_expanded = false;
    }

    if keyboard.just_pressed(KeyCode::Enter) || keyboard.just_pressed(KeyCode::Space) {
        if ui_state.selected == Some(ui_state.focused) {
            selections.write(StartingWeaponSelected {
                kind: ui_state.focused,
            });
        } else {
            ui_state.selected = Some(ui_state.focused);
            ui_state.details_expanded = true;
        }
    }
}

fn sync_confirm_button(
    mut commands: Commands,
    ui_state: Res<StartingWeaponUiState>,
    mut buttons: Query<
        (Entity, Option<&InteractionDisabled>, &mut BackgroundColor),
        With<ConfirmButton>,
    >,
) {
    let enabled = ui_state.selected.is_some();
    for (entity, disabled, mut background) in &mut buttons {
        if enabled == disabled.is_none() {
            if enabled {
                *background = BackgroundColor(Color::srgb(0.20, 0.52, 0.32));
            } else {
                *background = BackgroundColor(Color::srgb(0.20, 0.22, 0.27));
            }
            continue;
        }
        if enabled {
            commands.entity(entity).remove::<InteractionDisabled>();
            *background = BackgroundColor(Color::srgb(0.20, 0.52, 0.32));
        } else {
            commands.entity(entity).insert(InteractionDisabled);
            *background = BackgroundColor(Color::srgb(0.20, 0.22, 0.27));
        }
    }
}

fn button_hover(
    mut buttons: Query<(&Interaction, &ChoiceButton, &mut BackgroundColor), Changed<Interaction>>,
) {
    for (interaction, base, mut color) in &mut buttons {
        if base.0 == CARD_BG {
            continue;
        }
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

fn despawn_starting_weapon_screen(mut commands: Commands, roots: Query<Entity, With<ScreenRoot>>) {
    for root in &roots {
        commands.entity(root).despawn();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn choice_visual_queries_are_compatible() {
        let mut app = App::new();
        app.init_resource::<StartingWeaponUiState>()
            .add_systems(Update, update_choice_visuals);
        app.update();
    }
}
