#!/usr/bin/env bash
set -euo pipefail

APP_NAME="rsmsg"
VERSION="${VERSION:-$(grep '^version = ' Cargo.toml | head -n1 | cut -d'"' -f2)}"
TARGET="${TARGET:-$(rustc -vV | awk '/host:/ {print $2}')}"
DIST_DIR="dist/release/macos-${TARGET}"
APP_DIR="${DIST_DIR}/${APP_NAME}.app"
CONTENTS="${APP_DIR}/Contents"

cargo build --release -p client-ui --target "${TARGET}"
rm -rf "${DIST_DIR}"
mkdir -p "${CONTENTS}/MacOS" "${CONTENTS}/Resources"
cp "target/${TARGET}/release/client-ui" "${CONTENTS}/MacOS/${APP_NAME}"
cp crates/client-ui/assets/logo.png "${CONTENTS}/Resources/logo.png"
cp -R crates/client-ui/locales "${CONTENTS}/Resources/locales"
cat > "${CONTENTS}/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>rsmsg</string>
  <key>CFBundleDisplayName</key><string>rsmsg</string>
  <key>CFBundleIdentifier</key><string>ru.kevindev64.rsmsg</string>
  <key>CFBundleExecutable</key><string>rsmsg</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>${VERSION}</string>
  <key>CFBundleVersion</key><string>${VERSION}</string>
  <key>LSMinimumSystemVersion</key><string>11.0</string>
  <key>NSCameraUsageDescription</key><string>rsmsg uses the camera for video calls.</string>
  <key>NSMicrophoneUsageDescription</key><string>rsmsg uses the microphone for audio and video calls.</string>
</dict>
</plist>
PLIST
hdiutil create -volname "rsmsg ${VERSION}" -srcfolder "${APP_DIR}" -ov -format UDZO "${DIST_DIR}/rsmsg-${VERSION}-${TARGET}.dmg"
shasum -a 256 "${DIST_DIR}/rsmsg-${VERSION}-${TARGET}.dmg" > "${DIST_DIR}/rsmsg-${VERSION}-${TARGET}.dmg.sha256"
