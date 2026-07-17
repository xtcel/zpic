#!/usr/bin/env bash

set -euo pipefail

repo=""
version=""
macos_arm64_sha=""
macos_x86_64_sha=""
linux_x86_64_sha=""
output=""
template="Formula/zpic.rb"

usage() {
  cat <<'EOF'
Usage:
  scripts/render-homebrew-formula.sh \
    --repo xtcel/zpic \
    --version 0.1.0 \
    --macos-arm64-sha <sha256> \
    --macos-x86_64-sha <sha256> \
    --linux-x86_64-sha <sha256> \
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
    --macos-arm64-sha)
      macos_arm64_sha="$2"
      shift 2
      ;;
    --macos-x86_64-sha)
      macos_x86_64_sha="$2"
      shift 2
      ;;
    --linux-x86_64-sha)
      linux_x86_64_sha="$2"
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

if [[ -z "${repo}" || -z "${version}" || -z "${macos_arm64_sha}" || -z "${macos_x86_64_sha}" || -z "${linux_x86_64_sha}" ]]; then
  usage >&2
  exit 1
fi

if [[ ! "${version}" =~ ^[0-9]+\.[0-9]+\.[0-9]+([-.][0-9A-Za-z.]+)?$ ]]; then
  echo "Invalid version: ${version}" >&2
  exit 1
fi

for sha in "${macos_arm64_sha}" "${macos_x86_64_sha}" "${linux_x86_64_sha}"; do
  if [[ ! "${sha}" =~ ^[0-9a-f]{64}$ ]]; then
    echo "Invalid sha256: ${sha}" >&2
    exit 1
  fi
done

rendered="$(
  sed \
    -e "s#@@REPO@@#${repo}#g" \
    -e "s#@@VERSION@@#${version}#g" \
    -e "s#@@MACOS_ARM64_SHA256@@#${macos_arm64_sha}#g" \
    -e "s#@@MACOS_X86_64_SHA256@@#${macos_x86_64_sha}#g" \
    -e "s#@@LINUX_X86_64_SHA256@@#${linux_x86_64_sha}#g" \
    "${template}"
)"

if [[ -n "${output}" ]]; then
  mkdir -p "$(dirname "${output}")"
  printf '%s\n' "${rendered}" > "${output}"
else
  printf '%s\n' "${rendered}"
fi
