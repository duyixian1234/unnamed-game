//! Between-wave shop UI: catalog display and buttons. Purchase logic lives
//! in game-core; buttons send a `PurchaseRequest` (same path as the AI).

use bevy::prelude::*;

use game_core::economy::Materials;
use game_core::intent::PurchaseRequest;
use game_core::player::{Health, Player};
use game_core::shop::SHOP_ITEMS;
use game_core::GameState;

use super::ScreenRoot;

/// A clickable shop item button, tagged with its catalog index.
#[derive(Component)]
struct ShopButton {
    index: usize,
}

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
                (request_purchases, handle_continue).run_if(in_state(GameState::Shop)),
            );
    }
}

fn spawn_shop(
    mut commands: Commands,
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
                Text::new("Shop"),
                TextFont { font_size: 48.0, ..default() },
                TextColor(Color::WHITE),
            ));
            parent.spawn((
                Text::new(format!(
                    "Materials: {}   HP: {:.0}/{:.0}",
                    wallet, health.0, health.1
                )),
                TextFont { font_size: 24.0, ..default() },
                TextColor(Color::srgb(0.6, 0.4, 0.9)),
            ));

            for (index, item) in SHOP_ITEMS.iter().enumerate() {
                parent
                    .spawn((
                        ShopButton { index },
                        Button,
                        Node {
                            padding: UiRect::axes(Val::Px(32.0), Val::Px(10.0)),
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.2, 0.4, 0.8)),
                    ))
                    .with_child((
                        Text::new(format!("{}  —  {} mats", item.name, item.cost)),
                        TextFont { font_size: 24.0, ..default() },
                        TextColor(Color::WHITE),
                    ));
            }

            parent
                .spawn((
                    ContinueButton,
                    Button,
                    Node {
                        padding: UiRect::axes(Val::Px(40.0), Val::Px(12.0)),
                        margin: UiRect::top(Val::Px(20.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.2, 0.6, 0.3)),
                ))
                .with_child((
                    Text::new("Continue to next wave"),
                    TextFont { font_size: 24.0, ..default() },
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
