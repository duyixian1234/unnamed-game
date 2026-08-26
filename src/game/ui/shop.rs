//! Between-wave shop: spend Materials on pure stat-boost items.

use bevy::prelude::*;

use super::ScreenRoot;
use crate::game::economy::Materials;
use crate::game::player::{Health, Player, PlayerStats};
use crate::game::GameState;

/// One purchasable stat-boost item offered in the shop.
pub struct ShopItem {
    pub name: &'static str,
    pub cost: u32,
    /// Apply the boost to the player; returns the display string shown to the
    /// player after purchase.
    pub apply: fn(&mut PlayerStats),
}

/// The fixed shop catalog. Pure stat-gain items (per CONTEXT.md).
const SHOP_ITEMS: &[ShopItem] = &[
    ShopItem {
        name: "Sharpened Edge (+20% damage)",
        cost: 15,
        apply: |s| s.damage_mult += 0.2,
    },
    ShopItem {
        name: "Adrenal Gland (+15% speed)",
        cost: 12,
        apply: |s| s.speed_mult += 0.15,
    },
    ShopItem {
        name: "Titan's Heart (+25 max HP)",
        cost: 20,
        apply: |s| s.max_hp_bonus += 25.0,
    },
];

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
                (handle_purchases, handle_continue).run_if(in_state(GameState::Shop)),
            );
    }
}

fn spawn_shop(
    mut commands: Commands,
    materials: Res<Materials>,
    players: Query<&Health, With<Player>>,
) {
    let wallet = materials.count;
    let health = players.single().map(|h| (h.current, h.max)).unwrap_or((0.0, 0.0));

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

/// Spend materials to buy items; apply the stat boost to the player.
fn handle_purchases(
    mut materials: ResMut<Materials>,
    mut interactions: Query<(&Interaction, &ShopButton), Changed<Interaction>>,
    mut players: Query<(&mut PlayerStats, &mut Health), With<Player>>,
) {
    for (interaction, button) in &mut interactions {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let Some(item) = SHOP_ITEMS.get(button.index) else {
            continue;
        };
        if materials.count < item.cost {
            continue;
        }
        materials.count -= item.cost;

        let Ok((mut stats, mut health)) = players.single_mut() else {
            continue;
        };
        (item.apply)(&mut stats);
        // A max-HP boost also heals by that amount; keep current <= new max.
        if stats.max_hp_bonus > 0.0 {
            health.max = 100.0 + stats.max_hp_bonus;
            health.current = health.current.min(health.max);
        }
    }
}

/// Continue returns to the next wave (wave progression handled by T10).
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
