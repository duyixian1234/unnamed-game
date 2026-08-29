//! Between-wave shop UI: catalog display and buttons. Purchase logic lives
//! in game-core; buttons send a `PurchaseRequest` (same path as the AI).

use bevy::prelude::*;

use game_core::economy::Materials;
use game_core::intent::PurchaseRequest;
use game_core::player::{Health, Player};
use game_core::shop::SHOP_ITEMS;
use game_core::GameState;

use super::{ui_font, ScreenRoot};

/// A clickable shop item button, tagged with its catalog index.
#[derive(Component)]
struct ShopButton {
    index: usize,
}

/// Any button on the shop screen (for shared hover feedback).
#[derive(Component)]
struct ShopAnyButton(Color);

/// The wallet/HP status line; refreshed every frame so purchases are visible.
#[derive(Component)]
struct ShopStatusText;

/// The shop's Continue button that returns to the next wave.
#[derive(Component)]
struct ContinueButton;

/// Plugin for the shop screen.
pub struct ShopPlugin;

impl Plugin for ShopPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::Shop), spawn_shop)
            .add_systems(OnExit(GameState::Shop), despawn_shop)
            .add_systems(
                Update,
                (
                    request_purchases,
                    handle_continue,
                    update_shop_status,
                    button_hover,
                )
                    .run_if(in_state(GameState::Shop)),
            );
    }
}

fn spawn_shop(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    materials: Res<Materials>,
    players: Query<&Health, With<Player>>,
) {
    let wallet = materials.count;
    let health = players
        .single()
        .map(|h| (h.current, h.max))
        .unwrap_or((0.0, 0.0));

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
                ..default()
            },
            BackgroundColor(Color::srgb(0.08, 0.08, 0.1)),
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new("商店"),
                ui_font(&asset_server, 48.0),
                TextColor(Color::WHITE),
            ));
            parent.spawn((
                ShopStatusText,
                Text::new(format!(
                    "材料: {}   生命: {:.0}/{:.0}",
                    wallet, health.0, health.1
                )),
                ui_font(&asset_server, 24.0),
                TextColor(Color::srgb(0.6, 0.4, 0.9)),
            ));

            for (index, item) in SHOP_ITEMS.iter().enumerate() {
                parent
                    .spawn((
                        ShopButton { index },
                        ShopAnyButton(Color::srgb(0.2, 0.4, 0.8)),
                        Button,
                        Node {
                            padding: UiRect::axes(Val::Px(32.0), Val::Px(10.0)),
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.2, 0.4, 0.8)),
                    ))
                    .with_child((
                        Text::new(format!("{}  ·  {} 材料", item.name, item.cost)),
                        ui_font(&asset_server, 24.0),
                        TextColor(Color::WHITE),
                    ));
            }

            parent
                .spawn((
                    ContinueButton,
                    ShopAnyButton(Color::srgb(0.2, 0.6, 0.3)),
                    Button,
                    Node {
                        padding: UiRect::axes(Val::Px(40.0), Val::Px(12.0)),
                        margin: UiRect::top(Val::Px(20.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.2, 0.6, 0.3)),
                ))
                .with_child((
                    Text::new("前往下一波"),
                    ui_font(&asset_server, 24.0),
                    TextColor(Color::WHITE),
                ));
        });
}

fn despawn_shop(mut commands: Commands, roots: Query<Entity, With<ScreenRoot>>) {
    for root in &roots {
        commands.entity(root).despawn();
    }
}

/// Clicking a shop button sends a `PurchaseRequest`; the core shop system
/// validates funds and applies the boost.
fn request_purchases(
    mut requests: MessageWriter<PurchaseRequest>,
    interactions: Query<(&Interaction, &ShopButton), Changed<Interaction>>,
) {
    for (interaction, button) in &interactions {
        if *interaction == Interaction::Pressed {
            requests.write(PurchaseRequest {
                item_index: button.index,
            });
        }
    }
}

/// Refresh the wallet/HP line every frame — purchases must be immediately
/// visible, otherwise a successful buy looks like a dead click.
fn update_shop_status(
    mut status: Query<&mut Text, With<ShopStatusText>>,
    materials: Res<Materials>,
    players: Query<&Health, With<Player>>,
) {
    let Ok(health) = players.single() else {
        return;
    };
    for mut text in &mut status {
        text.0 = format!(
            "材料: {}   生命: {:.0}/{:.0}",
            materials.count, health.current, health.max
        );
    }
}

/// Hover highlight on all shop buttons (no pressed-state feedback otherwise).
fn button_hover(
    mut buttons: Query<(&Interaction, &ShopAnyButton, &mut BackgroundColor), Changed<Interaction>>,
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

/// Continue returns to the next wave (wave progression handled by core).
fn handle_continue(
    mut interactions: Query<&Interaction, (Changed<Interaction>, With<ContinueButton>)>,
    mut next_state: ResMut<NextState<GameState>>,
) {
    for interaction in &mut interactions {
        if *interaction == Interaction::Pressed {
            next_state.set(GameState::InGame);
        }
    }
}
