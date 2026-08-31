# Settings persistence via `bevy_persistent`

Status: accepted, **not yet implemented** — scheduled as the Settings-screen issue (#22). No `bevy_persistent` or `serde` dependency has been added to `Cargo.toml` yet.

Settings (SFX mute, SFX volume, Diagnostics Overlay toggle) must survive a reload, and the game targets both wasm (localStorage) and native (a file). We adopted `bevy_persistent` 0.9.0, which ships that split itself — `LocalStorage` via `gloo-storage` on wasm, `Filesystem` on native — instead of hand-rolling the project's first `cfg(target_arch = "wasm32")` block. 0.9.0 is pinned because it is the only release whose Bevy bound is `^0.17`; 0.10/0.11 target Bevy 0.18/0.19.

Considered alternatives: hand-rolling `web-sys` localStorage plus `std::fs` behind a `cfg` was rejected as roughly 40–60 lines of platform plumbing whose only payoff is avoiding a small dependency; not persisting at all was rejected because volume is a set-once-and-forget value and resetting it every launch is aggravating.

Consequences, several of them non-obvious:

- The crate requires `serde` with `derive` (the workspace's first serde dependency) and **exactly one** format feature; it emits `compile_error!` if none is enabled. We use `json`.
- On wasm the storage path string **must begin with `local` or `session`** or the builder panics. This is invisible at the call site and is the single most likely way to break this later.
- Saves are explicit: only `set()`, `update()`, and `persist()` write. Mutating through `DerefMut` silently does not save. Because the volume control is discrete (`−`/`+` in 10% steps) we save on every change; a continuous slider would have needed debouncing.
- `Persistent<R>` is not a plugin and registers no systems — the resource is inserted manually.
- **Fullscreen is deliberately not persisted.** Browsers require a user gesture to enter fullscreen, so restoring it at startup would either fail silently (panel shows "on", screen says otherwise) or force a startup failure path. It is session state, not a preference.
- A failed or corrupt load reverts to defaults and logs a warning rather than surfacing UI: settings are not a save file, and the game stays fully playable.
