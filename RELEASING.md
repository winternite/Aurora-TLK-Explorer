# Production release procedure

Aurora TLK Explorer uses Semantic Versioning. A production release is built
from a clean, annotated `vMAJOR.MINOR.PATCH` tag whose value matches
`Cargo.toml` and `Cargo.lock`.

The release toolchain is Rust 1.97.0 with cargo-zigbuild 0.23.0 and cargo-xwin
0.23.0. External converter, AppImage, audit, license, and policy tools are
downloaded only after their pinned SHA-256 digest has been verified.

## Required gates

1. Run `./scripts/check-release.sh` on Linux.
2. Build the AppImage with `./scripts/build-appimage.sh` and test opening,
   editing, saving, reopening, undoing, and closing TLK, text/binary 2DA, and ITP
   files on a supported Linux system.
3. Build the Windows package with `./scripts/build-windows.sh`. Test the same
   workflow on physical or virtual Windows 10 and Windows 11 systems.
4. Sign `Aurora-TLK-Explorer.exe` with the project's Authenticode certificate
   when one is available, then verify the signature before packaging.
5. Generate a source archive with `./scripts/build-source.sh` and publish it
   beside both binary archives.
6. Confirm each binary archive contains `LICENSE`, `README.md`,
   `THIRD_PARTY_NOTICES.md`, and the generated Rust dependency license report.

The CI workflow repeats formatting, linting, tests, dependency advisories,
license policy, and native Windows compilation. CI is not a substitute for the
manual Windows 10/11 GUI test in step 3.

## Supported systems

- 64-bit Linux distributions with glibc 2.17 or newer, Wayland or X11.
- 64-bit Windows 10 and Windows 11.

ATE is an application, not a crates.io library package. Its authoritative
source artifact is the complete, vendor-inclusive archive produced from the
release tag by the source-build script.

Release binaries are portable. The AppImage bundles the pinned ITP converter;
the Windows executable embeds it. The source archive is the corresponding
source required by GPL-3.0-or-later.
