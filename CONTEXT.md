# CONTEXT — unnamed-game (Brotato-like roguelike)

## Vision
2D top-down, auto-attack horde-survival roguelike modeled on Brotato ("土豆兄弟").
Target platform: WebAssembly (browser). Assets generated via the `mmx` CLI.

## Glossary

- **Player**: the player-controlled avatar. Manual movement (WASD), weapons auto-fire.
- **Weapon**: an auto-attacking loadout slot. Up to N equipped at once (see Weapon Slot).
- **Weapon Slot**: a distinct weapon that fires independently and can be upgraded. Max count TBD.
- **Auto-attack**: weapons target and fire at enemies without manual aiming.
- **Enemy**: a hostile unit spawned in waves that chases/attacks the Player.
- **Wave**: a timed phase during which enemies spawn; ends after a duration or condition (TBD).
- **Material**: dropped by defeated enemies; the currency for the shop. (Brotato: materials)
- **Shop**: between-wave screen where the Player spends Materials to buy items/upgrades.
- **Horde-survival**: the genre; the Player fights off waves of enemies while surviving.
- **Run**: one attempt of the game, from starting a new game to Victory or Defeat. Death ends the Run.
- **Seed**: the initial value of the Run's random number generator. Identical Seeds produce identical Runs (determinism).
- **Asset**: a static image or audio file generated via mmx, stored in `assets/`.

## Decisions (see docs/adr/)
- Use the Bevy engine, pinned to 0.17.
- Render as pure 2D sprites.
- Asset pipeline: mmx generated once, curated into `assets/`, loaded at runtime; `tools/` holds reproducible generation scripts.
- First playable (MVP) must close the full roguelike loop: move + auto-attack + damage/death + materials + waves + shop + upgrades.

## Game systems (settled)

- **Art style**: minimal geometric / cartoon — rounded shapes, high-saturation solid colors, thick outline.
- **Controls**: WASD movement only; weapons auto-aim and auto-fire. No mouse aiming.
- **Enemy model**: 3 types for MVP — melee-rusher, speed-burster, splitter; straight-line pursuit by default.
- **Combat / weapon model**: 3 weapons for MVP — melee swing, piercing projectile, orbiting orb. Projectile + hit-scan model done once; adding weapons later is data-only. Up to 6 Weapon Slots.
- **Economy**: enemies drop Materials → between-wave Shop buys Items (pure stat-boost gains) → character growth. In-wave 3-choice upgrade deferred past MVP.
- **Wave / difficulty**: fixed 20 waves, escalating count/intensity, shop between waves, survive to 20 to win. One-life roguelike (death = restart). Wave count configurable.
- **Audio**: SFX only for MVP (hit / pickup / hurt), via mmx TTS → ogg; BGM deferred.

## Technical decisions (settled)
- **Engine/rendering**: Bevy 0.17, pure 2D sprites. WebAssembly render backend = **WebGPU** (not WebGL2).
- **Build tooling**: Trunk (serves assets + auto-rebuild); wasm32-unknown-unknown target + wasm-bindgen-cli to be installed.
- **Dev loop**: dual-target — `cargo run` for native fast iteration, `trunk` for the wasm browser build.
- **Project structure**: Cargo workspace split into `game-core` (simulation logic) + `app` (bin: rendering, audio, UI) — see ADR 0004.
- **Determinism**: Runs are seeded and reproducible; a single managed RNG with explicit system ordering — see ADR 0005.
- **UI**: Bevy built-in `bevy_ui` for HUD / shop / menu.
- **State machine**: Bevy `States` for MainMenu → InGame(wave) → Shop → … → Victory/Defeat.

## Open questions
- (None — gameplay fully settled. Remaining decisions are technical implementation.)
