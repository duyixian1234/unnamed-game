//! Settings screen: four preference rows plus a back button.
//!
//! Swapped in and out imperatively rather than through a `GameState`
//! transition — `GameState` is a simulation state machine and this screen is
//! pure presentation (ADR-0004). `main_menu` hands control here and gets it
//! back by calling `main_menu::spawn_main_menu`.

use bevy::ecs::hierarchy::ChildSpawnerCommands;
use bevy::prelude::*;
use bevy::window::{MonitorSelection, WindowMode};

use crate::game::settings::{SettingsStore, VOLUME_STEP_PERCENT};

use super::main_menu::spawn_main_menu;
use super::{swap_screen, ui_font, ScreenRoot, BUTTON_HOVERED, BUTTON_IDLE, BUTTON_PRESSED};

/// Plugin for the settings screen's interactions and label syncing.
pub struct SettingsScreenPlugin;

impl Plugin for SettingsScreenPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                highlight_buttons,
                toggle_mute,
                adjust_volume,
                toggle_fullscreen,
                toggle_overlay,
                go_back,
                sync_settings_labels,
            ),
        );
    }
}

/// Which preference a value label displays. One query over this beats four
/// near-identical ones stitched together with `Without` filters.
#[derive(Component)]
enum SettingsValue {
    Mute,
    Volume,
    Fullscreen,
    Overlay,
}

/// Marks every button on this screen so one system can handle highlighting.
#[derive(Component)]
struct SettingsScreenButton;

#[derive(Component)]
struct MuteButton;

#[derive(Component)]
struct VolumeDownButton;

#[derive(Component)]
struct VolumeUpButton;

#[derive(Component)]
struct FullscreenButton;

#[derive(Component)]
struct OverlayButton;

#[derive(Component)]
struct BackButton;

/// Spawn the settings screen, replacing whatever screen is currently up.
pub fn spawn_settings_screen(commands: &mut Commands, asset_server: &AssetServer) {
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
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new("设置"),
                ui_font(asset_server, 48.0),
                TextColor(Color::WHITE),
            ));

            spawn_toggle_row(
                parent,
                asset_server,
                "SFX 静音",
                MuteButton,
                SettingsValue::Mute,
            );
            spawn_volume_row(parent, asset_server);
            spawn_toggle_row(
                parent,
                asset_server,
                "全屏",
                FullscreenButton,
                SettingsValue::Fullscreen,
            );
            spawn_toggle_row(
                parent,
                asset_server,
                "诊断叠层",
                OverlayButton,
                SettingsValue::Overlay,
            );

            parent
                .spawn((
                    BackButton,
                    SettingsScreenButton,
                    Button,
                    Node {
                        margin: UiRect::top(Val::Px(12.0)),
                        padding: UiRect::axes(Val::Px(32.0), Val::Px(12.0)),
                        ..default()
                    },
                    BackgroundColor(BUTTON_IDLE),
                ))
                .with_child((
                    Text::new("返回"),
                    ui_font(asset_server, 26.0),
                    TextColor(Color::WHITE),
                ));
        });
}

fn spawn_row_label(
    row: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    label: &'static str,
) {
    row.spawn((
        Node {
            width: Val::Px(140.0),
            ..default()
        },
        Text::new(label),
        ui_font(asset_server, 24.0),
        TextColor(Color::srgb(0.9, 0.9, 0.9)),
    ));
}

fn spawn_toggle_row<B: Component>(
    parent: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    label: &'static str,
    button: B,
    value: SettingsValue,
) {
    parent
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(16.0),
            ..default()
        })
        .with_children(|row| {
            spawn_row_label(row, asset_server, label);
            row.spawn((
                button,
                SettingsScreenButton,
                Button,
                Node {
                    padding: UiRect::axes(Val::Px(28.0), Val::Px(8.0)),
                    ..default()
                },
                BackgroundColor(BUTTON_IDLE),
            ))
            .with_child((
                value,
                Text::new(""),
                ui_font(asset_server, 24.0),
                TextColor(Color::WHITE),
            ));
        });
}

fn spawn_volume_row(parent: &mut ChildSpawnerCommands, asset_server: &AssetServer) {
    parent
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(12.0),
            ..default()
        })
        .with_children(|row| {
            spawn_row_label(row, asset_server, "SFX 音量");
            spawn_stepper(row, asset_server, VolumeDownButton, "-");
            row.spawn((
                SettingsValue::Volume,
                Node {
                    width: Val::Px(70.0),
                    justify_content: JustifyContent::Center,
                    ..default()
                },
                Text::new(""),
                ui_font(asset_server, 24.0),
                TextColor(Color::WHITE),
            ));
            spawn_stepper(row, asset_server, VolumeUpButton, "+");
        });
}

fn spawn_stepper<B: Component>(
    row: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    marker: B,
    label: &'static str,
) {
    row.spawn((
        marker,
        SettingsScreenButton,
        Button,
        Node {
            padding: UiRect::axes(Val::Px(16.0), Val::Px(8.0)),
            ..default()
        },
        BackgroundColor(BUTTON_IDLE),
    ))
    .with_child((
        Text::new(label),
        ui_font(asset_server, 24.0),
        TextColor(Color::WHITE),
    ));
}

#[allow(clippy::type_complexity)] // Changed<Interaction> + With<SettingsScreenButton> filter
fn highlight_buttons(
    mut buttons: Query<
        (&Interaction, &mut BackgroundColor),
        (Changed<Interaction>, With<SettingsScreenButton>),
    >,
) {
    for (interaction, mut color) in &mut buttons {
        *color = BackgroundColor(match *interaction {
            Interaction::Pressed => BUTTON_PRESSED,
            Interaction::Hovered => BUTTON_HOVERED,
            Interaction::None => BUTTON_IDLE,
        });
    }
}

fn toggle_mute(
    mut store: ResMut<SettingsStore>,
    buttons: Query<&Interaction, (Changed<Interaction>, With<MuteButton>)>,
) {
    if any_pressed(buttons.iter()) {
        store.toggle_mute();
    }
}

fn adjust_volume(
    mut store: ResMut<SettingsStore>,
    down: Query<&Interaction, (Changed<Interaction>, With<VolumeDownButton>)>,
    up: Query<&Interaction, (Changed<Interaction>, With<VolumeUpButton>)>,
) {
    let step = VOLUME_STEP_PERCENT as i32;
    if any_pressed(down.iter()) {
        store.step_volume(-step);
    } else if any_pressed(up.iter()) {
        store.step_volume(step);
    }
}

/// Whether any of these buttons is being pressed right now.
fn any_pressed<'a>(mut interactions: impl Iterator<Item = &'a Interaction>) -> bool {
    interactions.any(|interaction| matches!(*interaction, Interaction::Pressed))
}

fn toggle_fullscreen(
    mut windows: Query<&mut Window>,
    buttons: Query<&Interaction, (Changed<Interaction>, With<FullscreenButton>)>,
) {
    if !any_pressed(buttons.iter()) {
        return;
    }
    let Ok(mut window) = windows.single_mut() else {
        return;
    };
    window.mode = match window.mode {
        // Exclusive fullscreen reaches `monitor.video_modes()`, which is
        // `unreachable!()` in winit's web backend and panics on wasm.
        // Borderless is also all winit's web backend offers, and it already
        // calls `request_fullscreen()` for us, so no `cfg` split is needed.
        WindowMode::Windowed => WindowMode::BorderlessFullscreen(MonitorSelection::Current),
        _ => WindowMode::Windowed,
    };
}

fn toggle_overlay(
    mut store: ResMut<SettingsStore>,
    buttons: Query<&Interaction, (Changed<Interaction>, With<OverlayButton>)>,
) {
    if any_pressed(buttons.iter()) {
        store.change(|settings| settings.overlay = !settings.overlay);
    }
}

fn go_back(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    roots: Query<Entity, With<ScreenRoot>>,
    buttons: Query<&Interaction, (Changed<Interaction>, With<BackButton>)>,
) {
    if any_pressed(buttons.iter()) {
        swap_screen(&mut commands, &roots, |commands| {
            spawn_main_menu(commands, &asset_server);
        });
    }
}

/// Keep the labels in step with the underlying state, so a value changed by
/// hotkey (M, backquote) does not leave a stale button behind.
fn sync_settings_labels(
    store: Res<SettingsStore>,
    windows: Query<&Window>,
    mut labels: Query<(&mut Text, &SettingsValue)>,
) {
    for (mut text, value) in &mut labels {
        let rendered = match value {
            SettingsValue::Mute => on_off(store.settings.sfx_muted),
            SettingsValue::Volume => format!("{}%", store.settings.sfx_volume_percent),
            // Fullscreen is session state, not a preference, so its label
            // reads the window rather than the settings (ADR 0011).
            SettingsValue::Fullscreen => on_off(is_fullscreen(&windows)),
            SettingsValue::Overlay => on_off(store.settings.overlay),
        };
        if text.0 != rendered {
            text.0 = rendered;
        }
    }
}

fn on_off(enabled: bool) -> String {
    if enabled {
        "开".to_string()
    } else {
        "关".to_string()
    }
}

fn is_fullscreen(windows: &Query<&Window>) -> bool {
    windows
        .single()
        .is_ok_and(|window| !matches!(window.mode, WindowMode::Windowed))
}
