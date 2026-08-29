//! Weapon-slot HUD: a bottom-center row of icons, one per equipped weapon.
//! Re-syncs every frame in append-only fashion, so it also repopulates after
//! the HUD is respawned on Shop → InGame re-entry (weapons outlive the HUD).

use std::collections::HashSet;

use bevy::prelude::*;

use game_core::weapon::Weapon;
use game_core::GameState;

use super::ScreenRoot;
use crate::game::assets::{atlas_image, SpriteAssets};

/// Icon size in px on screen (icons are 128px atlas cells).
const ICON_PX: f32 = 44.0;

/// Root marker for the weapon bar container.
#[derive(Component)]
struct WeaponBarRoot;

/// An icon node spawned for the weapon entity it represents.
#[derive(Component)]
struct WeaponIcon(Entity);

/// Plugin for the weapon bar.
pub struct WeaponBarPlugin;

impl Plugin for WeaponBarPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::InGame), spawn_weapon_bar)
            .add_systems(OnExit(GameState::InGame), despawn_weapon_bar)
            .add_systems(Update, sync_weapon_bar.run_if(in_state(GameState::InGame)));
    }
}

fn spawn_weapon_bar(mut commands: Commands) {
    commands.spawn((
        WeaponBarRoot,
        ScreenRoot,
        Node {
            position_type: PositionType::Absolute,
            bottom: Val::Px(16.0),
            width: Val::Percent(100.0),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            column_gap: Val::Px(10.0),
            ..default()
        },
    ));
}

fn despawn_weapon_bar(mut commands: Commands, roots: Query<Entity, With<WeaponBarRoot>>) {
    for root in &roots {
        commands.entity(root).despawn();
    }
}

/// Append an icon for every weapon that doesn't have one yet.
fn sync_weapon_bar(
    mut commands: Commands,
    sprite_assets: Res<SpriteAssets>,
    bars: Query<Entity, With<WeaponBarRoot>>,
    weapons: Query<(Entity, &Weapon)>,
    icons: Query<&WeaponIcon>,
) {
    let Ok(bar) = bars.single() else {
        return;
    };
    let tagged: HashSet<Entity> = icons.iter().map(|icon| icon.0).collect();
    for (entity, weapon) in &weapons {
        if tagged.contains(&entity) {
            continue;
        }
        let index = SpriteAssets::weapon_icon_index(weapon.kind);
        commands.entity(bar).with_child((
            WeaponIcon(entity),
            Node {
                width: Val::Px(ICON_PX),
                height: Val::Px(ICON_PX),
                ..default()
            },
            atlas_image(&sprite_assets, index),
        ));
    }
}
