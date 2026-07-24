#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VERSION="$(sed -n 's/^version = "\([^"]*\)"/\1/p' "$ROOT/Cargo.toml" | head -1)"
OUTPUT="${1:-$ROOT/dist/Aurora-TLK-Explorer-$VERSION-source.tar.gz}"

git -C "$ROOT" diff --quiet
git -C "$ROOT" diff --cached --quiet
mkdir -p "$(dirname "$OUTPUT")"
PREFIX="Aurora-TLK-Explorer-$VERSION/"
git -C "$ROOT" archive --format=tar --prefix="$PREFIX" HEAD | gzip -n -9 > "$OUTPUT.tmp"
mv "$OUTPUT.tmp" "$OUTPUT"
printf '%s\n' "$OUTPUT"
