#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUTPUT="${1:-$ROOT/dist/THIRD-PARTY-LICENSES.html}"
VERSION=0.9.1
SHA256=c0e7dc6f5d74b0beec5c0053d39ab24514c717d19acd91886907a22457ea9e98
ARCHIVE="cargo-about-$VERSION-x86_64-unknown-linux-musl.tar.gz"
CACHE="$ROOT/dist/cache"
mkdir -p "$CACHE" "$(dirname "$OUTPUT")"

if [[ -n "${CARGO_ABOUT:-}" ]]; then
    TOOL="$CARGO_ABOUT"
else
    TAR="$CACHE/$ARCHIVE"
    URL="https://github.com/EmbarkStudios/cargo-about/releases/download/$VERSION/$ARCHIVE"
    if [[ ! -f "$TAR" ]] || ! printf '%s  %s\n' "$SHA256" "$TAR" | sha256sum --check --status; then
        curl --fail --location --retry 3 --output "$TAR.download" "$URL"
        printf '%s  %s\n' "$SHA256" "$TAR.download" | sha256sum --check --status
        mv "$TAR.download" "$TAR"
    fi
    tar -xzf "$TAR" -C "$CACHE"
    TOOL="$CACHE/cargo-about-$VERSION-x86_64-unknown-linux-musl/cargo-about"
fi

"$TOOL" generate --manifest-path "$ROOT/Cargo.toml" \
    --config "$ROOT/about.toml" "$ROOT/about.hbs" > "$OUTPUT.tmp"
mv "$OUTPUT.tmp" "$OUTPUT"
printf '%s\n' "$OUTPUT"
