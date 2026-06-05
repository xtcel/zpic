#!/usr/bin/env bash
set -euo pipefail

format="${1:-markdown}"
selection="${2:-}"
zpic_bin="${ZPIC_BIN:-zpic}"

normalize_text() {
  printf '%s' "$1" | tr '\r\n\t' '   ' | sed 's/[[:space:]]\+/ /g; s/^ //; s/ $//'
}

selection="$(normalize_text "$selection")"
args=(upload --clipboard --format "$format" --copy)

if [[ -n "$selection" ]]; then
  safe_name="$(printf '%s' "$selection" \
    | tr '[:upper:]' '[:lower:]' \
    | sed 's/[^[:alnum:]._-]\+/-/g; s/^-*//; s/-*$//' \
    | cut -c1-80)"
  if [[ -n "$safe_name" ]]; then
    args+=(--name "$safe_name")
  fi
  args+=(--alt "$selection")
fi

exec "$zpic_bin" "${args[@]}"
