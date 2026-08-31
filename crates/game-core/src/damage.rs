//! Effective-damage attribution and Wave/Run contribution snapshots.

use std::collections::BTreeMap;

use bevy::prelude::*;

use crate::upgrade::{path_for, Evolution};
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

impl DamageSource {
    /// The weapon slot this damage came from, if any.
    pub fn slot(self) -> Option<WeaponSlot> {
        match self {
            DamageSource::Weapon { slot, .. } => Some(slot),
            DamageSource::Other => None,
        }
    }
}

/// Effective Damage attributed to one Weapon Slot in a period.
#[derive(Debug, Clone, Copy)]
pub struct SlotDamage {
    pub kind: WeaponKind,
    /// Set when the slot's weapon reached Level 8, the Evolution level
    /// (ADR-0008): summaries
    /// display the evolution's name instead of the base kind's.
    pub evolution: Option<Evolution>,
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
                    evolution: None,
                    effective_damage: 0.0,
                });
                entry.effective_damage += effective_damage;
            }
            DamageSource::Other => self.other += effective_damage,
        }
    }

    /// Register (or refresh) a slot's identity. Public so tests can set up
    /// scenario summaries directly (same pattern as `WeaponLevels::set_level`).
    pub fn register_weapon(
        &mut self,
        slot: WeaponSlot,
        kind: WeaponKind,
        evolution: Option<Evolution>,
    ) {
        let entry = self.slots.entry(slot).or_insert(SlotDamage {
            kind,
            evolution,
            effective_damage: 0.0,
        });
        // Re-registering always refreshes the identity: a slot's weapon may
        // have evolved since the period began (e.g. the run total).
        entry.kind = kind;
        entry.evolution = evolution;
    }

    /// Fold `from`'s contribution into `into` and drop the `from` entry.
    fn merge_slot(&mut self, from: WeaponSlot, into: WeaponSlot) {
        let Some(removed) = self.slots.remove(&from) else {
            return;
        };
        let entry = self.slots.entry(into).or_insert(SlotDamage {
            kind: removed.kind,
            evolution: removed.evolution,
            effective_damage: 0.0,
        });
        entry.effective_damage += removed.effective_damage;
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

    /// Fold every `from` slot's contribution into `into` and remove the
    /// `from` entries, so a weapon merged away (Whirlwind evolution) keeps
    /// its accumulated history on the surviving slot instead of leaving
    /// ghost rows in summaries.
    pub(crate) fn merge_slots(&mut self, from: &[WeaponSlot], into: WeaponSlot) {
        for slot in from {
            self.current_wave.merge_slot(*slot, into);
            self.run.merge_slot(*slot, into);
        }
    }

    pub(crate) fn mark_wave_completed(&mut self) {
        self.current_wave_completed = true;
    }
}

pub(crate) fn begin_wave(
    mut stats: ResMut<DamageStats>,
    starting_weapon: Res<crate::weapon::StartingWeapon>,
    weapons: Query<(
        &WeaponSlot,
        &crate::weapon::Weapon,
        Option<&crate::upgrade::Evolved>,
    )>,
) {
    stats.current_wave = DamagePeriod::default();
    stats.current_wave_completed = false;
    if let Some(kind) = starting_weapon.selected {
        stats
            .current_wave
            .register_weapon(WeaponSlot(0), kind, None);
        stats.run.register_weapon(WeaponSlot(0), kind, None);
    }
    for (slot, weapon, evolved) in &weapons {
        let evolution = evolved.map(|_| path_for(weapon.kind).evolution);
        stats
            .current_wave
            .register_weapon(*slot, weapon.kind, evolution);
        stats.run.register_weapon(*slot, weapon.kind, evolution);
    }
}

pub(crate) fn finish_wave(mut stats: ResMut<DamageStats>) {
    stats.last_wave = stats.current_wave.clone();
    stats.last_wave_completed = stats.current_wave_completed;
}

pub(crate) fn reset_run(mut stats: ResMut<DamageStats>) {
    *stats = DamageStats::default();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::weapon::WeaponKind;

    #[test]
    fn other_bucket_accumulates_separately_from_weapon_slots() {
        // No gameplay source emits `DamageSource::Other` yet, so this is the
        // only way to exercise the bucket directly. Regression guard for the
        // "Other" row in the damage summary (issue #20, gap C): its total must
        // fold in non-weapon damage without mixing it into any slot.
        let mut stats = DamageStats::default();
        stats.record(DamageSource::Other, 7.0);
        stats.record(
            DamageSource::Weapon {
                slot: WeaponSlot(0),
                kind: WeaponKind::OrbitingOrb,
            },
            3.0,
        );

        assert_eq!(
            stats.current_wave.other, 7.0,
            "Other stays in its own bucket"
        );
        assert_eq!(
            stats.run.other, 7.0,
            "Other is tracked on the run total too"
        );
        assert_eq!(stats.current_wave.total(), 10.0, "total folds in Other");
        assert_eq!(
            stats
                .current_wave
                .slot(WeaponSlot(0))
                .unwrap()
                .effective_damage,
            3.0,
            "weapon slot is untouched by Other damage"
        );
    }

    #[test]
    fn non_positive_damage_is_not_recorded() {
        let mut stats = DamageStats::default();
        stats.record(DamageSource::Other, 0.0);
        stats.record(
            DamageSource::Weapon {
                slot: WeaponSlot(0),
                kind: WeaponKind::OrbitingOrb,
            },
            -4.0,
        );
        assert_eq!(stats.current_wave.total(), 0.0);
    }
}
