//! Seeded RNG (ADR-0005): one managed `GlobalRng` for all randomness.

use bevy::prelude::*;
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;

/// The seed of the current Run. Identical seeds produce identical Runs.
/// Injected by tests before `CorePlugin`; production (`app`) parses it from
/// the CLI/env and logs it at startup.
#[derive(Resource, Debug, Clone, Copy)]
pub struct Seed(pub u64);

/// The single managed RNG. Every random draw in the simulation comes from
/// here — never from a fresh thread-local generator.
#[derive(Resource)]
pub struct GlobalRng(pub ChaCha8Rng);

/// Marker set for systems that consume RNG.
///
/// Discipline (ADR-0005): Bevy's multithreaded scheduler does not guarantee
/// execution order of unordered systems, and a global RNG stream is
/// order-sensitive. Any system that draws from `GlobalRng` MUST be a member
/// of this set, so RNG consumers stay explicitly ordered with each other.
/// Integration tests additionally run the scheduler single-threaded.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RandomDraw;

/// Install `Seed` (generating a random one when absent) and derive
/// `GlobalRng` from it. Called by `CorePlugin::build`.
pub fn init_rng(app: &mut App) {
    if app.world().get_resource::<Seed>().is_none() {
        let nanos = web_time::SystemTime::now()
            .duration_since(web_time::SystemTime::UNIX_EPOCH)
            .expect("system clock before UNIX_EPOCH")
            .as_nanos() as u64;
        app.insert_resource(Seed(nanos));
    }
    let seed = app.world().resource::<Seed>().0;
    app.insert_resource(GlobalRng(ChaCha8Rng::seed_from_u64(seed)));
}
