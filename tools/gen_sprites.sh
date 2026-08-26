#!/usr/bin/env bash
# Regenerate the game's sprite art via mmx, key out the magenta background, and
# bake into the spritesheet atlas (ADR-0002).
#
# Assets are generated ONCE and committed; re-run only when new art is wanted.
# Requires `mmx` (authenticated), `ffmpeg`, and ImageMagick's `magick` or the
# ffmpeg overlay pipeline. This script records the pipeline; the committed
# outputs live under assets/sprites/.
#
# Usage:  bash tools/gen_sprites.sh
set -euo pipefail

cd "$(dirname "$0")/.."
OUT=assets/sprites
TMP=tools/out/sprites
mkdir -p "$OUT" "$TMP"

BG_PROMPT="Minimal geometric cartoon game sprite, top-down 2D, flat solid colors, thick dark outline, high saturation, isolated on a SOLID PURE MAGENTA background (#FF00FF), centered, no text, no shadow"

gen() { # prefix, prompt
  mmx image generate --prompt "$2" --width 512 --height 512 \
    --out-dir "$TMP" --out-prefix "$1" --quiet
}

gen player   "cute round hero character, green $BG_PROMPT"
gen enemy_rusher   "angry red blob monster enemy, $BG_PROMPT"
gen enemy_burster  "fast orange teardrop monster enemy, $BG_PROMPT"
gen enemy_splitter "brown splitter rock monster enemy, $BG_PROMPT"
gen material "small purple glowing crystal gem pickup item, $BG_PROMPT"

# Key out each sprite's background. mmx yields a solid pink/coral background;
# sample one corner pixel per sprite and use it as the colorkey.
for p in player enemy_rusher enemy_burster enemy_splitter material; do
  src=$(ls "$TMP"/${p}*.jpg | head -1)
  ffmpeg -y -i "$src" -vf "scale=128:128:flags=lanczos,colorkey=0xFF00FF:0.30:0.08" "$OUT/$p.png"
done

# Build a 3x2 atlas (384x256): player, rusher, burster / splitter, material.
ffmpeg -y \
  -i "$OUT/player.png" -i "$OUT/enemy_rusher.png" -i "$OUT/enemy_burster.png" \
  -i "$OUT/enemy_splitter.png" -i "$OUT/material.png" \
  -filter_complex "[0]scale=128:128[p];[1]scale=128:128[r];[2]scale=128:128[b];[3]scale=128:128[s];[4]scale=128:128[m];[p][r]hstack[top0];[top0][b]hstack[top];[s][m]hstack[bot];[bot]pad=384:128:0:0:color=#00000000[botpad];[top][botpad]vstack[atlas]" \
  -map "[atlas]" "$OUT/atlas.png"

echo "Sprites baked into $OUT/atlas.png"
