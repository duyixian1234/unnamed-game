#!/usr/bin/env bash
# Generate the subsetted Chinese UI font (ADR-0007).
#
# Downloads Noto Sans SC once, then subsets it to exactly the glyphs used by
# the UI: every rendered string lives in tools/ui_text.txt (add new UI copy
# there and re-run this script), plus ASCII so dynamic numbers/punctuation in
# format! strings always render. Output is committed as a static asset
# (ADR-0002) — it must stay well under the 25 MiB deploy limit (ADR-0006).
#
# Requires: python + fonttools (pip install fonttools), curl.
#
# Usage:  bash tools/gen_font.sh
set -euo pipefail

cd "$(dirname "$0")/.."
SRC=tools/out/NotoSansSC-Regular.otf
OUT=assets/fonts/ui.ttf
mkdir -p tools/out assets/fonts

if [ ! -f "$SRC" ]; then
  # github.com resets connections on this network; jsdelivr's GH mirror works.
  curl -L --fail -o "$SRC" \
    https://cdn.jsdelivr.net/gh/notofonts/noto-cjk@main/Sans/SubsetOTF/SC/NotoSansSC-Regular.otf
fi

pyftsubset() { python -m fontTools.subset "$@"; }
pyftsubset "$SRC" \
  --text-file=tools/ui_text.txt \
  --unicodes="U+0020-007E,U+00B7,U+2014,U+2026" \
  --output-file="$OUT" \
  --layout-features='' \
  --no-hinting \
  --desubroutinize

ls -l "$OUT"
echo "UI font subset written to $OUT"
