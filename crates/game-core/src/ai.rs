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
use crate::upgrade::{UpgradeSelected, WeaponLevels};
use crate::weapon::{StartingWeapon, StartingWeaponSelected, Weapon, WeaponKind};
use crate::GameState;

/// Threat radius: enemies closer than this make the AI flee.
const THREAT_RADIUS: f32 = 300.0;

/// Deterministic build used by headless balance scenarios.
#[derive(Resource, Debug, Clone)]
pub struct AiBuild {
    pub weapon: WeaponKind,
    pub upgrade_options: [usize; 4],
    pub buy_items: bool,
}

impl Default for AiBuild {
    fn default() -> Self {
        Self {
            weapon: WeaponKind::PiercingProjectile,
            upgrade_options: [0; 4],
            buy_items: true,
        }
    }
}

/// Plugin driving the game via the intent layer. Tests only.
pub struct AiPlugin;

impl Plugin for AiPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<AiBuild>().add_systems(
            Update,
            (
                ai_menu_navigation.run_if(in_state(GameState::MainMenu)),
                ai_starting_weapon.run_if(in_state(GameState::StartingWeaponChoice)),
                ai_combat_movement.run_if(in_state(GameState::InGame)),
                ai_upgrade.run_if(in_state(GameState::UpgradeChoice)),
                ai_continue.run_if(in_state(GameState::Shop)),
                ai_buy.run_if(in_state(GameState::Shop)),
                ai_play_again.run_if(in_state(GameState::Victory).or(in_state(GameState::Defeat))),
            ),
        );
    }
}

/// Start a run from the main menu.
fn ai_menu_navigation(mut next_state: ResMut<NextState<GameState>>) {
    next_state.set(GameState::StartingWeaponChoice);
}

fn ai_starting_weapon(build: Res<AiBuild>, mut writer: MessageWriter<StartingWeaponSelected>) {
    writer.write(StartingWeaponSelected { kind: build.weapon });
}

/// Continue to the next wave from the shop.
fn ai_continue(mut next_state: ResMut<NextState<GameState>>) {
    next_state.set(GameState::InGame);
}

/// The wave-end upgrade pick is mandatory: take the first non-maxed path,
/// option A. The core applies it and moves on to the Shop.
fn ai_upgrade(
    levels: Res<WeaponLevels>,
    build: Res<AiBuild>,
    starting_weapon: Res<StartingWeapon>,
    mut writer: MessageWriter<UpgradeSelected>,
) {
    if let Some(kind) = starting_weapon.selected {
        if !levels.maxed(kind) {
            let level = levels.level(kind);
            let option = if level < 5 {
                build.upgrade_options[(level - 1) as usize]
            } else {
                0
            };
            writer.write(UpgradeSelected { kind, option });
        }
    }
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
    weapons: Query<&Weapon>,
    mut intent: ResMut<PlayerMoveIntent>,
) {
    let Ok(player) = players.single() else {
        return;
    };
    let pos = player.translation.truncate();

    let close_threat_radius = match weapons.iter().next() {
        Some(weapon) => match weapon.kind {
            WeaponKind::PiercingProjectile => THREAT_RADIUS,
            WeaponKind::MeleeSwing => 55.0,
            WeaponKind::OrbitingOrb => weapon.orbit_radius * 0.75,
        },
        None => THREAT_RADIUS,
    };
    let mut flee = Vec2::ZERO;
    for enemy in &enemies {
        let away = pos - enemy.translation.truncate();
        let dist_sq = away.length_squared();
        if dist_sq < close_threat_radius * close_threat_radius {
            flee += away / dist_sq.max(1.0);
        }
    }
    if flee != Vec2::ZERO {
        intent.dir = flee.normalize_or_zero();
        return;
    }

    if let Some(weapon) = weapons.iter().next() {
        let engagement_range = match weapon.kind {
            WeaponKind::MeleeSwing => Some(weapon.range * 0.8),
            WeaponKind::OrbitingOrb => Some(weapon.orbit_radius),
            WeaponKind::PiercingProjectile => None,
        };
        if let Some(range) = engagement_range {
            let nearest = enemies
                .iter()
                .map(|enemy| enemy.translation.truncate())
                .min_by(|a, b| {
                    pos.distance_squared(*a)
                        .total_cmp(&pos.distance_squared(*b))
                });
            if let Some(target) = nearest {
                if pos.distance(target) > range {
                    intent.dir = (target - pos).normalize_or_zero();
                    return;
                }
            }
        }
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
fn ai_buy(build: Res<AiBuild>, mut writer: MessageWriter<PurchaseRequest>) {
    if !build.buy_items {
        return;
    }
    // Preference: +HP (survival) → +damage → +speed.
    for index in [2, 0, 1] {
        writer.write(PurchaseRequest { item_index: index });
    }
}
