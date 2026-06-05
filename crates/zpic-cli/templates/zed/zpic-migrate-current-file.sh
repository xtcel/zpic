#!/usr/bin/env bash
set -euo pipefail

file_path="${1:-}"
zpic_bin="${ZPIC_BIN:-zpic}"

if [[ -z "$file_path" ]]; then
  echo "zpic migrate requires a file path from Zed" >&2
  exit 1
fi

exec "$zpic_bin" migrate "$file_path"
