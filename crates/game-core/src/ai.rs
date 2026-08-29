//! A simple test AI that plays the game through the intent layer.
//!
//! The AI is the player for headless integration tests: it expresses agency
//! only via `PlayerMoveIntent`, `PurchaseRequest`, and `NextState<GameState>`
//! (state navigation is the same direct path the UI buttons take). It is NOT
//! registered by the production binary. Survival in flow tests is guaranteed
//! by test-side player buffs, not by AI skill.

use bevy::ecs::message::MessageWriter;
use bevy::prelude::*;

use crate::economy::Material;
use crate::enemy::Enemy;
use crate::intent::{PlayerMoveIntent, PurchaseRequest};
use crate::player::Player;
use crate::GameState;

/// Threat radius: enemies closer than this make the AI flee.
const THREAT_RADIUS: f32 = 300.0;

/// Plugin driving the game via the intent layer. Tests only.
pub struct AiPlugin;

impl Plugin for AiPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                ai_menu_navigation.run_if(in_state(GameState::MainMenu)),
                ai_combat_movement.run_if(in_state(GameState::InGame)),
                ai_continue.run_if(in_state(GameState::Shop)),
                ai_buy.run_if(in_state(GameState::Shop)),
                ai_play_again.run_if(in_state(GameState::Victory).or(in_state(GameState::Defeat))),
            ),
        );
    }
}

/// Start a run from the main menu.
fn ai_menu_navigation(mut next_state: ResMut<NextState<GameState>>) {
    next_state.set(GameState::InGame);
}

/// Continue to the next wave from the shop.
fn ai_continue(mut next_state: ResMut<NextState<GameState>>) {
    next_state.set(GameState::InGame);
}

/// Return to the main menu from an end screen (resets the run).
fn ai_play_again(mut next_state: ResMut<NextState<GameState>>) {
    next_state.set(GameState::MainMenu);
}

/// Flee nearby enemies; when safe, walk to the nearest material to collect it.
fn ai_combat_movement(
    players: Query<&Transform, With<Player>>,
    enemies: Query<&Transform, (With<Enemy>, Without<Player>)>,
    materials: Query<&Transform, (With<Material>, Without<Player>)>,
    mut intent: ResMut<PlayerMoveIntent>,
) {
    let Ok(player) = players.single() else {
        return;
    };
    let pos = player.translation.truncate();

    let mut flee = Vec2::ZERO;
    for enemy in &enemies {
        let away = pos - enemy.translation.truncate();
        let dist_sq = away.length_squared();
        if dist_sq < THREAT_RADIUS * THREAT_RADIUS {
            // Weight closer enemies far more heavily.
            flee += away / dist_sq.max(1.0);
        }
    }
    if flee != Vec2::ZERO {
        intent.dir = flee.normalize_or_zero();
        return;
    }

    // No threat: collect the nearest material, else idle.
    let mut nearest: Option<(f32, Vec2)> = None;
    for material in &materials {
        let target = material.translation.truncate();
        let dist_sq = pos.distance_squared(target);
        if nearest.is_none_or(|(best, _)| dist_sq < best) {
            nearest = Some((dist_sq, target));
        }
    }
    intent.dir = nearest
        .map(|(_, target)| (target - pos).normalize_or_zero())
        .unwrap_or(Vec2::ZERO);
}

/// Buy the first affordable catalog item each shop visit (the purchase
/// system ignores unaffordable requests).
fn ai_buy(mut writer: MessageWriter<PurchaseRequest>) {
    // Preference: +HP (survival) → +damage → +speed.
    for index in [2, 0, 1] {
        writer.write(PurchaseRequest { item_index: index });
    }
}
