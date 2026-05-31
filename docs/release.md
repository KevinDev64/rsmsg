# Release

## Production

### Release flow

1. Merge `dev` into `master`.

2. Set the workspace version in `Cargo.toml`.

3. Tag `master` with `vX.Y.Z` and push the tag. For example: v1.0.0

4. Wait for GitHub Actions to create the GitHub Release assets.

5. Keep installers in the GitHub Release.

6. Send me an email notification (see profile) about your release. I will publish `stable/manifest.json` as soon as possible. Please specify minimal requirements for client version for running this update.

### Static download layout

```text
rsmsg-downloads/
  stable/
    manifest.json
```

### Generating manifest locally

```bash
VERSION=X.Y.Z \
GITHUB_REPO=KevinDev64/rsmsg \
scripts/release/generate-manifest.sh
```

The generator reads asset names and `.sha256` files from GitHub Release through `gh`, writes installer URLs pointing directly to GitHub Release assets, and includes only platforms that exist in the GitHub Release assets.

Only `stable/manifest.json` is hosted on `kevindev64.ru`.

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
