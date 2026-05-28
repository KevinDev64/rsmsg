# Release

First-release flow:

1. Merge `dev` into `master`.
2. Set the workspace version in `Cargo.toml`.
3. Tag `master` with `v<version>` and push the tag.
4. Download installers from the GitHub Release or run packaging scripts locally.
5. Upload files under `https://kevindev64.ru/rsmsg-downloads/releases/<version>/`.
6. Generate and publish `stable/manifest.json`.
7. Set `MIN_CLIENT_VERSION` on the production server when a mandatory update is required.

Static download layout:

```text
rsmsg-downloads/
  stable/
    manifest.json
  releases/1.0.0/
    notes.html
    windows/rsmsg-setup-1.0.0-x86_64.exe
    macos/rsmsg-1.0.0-aarch64-apple-darwin.dmg
    macos/rsmsg-1.0.0-x86_64-apple-darwin.dmg
    linux/rsmsg-1.0.0-x86_64-unknown-linux-gnu.tar.gz
```

Local packaging commands:

Linux packaging requires `libgtk-3-dev libayatana-appindicator3-dev libasound2-dev libudev-dev libxdo-dev cmake pkg-config`.

```bash
scripts/release/build-linux.sh
scripts/release/build-macos.sh
VERSION=1.0.0 scripts/release/generate-manifest.sh
```

The manifest generator includes only files that exist locally. This allows creating a manifest from a partial local build without empty platform hashes.

Windows packaging runs from PowerShell:

```powershell
scripts/release/build-windows.ps1
```

Windows icon embedding requires `crates/client-ui/assets/logo.ico`. The Windows release script generates it from `logo.png` when ImageMagick `magick` is available.
