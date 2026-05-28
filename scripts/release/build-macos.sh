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
ICONSET="${DIST_DIR}/logo.iconset"
mkdir -p "${ICONSET}"
for size in 16 32 64 128 256 512; do
  sips -z "${size}" "${size}" crates/client-ui/assets/logo.png --out "${ICONSET}/icon_${size}x${size}.png" >/dev/null
done
sips -z 32 32 crates/client-ui/assets/logo.png --out "${ICONSET}/icon_16x16@2x.png" >/dev/null
sips -z 64 64 crates/client-ui/assets/logo.png --out "${ICONSET}/icon_32x32@2x.png" >/dev/null
sips -z 256 256 crates/client-ui/assets/logo.png --out "${ICONSET}/icon_128x128@2x.png" >/dev/null
sips -z 512 512 crates/client-ui/assets/logo.png --out "${ICONSET}/icon_256x256@2x.png" >/dev/null
sips -z 1024 1024 crates/client-ui/assets/logo.png --out "${ICONSET}/icon_512x512@2x.png" >/dev/null
iconutil -c icns "${ICONSET}" -o "${CONTENTS}/Resources/logo.icns"
cat > "${CONTENTS}/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>rsmsg</string>
  <key>CFBundleDisplayName</key><string>rsmsg</string>
  <key>CFBundleIdentifier</key><string>ru.kevindev64.rsmsg</string>
  <key>CFBundleExecutable</key><string>rsmsg</string>
  <key>CFBundleIconFile</key><string>logo</string>
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
