//! Effective-damage attribution and Wave/Run contribution snapshots.

use std::collections::BTreeMap;

use bevy::prelude::*;

use crate::weapon::WeaponKind;

/// Stable identity of one equipped Weapon Slot.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WeaponSlot(pub u8);

/// Attribution carried by every damaging attack.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DamageSource {
    Weapon { slot: WeaponSlot, kind: WeaponKind },
    Other,
}

/// Effective Damage attributed to one Weapon Slot in a period.
#[derive(Debug, Clone, Copy)]
pub struct SlotDamage {
    pub kind: WeaponKind,
    pub effective_damage: f32,
}

/// Damage contributions for one Wave or Run.
#[derive(Debug, Clone, Default)]
pub struct DamagePeriod {
    slots: BTreeMap<WeaponSlot, SlotDamage>,
    pub other: f32,
}

impl DamagePeriod {
    pub fn slot(&self, index: WeaponSlot) -> Option<&SlotDamage> {
        self.slots.get(&index)
    }

    pub fn slots(&self) -> impl Iterator<Item = (WeaponSlot, &SlotDamage)> {
        self.slots.iter().map(|(slot, damage)| (*slot, damage))
    }

    pub fn total(&self) -> f32 {
        self.slots
            .values()
            .map(|slot| slot.effective_damage)
            .sum::<f32>()
            + self.other
    }

    pub fn percentage(&self, damage: f32) -> f32 {
        let total = self.total();
        if total > 0.0 {
            damage / total * 100.0
        } else {
            0.0
        }
    }

    fn record(&mut self, source: DamageSource, effective_damage: f32) {
        match source {
            DamageSource::Weapon { slot, kind } => {
                let entry = self.slots.entry(slot).or_insert(SlotDamage {
                    kind,
                    effective_damage: 0.0,
                });
                entry.effective_damage += effective_damage;
            }
            DamageSource::Other => self.other += effective_damage,
        }
    }

    fn register_weapon(&mut self, slot: WeaponSlot, kind: WeaponKind) {
        self.slots.entry(slot).or_insert(SlotDamage {
            kind,
            effective_damage: 0.0,
        });
    }
}

/// Current Wave, previous Wave snapshot, and current Run damage totals.
#[derive(Resource, Debug, Default)]
pub struct DamageStats {
    pub current_wave: DamagePeriod,
    pub last_wave: DamagePeriod,
    pub run: DamagePeriod,
    pub last_wave_completed: bool,
    current_wave_completed: bool,
}

impl DamageStats {
    pub(crate) fn record(&mut self, source: DamageSource, effective_damage: f32) {
        if effective_damage <= 0.0 {
            return;
        }
        self.current_wave.record(source, effective_damage);
        self.run.record(source, effective_damage);
    }

    pub(crate) fn mark_wave_completed(&mut self) {
        self.current_wave_completed = true;
    }
}

pub(crate) fn begin_wave(
    mut stats: ResMut<DamageStats>,
    starting_weapon: Res<crate::weapon::StartingWeapon>,
    weapons: Query<(&WeaponSlot, &crate::weapon::Weapon)>,
) {
    stats.current_wave = DamagePeriod::default();
    stats.current_wave_completed = false;
    if let Some(kind) = starting_weapon.selected {
        stats.current_wave.register_weapon(WeaponSlot(0), kind);
        stats.run.register_weapon(WeaponSlot(0), kind);
    }
    for (slot, weapon) in &weapons {
        stats.current_wave.register_weapon(*slot, weapon.kind);
        stats.run.register_weapon(*slot, weapon.kind);
    }
}

pub(crate) fn finish_wave(mut stats: ResMut<DamageStats>) {
    stats.last_wave = stats.current_wave.clone();
    stats.last_wave_completed = stats.current_wave_completed;
}

pub(crate) fn reset_run(mut stats: ResMut<DamageStats>) {
    *stats = DamageStats::default();
}
