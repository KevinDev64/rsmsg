#!/usr/bin/env bash
set -euo pipefail

VERSION="${VERSION:?VERSION is required, for example VERSION=1.0.0}"
GITHUB_REPO="${GITHUB_REPO:-KevinDev64/rsmsg}"
MINIMUM_SUPPORTED_VERSION="${MINIMUM_SUPPORTED_VERSION:-${VERSION}}"
BASE_URL="${BASE_URL:-https://kevindev64.ru/rsmsg-downloads/releases/${VERSION}}"
OUT="${OUT:-dist/release/manifest.json}"
TAG="${TAG:-v${VERSION}}"

require_asset() {
  local name="$1"
  local value
  value=$(gh release view "${TAG}" --repo "${GITHUB_REPO}" --json assets --jq ".assets[] | select(.name == \"${name}\") | .name" | head -n1)
  if [[ -z "${value}" ]]; then
    return 1
  fi
}

asset_hash() {
  local asset="$1"
  local hash_asset="${asset}.sha256"
  local raw
  raw=$(gh release download "${TAG}" --repo "${GITHUB_REPO}" --pattern "${hash_asset}" --output - 2>/dev/null || true)
  if [[ -z "${raw}" ]]; then
    echo "missing sha256 asset ${hash_asset}" >&2
    return 1
  fi
  awk '{print $1}' <<< "${raw}"
}

platform_entry() {
  local key="$1"
  local url="$2"
  local asset="$3"
  if ! require_asset "${asset}"; then
    return 0
  fi
  if [[ "${first}" == "false" ]]; then
    printf ',\n' >> "${OUT}"
  fi
  first=false
  cat >> "${OUT}" <<JSON
    "${key}": {
      "url": "${url}",
      "sha256": "$(asset_hash "${asset}")"
    }
JSON
}

mkdir -p "$(dirname "${OUT}")"
cat > "${OUT}" <<JSON
{
  "version": "${VERSION}",
  "minimum_supported_version": "${MINIMUM_SUPPORTED_VERSION}",
  "mandatory": false,
  "notes_url": "${BASE_URL}/notes.html",
  "platforms": {
JSON

first=true
platform_entry "windows-x86_64" "${BASE_URL}/windows/rsmsg-setup-${VERSION}-x86_64.exe" "rsmsg-setup-${VERSION}-x86_64.exe"
platform_entry "macos-aarch64" "${BASE_URL}/macos/rsmsg-${VERSION}-aarch64-apple-darwin.dmg" "rsmsg-${VERSION}-aarch64-apple-darwin.dmg"
platform_entry "macos-x86_64" "${BASE_URL}/macos/rsmsg-${VERSION}-x86_64-apple-darwin.dmg" "rsmsg-${VERSION}-x86_64-apple-darwin.dmg"
platform_entry "linux-x86_64" "${BASE_URL}/linux/rsmsg-${VERSION}-x86_64-unknown-linux-gnu.tar.gz" "rsmsg-${VERSION}-x86_64-unknown-linux-gnu.tar.gz"

cat >> "${OUT}" <<JSON

  }
}
JSON
