#!/usr/bin/env bash
# Regenerate the game's sprite art via mmx, key out the background, and bake
# into the spritesheet atlas (ADR-0002).
#
# Assets are generated ONCE and committed; re-run only when new art is wanted.
# Re-running regenerates EVERY sprite (mmx is non-deterministic — a full run
# will also redraw the player and material gem). To add sprites incrementally,
# run the individual `gen` + colorkey steps by hand and re-run only the atlas
# assembly.
#
# Background-color lessons learned (do not rediscover the hard way):
#   - The model drifts off pure magenta toward pink/red, which is fatal for
#     red/orange sprites (colorkey eats the body). Prompting "no gradient /
#     no glow" makes it worse.
#   - A "green screen" prompt is a strong prior and stays flat; use it for
#     every sprite that isn't itself green. Add `despill` to remove green
#     fringe. Only the green player keeps the magenta background.
#
# Requires `mmx` (authenticated), `ffmpeg`. This script records the pipeline;
# the committed outputs live under assets/sprites/.
#
# Usage:  bash tools/gen_sprites.sh
set -euo pipefail

cd "$(dirname "$0")/.."
OUT=assets/sprites
TMP=tools/out/sprites
mkdir -p "$OUT" "$TMP"

MAGENTA_BG="Minimal geometric cartoon game sprite, top-down 2D, flat solid colors, thick dark outline, high saturation, isolated on a SOLID PURE MAGENTA flat background (#FF00FF), centered, no text, no shadow"
GREEN_BG="Minimal geometric cartoon game sprite, top-down 2D, flat solid colors, thick dark outline, high saturation, centered, no text, no shadow, isolated on a flat pure green screen background (#00FF00) like a chroma key green screen, solid flat green color edge to edge"

gen() { # prefix, bg, subject
  mmx image generate --prompt "$3, $2" --width 512 --height 512 \
    --out-dir "$TMP" --out-prefix "$1" --quiet
}

gen player        "$MAGENTA_BG" "cute round hero character, green"
gen material      "$MAGENTA_BG" "small purple glowing crystal gem pickup item"
gen enemy_rusher  "$GREEN_BG" "angry bright red blob monster enemy, vivid saturated pure red body"
gen enemy_burster "$GREEN_BG" "fast bright orange teardrop monster enemy, vivid saturated pure orange body"
gen enemy_splitter "$GREEN_BG" "grey-brown rocky splitter monster enemy with light grey highlights"
gen projectile    "$GREEN_BG" "small golden yellow energy bolt bullet with a bright glowing core and a short motion trail, tip pointing right, game projectile"
gen orb           "$GREEN_BG" "cyan glowing energy orb sphere, game projectile orb"
gen icon_pierce   "$GREEN_BG" "sharp steel arrow bolt with a crosshair symbol, piercing weapon icon"
gen icon_melee    "$GREEN_BG" "curved sword blade slash arc, melee weapon icon"
gen icon_orb      "$GREEN_BG" "cyan glowing energy orb with an orbit ring around it, orbiting weapon icon"

# Key out each sprite's background. mmx backgrounds drift from the requested
# hue, so sample the corner pixel of each source and key on THAT. Green-screen
# sprites additionally get `despill` to kill green fringe.
key() { # prefix, extra-vf
  local src hex
  src=$(ls "$TMP"/$1*.jpg | head -1)
  hex=$(ffmpeg -loglevel error -i "$src" -vf "crop=1:1:2:2" -frames:v 1 \
        -f rawvideo -pix_fmt rgb24 - | od -An -tx1 | tr -d ' \n')
  ffmpeg -y -i "$src" \
    -vf "scale=128:128:flags=lanczos,colorkey=0x$hex:0.18:0.06$2" \
    "$OUT/$1.png"
}

key player        ""
key material      ""
key enemy_rusher  ",despill=type=green"
key enemy_burster ",despill=type=green"
key enemy_splitter ",despill=type=green"
key projectile    ",despill=type=green"
key orb           ",despill=type=green"
key icon_pierce   ",despill=type=green"
key icon_melee    ""
key icon_orb      ",despill=type=green"

# Melee swing arc (atlas cell 5): a thin translucent white ring, drawn
# procedurally with ffmpeg's geq (no mmx needed — the hitbox is a plain
# circle around the player, so the visual must be exact, not generated).
# Ring: center 64,64, inner radius 48, outer 60, 1px alpha ramp on each edge.
ffmpeg -y -f lavfi -i "color=black@0.0:s=128x128:d=1,format=rgba" \
  -vf "geq=r='255':g='255':b='255':a='if(between(hypot(X-63.5,Y-63.5),48,60),255*clip(min(60-hypot(X-63.5,Y-63.5),hypot(X-63.5,Y-63.5)-48),0,1),0)'" \
  -frames:v 1 "$OUT/melee_arc.png"

# Build a 4x4 atlas. Cell layout (row-major) must match
# crates/app/src/game/assets.rs `atlas_index`.
#
# The output filename carries a version suffix (atlas-v2.png): trunk serves
# assets with no Cache-Control, so browsers heuristically cache the old file
# and serve stale art against the new grid layout. Bump the version here AND
# in assets.rs whenever the atlas is regenerated.
#   row 0: player, rusher, burster, splitter
#   row 1: material, melee arc, projectile, orb
#   row 2: icon_pierce, icon_melee, icon_orb, (transparent)
#   row 3: (transparent)
ffmpeg -y \
  -i "$OUT/player.png" -i "$OUT/enemy_rusher.png" -i "$OUT/enemy_burster.png" -i "$OUT/enemy_splitter.png" \
  -i "$OUT/material.png" -i "$OUT/melee_arc.png" -i "$OUT/projectile.png" -i "$OUT/orb.png" \
  -i "$OUT/icon_pierce.png" -i "$OUT/icon_melee.png" -i "$OUT/icon_orb.png" \
  -f lavfi -i "color=black@0.0:s=128x128:d=1,format=rgba" \
  -filter_complex "\
[0]scale=128:128[p];[1]scale=128:128[r];[2]scale=128:128[b];[3]scale=128:128[s];\
[4]scale=128:128[m];[5]scale=128:128[a];[6]scale=128:128[j];[7]scale=128:128[o];\
[8]scale=128:128[i1];[9]scale=128:128[i2];[10]scale=128:128[i3];[11]format=rgba[t];\
[p][r][b][s]hstack=inputs=4[row0];\
[m][a][j][o]hstack=inputs=4[row1];\
[i1][i2][i3][t]hstack=inputs=4[row2];\
[row0][row1][row2]vstack=inputs=3[stacked];\
[stacked]pad=512:512:0:0:black@0.0,format=rgba[atlas]" \
  -map "[atlas]" -frames:v 1 "$OUT/atlas-v2.png"

echo "Sprites baked into $OUT/atlas-v2.png"
