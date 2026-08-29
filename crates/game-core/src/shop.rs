//! The shop: catalog, purchase validation, and stat application.
//!
//! Purchase logic is converged here so every client (UI buttons in the app
//! crate, the test AI) triggers the exact same system via `PurchaseRequest`.

use bevy::ecs::message::Message;
use bevy::ecs::message::MessageWriter;
use bevy::prelude::*;

use crate::economy::Materials;
use crate::intent::PurchaseRequest;
use crate::player::{Health, PlayerStats};
use crate::GameState;

/// One purchasable stat-boost item offered in the shop.
pub struct ShopItem {
    pub name: &'static str,
    pub cost: u32,
    /// Apply the boost to the player.
    pub apply: fn(&mut PlayerStats),
}

/// The fixed shop catalog. Pure stat-gain items (per CONTEXT.md).
pub const SHOP_ITEMS: &[ShopItem] = &[
    ShopItem {
        name: "磨砺之刃（伤害 +20%）",
        cost: 15,
        apply: |s| s.damage_mult += 0.2,
    },
    ShopItem {
        name: "肾上腺（速度 +15%）",
        cost: 12,
        apply: |s| s.speed_mult += 0.15,
    },
    ShopItem {
        name: "泰坦之心（最大生命 +25）",
        cost: 20,
        apply: |s| s.max_hp_bonus += 25.0,
    },
];

/// A purchase succeeded.
#[derive(Message, Debug, Clone, Copy)]
pub struct ItemPurchased {
    pub item_index: usize,
    pub cost: u32,
}

/// Plugin for the shop purchase logic (the shop UI lives in the app crate).
pub struct ShopPlugin;

impl Plugin for ShopPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<PurchaseRequest>()
            .add_message::<ItemPurchased>()
            .add_systems(Update, process_purchases.run_if(in_state(GameState::Shop)));
    }
}

/// Spend materials to buy items; apply the stat boost to the player.
fn process_purchases(
    mut requests: MessageReader<PurchaseRequest>,
    mut purchased_writer: MessageWriter<ItemPurchased>,
    mut materials: ResMut<Materials>,
    mut players: Query<(&mut PlayerStats, &mut Health)>,
) {
    for request in requests.read() {
        let Some(item) = SHOP_ITEMS.get(request.item_index) else {
            continue;
        };
        let Ok((mut stats, mut health)) = players.single_mut() else {
            continue;
        };
        if materials.count < item.cost {
            continue;
        }
        materials.count -= item.cost;

        (item.apply)(&mut stats);
        // A max-HP boost raises the cap; current HP is clamped to it
        // (no heal — the boost only enlarges the pool).
        if stats.max_hp_bonus > 0.0 {
            health.max = 100.0 + stats.max_hp_bonus;
            health.current = health.current.min(health.max);
        }
        purchased_writer.write(ItemPurchased {
            item_index: request.item_index,
            cost: item.cost,
        });
    }
}
