# Aurora TLK Explorer

A native Rust editor for Neverwinter Nights talk tables, 2DA data files, and ITP palettes.

Aurora TLK Explorer 1.1.1 is production software distributed under the
[GNU General Public License version 3 or later](LICENSE). Release archives include
the corresponding source and third-party notices.

## Current features

- Open and edit multiple `.tlk`, `.2da`, and `.itp` files in tabs
- Open files from the desktop, file manager, or drag-and-drop; additional
  files are handed to the existing Aurora window rather than creating a second
  application session
- Hierarchical ITP palette tree editing with preserved unknown GFF fields
- Native KDE file dialogs through XDG Desktop Portal
- Atomic Save and Save As
- Save / Discard / Cancel protection for unsaved tabs and application exit
- Restores the last open files and active tab
- System, dark, and light themes
- Search/filter, add, and delete rows
- Insert one or more blank TLK or 2DA rows above or below the selected row
- Use Up/Down to move between editable TLK and 2DA table text fields
- Delete all selected 2DA rows in one undoable operation
- Find/replace and find-next navigation
- Row cut/copy/paste through the system clipboard and between tabs
- Spreadsheet-style 2DA field pasting and double-click Select All
- Undo/redo for cell, row, resize, renumber, and column operations
- TLK V3.0 encoding selection and multiline entry editing
- User StrRef and hexadecimal StrRef display modes
- Sticky 2DA index and TLK StrRef columns, including row-selection highlighting
- Focused TLK view with StrRef and text columns; hidden sound metadata is preserved
- TlkEdit-compatible diff save/merge, DTU merge, marking, and overview
- 2DA physical-index and stored-row comparison, resize, renumber, User StrRef flag toggling, and column insert/drop/rename
- Text `2DA V2.0` and binary `2DA V2.b` loading
- Wayland and X11 fallback support
- Browser-style middle-click table autoscroll and tuned wheel scrolling
- KDE Wayland active-output placement while retaining XWayland file dropping

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

## Linux desktop integration

The AppImage advertises TLK and 2DA file associations and supports opening
files from Dolphin and other desktop file managers. On KDE Plasma Wayland,
Aurora loads a small session-only KWin helper that places new ordinary Aurora
windows on the output under the pointer. Fullscreen and maximized windows keep
their compositor-managed geometry. The helper is not installed persistently
and is never used on Windows or non-KDE desktops.
