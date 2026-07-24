# Changelog

All notable user-visible changes are documented here. This project follows
Semantic Versioning.

## 1.1.1 - 2026-07-24

- Made 2DA field pastes replace the clicked destination cells, matching normal
  spreadsheet behavior.
- Added double-click Select All for editable 2DA table fields.

## 1.1.0 - 2026-07-24

- Added desktop/file-manager opening, drag-and-drop, and single-instance file
  handoff for TLK, 2DA, and ITP documents.
- Added sticky 2DA Index and TLK StrRef columns with synchronized selection
  highlighting.
- Added multi-row insertion above or below a selected row and correct
  multi-row deletion with a single undo step.
- Added spreadsheet-style Up/Down arrow navigation between editable TLK and
  2DA table text fields.
- Improved tab-strip sizing, active-tab contrast, light-theme readability,
  scrolling, and browser-style middle-click autoscroll.
- Added KWin Wayland active-output placement without sacrificing XWayland file
  drag-and-drop; fullscreen and maximized geometry remain compositor-managed.
- Improved release packaging, desktop startup notification, and bundled
  AppImage assets.

## 1.0.0 - 2026-07-17

- First production release.
- Native editing for TLK V3.0, text and binary 2DA, and ITP palettes.
- Atomic saving, undo/redo, multi-document tabs, search, and row/column editing.
- Linux AppImage and portable Windows 10/11 release targets.
- Reproducible release inputs, automated compatibility/security gates, and
  complete GPL/third-party release notices.
