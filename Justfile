# Justfile — dev / build / deploy shortcuts for the unnamed-game workspace.
# Layout notes (ADR-0003 / ADR-0004):
#   - trunk must run from crates/app (Trunk.toml lives there)
#   - trunk writes dist output to repo-root dist/, which is what gets deployed
#   - deploy target is Cloudflare Pages (project: unnamed-game, <=25 MiB limit)
#
# Shell is pwsh (PowerShell 7+).
# The tools/gen_*.sh scripts stay bash and are invoked via `bash`.

set shell := ["pwsh", "-NoProfile", "-Command"]

default := "dev"

# List available recipes
@list:
    just --list

# ---- Development -----------------------------------------------------------

# Wasm browser dev loop: serves at http://127.0.0.1:8080 + auto-builds/reloads on file change
# CARGO_INCREMENTAL=0: wasm incremental cache on Windows triggers rustc os error 5
# ("did not finalize incremental compilation session directory") and adds little.
@dev:
    $env:CARGO_INCREMENTAL='0'; cd crates/app; trunk serve --port 8080

# Native dev loop (fastest iteration path, ADR-0003).
# BEVY_ASSET_ROOT: bevy 0.17 resolves the native asset root to the crate dir
# (crates/app), but assets live at the repo root — point it back explicitly.
@dev-native:
    $env:BEVY_ASSET_ROOT=(Get-Location).Path; cargo run -p unnamed-game

# Alias for `dev` (wasm browser dev server on 8080)
@serve: dev

# ---- Checks ----------------------------------------------------------------

# Format + lint + test (mirrors .github/workflows/ci.yml)
@ci: fmt-check clippy test

# Run all workspace tests (game-core headless integration tests included)
@test:
    cargo test --workspace

# Clippy across the workspace
@clippy:
    cargo clippy --workspace --all-targets -- -D warnings

# Check formatting without writing
@fmt-check:
    cargo fmt --all -- --check

# Auto-format the workspace
@fmt:
    cargo fmt --all

# ---- Build -----------------------------------------------------------------

# Native release build
@build-native:
    cargo build --workspace --release

# Wasm release build into dist/ (deploy artifact).
# wasm-opt runs manually afterwards: trunk 0.21's built-in wasm-opt step has a
# Windows path bug ("error copying (optimized) wasm file", os error 3), and its
# level is only configurable via a data-wasm-opt attribute we don't want.
@build-web:
    cd crates/app; trunk build --release
    $w = Get-Item dist/*_bg.wasm; wasm-opt -Os $w.FullName -o "$($w.FullName).opt"; Move-Item -Force "$($w.FullName).opt" $w.FullName
    Get-ChildItem dist | Select-Object Name, Length

# Build web + fail if the wasm bundle exceeds the 25 MiB Pages limit
@build-web-checked: build-web
    @python -c "import os,glob; p=glob.glob('dist/*_bg.wasm')[0]; s=os.path.getsize(p); print(f'wasm: {s/1048576:.1f} MiB'); exit(1 if s > 25*1048576 else 0)"

# ---- Deploy ----------------------------------------------------------------

# Deploy dist/ to Cloudflare Pages: build (incl. manual wasm-opt -Os) + 25 MiB
# size gate, so an oversized bundle can never reach Pages (auth via wrangler login)
@deploy: build-web-checked
    npx wrangler pages deploy dist --project-name=unnamed-game

# Deploy the freshly built dist/ as a Production deployment (Pages production
# branch is "main"; the repo itself only has master, so we label the upload).
# Skips the rebuild when dist/ is current — run `just build-web-checked` first.
@deploy-prod:
    npx wrangler pages deploy dist --project-name=unnamed-game --branch=main

# ---- Assets (mmx-generated sprites/sfx + subsetted font, ADR-0002/0007) ----

# Regenerate sfx + sprites via the mmx CLI, and the subsetted UI font
@assets:
    bash tools/gen_sfx.sh
    bash tools/gen_sprites.sh
    bash tools/gen_font.sh

# ---- Misc ------------------------------------------------------------------

# Remove build artifacts (dist/, target/ is left for cargo to manage)
@clean-dist:
    if (Test-Path dist) { Remove-Item -Recurse -Force dist -Confirm:$false }
    New-Item -ItemType Directory -Force dist/assets | Out-Null

# Full clean: cargo target dir + dist
@clean: clean-dist
    cargo clean
