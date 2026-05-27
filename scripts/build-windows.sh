#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
TARGET="${WINDOWS_TARGET:-x86_64-pc-windows-gnu}"
PACKAGE="${WINDOWS_PACKAGE:-client-ui}"
BIN_NAME="${WINDOWS_BIN:-client-ui}"
OUT_DIR="${WINDOWS_OUT_DIR:-$ROOT_DIR/dist/windows/$TARGET}"
EXE_NAME="$BIN_NAME.exe"
BUILD_EXE="$ROOT_DIR/target/$TARGET/release/$EXE_NAME"

echo "rsmsg windows build started"
echo "target: $TARGET"
echo "package: $PACKAGE"
echo "binary: $BIN_NAME"
echo "output: $OUT_DIR"

echo "checking rust target"
if ! rustup target list --installed | grep -qx "$TARGET"; then
  echo "rust target is not installed: $TARGET"
  echo "run: rustup target add $TARGET"
  exit 1
fi

echo "building release binary"
cargo build --release --target "$TARGET" -p "$PACKAGE" --bin "$BIN_NAME"

echo "preparing output directory"
rm -rf "$OUT_DIR"
mkdir -p "$OUT_DIR"

echo "copying exe"
cp "$BUILD_EXE" "$OUT_DIR/$EXE_NAME"

if [[ "$TARGET" == *"-gnu" ]]; then
  echo "copying mingw runtime dlls"
  DLLS=(libgcc_s_seh-1.dll libstdc++-6.dll libwinpthread-1.dll)
  for dll in "${DLLS[@]}"; do
    dll_path=""
    if command -v x86_64-w64-mingw32-gcc >/dev/null 2>&1; then
      candidate="$(x86_64-w64-mingw32-gcc -print-file-name="$dll")"
      if [ -f "$candidate" ]; then
        dll_path="$candidate"
      fi
    fi
    if [ -z "$dll_path" ] && [ -n "${WINDOWS_EXTRA_DLL_DIR:-}" ] && [ -f "$WINDOWS_EXTRA_DLL_DIR/$dll" ]; then
      dll_path="$WINDOWS_EXTRA_DLL_DIR/$dll"
    fi
    if [ -z "$dll_path" ]; then
      for dir in \
        /opt/homebrew/opt/mingw-w64/toolchain-x86_64/x86_64-w64-mingw32/bin \
        /usr/local/opt/mingw-w64/toolchain-x86_64/x86_64-w64-mingw32/bin \
        /usr/x86_64-w64-mingw32/bin \
        /opt/homebrew/bin \
        /usr/local/bin; do
        if [ -f "$dir/$dll" ]; then
          dll_path="$dir/$dll"
          break
        fi
      done
    fi
    if [ -z "$dll_path" ]; then
      echo "missing dll: $dll"
      echo "set WINDOWS_EXTRA_DLL_DIR to the directory containing mingw runtime dlls"
      exit 1
    fi
    echo "copying $dll"
    cp "$dll_path" "$OUT_DIR/$dll"
  done
else
  echo "target is not mingw gnu, skipping mingw dll copy"
fi

echo "windows bundle created: $OUT_DIR"
echo "rsmsg windows build finished"
