#!/usr/bin/env bash

set -euo pipefail

repo=""
version=""
source_sha=""
output=""
template="Formula/zpic.rb"

usage() {
  cat <<'EOF'
Usage:
  scripts/render-homebrew-formula.sh \
    --repo xtcel/zpic \
    --version 0.1.0 \
    --source-sha <sha256> \
    [--output path]
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --repo)
      repo="$2"
      shift 2
      ;;
    --version)
      version="$2"
      shift 2
      ;;
    --source-sha)
      source_sha="$2"
      shift 2
      ;;
    --output)
      output="$2"
      shift 2
      ;;
    --help|-h)
      usage
      exit 0
      ;;
    *)
      echo "Unknown argument: $1" >&2
      usage >&2
      exit 1
      ;;
  esac
done

if [[ -z "${repo}" || -z "${version}" || -z "${source_sha}" ]]; then
  usage >&2
  exit 1
fi

if [[ ! "${version}" =~ ^[0-9]+\.[0-9]+\.[0-9]+([-.][0-9A-Za-z.]+)?$ ]]; then
  echo "Invalid version: ${version}" >&2
  exit 1
fi

if [[ ! "${source_sha}" =~ ^[0-9a-f]{64}$ ]]; then
  echo "Invalid sha256: ${source_sha}" >&2
  exit 1
fi

rendered="$(
  sed \
    -e "s#@@REPO@@#${repo}#g" \
    -e "s#@@VERSION@@#${version}#g" \
    -e "s#@@SOURCE_SHA256@@#${source_sha}#g" \
    "${template}"
)"

if [[ -n "${output}" ]]; then
  mkdir -p "$(dirname "${output}")"
  printf '%s\n' "${rendered}" > "${output}"
else
  printf '%s\n' "${rendered}"
fi
