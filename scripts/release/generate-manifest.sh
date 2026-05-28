#!/usr/bin/env bash
set -euo pipefail

VERSION="${VERSION:?VERSION is required, for example VERSION=1.0.0}"
MINIMUM_SUPPORTED_VERSION="${MINIMUM_SUPPORTED_VERSION:-${VERSION}}"
BASE_URL="${BASE_URL:-https://kevindev64.ru/rsmsg-downloads/releases/${VERSION}}"
OUT="${OUT:-dist/release/manifest.json}"

sha() {
  shasum -a 256 "$1" | awk '{print $1}'
}

platform_entry() {
  local key="$1"
  local url="$2"
  local path="$3"
  if [[ ! -f "$path" ]]; then
    return 0
  fi
  if [[ "$first" == "false" ]]; then
    printf ',\n' >> "$OUT"
  fi
  first=false
  cat >> "$OUT" <<JSON
    "${key}": {
      "url": "${url}",
      "sha256": "$(sha "$path")"
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
platform_entry "windows-x86_64" "${BASE_URL}/windows/rsmsg-setup-${VERSION}-x86_64.exe" "dist/release/windows-x86_64-pc-windows-gnu/rsmsg-setup-${VERSION}-x86_64.exe"
platform_entry "macos-aarch64" "${BASE_URL}/macos/rsmsg-${VERSION}-aarch64-apple-darwin.dmg" "dist/release/macos-aarch64-apple-darwin/rsmsg-${VERSION}-aarch64-apple-darwin.dmg"
platform_entry "macos-x86_64" "${BASE_URL}/macos/rsmsg-${VERSION}-x86_64-apple-darwin.dmg" "dist/release/macos-x86_64-apple-darwin/rsmsg-${VERSION}-x86_64-apple-darwin.dmg"
platform_entry "linux-x86_64" "${BASE_URL}/linux/rsmsg-${VERSION}-x86_64-unknown-linux-gnu.tar.gz" "dist/release/linux-x86_64-unknown-linux-gnu/rsmsg-${VERSION}-x86_64-unknown-linux-gnu.tar.gz"

cat >> "${OUT}" <<JSON

  }
}
JSON
