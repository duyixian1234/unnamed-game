//! game-core — the simulation layer of a Brotato-like horde-survival
//! roguelike (ADR-0004). Contains the state machine, waves, combat, economy,
//! enemies, player, weapons, the seeded RNG, the intent layer, and a
//! test-only AI. No rendering, windowing, or audio lives here: the `app`
//! crate reads core state and attaches presentation.

pub mod ai;
pub mod combat;
pub mod damage;
pub mod economy;
pub mod enemy;
pub mod intent;
pub mod player;
pub mod rng;
pub mod shop;
pub mod upgrade;
pub mod waves;
pub mod weapon;

use bevy::ecs::message::Message;
use bevy::prelude::*;

use self::combat::CombatPlugin;
use self::economy::EconomyPlugin;
use self::enemy::EnemyPlugin;
use self::player::PlayerPlugin;
use self::shop::ShopPlugin;
use self::upgrade::UpgradePlugin;
use self::waves::WavesPlugin;
use self::weapon::WeaponPlugin;

/// The high-level game state machine. Drives the whole roguelike loop:
/// MainMenu → StartingWeaponChoice → InGame(wave) → UpgradeChoice → Shop →
/// InGame … → Victory / Defeat.
#[derive(States, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum GameState {
    /// Title screen with a Start button.
    #[default]
    MainMenu,
    /// Choose the one Weapon used for the current Run.
    StartingWeaponChoice,
    /// An active wave of combat (movement + auto-attack + spawning).
    InGame,
    /// Between-wave weapon upgrade pick (one mandatory choice per wave).
    UpgradeChoice,
    /// Between-wave shop where the player spends Materials on items.
    Shop,
    /// Survived all waves.
    Victory,
    /// Player died (one-life roguelike).
    Defeat,
}

/// Whether the simulation is frozen (CONTEXT.md: 暂停).
///
/// A simulation concern, not a presentation one: pausing has to stop the wave
/// clock and the RNG draws, so it lives here rather than in `app`. Only the
/// simulation stops — the HUD and the frozen field stay on screen.
#[derive(Resource, Default, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Paused(pub bool);

/// Run condition for simulation systems: a wave is live and not paused.
///
/// Named for what it guards rather than for waves alone — it also gates player
/// movement, pickups, and hitbox lifetimes. Declared once so every simulation
/// system freezes together, instead of each growing its own
/// `in_state(..) && !paused` check that one of them can forget.
pub fn sim_running(state: Res<State<GameState>>, paused: Res<Paused>) -> bool {
    *state.get() == GameState::InGame && !paused.0
}

/// How a Run ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunOutcome {
    Victory,
    Defeat,
}

/// Fired when a fresh Run begins (first wave entered after a reset).
#[derive(Message)]
pub struct RunStarted;

/// Fired when a Run ends, with its outcome.
#[derive(Message)]
pub struct RunEnded {
    pub outcome: RunOutcome,
}

/// Top-level plugin for the simulation. Presentation-free: tests add this to
/// a headless `App`; the `app` crate adds it alongside `DefaultPlugins`.
pub struct CorePlugin;

impl Plugin for CorePlugin {
    fn build(&self, app: &mut App) {
        // Seed/GlobalRng must exist before any system runs (ADR-0005).
        rng::init_rng(app);
        app.init_state::<GameState>()
            .init_resource::<intent::PlayerMoveIntent>()
            .init_resource::<damage::DamageStats>()
            .init_resource::<Paused>()
            .add_message::<RunStarted>()
            .add_message::<RunEnded>()
            .add_plugins((
                CombatPlugin,
                EconomyPlugin,
                EnemyPlugin,
                PlayerPlugin,
                ShopPlugin,
                UpgradePlugin,
                WavesPlugin,
                WeaponPlugin,
            ))
            .add_systems(
                OnEnter(GameState::StartingWeaponChoice),
                (damage::reset_run, intent::reset_move_intent),
            )
            .add_systems(OnEnter(GameState::InGame), damage::begin_wave)
            .add_systems(OnExit(GameState::InGame), damage::finish_wave)
            .add_systems(OnEnter(GameState::Victory), report_victory)
            .add_systems(OnEnter(GameState::Defeat), report_defeat);
    }
}

fn report_victory(mut writer: MessageWriter<RunEnded>) {
    writer.write(RunEnded {
        outcome: RunOutcome::Victory,
    });
}

fn report_defeat(mut writer: MessageWriter<RunEnded>) {
    writer.write(RunEnded {
        outcome: RunOutcome::Defeat,
    });
}
