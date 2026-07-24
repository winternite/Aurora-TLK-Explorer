#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CACHE="$ROOT/dist/cache"
TOOLS="$ROOT/dist/tools"
mkdir -p "$CACHE" "$TOOLS"

fetch() {
    local url="$1" sha="$2" archive="$3" directory="$4"
    if [[ ! -f "$archive" ]] || ! printf '%s  %s\n' "$sha" "$archive" | sha256sum --check --status; then
        curl --fail --location --retry 3 --output "$archive.download" "$url"
        printf '%s  %s\n' "$sha" "$archive.download" | sha256sum --check --status
        mv "$archive.download" "$archive"
    fi
    mkdir -p "$directory"
    tar -xzf "$archive" -C "$directory"
}

fetch \
    https://github.com/rustsec/rustsec/releases/download/cargo-audit/v0.22.2/cargo-audit-x86_64-unknown-linux-musl-v0.22.2.tgz \
    7fb9497f8594b389e5fce5ef9b92db08432996895b2e0c5a0167a69ed445c428 \
    "$CACHE/cargo-audit-0.22.2.tgz" "$TOOLS/cargo-audit"
fetch \
    https://github.com/EmbarkStudios/cargo-deny/releases/download/0.20.2/cargo-deny-0.20.2-x86_64-unknown-linux-musl.tar.gz \
    9f12ed4c49936e09b48bf862b595cde2fe64fcbd9d74dfacac6131ca824c8d5f \
    "$CACHE/cargo-deny-0.20.2.tar.gz" "$TOOLS/cargo-deny"

ln -sfn cargo-audit-x86_64-unknown-linux-musl-v0.22.2/cargo-audit \
    "$TOOLS/cargo-audit/current"
ln -sfn cargo-deny-0.20.2-x86_64-unknown-linux-musl/cargo-deny \
    "$TOOLS/cargo-deny/current"
printf '%s\n' "$TOOLS"
