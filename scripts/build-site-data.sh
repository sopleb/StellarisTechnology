#!/usr/bin/env bash
# Regenerate the viewer's datasets in site/data from the local Stellaris install.
# Usage: scripts/build-site-data.sh [--lang english]   (extra args go to every run)
# Add more datasets by appending `gen <id> <label> <stellaris-tech args...>` lines below.
set -euo pipefail
cd "$(dirname "$0")/.."
cargo build --release --quiet
BIN=target/release/stellaris-tech
OUT=site/data
mkdir -p "$OUT"
EXTRA=("$@")
entries=()

gen() {
  local id=$1 label=$2; shift 2
  if "$BIN" "${EXTRA[@]}" -o "$OUT/$id.json" "$@"; then
    local ver
    ver=$(jq -r '.meta.game_version // ""' "$OUT/$id.json")
    entries+=("$(jq -cn --arg id "$id" --arg label "$label ${ver}" --arg file "$id.json" '{id:$id,label:$label,file:$file}')")
  else
    echo "skipped $id" >&2
  fi
}

gen vanilla "Vanilla"
gen gigastructures "GigaStructures" --mod 1121692237

printf '%s\n' "${entries[@]}" | jq -s '.' > "$OUT/datasets.json"
echo "wrote $OUT/datasets.json (${#entries[@]} datasets)"
