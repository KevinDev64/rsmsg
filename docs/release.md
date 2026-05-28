# Release

First-release flow:

1. Merge `dev` into `master`.
2. Set the workspace version in `Cargo.toml`.
3. Tag `master` with `v<version>` and push the tag.
4. Wait for GitHub Actions to create the GitHub Release assets.
5. Download installers from the GitHub Release.
6. Upload installers under `https://kevindev64.ru/rsmsg-downloads/releases/<version>/`.
7. Generate `stable/manifest.json` from the GitHub Release asset hashes and upload it.
8. Set `MIN_CLIENT_VERSION` on the production server when a mandatory update is required.

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

Manifest generation:

The manifest generator reads asset names and `.sha256` files from GitHub Release through `gh`, but writes download URLs pointing to `kevindev64.ru`.

```bash
VERSION=1.0.0 \
GITHUB_REPO=KevinDev64/rsmsg \
BASE_URL=https://kevindev64.ru/rsmsg-downloads/releases/1.0.0 \
scripts/release/generate-manifest.sh
```

Upload the result to:

```text
https://kevindev64.ru/rsmsg-downloads/stable/manifest.json
```

The generator includes only platforms that exist in the GitHub Release assets.

Local packaging commands:

Linux packaging requires `libgtk-3-dev libayatana-appindicator3-dev libasound2-dev libudev-dev libxdo-dev cmake pkg-config`.

```bash
scripts/release/build-linux.sh
scripts/release/build-macos.sh
```

Windows packaging runs from PowerShell:

```powershell
scripts/release/build-windows.ps1
```

Windows icon embedding requires `crates/client-ui/assets/logo.ico`. The Windows release script generates it from `logo.png` when ImageMagick `magick` is available.
