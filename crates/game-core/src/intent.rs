//! The intent layer: how players (human or AI) express actions.
//!
//! Player agency enters the simulation only through these types: the
//! keyboard system (app crate) or the AI (`ai`, tests) writes them, and core
//! systems read them. Scenario setup in tests may bypass this boundary and
//! write state directly, but never player actions.

use bevy::ecs::message::Message;
use bevy::math::Vec2;
use bevy::prelude::Resource;

/// The direction the player wants to move this frame (normalized by the
/// movement system). Written each frame by the keyboard system or the AI.
#[derive(Resource, Debug, Clone, Copy, Default)]
pub struct PlayerMoveIntent {
    pub dir: Vec2,
}

/// A request to buy the shop catalog item at `item_index`. Sent by the shop
/// UI buttons or the AI; the core purchase system validates and applies it.
#[derive(Message, Debug, Clone, Copy)]
pub struct PurchaseRequest {
    pub item_index: usize,
}

pub(crate) fn reset_move_intent(mut intent: bevy::prelude::ResMut<PlayerMoveIntent>) {
    intent.dir = Vec2::ZERO;
}
