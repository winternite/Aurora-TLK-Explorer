# Aurora TLK Explorer

A native Rust editor for Neverwinter Nights talk tables, 2DA data files, and ITP palettes.

Aurora TLK Explorer 1.0 is production software distributed under the
[GNU General Public License version 3 or later](LICENSE). Release archives include
the corresponding source and third-party notices.

## Current features

- Open and edit multiple `.tlk`, `.2da`, and `.itp` files in tabs
- Hierarchical ITP palette tree editing with preserved unknown GFF fields
- Native KDE file dialogs through XDG Desktop Portal
- Atomic Save and Save As
- Save / Discard / Cancel protection for unsaved tabs and application exit
- Restores the last open files and active tab
- System, dark, and light themes
- Search/filter, add, and delete rows
- Find/replace and find-next navigation
- Row cut/copy/paste through the system clipboard and between tabs
- Undo/redo for cell, row, resize, renumber, and column operations
- TLK V3.0 encoding selection and multiline entry editing
- User StrRef and hexadecimal StrRef display modes
- Focused TLK view with StrRef and text columns; hidden sound metadata is preserved
- TlkEdit-compatible diff save/merge, DTU merge, marking, and overview
- 2DA physical-index and stored-row comparison, resize, renumber, User StrRef flag toggling, and column insert/drop/rename
- Text `2DA V2.0` and binary `2DA V2.b` loading
- Wayland and X11 fallback support

## Development

```sh
cargo test
cargo run -- /path/to/dialog.tlk
./scripts/check-release.sh
./scripts/build-appimage.sh
./scripts/build-windows.sh
```

The Windows script creates a self-contained 64-bit Windows 10/11 executable,
including its icon and embedded ITP converter, at
`~/Documents/projects/Aurora-TLK-Explorer-Windows/Aurora-TLK-Explorer.exe` by
default.

The embedded `nwn_gff` converter and SQLite runtime come from
[neverwinter.nim](https://github.com/niv/neverwinter.nim) 2.1.2 under its MIT
license.

See [SECURITY.md](SECURITY.md) for vulnerability reporting and
[RELEASING.md](RELEASING.md) for the release gates and supported systems.

Session state is stored below the user's XDG configuration directory. Unsaved document contents are never silently persisted; closing them always asks what to do.
