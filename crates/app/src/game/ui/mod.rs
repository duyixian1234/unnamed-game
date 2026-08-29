//! UI screens built on `bevy_ui` (HUD, menus, shop). Pure presentation:
//! state navigation is written directly as `NextState`, purchases go through
//! the core `PurchaseRequest` message.

pub mod end_screen;
pub mod hud;
pub mod main_menu;
pub mod shop;
pub mod starting_weapon;
pub mod upgrade;
pub mod weapon_bar;

use bevy::prelude::*;

use game_core::damage::{DamagePeriod, DamageStats, WeaponSlot};
use game_core::weapon::MAX_WEAPON_SLOTS;

/// Plugin for all UI screens.
pub struct UIPlugin;

impl Plugin for UIPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            end_screen::EndScreenPlugin,
            hud::HudPlugin,
            main_menu::MainMenuPlugin,
            shop::ShopPlugin,
            starting_weapon::StartingWeaponPlugin,
            upgrade::UpgradeScreenPlugin,
            weapon_bar::WeaponBarPlugin,
        ));
    }
}

/// Marker for a UI screen root node, used to clean up screens on state exit.
#[derive(Component)]
pub struct ScreenRoot;

/// The subsetted Chinese UI font (ADR-0007). `AssetServer.load` dedups by
/// path, so calling this per screen is cheap and avoids load-order hazards.
pub fn ui_font(asset_server: &AssetServer, font_size: f32) -> TextFont {
    TextFont {
        font: asset_server.load("fonts/ui.ttf"),
        font_size,
        ..default()
    }
}

/// Format the stable slot-ordered damage summary shared by upgrade and end
/// screens. Empty slots stay hidden; non-zero Other damage remains visible.
pub fn damage_summary_text(stats: &DamageStats, incomplete_label: bool) -> String {
    let mut lines = Vec::new();
    let wave_label = if incomplete_label {
        "本波（未完成）"
    } else {
        "本波"
    };
    lines.push(format!(
        "伤害统计　{}主数据　·　整局累计次级数据",
        wave_label
    ));

    let mut has_slot = false;
    for index in 0..MAX_WEAPON_SLOTS as u8 {
        let last = stats.last_wave.slot(WeaponSlot(index));
        let run = stats.run.slot(WeaponSlot(index));
        let Some(slot) = last.or(run) else {
            continue;
        };
        has_slot = true;
        let last_damage = last.map_or(0.0, |value| value.effective_damage);
        let run_damage = run.map_or(0.0, |value| value.effective_damage);
        lines.push(format!(
            "槽位{} · {}　{}：{}　累计：{}",
            index + 1,
            slot.kind.display_name(),
            wave_label,
            format_damage(&stats.last_wave, last_damage),
            format_damage(&stats.run, run_damage),
        ));
    }

    if stats.last_wave.other > 0.0 || stats.run.other > 0.0 {
        has_slot = true;
        lines.push(format!(
            "其他伤害　{}：{}　累计：{}",
            wave_label,
            format_damage(&stats.last_wave, stats.last_wave.other),
            format_damage(&stats.run, stats.run.other),
        ));
    }

    if !has_slot {
        lines.push("暂无伤害数据".to_string());
    }
    lines.join("\n")
}

fn format_damage(period: &DamagePeriod, damage: f32) -> String {
    if period.total() <= 0.0 {
        "0 伤害 · 0%".to_string()
    } else {
        format!(
            "{} 伤害 · {:.1}%",
            damage.round() as i32,
            period.percentage(damage)
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_damage_uses_stable_percentage_format() {
        assert_eq!(format_damage(&DamagePeriod::default(), 0.0), "0 伤害 · 0%");
    }
}
