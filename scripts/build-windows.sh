#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
OUTPUT_DIR="${1:-$HOME/Documents/projects/Aurora-TLK-Explorer-Windows}"
TARGET="x86_64-pc-windows-msvc"
ZIP_OUTPUT="${WINDOWS_ZIP:-$OUTPUT_DIR/Aurora-TLK-Explorer-Windows-x86_64.zip}"
VERSION="$(sed -n 's/^version = "\([^"]*\)"/\1/p' "$ROOT/Cargo.toml" | head -1)"

if ! command -v cargo-xwin >/dev/null 2>&1; then
    echo "cargo-xwin is required to build the Windows executable" >&2
    exit 1
fi

if command -v llvm-rc >/dev/null 2>&1; then
    RC_DIR="$(dirname "$(command -v llvm-rc)")"
elif [[ -x /usr/lib64/rocm/llvm/bin/llvm-rc ]]; then
    RC_DIR=/usr/lib64/rocm/llvm/bin
else
    echo "llvm-rc is required to embed the Windows icon" >&2
    exit 1
fi

PATH="$RC_DIR:$PATH" \
RUST_MIN_STACK="${RUST_MIN_STACK:-33554432}" \
RUSTFLAGS="${RUSTFLAGS:+$RUSTFLAGS }-C target-feature=+crt-static" \
    cargo xwin build --manifest-path "$ROOT/Cargo.toml" --locked --release --target "$TARGET"

mkdir -p "$OUTPUT_DIR"
install -m755 \
    "$ROOT/target/$TARGET/release/aurora-tlk-explorer.exe" \
    "$OUTPUT_DIR/Aurora-TLK-Explorer.exe"
"$ROOT/scripts/fetch-nwn-gff.sh" windows
cmp "$ROOT/assets/nwn_gff-x86_64-windows.exe" \
    "$ROOT/dist/tools/nwn-gff-windows/nwn_gff.exe"
cmp "$ROOT/assets/sqlite3_64-windows.dll" \
    "$ROOT/dist/tools/nwn-gff-windows/sqlite3_64.dll"
"$ROOT/scripts/generate-third-party.sh"

if [[ -n "${WINDOWS_SIGN_PFX:-}" ]]; then
    command -v osslsigncode >/dev/null
    SIGNED="$OUTPUT_DIR/Aurora-TLK-Explorer.signed.exe"
    osslsigncode sign -pkcs12 "$WINDOWS_SIGN_PFX" \
        -pass "${WINDOWS_SIGN_PASSWORD:?WINDOWS_SIGN_PASSWORD is required}" \
        -n "Aurora TLK Explorer" -t http://timestamp.digicert.com \
        -in "$OUTPUT_DIR/Aurora-TLK-Explorer.exe" -out "$SIGNED"
    osslsigncode verify "$SIGNED"
    mv "$SIGNED" "$OUTPUT_DIR/Aurora-TLK-Explorer.exe"
fi

PACKAGE="$ROOT/dist/package-windows"
rm -rf "$PACKAGE"
mkdir -p "$PACKAGE"
install -m755 "$OUTPUT_DIR/Aurora-TLK-Explorer.exe" \
    "$PACKAGE/Aurora-TLK-Explorer-$VERSION.exe"
install -m644 "$ROOT/LICENSE" "$ROOT/README.md" "$ROOT/CHANGELOG.md" \
    "$ROOT/THIRD_PARTY_NOTICES.md" "$ROOT/dist/THIRD-PARTY-LICENSES.html" "$PACKAGE/"
rm -f "$OUTPUT_DIR/Aurora-TLK-Explorer.exe.sha256" "$ZIP_OUTPUT"
(cd "$PACKAGE" && zip -q -r "$ZIP_OUTPUT" .)
echo "$OUTPUT_DIR/Aurora-TLK-Explorer.exe"
echo "$ZIP_OUTPUT"
