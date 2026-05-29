# Release

## Production

### Release flow

1. Merge `dev` into `master`.

2. Set the workspace version in `Cargo.toml`.

3. Tag `master` with `vX.Y.Z` and push the tag. For example: v1.0.0

4. Wait for GitHub Actions to create the GitHub Release assets.

5. Download installers from the GitHub Release.

6. Send me an email notification (see profile) about your release. I will publish this release as soon as possible. Please specify minimal requirements for client version for running this update.

### Static download layout

```text
rsmsg-downloads/
  stable/
    manifest.json
  releases/X.Y.Z/
    notes.html
    windows/rsmsg-setup-X.Y.Z-x86_64.exe
    macos/rsmsg-X.Y.Z-aarch64-apple-darwin.dmg
    macos/rsmsg-X.Y.Z-x86_64-apple-darwin.dmg
    linux/rsmsg-X.Y.Z-x86_64-unknown-linux-gnu.tar.gz
```

### Generating manifest locally

```bash
VERSION=X.Y.Z \
GITHUB_REPO=KevinDev64/rsmsg \
BASE_URL=https://kevindev64.ru/rsmsg-downloads/releases/X.Y.Z \
scripts/release/generate-manifest.sh
```

The generator includes only platforms that exist in the GitHub Release assets.

## Local work

### How to package

- **Linux and MacOS(x86_64+arm)** packaging requires `libgtk-3-dev libayatana-appindicator3-dev libasound2-dev libudev-dev libxdo-dev cmake pkg-config`.

  ```bash
  scripts/release/build-linux.sh
  scripts/release/build-macos.sh
  ```

- **Windows** packaging runs from PowerShell:

  ```powershell
  scripts/release/build-windows.ps1
  ```

### Important note about Windows

Windows icon embedding requires `crates/client-ui/assets/logo.ico`. The Windows release script generates it from `logo.png` when ImageMagick `magick` is available.
