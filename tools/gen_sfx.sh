#!/usr/bin/env bash
# Regenerate the game's SFX via mmx TTS, then convert to ogg (ADR-0002).
#
# Assets are generated ONCE and committed; re-running this script only when new
# sounds are actually wanted. Requires `mmx` (authenticated) and `ffmpeg`.
#
# Usage:  bash tools/gen_sfx.sh
set -euo pipefail

cd "$(dirname "$0")/.."
OUT=assets/audio/sfx
TMP=tools/out/sfx
mkdir -p "$OUT" "$TMP"

# name, spoken text
sounds=(
  "hit      Tik"
  "pickup   Pop"
  "hurt     Ouch"
)

for entry in "${sounds[@]}"; do
  read -r name text <<<"$entry"
  echo "== $name =="
  mmx speech synthesize \
    --text "$text" \
    --speed 1.0 \
    --format mp3 \
    --out "$TMP/$name.mp3" \
    --quiet
  ffmpeg -y -i "$TMP/$name.mp3" -codec:a libvorbis -qscale:a 5 "$OUT/$name.ogg" >/dev/null 2>&1
done

echo "SFX generated into $OUT"
