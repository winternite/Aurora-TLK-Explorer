#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PLATFORM="${1:?usage: fetch-nwn-gff.sh linux|windows [output-directory]}"
OUTPUT="${2:-$ROOT/dist/tools/nwn-gff-$PLATFORM}"
VERSION=2.1.2

case "$PLATFORM" in
    linux)
        ARCHIVE=neverwinter-x86_64-linux-gnu.zip
        SHA256=f9cc2e50fbe6f750954d11b824434a92b12913cc0a3442e95dcc5eefd4ddc387
        FILES=(nwn_gff)
        ;;
    windows)
        ARCHIVE=neverwinter-x86_64-windows.zip
        SHA256=b00501cc57adc63392f17d460d712edcdcbe35cb37f7d7257ab23806ed86aed1
        FILES=(nwn_gff.exe sqlite3_64.dll)
        ;;
    *)
        echo "platform must be linux or windows" >&2
        exit 2
        ;;
esac

CACHE="$ROOT/dist/cache"
mkdir -p "$CACHE" "$OUTPUT"
ZIP="$CACHE/$ARCHIVE"
URL="https://github.com/niv/neverwinter.nim/releases/download/$VERSION/$ARCHIVE"
if [[ ! -f "$ZIP" ]] || ! printf '%s  %s\n' "$SHA256" "$ZIP" | sha256sum --check --status; then
    curl --fail --location --retry 3 --output "$ZIP.download" "$URL"
    printf '%s  %s\n' "$SHA256" "$ZIP.download" | sha256sum --check --status
    mv "$ZIP.download" "$ZIP"
fi
for file in "${FILES[@]}"; do
    unzip -p "$ZIP" "$file" > "$OUTPUT/$file.tmp"
    mv "$OUTPUT/$file.tmp" "$OUTPUT/$file"
done
chmod +x "$OUTPUT/nwn_gff" 2>/dev/null || true
printf '%s\n' "$OUTPUT"
