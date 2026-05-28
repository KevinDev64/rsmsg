#!/usr/bin/env bash
set -euo pipefail

VERSION="${VERSION:-$(grep '^version = ' Cargo.toml | head -n1 | cut -d'"' -f2)}"
TARGET="${TARGET:-x86_64-unknown-linux-gnu}"
DIST_DIR="dist/release/linux-${TARGET}"
APPDIR="${DIST_DIR}/rsmsg.AppDir"

cargo build --release -p client-ui --target "${TARGET}"
rm -rf "${DIST_DIR}"
mkdir -p "${APPDIR}/usr/bin" "${APPDIR}/usr/share/applications" "${APPDIR}/usr/share/icons/hicolor/256x256/apps" "${APPDIR}/usr/share/rsmsg"
cp "target/${TARGET}/release/client-ui" "${APPDIR}/usr/bin/rsmsg"
cp crates/client-ui/assets/logo.png "${APPDIR}/usr/share/icons/hicolor/256x256/apps/rsmsg.png"
cp -R crates/client-ui/locales "${APPDIR}/usr/share/rsmsg/locales"
cat > "${APPDIR}/rsmsg.desktop" <<DESKTOP
[Desktop Entry]
Type=Application
Name=rsmsg
Comment=Encrypted desktop messenger
Exec=rsmsg
Icon=rsmsg
Categories=Network;InstantMessaging;
DESKTOP
cp "${APPDIR}/rsmsg.desktop" "${APPDIR}/usr/share/applications/rsmsg.desktop"
tar -C "${APPDIR}" -czf "${DIST_DIR}/rsmsg-${VERSION}-${TARGET}.tar.gz" .
sha256sum "${DIST_DIR}/rsmsg-${VERSION}-${TARGET}.tar.gz" > "${DIST_DIR}/rsmsg-${VERSION}-${TARGET}.tar.gz.sha256"
if command -v linuxdeploy >/dev/null 2>&1; then
  (cd "${DIST_DIR}" && linuxdeploy --appdir rsmsg.AppDir --output appimage)
  mv "${DIST_DIR}"/*.AppImage "${DIST_DIR}/rsmsg-${VERSION}-${TARGET}.AppImage"
  sha256sum "${DIST_DIR}/rsmsg-${VERSION}-${TARGET}.AppImage" > "${DIST_DIR}/rsmsg-${VERSION}-${TARGET}.AppImage.sha256"
fi
