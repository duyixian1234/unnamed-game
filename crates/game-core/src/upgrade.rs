//! Weapon upgrade paths: per-WeaponKind level progression with a fixed
//! 2-choice data table, ending in a Level-6 Evolution (a behavior change,
//! per ADR-0008).
//!
//! Flow: a wave ends -> `GameState::UpgradeChoice`. The player must make one
//! choice per wave (no skipping, no rerolls); the chosen WeaponKind advances
//! one level and the run proceeds to the Shop. Options are a fixed table —
//! no RNG (ADR-0005).

use std::collections::HashMap;

use bevy::ecs::message::Message;
use bevy::prelude::*;

use crate::damage::WeaponSlot;
use crate::player::Player;
use crate::weapon::{Weapon, WeaponKind, MAX_WEAPON_SLOTS};
use crate::GameState;

/// One pick on a weapon's path: a short display label plus the stat
/// multipliers it applies.
pub struct UpgradeOption {
    pub label: &'static str,
    pub mods: &'static [StatMod],
}

/// A multiplicative stat change applied to the chosen `Weapon` slot.
#[derive(Debug, Clone, Copy)]
pub enum StatMod {
    Damage(f32),
    Cooldown(f32),
    Range(f32),
    Knockback(f32),
    ProjectileSpeed(f32),
    OrbitSpeed(f32),
    OrbitRadius(f32),
    AdditionalWeapon,
    AdditionalOrb,
}

/// The Level-6 Evolution of a path (ADR-0008: hard-coded behavior change).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Evolution {
    Whirlwind,
    Splitshot,
    BomberOrb,
}

impl Evolution {
    /// Display name shown in the upgrade UI (Chinese, per ADR-0007).
    pub fn name(self) -> &'static str {
        match self {
            Evolution::Whirlwind => "旋风刃",
            Evolution::Splitshot => "散裂弹",
            Evolution::BomberOrb => "自爆卫星",
        }
    }

    /// Description of the behavior change.
    pub fn description(self) -> &'static str {
        match self {
            Evolution::Whirlwind => "刀刃持续环绕自身旋转，全方位连续命中，移动不打断攻击",
            Evolution::Splitshot => "首次命中后分裂为 3 枚短程扇形弹片（50% 伤害）",
            Evolution::BomberOrb => "球体接触敌人时爆炸（小范围伤害），0.6 秒后重生",
        }
    }
}

/// One WeaponKind's full route: stat options for level-ups 1->2 .. 4->5
/// (four rows, two options each), then the fixed Level-6 Evolution.
pub struct WeaponUpgradePath {
    pub kind: WeaponKind,
    pub levels: [[UpgradeOption; 2]; 4],
    pub evolution: Evolution,
}

/// The fixed upgrade table (issue #19). Order matches `UPGRADE_PATHS` lookup.
pub const UPGRADE_PATHS: &[WeaponUpgradePath] = &[
    // Melee Swing -> Whirlwind
    WeaponUpgradePath {
        kind: WeaponKind::MeleeSwing,
        levels: [
            [
                UpgradeOption {
                    label: "伤害 +25%",
                    mods: &[StatMod::Damage(1.25)],
                },
                UpgradeOption {
                    label: "额外近战武器 +1",
                    mods: &[StatMod::AdditionalWeapon],
                },
            ],
            [
                UpgradeOption {
                    label: "冷却 -15%",
                    mods: &[StatMod::Cooldown(0.85)],
                },
                UpgradeOption {
                    label: "额外近战武器 +1",
                    mods: &[StatMod::AdditionalWeapon],
                },
            ],
            [
                UpgradeOption {
                    label: "伤害 +25%",
                    mods: &[StatMod::Damage(1.25)],
                },
                UpgradeOption {
                    label: "冷却 -15%",
                    mods: &[StatMod::Cooldown(0.85)],
                },
            ],
            [
                UpgradeOption {
                    label: "范围 +20%",
                    mods: &[StatMod::Range(1.2)],
                },
                UpgradeOption {
                    label: "伤害 +30%",
                    mods: &[StatMod::Damage(1.3)],
                },
            ],
        ],
        evolution: Evolution::Whirlwind,
    },
    // Piercing Projectile -> Splitshot
    WeaponUpgradePath {
        kind: WeaponKind::PiercingProjectile,
        levels: [
            [
                UpgradeOption {
                    label: "伤害 +25%",
                    mods: &[StatMod::Damage(1.25)],
                },
                UpgradeOption {
                    label: "冷却 -15%",
                    mods: &[StatMod::Cooldown(0.85)],
                },
            ],
            [
                UpgradeOption {
                    label: "弹速 +20%",
                    mods: &[StatMod::ProjectileSpeed(1.2)],
                },
                UpgradeOption {
                    label: "射程 +25%",
                    mods: &[StatMod::Range(1.25)],
                },
            ],
            [
                UpgradeOption {
                    label: "伤害 +30%",
                    mods: &[StatMod::Damage(1.3)],
                },
                UpgradeOption {
                    label: "冷却 -15%",
                    mods: &[StatMod::Cooldown(0.85)],
                },
            ],
            [
                UpgradeOption {
                    label: "弹速 +15%",
                    mods: &[StatMod::ProjectileSpeed(1.15)],
                },
                UpgradeOption {
                    label: "伤害 +25%",
                    mods: &[StatMod::Damage(1.25)],
                },
            ],
        ],
        evolution: Evolution::Splitshot,
    },
    // Orbiting Orb -> Bomber Orb
    WeaponUpgradePath {
        kind: WeaponKind::OrbitingOrb,
        levels: [
            [
                UpgradeOption {
                    label: "伤害 +25%",
                    mods: &[StatMod::Damage(1.25)],
                },
                UpgradeOption {
                    label: "环绕速度 +20%",
                    mods: &[StatMod::OrbitSpeed(1.2)],
                },
            ],
            [
                UpgradeOption {
                    label: "最大半径 +25%",
                    mods: &[StatMod::OrbitRadius(1.25)],
                },
                UpgradeOption {
                    label: "伤害 +25%",
                    mods: &[StatMod::Damage(1.25)],
                },
            ],
            [
                UpgradeOption {
                    label: "伤害 +25%",
                    mods: &[StatMod::Damage(1.25)],
                },
                UpgradeOption {
                    label: "额外环绕球 +1",
                    mods: &[StatMod::AdditionalOrb],
                },
            ],
            [
                UpgradeOption {
                    label: "环绕速度 +25%",
                    mods: &[StatMod::OrbitSpeed(1.25)],
                },
                UpgradeOption {
                    label: "伤害 +30%",
                    mods: &[StatMod::Damage(1.3)],
                },
            ],
        ],
        evolution: Evolution::BomberOrb,
    },
];

/// The upgrade path for a weapon kind.
pub fn path_for(kind: WeaponKind) -> &'static WeaponUpgradePath {
    UPGRADE_PATHS
        .iter()
        .find(|path| path.kind == kind)
        .expect("every WeaponKind has an upgrade path")
}

/// Per-WeaponKind upgrade level (1..=6). Shared across every instance of a
/// kind; starts at 1.
#[derive(Resource, Debug, Default)]
pub struct WeaponLevels {
    levels: HashMap<WeaponKind, u8>,
}

impl WeaponLevels {
    pub fn level(&self, kind: WeaponKind) -> u8 {
        self.levels.get(&kind).copied().unwrap_or(1)
    }

    /// Write a level directly (1..=6). Used by the apply system; also usable
    /// by tests as scenario setup.
    pub fn set_level(&mut self, kind: WeaponKind, level: u8) {
        self.levels.insert(kind, level.clamp(1, 6));
    }

    pub fn maxed(&self, kind: WeaponKind) -> bool {
        self.level(kind) >= 6
    }
}

/// The player picked one upgrade option for `kind` (option 0/1; ignored for
/// the Level-6 Evolution). Sent by the upgrade UI buttons or the test AI.
#[derive(Message, Debug, Clone, Copy)]
pub struct UpgradeSelected {
    pub kind: WeaponKind,
    pub option: usize,
}

/// Plugin for upgrade levels and choice application. The choice UI lives in
/// the app crate.
pub struct UpgradePlugin;

impl Plugin for UpgradePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<WeaponLevels>()
            .add_message::<UpgradeSelected>()
            .add_systems(
                Update,
                (apply_upgrades, auto_advance_when_maxed)
                    .run_if(in_state(GameState::UpgradeChoice)),
            )
            .add_systems(OnEnter(GameState::StartingWeaponChoice), reset_levels);
    }
}

/// Fresh run: every path starts over at level 1.
fn reset_levels(mut levels: ResMut<WeaponLevels>) {
    *levels = WeaponLevels::default();
}

/// Apply one valid choice: bump the kind's level, mutate its weapon stats or
/// grant the Evolution, then move on to the Shop (exactly one choice per wave).
#[allow(clippy::type_complexity, clippy::too_many_arguments)] // query disambiguation
fn apply_upgrades(
    mut requests: MessageReader<UpgradeSelected>,
    mut levels: ResMut<WeaponLevels>,
    mut next_state: ResMut<NextState<GameState>>,
    mut commands: Commands,
    mut damage_stats: ResMut<crate::damage::DamageStats>,
    players: Query<(Entity, &Transform), With<Player>>,
    mut weapons: Query<(Entity, &mut Weapon, &WeaponSlot), Without<Player>>,
    weapon_slots: Query<&WeaponSlot, Without<Player>>,
) {
    for request in requests.read() {
        let kind = request.kind;
        let level = levels.level(kind);
        if level >= 6 {
            continue;
        }
        // Stat levels are 2-choice; the Evolution pick ignores the index.
        if level < 5 && request.option > 1 {
            continue;
        }
        if !weapons.iter().any(|(_, weapon, _)| weapon.kind == kind) {
            continue;
        }

        if level == 5 {
            if kind == WeaponKind::MeleeSwing {
                // Whirlwind evolution: merge every MeleeSwing instance into
                // the lowest slot, pooling their damage into the single blade
                // (ADR-0009 amendment) — per-instance blades were
                // position-identical, so extra slots granted nothing.
                let mut melee: Vec<(WeaponSlot, Entity)> = weapons
                    .iter()
                    .filter(|(_, weapon, _)| weapon.kind == WeaponKind::MeleeSwing)
                    .map(|(entity, _, slot)| (*slot, entity))
                    .collect();
                melee.sort();
                let mut total_damage = 0.0f32;
                let mut max_range = 0.0f32;
                for (_, weapon, _) in weapons.iter() {
                    if weapon.kind == WeaponKind::MeleeSwing {
                        total_damage += weapon.damage;
                        max_range = max_range.max(weapon.range);
                    }
                }
                let Some(&(_, keep)) = melee.first() else {
                    continue;
                };
                let merged_away: Vec<WeaponSlot> =
                    melee.iter().skip(1).map(|(slot, _)| *slot).collect();
                for &(_, entity) in melee.iter().skip(1) {
                    commands.entity(entity).despawn();
                }
                commands.entity(keep).insert(Evolved);
                if let Ok((_, mut weapon, _)) = weapons.get_mut(keep) {
                    weapon.damage = total_damage;
                    weapon.range = max_range;
                }
                // The merged-away slots' accumulated damage history moves
                // onto the surviving slot: the summary shows one 旋风刃 line,
                // not ghost rows for despawned slots.
                let keep_slot = melee[0].0;
                damage_stats.merge_slots(&merged_away, keep_slot);
            } else {
                for (entity, weapon, _) in &mut weapons {
                    if weapon.kind == kind {
                        commands.entity(entity).insert(Evolved);
                    }
                }
            }
        } else {
            let option = &path_for(kind).levels[(level - 1) as usize][request.option];
            let mut add_weapon = false;
            let mut template = None;
            for m in option.mods {
                if matches!(m, StatMod::AdditionalWeapon) {
                    add_weapon = true;
                }
                for (_, mut weapon, _) in &mut weapons {
                    if weapon.kind == kind {
                        apply_mod(&mut weapon, *m);
                        template = Some(weapon.clone_for_new_slot());
                    }
                }
            }
            if add_weapon && kind == WeaponKind::MeleeSwing {
                let Some((player, transform)) = players.single().ok() else {
                    continue;
                };
                let used: Vec<_> = weapon_slots.iter().copied().collect();
                let Some(slot) = (0..MAX_WEAPON_SLOTS as u8)
                    .map(WeaponSlot)
                    .find(|candidate| !used.contains(candidate))
                else {
                    continue;
                };
                let mut weapon = template.unwrap_or_else(|| Weapon::new(kind));
                weapon.cooldown = Timer::from_seconds(
                    weapon.cooldown.duration().as_secs_f32(),
                    TimerMode::Repeating,
                );
                // Spread melee attack phases evenly across the shared cooldown
                // (ADR-0009 amendment): existing instances re-phase to i/n,
                // the newcomer takes the last n/n slot, so no two melee
                // weapons ever swing on the same frame.
                let mut melee: Vec<(WeaponSlot, Entity)> = weapons
                    .iter_mut()
                    .filter(|(_, weapon, _)| weapon.kind == WeaponKind::MeleeSwing)
                    .map(|(entity, _, slot)| (*slot, entity))
                    .collect();
                melee.sort();
                let count = melee.len() + 1;
                for (index, &(_, entity)) in melee.iter().enumerate() {
                    if let Ok((_, mut weapon, _)) = weapons.get_mut(entity) {
                        let offset = weapon.cooldown.duration() * index as u32 / count as u32;
                        weapon.cooldown.set_elapsed(offset);
                    }
                }
                let offset =
                    weapon.cooldown.duration() * (count - 1) as u32 / count as u32;
                weapon.cooldown.set_elapsed(offset);
                commands.entity(player).with_child((
                    weapon,
                    slot,
                    Transform::from_translation(transform.translation),
                ));
            }
        }
        levels.set_level(kind, level + 1);
        next_state.set(GameState::Shop);
        return; // exactly one choice per wave
    }
}

/// When every equipped path is already at Lv6 there is nothing left to offer,
/// so the mandatory choice is vacuous — proceed to the Shop instead of
/// dead-locking. Keyed on all *equipped* kinds (not just the starting one):
/// a maxed starting weapon must not skip the upgrade screen while other
/// weapons can still level — that screen is where the wave damage summary
/// lives.
#[allow(clippy::type_complexity)]
fn auto_advance_when_maxed(
    levels: Res<WeaponLevels>,
    weapons: Query<&Weapon, Without<Player>>,
    mut next_state: ResMut<NextState<GameState>>,
) {
    let mut kinds: Vec<WeaponKind> = weapons.iter().map(|weapon| weapon.kind).collect();
    kinds.sort();
    kinds.dedup();
    if !kinds.is_empty() && kinds.iter().all(|kind| levels.maxed(*kind)) {
        next_state.set(GameState::Shop);
    }
}

/// A weapon that reached Level 6 and changed behavior (ADR-0008). The
/// evolution-specific systems in `weapon.rs` key off this marker.
#[derive(Component, Debug)]
pub struct Evolved;

fn apply_mod(weapon: &mut Weapon, m: StatMod) {
    match m {
        StatMod::Damage(x) => weapon.damage *= x,
        StatMod::Cooldown(x) => {
            let reduced = weapon.cooldown.duration().mul_f32(x);
            weapon.cooldown.set_duration(reduced);
        }
        StatMod::Range(x) => weapon.range *= x,
        StatMod::Knockback(x) => weapon.knockback_mult *= x,
        StatMod::ProjectileSpeed(x) => weapon.projectile_speed *= x,
        StatMod::OrbitSpeed(x) => weapon.orbit_speed *= x,
        StatMod::OrbitRadius(x) => weapon.orbit_radius *= x,
        StatMod::AdditionalWeapon => {}
        StatMod::AdditionalOrb => weapon.orb_count = weapon.orb_count.saturating_add(1),
    }
}
