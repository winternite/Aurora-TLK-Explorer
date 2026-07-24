#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APPDIR="$ROOT/AppDir"
OUTPUT="${1:-$HOME/Desktop/Aurora-TLK-Explorer-x86_64.AppImage}"
ZIP_OUTPUT="${APPIMAGE_ZIP:-$(dirname "$OUTPUT")/Aurora-TLK-Explorer-Linux-x86_64.zip}"
VERSION="$(sed -n 's/^version = "\([^"]*\)"/\1/p' "$ROOT/Cargo.toml" | head -1)"
APPIMAGETOOL_SHA256=b90f4a8b18967545fda78a445b27680a1642f1ef9488ced28b65398f2be7add2
APPIMAGETOOL="${APPIMAGETOOL:-$ROOT/dist/tools/appimagetool-x86_64.AppImage}"

if ! command -v cargo-zigbuild >/dev/null 2>&1; then
    echo "cargo-zigbuild is required for the glibc 2.17 compatibility build" >&2
    exit 1
fi
if [[ ! -f "$APPIMAGETOOL" ]] || ! printf '%s  %s\n' "$APPIMAGETOOL_SHA256" "$APPIMAGETOOL" | sha256sum --check --status; then
    mkdir -p "$(dirname "$APPIMAGETOOL")"
    curl --fail --location --retry 3 \
        --output "$APPIMAGETOOL.download" \
        https://github.com/AppImage/AppImageKit/releases/download/continuous/appimagetool-x86_64.AppImage
    printf '%s  %s\n' "$APPIMAGETOOL_SHA256" "$APPIMAGETOOL.download" | sha256sum --check --status
    mv "$APPIMAGETOOL.download" "$APPIMAGETOOL"
fi
chmod +x "$APPIMAGETOOL"

"$ROOT/scripts/fetch-nwn-gff.sh" linux
"$ROOT/scripts/generate-third-party.sh"
cargo zigbuild --manifest-path "$ROOT/Cargo.toml" --locked --release \
    --target x86_64-unknown-linux-gnu.2.17
rm -rf "$APPDIR"
install -Dm755 "$ROOT/target/x86_64-unknown-linux-gnu/release/aurora-tlk-explorer" \
    "$APPDIR/usr/bin/aurora-tlk-explorer"
install -Dm755 "$ROOT/dist/tools/nwn-gff-linux/nwn_gff" "$APPDIR/usr/bin/nwn_gff"
install -Dm755 "$ROOT/scripts/AppRun" "$APPDIR/AppRun"
install -Dm644 "$ROOT/assets/org.aurora_tools.AuroraTlkExplorer.desktop" \
    "$APPDIR/org.aurora_tools.AuroraTlkExplorer.desktop"
install -Dm644 "$ROOT/assets/aurora-tlk-explorer.png" "$APPDIR/aurora-tlk-explorer.png"
install -Dm644 "$ROOT/assets/aurora-tlk-explorer.png" \
    "$APPDIR/usr/share/icons/hicolor/512x512/apps/aurora-tlk-explorer.png"
install -Dm644 "$ROOT/assets/ateicon.png" \
    "$APPDIR/usr/share/aurora-tlk-explorer/ateicon.png"
install -Dm644 "$ROOT/assets/org.aurora_tools.AuroraTlkExplorer.desktop" \
    "$APPDIR/usr/share/applications/org.aurora_tools.AuroraTlkExplorer.desktop"
install -Dm644 "$ROOT/assets/org.aurora_tools.AuroraTlkExplorer.metainfo.xml" \
    "$APPDIR/usr/share/metainfo/org.aurora_tools.AuroraTlkExplorer.metainfo.xml"
ln -sfn aurora-tlk-explorer.png "$APPDIR/.DirIcon"

ARCH=x86_64 "$APPIMAGETOOL" --appimage-extract-and-run "$APPDIR" "$OUTPUT"
chmod +x "$OUTPUT"
PACKAGE="$ROOT/dist/package-linux"
rm -rf "$PACKAGE"
mkdir -p "$PACKAGE"
install -m755 "$OUTPUT" "$PACKAGE/Aurora-TLK-Explorer-$VERSION-x86_64.AppImage"
install -m644 "$ROOT/LICENSE" "$ROOT/README.md" "$ROOT/CHANGELOG.md" \
    "$ROOT/THIRD_PARTY_NOTICES.md" "$ROOT/dist/THIRD-PARTY-LICENSES.html" "$PACKAGE/"
rm -f "$ZIP_OUTPUT"
(cd "$PACKAGE" && zip -q -r "$ZIP_OUTPUT" .)
echo "$OUTPUT"
echo "$ZIP_OUTPUT"
