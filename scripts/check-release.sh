#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

cargo fmt --all --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-targets
if command -v cargo-xwin >/dev/null 2>&1; then
    if command -v llvm-rc >/dev/null 2>&1; then
        RC_DIR="$(dirname "$(command -v llvm-rc)")"
    elif [[ -x /usr/lib64/rocm/llvm/bin/llvm-rc ]]; then
        RC_DIR=/usr/lib64/rocm/llvm/bin
    else
        echo "llvm-rc is required for the Windows compatibility check" >&2
        exit 1
    fi
    PATH="$RC_DIR:$PATH" cargo xwin check --locked --target x86_64-pc-windows-msvc
else
    echo "cargo-xwin is required for the Windows compatibility check" >&2
    exit 1
fi
./scripts/fetch-quality-tools.sh
dist/tools/cargo-audit/current audit
dist/tools/cargo-deny/current check
./scripts/generate-third-party.sh
test -s dist/THIRD-PARTY-LICENSES.html
