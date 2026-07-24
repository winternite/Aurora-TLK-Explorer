use aurora_tlk_explorer::{
    formats::tlk::{TlkEncoding, TlkEntry},
    model::{Document, DocumentData, EditAction},
    state::{PersistentState, ThemeChoice},
};
use eframe::egui::{self, Align, Align2, Color32, Id, Layout, RichText, TextEdit, ThemePreference};
use egui_extras::{Column, TableBuilder};
use rfd::FileDialog;
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};

const APP_NAME: &str = "Aurora TLK Explorer";

#[derive(Clone, Copy)]
struct PendingClose {
    index: usize,
    quitting: bool,
}

#[derive(Clone, Copy)]
enum Command {
    NewTlk,
    NewTwoDa,
    Open,
    Save,
    SaveAs,
    SaveAll,
    Close,
    Quit,
    Cut,
    Copy,
    Paste,
    DeleteRows,
    DeleteColumns,
    Undo,
    Redo,
    ResizeTable,
    RenumberTwoDa,
    AlterColumns,
    ToggleUserFlag,
    Find,
    FindNext,
    MarkModified,
    MarkUnmodified,
    SaveDiff,
    MergeDiff,
    MergeDtu,
    DiscardDiff,
    DiffOverview,
}

#[derive(Clone, Copy)]
enum CloseChoice {
    Save,
    Discard,
    Cancel,
}

#[derive(Clone)]
enum ClipboardRows {
    Tlk(Vec<(usize, TlkEntry)>),
    TwoDa(Vec<Vec<String>>),
    TwoDaCells(Vec<(usize, String)>),
}

struct ColumnDialogState {
    index: String,
    name: String,
    default_value: String,
}

#[derive(Clone, Copy)]
enum ColumnAction {
    Insert,
    Drop,
    Rename,
}

#[derive(Clone, Copy)]
enum SearchAction {
    Next,
    Previous,
    ReplaceSelected,
    ReplaceAll,
}

#[derive(Clone, Copy)]
enum RowMenuAction {
    Delete,
    InsertAbove(usize),
    InsertBelow(usize),
}

pub struct AuroraApp {
    documents: Vec<Document>,
    active: Option<usize>,
    state: PersistentState,
    pending_close: Option<PendingClose>,
    allow_exit: bool,
    message: Option<(String, bool)>,
    clipboard: Option<ClipboardRows>,
    clipboard_text: Option<String>,
    pending_paste_text: Option<String>,
    resize_value: Option<String>,
    column_dialog: Option<ColumnDialogState>,
    show_diff_overview: bool,
    search_window_open: bool,
    focus_search_window: bool,
}

impl AuroraApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let state = PersistentState::load();
        Self::apply_theme(&cc.egui_ctx, state.theme);
        let mut app = Self {
            documents: Vec::new(),
            active: None,
            state,
            pending_close: None,
            allow_exit: false,
            message: None,
            clipboard: None,
            clipboard_text: None,
            pending_paste_text: None,
            resize_value: None,
            column_dialog: None,
            show_diff_overview: false,
            search_window_open: false,
            focus_search_window: false,
        };

        let startup_files: Vec<PathBuf> = std::env::args_os().skip(1).map(PathBuf::from).collect();
        let restore = app.state.open_files.clone();
        for path in restore.into_iter().chain(startup_files) {
            if path.is_file() {
                app.open_path(&path, false);
            }
        }
        if let Some(wanted) = app.state.active_file.clone()
            && let Some(index) = app
                .documents
                .iter()
                .position(|d| d.path.as_ref() == Some(&wanted))
        {
            app.active = Some(index);
        }
        app
    }

    fn apply_theme(ctx: &egui::Context, theme: ThemeChoice) {
        ctx.set_theme(match theme {
            ThemeChoice::System => ThemePreference::System,
            ThemeChoice::Dark => ThemePreference::Dark,
            ThemeChoice::Light => ThemePreference::Light,
        });
        ctx.style_mut_of(egui::Theme::Dark, |style| {
            style.spacing.scroll = egui::style::ScrollStyle::solid();
            style.spacing.scroll.bar_width = 16.0;
            style.spacing.scroll.bar_inner_margin = 0.0;
            style.spacing.scroll.bar_outer_margin = 0.0;
            style.spacing.scroll.handle_min_length = 48.0;
            style.spacing.scroll.foreground_color = true;
            style.visuals.selection.bg_fill = Color32::from_rgb(45, 67, 82);
            style.visuals.selection.stroke =
                egui::Stroke::new(1.0, Color32::from_rgb(225, 233, 238));
            style.visuals.extreme_bg_color = Color32::from_rgb(15, 18, 22);
        });
        ctx.style_mut_of(egui::Theme::Light, |style| {
            style.spacing.scroll = egui::style::ScrollStyle::solid();
            style.spacing.scroll.bar_width = 16.0;
            style.spacing.scroll.bar_inner_margin = 0.0;
            style.spacing.scroll.bar_outer_margin = 0.0;
            style.spacing.scroll.handle_min_length = 48.0;
            style.spacing.scroll.foreground_color = true;
            style.visuals.selection.bg_fill = Color32::from_rgb(178, 205, 226);
            style.visuals.selection.stroke = egui::Stroke::new(1.0, Color32::from_rgb(30, 45, 56));
            style.visuals.extreme_bg_color = Color32::from_rgb(250, 252, 254);
        });
    }

    fn set_message(&mut self, text: impl Into<String>, error: bool) {
        self.message = Some((text.into(), error));
    }

    fn sync_state(&mut self) {
        self.state.open_files = self
            .documents
            .iter()
            .filter_map(|d| d.path.clone())
            .collect();
        self.state.active_file = self
            .active
            .and_then(|i| self.documents.get(i))
            .and_then(|d| d.path.clone());
        self.state.store();
    }

    fn open_path(&mut self, path: &Path, report_errors: bool) {
        let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        if let Some(index) = self.documents.iter().position(|d| {
            d.path
                .as_ref()
                .map(|p| p.canonicalize().unwrap_or_else(|_| p.clone()))
                == Some(canonical.clone())
        }) {
            self.active = Some(index);
            return;
        }
        match Document::open(&canonical) {
            Ok(document) => {
                self.state.last_directory = canonical.parent().map(Path::to_path_buf);
                self.documents.push(document);
                self.active = Some(self.documents.len() - 1);
                self.sync_state();
            }
            Err(error) if report_errors => self.set_message(
                format!("Could not open {}: {error:#}", path.display()),
                true,
            ),
            Err(_) => {}
        }
    }

    fn open_dialog(&mut self) {
        let mut dialog = FileDialog::new()
            .set_title("Open TLK or 2DA files")
            .add_filter("Aurora files", &["tlk", "2da", "itp"])
            .add_filter("Talk tables", &["tlk"])
            .add_filter("2DA tables", &["2da"])
            .add_filter("ITP palettes", &["itp"]);
        if let Some(dir) = &self.state.last_directory {
            dialog = dialog.set_directory(dir);
        }
        if let Some(paths) = dialog.pick_files() {
            for path in paths {
                self.open_path(&path, true);
            }
        }
    }

    fn save_document(&mut self, index: usize, force_as: bool) -> bool {
        if index >= self.documents.len() {
            return false;
        }
        if force_as || self.documents[index].path.is_none() {
            let ext = self.documents[index].default_extension();
            let mut dialog = FileDialog::new()
                .set_title("Save document")
                .set_file_name(self.documents[index].title())
                .add_filter(self.documents[index].kind_name(), &[ext]);
            if let Some(dir) = &self.state.last_directory {
                dialog = dialog.set_directory(dir);
            }
            let Some(mut path) = dialog.save_file() else {
                return false;
            };
            if path.extension().is_none() {
                path.set_extension(ext);
            }
            self.state.last_directory = path.parent().map(Path::to_path_buf);
            if let Err(error) = self.documents[index].save_as(path.clone()) {
                self.set_message(format!("Save failed: {error:#}"), true);
                return false;
            }
            self.set_message(format!("Saved {}", path.display()), false);
        } else {
            let title = self.documents[index].title();
            if let Err(error) = self.documents[index].save() {
                self.set_message(format!("Save failed: {error:#}"), true);
                return false;
            }
            self.set_message(format!("Saved {title}"), false);
        }
        self.sync_state();
        true
    }

    fn remove_document(&mut self, index: usize) {
        if index >= self.documents.len() {
            return;
        }
        self.documents.remove(index);
        self.active = if self.documents.is_empty() {
            None
        } else {
            Some(self.active.unwrap_or(0).min(self.documents.len() - 1))
        };
        self.sync_state();
    }

    fn displayed_strref(&self, row: usize) -> String {
        let value = row as u32
            + if self.state.display_user_strref {
                1 << 24
            } else {
                0
            };
        if self.state.display_hex_strref {
            format!("0x{value:x}")
        } else {
            value.to_string()
        }
    }

    fn tlk_rows_inserted(
        modified: &mut std::collections::BTreeSet<usize>,
        index: usize,
        count: usize,
    ) {
        *modified = modified
            .iter()
            .map(|row| if *row >= index { row + count } else { *row })
            .collect();
        modified.extend(index..index + count);
    }

    fn tlk_rows_deleted(
        modified: &mut std::collections::BTreeSet<usize>,
        index: usize,
        count: usize,
    ) {
        *modified = modified
            .iter()
            .filter_map(|row| {
                if *row < index {
                    Some(*row)
                } else if *row >= index + count {
                    Some(row - count)
                } else {
                    None
                }
            })
            .collect();
    }

    fn selected_rows(document: &Document, count: usize) -> Vec<usize> {
        let mut rows: Vec<_> = document
            .selected_rows
            .iter()
            .copied()
            .filter(|row| *row < count)
            .collect();
        if rows.is_empty()
            && let Some(row) = document.selected_row.filter(|row| *row < count)
        {
            rows.push(row);
        }
        rows
    }

    fn update_row_selection(
        selected_row: &mut Option<usize>,
        selected_rows: &mut BTreeSet<usize>,
        selection_anchor: &mut Option<usize>,
        selected_column: &mut Option<usize>,
        row: usize,
        column: Option<usize>,
        modifiers: egui::Modifiers,
    ) {
        if modifiers.shift {
            let anchor = selection_anchor.or(*selected_row).unwrap_or(row);
            selected_rows.clear();
            selected_rows.extend(anchor.min(row)..=anchor.max(row));
        } else if modifiers.ctrl || modifiers.command {
            if !selected_rows.remove(&row) {
                selected_rows.insert(row);
            }
            *selection_anchor = Some(row);
        } else {
            selected_rows.clear();
            selected_rows.insert(row);
            *selection_anchor = Some(row);
        }
        *selected_row = if selected_rows.contains(&row) {
            Some(row)
        } else {
            selected_rows.iter().next_back().copied()
        };
        *selected_column = column;
    }

    fn update_cell_selection(
        selected_cells: &mut BTreeSet<(usize, usize)>,
        cell_selection_anchor: &mut Option<(usize, usize)>,
        row: usize,
        column: usize,
        modifiers: egui::Modifiers,
    ) {
        if modifiers.shift {
            let anchor = cell_selection_anchor.unwrap_or((row, column));
            selected_cells.clear();
            for selected_row in anchor.0.min(row)..=anchor.0.max(row) {
                for selected_column in anchor.1.min(column)..=anchor.1.max(column) {
                    selected_cells.insert((selected_row, selected_column));
                }
            }
        } else if modifiers.ctrl || modifiers.command {
            if !selected_cells.remove(&(row, column)) {
                selected_cells.insert((row, column));
            }
            *cell_selection_anchor = Some((row, column));
        } else {
            selected_cells.clear();
            selected_cells.insert((row, column));
            *cell_selection_anchor = Some((row, column));
        }
    }

    fn keyboard_row_selection(
        ui: &mut egui::Ui,
        selected_row: &mut Option<usize>,
        selected_rows: &mut BTreeSet<usize>,
        selection_anchor: &mut Option<usize>,
        selected_column: &mut Option<usize>,
        scroll_to_selected: &mut bool,
        rows: &[usize],
    ) {
        if rows.is_empty() || selected_column.is_some() {
            return;
        }
        let movement = ui.input_mut(|input| {
            if input.consume_key(egui::Modifiers::SHIFT, egui::Key::ArrowUp) {
                Some((-1_isize, true))
            } else if input.consume_key(egui::Modifiers::SHIFT, egui::Key::ArrowDown) {
                Some((1, true))
            } else if input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp) {
                Some((-1, false))
            } else if input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown) {
                Some((1, false))
            } else {
                None
            }
        });
        let Some((direction, extend)) = movement else {
            return;
        };
        let current = selected_row
            .and_then(|selected| rows.iter().position(|row| *row == selected))
            .unwrap_or(0);
        let target = current.saturating_add_signed(direction).min(rows.len() - 1);
        Self::update_row_selection(
            selected_row,
            selected_rows,
            selection_anchor,
            selected_column,
            rows[target],
            None,
            if extend {
                egui::Modifiers::SHIFT
            } else {
                egui::Modifiers::NONE
            },
        );
        *scroll_to_selected = true;
    }

    fn row_action_context_menu(
        response: &egui::Response,
        row: usize,
        selected_count: usize,
        action: &mut Option<RowMenuAction>,
    ) {
        response.context_menu(|ui| {
            if ui.button("Add row above").clicked() {
                *action = Some(RowMenuAction::InsertAbove(row));
                ui.close();
            }
            if ui.button("Add row below").clicked() {
                *action = Some(RowMenuAction::InsertBelow(row));
                ui.close();
            }
            ui.separator();
            if ui
                .button(if selected_count == 1 {
                    "Delete selected row".to_owned()
                } else {
                    format!("Delete {selected_count} selected rows")
                })
                .clicked()
            {
                *action = Some(RowMenuAction::Delete);
                ui.close();
            }
        });
    }

    fn empty_table_context_menu(
        ui: &mut egui::Ui,
        id: Id,
        viewport: egui::Rect,
        rendered_rows: usize,
        row_step: f32,
        action: &mut Option<RowMenuAction>,
    ) {
        let top = (viewport.top() + rendered_rows as f32 * row_step).min(viewport.bottom());
        let empty_rect = egui::Rect::from_min_max(
            egui::pos2(viewport.left(), top),
            egui::pos2(viewport.right(), viewport.bottom()),
        );
        if !empty_rect.is_positive() {
            return;
        }
        let response = ui.interact(empty_rect, id, egui::Sense::click());
        response.context_menu(|ui| {
            if ui.button("Add new row").clicked() {
                *action = Some(RowMenuAction::InsertBelow(usize::MAX));
                ui.close();
            }
        });
    }

    fn copy_selected_row(&mut self, ctx: &egui::Context) -> bool {
        let Some(index) = self.active else {
            return false;
        };
        let Some(document) = self.documents.get(index) else {
            return false;
        };
        match &document.data {
            DocumentData::Tlk(tlk) => {
                let rows = Self::selected_rows(document, tlk.entries.len());
                if rows.is_empty() {
                    return false;
                }
                let copied: Vec<_> = rows
                    .iter()
                    .map(|row| (*row, tlk.entries[*row].clone()))
                    .collect();
                let text = copied
                    .iter()
                    .map(|(row, entry)| {
                        format!(
                            "{}\t{}\t{}\t{}\t{}",
                            self.displayed_strref(*row),
                            entry.flags,
                            entry.sound_resref,
                            entry.sound_length,
                            entry.text.replace('\t', "\\t").replace('\n', "\\n")
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                ctx.copy_text(text.clone());
                self.clipboard_text = Some(text);
                self.clipboard = Some(ClipboardRows::Tlk(copied));
                self.set_message(format!("Copied {} TLK row(s)", rows.len()), false);
                true
            }
            DocumentData::TwoDa(table) => {
                if !document.selected_cells.is_empty() || !document.selected_columns.is_empty() {
                    let source_row = document.selected_row.unwrap_or(0);
                    let Some(source) = table.rows.get(source_row) else {
                        return false;
                    };
                    let mut columns: BTreeSet<_> = document
                        .selected_cells
                        .iter()
                        .filter(|(row, column)| *row == source_row && *column < table.columns.len())
                        .map(|(_, column)| *column)
                        .collect();
                    columns.extend(
                        document
                            .selected_columns
                            .iter()
                            .copied()
                            .filter(|column| *column < table.columns.len()),
                    );
                    let copied: Vec<_> = columns
                        .into_iter()
                        .map(|column| (column, source[column].clone()))
                        .collect();
                    if copied.is_empty() {
                        return false;
                    }
                    let text = copied
                        .iter()
                        .map(|(_, value)| value.as_str())
                        .collect::<Vec<_>>()
                        .join("\t");
                    ctx.copy_text(text.clone());
                    self.clipboard_text = Some(text);
                    self.clipboard = Some(ClipboardRows::TwoDaCells(copied));
                    self.set_message("Copied selected 2DA field(s)", false);
                    return true;
                }
                let rows = Self::selected_rows(document, table.rows.len());
                if rows.is_empty() {
                    return false;
                }
                let copied: Vec<_> = rows.iter().map(|row| table.rows[*row].clone()).collect();
                let text = copied
                    .iter()
                    .map(|row| row.join("\t"))
                    .collect::<Vec<_>>()
                    .join("\n");
                ctx.copy_text(text.clone());
                self.clipboard_text = Some(text);
                self.clipboard = Some(ClipboardRows::TwoDa(copied));
                self.set_message(format!("Copied {} 2DA row(s)", rows.len()), false);
                true
            }
            DocumentData::Itp(_) => false,
        }
    }

    fn vertical_table_scrollbar(
        ui: &mut egui::Ui,
        id: Id,
        viewport: egui::Rect,
        row_count: usize,
        visible_rows: usize,
        first_row: &mut usize,
    ) {
        let max_first = row_count.saturating_sub(visible_rows);
        *first_row = (*first_row).min(max_first);

        let width = ui.spacing().scroll.bar_width.max(12.0);
        let track = egui::Rect::from_min_max(
            egui::pos2(viewport.right() - width, viewport.top()),
            egui::pos2(viewport.right(), viewport.bottom() - width),
        );
        let response = ui.interact(track, id, egui::Sense::click_and_drag());

        let wheel_delta = ui.input(|input| {
            if viewport
                .union(track)
                .contains(input.pointer.hover_pos().unwrap_or_default())
            {
                input.smooth_scroll_delta.y
            } else {
                0.0
            }
        });
        if wheel_delta != 0.0 {
            let rows = (wheel_delta.abs() / 12.0).ceil().max(1.0) as usize;
            if wheel_delta < 0.0 {
                *first_row = first_row.saturating_add(rows).min(max_first);
            } else {
                *first_row = first_row.saturating_sub(rows);
            }
            ui.ctx().request_repaint();
        }

        let handle_height = if row_count > 0 {
            (track.height() * visible_rows as f32 / row_count as f32)
                .clamp(ui.spacing().scroll.handle_min_length, track.height())
        } else {
            track.height()
        };
        if (response.dragged() || response.clicked())
            && let Some(pointer) = response.interact_pointer_pos()
        {
            let travel = (track.height() - handle_height).max(1.0);
            let position = (pointer.y - track.top() - handle_height * 0.5).clamp(0.0, travel);
            *first_row = (position / travel * max_first as f32).round() as usize;
            ui.ctx().request_repaint();
        }

        let travel = (track.height() - handle_height).max(0.0);
        let handle_top = track.top()
            + if max_first > 0 {
                travel * *first_row as f32 / max_first as f32
            } else {
                0.0
            };
        let handle = egui::Rect::from_min_size(
            egui::pos2(track.left(), handle_top),
            egui::vec2(track.width(), handle_height),
        )
        .shrink(2.0);
        let visuals = ui.style().interact(&response);
        ui.painter()
            .rect_filled(track, 2.0, ui.visuals().extreme_bg_color);
        ui.painter().rect_filled(handle, 3.0, visuals.bg_fill);
        ui.painter()
            .rect_stroke(handle, 3.0, visuals.bg_stroke, egui::StrokeKind::Inside);
    }

    fn horizontal_table_scrollbar(
        ui: &mut egui::Ui,
        id: Id,
        viewport: egui::Rect,
        content_width: f32,
        offset: &mut f32,
    ) {
        let width = ui.spacing().scroll.bar_width.max(12.0);
        let track = egui::Rect::from_min_max(
            egui::pos2(viewport.left(), viewport.bottom() - width),
            egui::pos2(viewport.right() - width, viewport.bottom()),
        );
        let max_offset = (content_width - viewport.width()).max(0.0);
        *offset = offset.clamp(0.0, max_offset);
        let response = ui.interact(track, id, egui::Sense::click_and_drag());
        let handle_width = if content_width > 0.0 {
            (track.width() * viewport.width() / content_width)
                .clamp(ui.spacing().scroll.handle_min_length, track.width())
        } else {
            track.width()
        };
        if (response.dragged() || response.clicked())
            && let Some(pointer) = response.interact_pointer_pos()
        {
            let travel = (track.width() - handle_width).max(1.0);
            let position = (pointer.x - track.left() - handle_width * 0.5).clamp(0.0, travel);
            *offset = (position / travel * max_offset).clamp(0.0, max_offset);
            ui.ctx().request_repaint();
        }
        let travel = (track.width() - handle_width).max(0.0);
        let handle_left = track.left()
            + if max_offset > 0.0 {
                travel * *offset / max_offset
            } else {
                0.0
            };
        let handle = egui::Rect::from_min_size(
            egui::pos2(handle_left, track.top()),
            egui::vec2(handle_width, track.height()),
        )
        .shrink(2.0);
        let visuals = ui.style().interact(&response);
        ui.painter()
            .rect_filled(track, 2.0, ui.visuals().extreme_bg_color);
        ui.painter().rect_filled(handle, 3.0, visuals.bg_fill);
        ui.painter()
            .rect_stroke(handle, 3.0, visuals.bg_stroke, egui::StrokeKind::Inside);
    }

    fn middle_mouse_table_scroll(
        ui: &mut egui::Ui,
        viewport: egui::Rect,
        row_count: usize,
        visible_rows: usize,
        content_width: f32,
        first_row: &mut usize,
        horizontal_offset: &mut f32,
    ) {
        let active = ui.input(|input| {
            input.pointer.button_down(egui::PointerButton::Middle)
                && input
                    .pointer
                    .hover_pos()
                    .is_some_and(|pos| viewport.contains(pos))
        });
        if active {
            let delta = ui.input(|input| input.pointer.delta());
            let max_first = row_count.saturating_sub(visible_rows);
            let vertical_rows = (delta.y.abs() * 0.75).round() as usize;
            if delta.y > 0.0 {
                *first_row = first_row.saturating_add(vertical_rows).min(max_first);
            } else if delta.y < 0.0 {
                *first_row = first_row.saturating_sub(vertical_rows);
            }

            let max_horizontal = (content_width - viewport.width()).max(0.0);
            *horizontal_offset = (*horizontal_offset + delta.x * 3.0).clamp(0.0, max_horizontal);
            ui.ctx().set_cursor_icon(egui::CursorIcon::AllScroll);
            ui.ctx().request_repaint();
        }
    }

    fn delete_selected_row(&mut self) -> bool {
        let Some(index) = self.active else {
            return false;
        };
        let Some(document) = self.documents.get_mut(index) else {
            return false;
        };
        Self::delete_document_rows(document) > 0
    }

    fn delete_document_rows(document: &mut Document) -> usize {
        let count = match &document.data {
            DocumentData::Tlk(tlk) => tlk.entries.len(),
            DocumentData::TwoDa(table) => table.rows.len(),
            DocumentData::Itp(_) => return 0,
        };
        let mut rows = Self::selected_rows(document, count);
        if rows.is_empty() {
            return 0;
        }
        let next_row = rows[0];
        rows.reverse();
        let mut actions = Vec::with_capacity(rows.len());
        for row in rows {
            match &mut document.data {
                DocumentData::Tlk(tlk) => {
                    let removed = tlk.entries.remove(row);
                    Self::tlk_rows_deleted(&mut document.tlk_modified, row, 1);
                    actions.push(EditAction::TlkRows {
                        index: row,
                        removed: vec![removed],
                        inserted: Vec::new(),
                    });
                }
                DocumentData::TwoDa(table) => {
                    actions.push(EditAction::TwoDaRows {
                        index: row,
                        removed: vec![table.rows.remove(row)],
                        inserted: Vec::new(),
                    });
                }
                DocumentData::Itp(_) => unreachable!(),
            }
        }
        let removed = actions.len();
        document.record(if actions.len() == 1 {
            actions.pop().unwrap()
        } else {
            EditAction::Batch(actions)
        });
        let remaining = match &document.data {
            DocumentData::Tlk(tlk) => tlk.entries.len(),
            DocumentData::TwoDa(table) => table.rows.len(),
            DocumentData::Itp(_) => 0,
        };
        document.selected_rows.clear();
        document.selected_row = (remaining > 0).then(|| next_row.min(remaining - 1));
        if let Some(row) = document.selected_row {
            document.selected_rows.insert(row);
        }
        document.selection_anchor = document.selected_row;
        document.scroll_to_selected = document.selected_row.is_some();
        removed
    }

    fn apply_row_menu_action(document: &mut Document, action: RowMenuAction) {
        if matches!(action, RowMenuAction::Delete) {
            Self::delete_document_rows(document);
            return;
        }
        let requested = match action {
            RowMenuAction::InsertAbove(row) => row,
            RowMenuAction::InsertBelow(row) => row.saturating_add(1),
            RowMenuAction::Delete => unreachable!(),
        };
        let position = match &mut document.data {
            DocumentData::Tlk(tlk) => {
                let position = requested.min(tlk.entries.len());
                let entry = TlkEntry::default();
                tlk.entries.insert(position, entry.clone());
                Self::tlk_rows_inserted(&mut document.tlk_modified, position, 1);
                document.history.record(EditAction::TlkRows {
                    index: position,
                    removed: Vec::new(),
                    inserted: vec![entry],
                });
                position
            }
            DocumentData::TwoDa(table) => {
                let position = requested.min(table.rows.len());
                let mut row = vec!["****".to_owned(); table.columns.len()];
                if let Some(label) = row.first_mut() {
                    *label = position.to_string();
                }
                table.rows.insert(position, row.clone());
                document.history.record(EditAction::TwoDaRows {
                    index: position,
                    removed: Vec::new(),
                    inserted: vec![row],
                });
                position
            }
            DocumentData::Itp(_) => return,
        };
        document.selected_rows.clear();
        document.selected_rows.insert(position);
        document.selected_row = Some(position);
        document.selection_anchor = Some(position);
        document.selected_column = None;
        document.scroll_to_selected = true;
        document.dirty = !document.history.is_clean();
    }

    fn fully_selected_twoda_columns(document: &Document) -> Vec<usize> {
        let DocumentData::TwoDa(table) = &document.data else {
            return Vec::new();
        };
        if !document.selected_columns.is_empty() {
            return document
                .selected_columns
                .iter()
                .copied()
                .filter(|column| *column < table.columns.len())
                .collect();
        }
        (0..table.columns.len())
            .filter(|column| {
                (0..table.rows.len()).all(|row| document.selected_cells.contains(&(row, *column)))
            })
            .collect()
    }

    fn delete_twoda_columns(document: &mut Document, mut columns: Vec<usize>) -> usize {
        let DocumentData::TwoDa(table) = &mut document.data else {
            return 0;
        };
        columns.sort_unstable();
        columns.dedup();
        columns.retain(|column| *column < table.columns.len());
        if columns.is_empty() || columns.len() >= table.columns.len() {
            return 0;
        }
        let before = table.clone();
        for column in columns.iter().copied().rev() {
            table.columns.remove(column);
            for row in &mut table.rows {
                if column < row.len() {
                    row.remove(column);
                }
            }
        }
        let after = table.clone();
        document.selected_cells.clear();
        document.selected_columns.clear();
        document.cell_selection_anchor = None;
        document.selected_column = None;
        document.record(EditAction::TwoDaTable { before, after });
        columns.len()
    }

    fn cut_selected_row(&mut self, ctx: &egui::Context) {
        if self.copy_selected_row(ctx) && self.delete_selected_row() {
            self.set_message("Cut selected row(s)", false);
        }
    }

    fn paste_rows(&mut self) {
        let external = self.pending_paste_text.take();
        let clipboard = if external
            .as_ref()
            .is_some_and(|text| self.clipboard_text.as_ref() != Some(text))
        {
            let text = external.unwrap();
            let is_tlk = self
                .active
                .and_then(|index| self.documents.get(index))
                .is_some_and(|document| matches!(document.data, DocumentData::Tlk(_)));
            if is_tlk {
                let rows = text
                    .lines()
                    .filter(|line| !line.is_empty())
                    .enumerate()
                    .map(|(index, line)| {
                        let parts: Vec<_> = line.splitn(5, '\t').collect();
                        let entry = if parts.len() == 5 {
                            TlkEntry {
                                flags: parts[1].parse().unwrap_or(1),
                                sound_resref: parts[2].to_owned(),
                                sound_length: parts[3].parse().unwrap_or(0.0),
                                text: parts[4].replace("\\n", "\n").replace("\\t", "\t"),
                                ..Default::default()
                            }
                        } else {
                            TlkEntry {
                                flags: 1,
                                text: line.to_owned(),
                                ..Default::default()
                            }
                        };
                        (index, entry)
                    })
                    .collect();
                Some(ClipboardRows::Tlk(rows))
            } else {
                let rows = text
                    .lines()
                    .filter(|line| !line.trim().is_empty())
                    .map(|line| {
                        if line.contains('\t') {
                            line.split('\t').map(str::to_owned).collect()
                        } else {
                            line.split_whitespace().map(str::to_owned).collect()
                        }
                    })
                    .collect();
                Some(ClipboardRows::TwoDa(rows))
            }
        } else {
            self.clipboard.clone()
        };
        let Some(clipboard) = clipboard else {
            self.set_message("Copy a TLK or 2DA row first", true);
            return;
        };
        let Some(index) = self.active else { return };
        let document = &mut self.documents[index];
        let insert_at = document.selected_row.map_or_else(
            || match &document.data {
                DocumentData::Tlk(tlk) => tlk.entries.len(),
                DocumentData::TwoDa(table) => table.rows.len(),
                DocumentData::Itp(_) => 0,
            },
            |row| row + 1,
        );
        let mut inserted = 0;
        let mut type_mismatch = false;
        let mut action = None;
        let mut pasted_cells = None;
        match (&mut document.data, clipboard) {
            (DocumentData::Tlk(tlk), ClipboardRows::Tlk(rows)) => {
                inserted = rows.len();
                let entries: Vec<_> = rows.into_iter().map(|(_, entry)| entry).collect();
                tlk.entries.splice(insert_at..insert_at, entries.clone());
                Self::tlk_rows_inserted(&mut document.tlk_modified, insert_at, inserted);
                action = Some(EditAction::TlkRows {
                    index: insert_at,
                    removed: Vec::new(),
                    inserted: entries,
                });
            }
            (DocumentData::TwoDa(table), ClipboardRows::TwoDaCells(cells)) => {
                if let Some(destination_row) = document.selected_row
                    && destination_row < table.rows.len()
                {
                    let mut actions = Vec::new();
                    let mut selected = BTreeSet::new();
                    for (column, value) in cells {
                        if let Some(cell) = table.rows[destination_row].get_mut(column) {
                            let before = std::mem::replace(cell, value);
                            if *cell != before {
                                actions.push(EditAction::TwoDaCell {
                                    row: destination_row,
                                    column,
                                    before,
                                    after: cell.clone(),
                                });
                            }
                            selected.insert((destination_row, column));
                        }
                    }
                    inserted = selected.len();
                    pasted_cells = Some(selected);
                    action = match actions.len() {
                        0 => None,
                        1 => actions.pop(),
                        _ => Some(EditAction::Batch(actions)),
                    };
                }
            }
            (DocumentData::TwoDa(table), ClipboardRows::TwoDa(rows)) => {
                let width = table.columns.len();
                let rows: Vec<_> = rows
                    .into_iter()
                    .map(|mut row| {
                        row.resize(width, "****".to_owned());
                        row.truncate(width);
                        row
                    })
                    .collect();
                inserted = rows.len();
                table.rows.splice(insert_at..insert_at, rows.clone());
                action = Some(EditAction::TwoDaRows {
                    index: insert_at,
                    removed: Vec::new(),
                    inserted: rows,
                });
            }
            (DocumentData::TwoDa(table), ClipboardRows::Tlk(rows)) => {
                if let (Some(row), Some(column), Some((strref, _))) = (
                    document.selected_row,
                    document.selected_column,
                    rows.first(),
                ) && let Some(cell) = table.rows.get_mut(row).and_then(|r| r.get_mut(column))
                {
                    let value = *strref as u32
                        + if self.state.display_user_strref {
                            1 << 24
                        } else {
                            0
                        };
                    let before = cell.clone();
                    *cell = if self.state.display_hex_strref {
                        format!("0x{value:x}")
                    } else {
                        value.to_string()
                    };
                    action = Some(EditAction::TwoDaCell {
                        row,
                        column,
                        before,
                        after: cell.clone(),
                    });
                    inserted = 1;
                }
            }
            _ => type_mismatch = true,
        }
        if inserted > 0 {
            if let Some(action) = action {
                document.record(action);
            }
            if let Some(cells) = pasted_cells {
                document.selected_cells = cells;
                document.cell_selection_anchor = document.selected_cells.iter().next().copied();
                document.selected_rows.clear();
                document.selection_anchor = None;
            } else {
                document.selected_row = Some(insert_at.min(match &document.data {
                    DocumentData::Tlk(tlk) => tlk.entries.len().saturating_sub(1),
                    DocumentData::TwoDa(table) => table.rows.len().saturating_sub(1),
                    DocumentData::Itp(_) => 0,
                }));
                document.selected_rows.clear();
                if let Some(row) = document.selected_row {
                    document.selected_rows.insert(row);
                }
                document.selection_anchor = document.selected_row;
                document.selected_cells.clear();
                document.selected_columns.clear();
                document.cell_selection_anchor = None;
            }
            document.scroll_to_selected = true;
            let message =
                if document.selected_cells.is_empty() && document.selected_columns.is_empty() {
                    format!("Pasted {inserted} row(s)")
                } else {
                    format!("Pasted {inserted} field(s)")
                };
            self.set_message(message, false);
        } else if type_mismatch {
            self.set_message("The copied rows are for a different file type", true);
        }
    }

    fn request_close(&mut self, index: usize) {
        if self.documents.get(index).is_some_and(|d| d.dirty) {
            self.pending_close = Some(PendingClose {
                index,
                quitting: false,
            });
        } else {
            self.remove_document(index);
        }
    }

    fn request_quit(&mut self, ctx: &egui::Context) {
        if let Some(index) = self.documents.iter().position(|d| d.dirty) {
            self.pending_close = Some(PendingClose {
                index,
                quitting: true,
            });
            self.active = Some(index);
        } else {
            self.allow_exit = true;
            self.sync_state();
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }

    fn continue_quit(&mut self, ctx: &egui::Context) {
        self.pending_close = None;
        self.request_quit(ctx);
    }

    fn handle_close_choice(&mut self, ctx: &egui::Context, choice: CloseChoice) {
        let Some(pending) = self.pending_close else {
            return;
        };
        match choice {
            CloseChoice::Cancel => self.pending_close = None,
            CloseChoice::Save => {
                if self.save_document(pending.index, false) {
                    if pending.quitting {
                        self.continue_quit(ctx);
                    } else {
                        self.pending_close = None;
                        self.remove_document(pending.index);
                    }
                }
            }
            CloseChoice::Discard => {
                if pending.quitting {
                    if let Some(doc) = self.documents.get_mut(pending.index) {
                        doc.dirty = false;
                    }
                    self.continue_quit(ctx);
                } else {
                    self.pending_close = None;
                    self.remove_document(pending.index);
                }
            }
        }
    }

    fn open_resize_dialog(&mut self) {
        let value = self
            .active
            .and_then(|index| self.documents.get(index))
            .map(|document| {
                match &document.data {
                    DocumentData::Tlk(tlk) => tlk.entries.len(),
                    DocumentData::TwoDa(table) => table.rows.len(),
                    DocumentData::Itp(_) => 0,
                }
                .to_string()
            });
        self.resize_value = value;
    }

    fn resize_active_table(&mut self, new_size: usize) {
        let Some(document) = self.active.and_then(|index| self.documents.get_mut(index)) else {
            return;
        };
        let action = match &mut document.data {
            DocumentData::Tlk(tlk) => {
                let old_size = tlk.entries.len();
                if new_size > old_size {
                    let inserted = vec![TlkEntry::default(); new_size - old_size];
                    tlk.entries.extend(inserted.clone());
                    Self::tlk_rows_inserted(&mut document.tlk_modified, old_size, inserted.len());
                    Some(EditAction::TlkRows {
                        index: old_size,
                        removed: Vec::new(),
                        inserted,
                    })
                } else if new_size < old_size {
                    let removed = tlk.entries.split_off(new_size);
                    Self::tlk_rows_deleted(&mut document.tlk_modified, new_size, removed.len());
                    Some(EditAction::TlkRows {
                        index: new_size,
                        removed,
                        inserted: Vec::new(),
                    })
                } else {
                    None
                }
            }
            DocumentData::TwoDa(table) => {
                let old_size = table.rows.len();
                if new_size > old_size {
                    let inserted: Vec<Vec<String>> = (old_size..new_size)
                        .map(|row_index| {
                            let mut row = vec!["****".to_owned(); table.columns.len()];
                            if let Some(first) = row.first_mut() {
                                *first = row_index.to_string();
                            }
                            row
                        })
                        .collect();
                    table.rows.extend(inserted.clone());
                    Some(EditAction::TwoDaRows {
                        index: old_size,
                        removed: Vec::new(),
                        inserted,
                    })
                } else if new_size < old_size {
                    let removed = table.rows.split_off(new_size);
                    Some(EditAction::TwoDaRows {
                        index: new_size,
                        removed,
                        inserted: Vec::new(),
                    })
                } else {
                    None
                }
            }
            DocumentData::Itp(_) => None,
        };
        if let Some(action) = action {
            document.record(action);
            document.selected_row = new_size.checked_sub(1);
            document.selected_rows.clear();
            if let Some(row) = document.selected_row {
                document.selected_rows.insert(row);
            }
            document.selection_anchor = document.selected_row;
        }
    }

    fn renumber_twoda(&mut self) {
        let Some(document) = self.active.and_then(|index| self.documents.get_mut(index)) else {
            return;
        };
        let DocumentData::TwoDa(table) = &mut document.data else {
            return;
        };
        let before = table.clone();
        for (index, row) in table.rows.iter_mut().enumerate() {
            if let Some(value) = row.first_mut()
                && !value.starts_with('!')
            {
                *value = index.to_string();
            }
        }
        if *table != before {
            let after = table.clone();
            document.record(EditAction::TwoDaTable { before, after });
        }
    }

    fn toggle_user_flag(&mut self) {
        let Some(index) = self.active else {
            return;
        };
        let document = &mut self.documents[index];
        let mut edit = None;
        match &mut document.data {
            DocumentData::Tlk(_) => {
                self.state.display_user_strref = !self.state.display_user_strref;
                self.state.store();
            }
            DocumentData::TwoDa(table) => {
                let (Some(row), Some(column)) = (document.selected_row, document.selected_column)
                else {
                    return;
                };
                let Some(cell) = table.rows.get_mut(row).and_then(|row| row.get_mut(column)) else {
                    return;
                };
                let before = cell.clone();
                let parsed = if let Some(hex) =
                    cell.strip_prefix("0x").or_else(|| cell.strip_prefix("0X"))
                {
                    u32::from_str_radix(hex, 16).ok().map(|value| (value, true))
                } else {
                    cell.parse::<u32>().ok().map(|value| (value, false))
                };
                if let Some((value, hex)) = parsed {
                    let value = value ^ (1 << 24);
                    *cell = if hex {
                        format!("0x{value:x}")
                    } else {
                        value.to_string()
                    };
                    edit = Some(EditAction::TwoDaCell {
                        row,
                        column,
                        before,
                        after: cell.clone(),
                    });
                }
            }
            DocumentData::Itp(_) => {}
        }
        if let Some(edit) = edit {
            document.record(edit);
        }
    }

    fn find_next(&mut self) {
        self.find_match(false);
    }

    fn find_previous(&mut self) {
        self.find_match(true);
    }

    fn find_match(&mut self, backwards: bool) {
        let display_user_strref = self.state.display_user_strref;
        let display_hex_strref = self.state.display_hex_strref;
        let Some(document) = self.active.and_then(|index| self.documents.get_mut(index)) else {
            return;
        };
        if document.find_query.is_empty() {
            self.search_window_open = true;
            self.focus_search_window = true;
            return;
        }
        let needle = document.find_query.trim().to_lowercase();
        let numeric_strref = if let Some(hex) = needle.strip_prefix("0x") {
            u32::from_str_radix(hex, 16).ok()
        } else {
            needle.parse::<u32>().ok()
        };
        let count = match &document.data {
            DocumentData::Tlk(tlk) => tlk.entries.len(),
            DocumentData::TwoDa(table) => table.rows.len(),
            DocumentData::Itp(_) => return,
        };
        if count == 0 {
            return;
        }
        let start = document.selected_row.map_or_else(
            || if backwards { count - 1 } else { 0 },
            |row| {
                if backwards {
                    (row + count - 1) % count
                } else {
                    (row + 1) % count
                }
            },
        );
        for offset in 0..count {
            let row = if backwards {
                (start + count - offset) % count
            } else {
                (start + offset) % count
            };
            let matches = match &document.data {
                DocumentData::Tlk(tlk) => {
                    let displayed_strref =
                        row as u32 + if display_user_strref { 1 << 24 } else { 0 };
                    numeric_strref == Some(displayed_strref)
                        || (display_hex_strref && format!("0x{displayed_strref:x}") == needle)
                        || tlk.entries[row].text.to_lowercase().contains(&needle)
                }
                DocumentData::TwoDa(table) => table.rows[row]
                    .iter()
                    .any(|value| value.to_lowercase().contains(&needle)),
                DocumentData::Itp(_) => false,
            };
            if matches {
                document.selected_row = Some(row);
                document.selected_rows.clear();
                document.selected_rows.insert(row);
                document.selection_anchor = Some(row);
                document.scroll_to_selected = true;
                return;
            }
        }
        self.set_message("No matching row", true);
    }

    fn replace_selected_match(&mut self) {
        let Some(document) = self.active.and_then(|index| self.documents.get_mut(index)) else {
            return;
        };
        if document.find_query.is_empty() {
            return;
        }
        let find = document.find_query.clone();
        let replacement = document.replace.clone();
        let (Some(row), column) = (document.selected_row, document.selected_column) else {
            return;
        };
        let action = match &mut document.data {
            DocumentData::Tlk(tlk) => tlk.entries.get_mut(row).and_then(|entry| {
                let before = entry.clone();
                entry.text = entry.text.replace(&find, &replacement);
                entry.flags = (entry.flags & !1) | u32::from(!entry.text.is_empty());
                (*entry != before).then(|| EditAction::TlkEntry {
                    row,
                    before,
                    after: entry.clone(),
                })
            }),
            DocumentData::TwoDa(table) => column.and_then(|column| {
                table
                    .rows
                    .get_mut(row)
                    .and_then(|values| values.get_mut(column))
                    .and_then(|cell| {
                        let before = cell.clone();
                        *cell = cell.replace(&find, &replacement);
                        (*cell != before).then(|| EditAction::TwoDaCell {
                            row,
                            column,
                            before,
                            after: cell.clone(),
                        })
                    })
            }),
            DocumentData::Itp(_) => None,
        };
        if let Some(action) = action {
            if matches!(action, EditAction::TlkEntry { .. }) {
                document.tlk_modified.insert(row);
            }
            document.record(action);
        }
    }

    fn replace_all_matches(&mut self) {
        let Some(document) = self.active.and_then(|index| self.documents.get_mut(index)) else {
            return;
        };
        if document.find_query.is_empty() {
            return;
        }
        let find = document.find_query.clone();
        let replacement = document.replace.clone();
        let mut actions = Vec::new();
        match &mut document.data {
            DocumentData::Tlk(tlk) => {
                for (row, entry) in tlk.entries.iter_mut().enumerate() {
                    let before = entry.clone();
                    entry.text = entry.text.replace(&find, &replacement);
                    entry.flags = (entry.flags & !1) | u32::from(!entry.text.is_empty());
                    if *entry != before {
                        document.tlk_modified.insert(row);
                        actions.push(EditAction::TlkEntry {
                            row,
                            before,
                            after: entry.clone(),
                        });
                    }
                }
            }
            DocumentData::TwoDa(table) => {
                for (row, values) in table.rows.iter_mut().enumerate() {
                    for (column, cell) in values.iter_mut().enumerate() {
                        let before = cell.clone();
                        *cell = cell.replace(&find, &replacement);
                        if *cell != before {
                            actions.push(EditAction::TwoDaCell {
                                row,
                                column,
                                before,
                                after: cell.clone(),
                            });
                        }
                    }
                }
            }
            DocumentData::Itp(_) => {}
        }
        let count = actions.len();
        for action in actions {
            document.history.record(action);
        }
        document.dirty = !document.history.is_clean();
        self.set_message(format!("Replaced {count} matching cells"), false);
    }

    fn open_column_dialog(&mut self) {
        let index = self
            .active
            .and_then(|index| self.documents.get(index))
            .and_then(|document| document.selected_column)
            .unwrap_or(1);
        let name = self
            .active
            .and_then(|active| self.documents.get(active))
            .and_then(|document| match &document.data {
                DocumentData::TwoDa(table) => table.columns.get(index).cloned(),
                _ => None,
            })
            .unwrap_or_default();
        self.column_dialog = Some(ColumnDialogState {
            index: index.to_string(),
            name,
            default_value: "****".to_owned(),
        });
    }

    fn mark_selected_tlk(&mut self, modified: bool) {
        let Some(document) = self.active.and_then(|index| self.documents.get_mut(index)) else {
            return;
        };
        let Some(row) = document.selected_row else {
            return;
        };
        if !matches!(document.data, DocumentData::Tlk(_)) {
            return;
        }
        if modified {
            document.tlk_modified.insert(row);
        } else {
            document.tlk_modified.remove(&row);
        }
    }

    fn save_tlk_diff(&mut self) {
        let Some(document) = self.active.and_then(|index| self.documents.get(index)) else {
            return;
        };
        let DocumentData::Tlk(tlk) = &document.data else {
            return;
        };
        let entries: Vec<_> = document
            .tlk_modified
            .iter()
            .filter_map(|row| tlk.entries.get(*row).cloned().map(|entry| (*row, entry)))
            .collect();
        let mut dialog = FileDialog::new()
            .set_title("Save TLK diff")
            .set_file_name("changes.diff")
            .add_filter("TlkEdit diff", &["diff"]);
        if let Some(dir) = &self.state.last_directory {
            dialog = dialog.set_directory(dir);
        }
        let Some(path) = dialog.save_file() else {
            return;
        };
        match aurora_tlk_explorer::formats::tlk::TlkFile::write_diff(&path, &entries) {
            Ok(()) => self.set_message(format!("Saved {} modified entries", entries.len()), false),
            Err(error) => self.set_message(format!("Could not save diff: {error:#}"), true),
        }
    }

    fn merge_tlk_entries(&mut self, entries: Vec<(usize, TlkEntry)>) {
        let count = entries.len();
        let Some(document) = self.active.and_then(|index| self.documents.get_mut(index)) else {
            return;
        };
        let DocumentData::Tlk(tlk) = &mut document.data else {
            return;
        };
        for (row, entry) in entries {
            if row >= tlk.entries.len() {
                let index = tlk.entries.len();
                let inserted = vec![TlkEntry::default(); row + 1 - index];
                tlk.entries.extend(inserted.clone());
                document.history.record(EditAction::TlkRows {
                    index,
                    removed: Vec::new(),
                    inserted,
                });
            }
            let before = std::mem::replace(&mut tlk.entries[row], entry.clone());
            document.history.record(EditAction::TlkEntry {
                row,
                before,
                after: entry,
            });
            document.tlk_modified.insert(row);
        }
        document.dirty = !document.history.is_clean();
        self.set_message(format!("Merged {count} TLK entries"), false);
    }

    fn merge_diff_dialog(&mut self, dtu: bool) {
        let title = if dtu {
            "Merge DialogTLK DTU"
        } else {
            "Merge TlkEdit diff"
        };
        let extension = if dtu { "dtu" } else { "diff" };
        let mut dialog = FileDialog::new()
            .set_title(title)
            .add_filter(title, &[extension]);
        if let Some(dir) = &self.state.last_directory {
            dialog = dialog.set_directory(dir);
        }
        let Some(path) = dialog.pick_file() else {
            return;
        };
        let result = if dtu {
            aurora_tlk_explorer::formats::tlk::TlkFile::read_dtu(&path)
        } else {
            aurora_tlk_explorer::formats::tlk::TlkFile::read_diff(&path)
        };
        match result {
            Ok(entries) => self.merge_tlk_entries(entries),
            Err(error) => self.set_message(
                format!("Could not merge {}: {error:#}", path.display()),
                true,
            ),
        }
    }

    fn run_command(&mut self, ctx: &egui::Context, command: Command) {
        match command {
            Command::NewTlk => {
                self.documents.push(Document::new_tlk());
                self.active = Some(self.documents.len() - 1);
            }
            Command::NewTwoDa => {
                self.documents.push(Document::new_twoda());
                self.active = Some(self.documents.len() - 1);
            }
            Command::Open => self.open_dialog(),
            Command::Save => {
                if let Some(i) = self.active {
                    self.save_document(i, false);
                }
            }
            Command::SaveAs => {
                if let Some(i) = self.active {
                    self.save_document(i, true);
                }
            }
            Command::SaveAll => {
                let count = self.documents.len();
                for index in 0..count {
                    if self.documents[index].dirty {
                        self.save_document(index, false);
                    }
                }
            }
            Command::Close => {
                if let Some(i) = self.active {
                    self.request_close(i);
                }
            }
            Command::Quit => self.request_quit(ctx),
            Command::Cut => self.cut_selected_row(ctx),
            Command::Copy => {
                self.copy_selected_row(ctx);
            }
            Command::Paste => self.paste_rows(),
            Command::DeleteRows => {
                if self.delete_selected_row() {
                    self.set_message("Deleted selected row(s)", false);
                }
            }
            Command::DeleteColumns => {
                let deleted = self
                    .active
                    .and_then(|index| self.documents.get_mut(index))
                    .map(|document| {
                        let columns = Self::fully_selected_twoda_columns(document);
                        Self::delete_twoda_columns(document, columns)
                    })
                    .unwrap_or(0);
                if deleted > 0 {
                    self.set_message(format!("Deleted {deleted} selected column(s)"), false);
                }
            }
            Command::Undo => {
                if let Some(document) = self.active.and_then(|index| self.documents.get_mut(index))
                {
                    document.undo();
                }
            }
            Command::Redo => {
                if let Some(document) = self.active.and_then(|index| self.documents.get_mut(index))
                {
                    document.redo();
                }
            }
            Command::ResizeTable => self.open_resize_dialog(),
            Command::RenumberTwoDa => self.renumber_twoda(),
            Command::AlterColumns => self.open_column_dialog(),
            Command::ToggleUserFlag => self.toggle_user_flag(),
            Command::Find => {
                self.search_window_open = true;
                self.focus_search_window = true;
            }
            Command::FindNext => self.find_next(),
            Command::MarkModified => self.mark_selected_tlk(true),
            Command::MarkUnmodified => self.mark_selected_tlk(false),
            Command::SaveDiff => self.save_tlk_diff(),
            Command::MergeDiff => self.merge_diff_dialog(false),
            Command::MergeDtu => self.merge_diff_dialog(true),
            Command::DiscardDiff => {
                if let Some(document) = self.active.and_then(|index| self.documents.get_mut(index))
                {
                    document.tlk_modified.clear();
                }
            }
            Command::DiffOverview => self.show_diff_overview = true,
        }
    }

    fn shortcuts(&mut self, ctx: &egui::Context) {
        // Keep clipboard and keyboard commands inside the Find dialog. Without
        // this, pasting a query can also paste into the selected table row.
        if self.search_window_open {
            return;
        }
        let text_editor_focused = ctx.memory(|memory| memory.focused().is_some());
        let row_selection_active = self
            .active
            .and_then(|index| self.documents.get(index))
            .is_some_and(|document| {
                !document.selected_rows.is_empty() && document.selected_column.is_none()
            });
        let cell_selection_active = self
            .active
            .and_then(|index| self.documents.get(index))
            .is_some_and(|document| {
                !document.selected_cells.is_empty() || !document.selected_columns.is_empty()
            });
        let column_selection_active = self
            .active
            .and_then(|index| self.documents.get(index))
            .is_some_and(|document| !Self::fully_selected_twoda_columns(document).is_empty());
        let clipboard_selection_active = row_selection_active || cell_selection_active;
        let pasted_text = ctx.input(|input| {
            input.events.iter().rev().find_map(|event| {
                if let egui::Event::Paste(text) = event {
                    Some(text.clone())
                } else {
                    None
                }
            })
        });
        let command = ctx.input_mut(|input| {
            let copy_event = clipboard_selection_active
                && input
                    .events
                    .iter()
                    .any(|event| matches!(event, egui::Event::Copy));
            let cut_event = row_selection_active
                && input
                    .events
                    .iter()
                    .any(|event| matches!(event, egui::Event::Cut));
            let paste_event = clipboard_selection_active
                && input
                    .events
                    .iter()
                    .any(|event| matches!(event, egui::Event::Paste(_)));
            if cut_event {
                Some(Command::Cut)
            } else if copy_event {
                Some(Command::Copy)
            } else if paste_event {
                Some(Command::Paste)
            } else if row_selection_active
                && input.consume_key(egui::Modifiers::NONE, egui::Key::Delete)
            {
                Some(Command::DeleteRows)
            } else if column_selection_active
                && input.consume_key(egui::Modifiers::NONE, egui::Key::Delete)
            {
                Some(Command::DeleteColumns)
            } else if input.consume_key(
                egui::Modifiers {
                    ctrl: true,
                    shift: true,
                    ..Default::default()
                },
                egui::Key::Z,
            ) {
                Some(Command::Redo)
            } else if input.consume_key(egui::Modifiers::CTRL, egui::Key::Z) {
                Some(Command::Undo)
            } else if input.consume_key(egui::Modifiers::CTRL, egui::Key::F) {
                Some(Command::Find)
            } else if input.consume_key(egui::Modifiers::CTRL, egui::Key::G) {
                Some(Command::FindNext)
            } else if input.consume_key(egui::Modifiers::CTRL, egui::Key::U) {
                Some(Command::ToggleUserFlag)
            } else if input.consume_key(egui::Modifiers::CTRL, egui::Key::O) {
                Some(Command::Open)
            } else if input.consume_key(egui::Modifiers::CTRL, egui::Key::S) {
                Some(Command::Save)
            } else if input.consume_key(egui::Modifiers::CTRL, egui::Key::W) {
                Some(Command::Close)
            } else if input.consume_key(egui::Modifiers::CTRL, egui::Key::N) {
                Some(Command::NewTlk)
            } else if (!text_editor_focused || row_selection_active)
                && input.consume_key(egui::Modifiers::CTRL, egui::Key::X)
            {
                Some(Command::Cut)
            } else if (!text_editor_focused || clipboard_selection_active)
                && input.consume_key(egui::Modifiers::CTRL, egui::Key::C)
            {
                Some(Command::Copy)
            } else if (!text_editor_focused || clipboard_selection_active)
                && input.consume_key(egui::Modifiers::CTRL, egui::Key::V)
            {
                Some(Command::Paste)
            } else {
                None
            }
        });
        if let Some(command) = command {
            if matches!(command, Command::Paste) {
                self.pending_paste_text = pasted_text;
            }
            self.run_command(ctx, command);
        }
    }

    fn top_bar(&mut self, root: &mut egui::Ui, ctx: &egui::Context) {
        let mut command = None;
        egui::Panel::top("top_bar").show(root, |ui| {
            ui.add_space(3.0);
            egui::MenuBar::new().ui(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("New TLK    Ctrl+N").clicked() {
                        command = Some(Command::NewTlk);
                        ui.close();
                    }
                    if ui.button("New 2DA").clicked() {
                        command = Some(Command::NewTwoDa);
                        ui.close();
                    }
                    ui.separator();
                    if ui.button("Open…    Ctrl+O").clicked() {
                        command = Some(Command::Open);
                        ui.close();
                    }
                    if ui
                        .add_enabled(self.active.is_some(), egui::Button::new("Save    Ctrl+S"))
                        .clicked()
                    {
                        command = Some(Command::Save);
                        ui.close();
                    }
                    if ui
                        .add_enabled(
                            self.documents.iter().any(|document| document.dirty),
                            egui::Button::new("Save All"),
                        )
                        .clicked()
                    {
                        command = Some(Command::SaveAll);
                        ui.close();
                    }
                    if ui
                        .add_enabled(self.active.is_some(), egui::Button::new("Save As…"))
                        .clicked()
                    {
                        command = Some(Command::SaveAs);
                        ui.close();
                    }
                    ui.separator();
                    if ui
                        .add_enabled(self.active.is_some(), egui::Button::new("Close    Ctrl+W"))
                        .clicked()
                    {
                        command = Some(Command::Close);
                        ui.close();
                    }
                    if ui.button("Quit").clicked() {
                        command = Some(Command::Quit);
                        ui.close();
                    }
                });
                ui.menu_button("Edit", |ui| {
                    let can_undo = self
                        .active
                        .and_then(|index| self.documents.get(index))
                        .is_some_and(|document| document.history.can_undo());
                    let can_redo = self
                        .active
                        .and_then(|index| self.documents.get(index))
                        .is_some_and(|document| document.history.can_redo());
                    if ui
                        .add_enabled(can_undo, egui::Button::new("Undo    Ctrl+Z"))
                        .clicked()
                    {
                        command = Some(Command::Undo);
                        ui.close();
                    }
                    if ui
                        .add_enabled(can_redo, egui::Button::new("Redo    Ctrl+Shift+Z"))
                        .clicked()
                    {
                        command = Some(Command::Redo);
                        ui.close();
                    }
                    ui.separator();
                    let has_selection = self
                        .active
                        .and_then(|index| self.documents.get(index))
                        .is_some_and(|document| document.selected_row.is_some());
                    if ui
                        .add_enabled(has_selection, egui::Button::new("Cut Row    Ctrl+X"))
                        .clicked()
                    {
                        command = Some(Command::Cut);
                        ui.close();
                    }
                    if ui
                        .add_enabled(has_selection, egui::Button::new("Copy Row    Ctrl+C"))
                        .clicked()
                    {
                        command = Some(Command::Copy);
                        ui.close();
                    }
                    if ui
                        .add_enabled(
                            self.clipboard.is_some(),
                            egui::Button::new("Paste Row(s)    Ctrl+V"),
                        )
                        .clicked()
                    {
                        command = Some(Command::Paste);
                        ui.close();
                    }
                    ui.separator();
                    if ui.button("Find / Replace    Ctrl+F").clicked() {
                        command = Some(Command::Find);
                        ui.close();
                    }
                    if ui.button("Find Again    Ctrl+G").clicked() {
                        command = Some(Command::FindNext);
                        ui.close();
                    }
                });
                ui.menu_button("View", |ui| {
                    ui.label("Appearance");
                    for choice in ThemeChoice::ALL {
                        if ui
                            .selectable_value(&mut self.state.theme, choice, choice.label())
                            .changed()
                        {
                            Self::apply_theme(ctx, choice);
                            self.state.store();
                        }
                    }
                    let is_tlk = self
                        .active
                        .and_then(|index| self.documents.get(index))
                        .is_some_and(|document| matches!(document.data, DocumentData::Tlk(_)));
                    if is_tlk {
                        ui.separator();
                        ui.label("TLK display");
                        let changed = ui
                            .checkbox(
                                &mut self.state.display_user_strref,
                                "Display User StrRef    Ctrl+U",
                            )
                            .changed()
                            | ui.checkbox(
                                &mut self.state.display_hex_strref,
                                "Display StrRef as hex",
                            )
                            .changed();
                        if changed {
                            self.state.store();
                        }
                    }
                });
                if let Some(document) = self.active.and_then(|index| self.documents.get(index)) {
                    let is_twoda = matches!(document.data, DocumentData::TwoDa(_));
                    let is_tlk = matches!(document.data, DocumentData::Tlk(_));
                    ui.menu_button("Table", |ui| {
                        // Keep the command labels on one line instead of letting
                        // the popup inherit the narrow width of the menu title.
                        ui.set_min_width(300.0);
                        if ui.button("Resize…").clicked() {
                            command = Some(Command::ResizeTable);
                            ui.close();
                        }
                        if is_twoda {
                            if ui.button("Renumber rows").clicked() {
                                command = Some(Command::RenumberTwoDa);
                                ui.close();
                            }
                            if ui.button("Alter columns…").clicked() {
                                command = Some(Command::AlterColumns);
                                ui.close();
                            }
                            if ui
                                .button("Toggle User Flag on selected StrRef    Ctrl+U")
                                .clicked()
                            {
                                command = Some(Command::ToggleUserFlag);
                                ui.close();
                            }
                        }
                    });
                    if is_tlk {
                        ui.menu_button("TLK Diff", |ui| {
                            if ui.button("Mark selected row modified").clicked() {
                                command = Some(Command::MarkModified);
                                ui.close();
                            }
                            if ui.button("Unmark selected row").clicked() {
                                command = Some(Command::MarkUnmodified);
                                ui.close();
                            }
                            ui.separator();
                            if ui.button("Merge Diff…").clicked() {
                                command = Some(Command::MergeDiff);
                                ui.close();
                            }
                            if ui.button("Merge DTU…").clicked() {
                                command = Some(Command::MergeDtu);
                                ui.close();
                            }
                            if ui.button("Save Diff…").clicked() {
                                command = Some(Command::SaveDiff);
                                ui.close();
                            }
                            ui.separator();
                            if ui.button("Diff Overview").clicked() {
                                command = Some(Command::DiffOverview);
                                ui.close();
                            }
                            if ui.button("Discard Diff Info").clicked() {
                                command = Some(Command::DiscardDiff);
                                ui.close();
                            }
                        });
                    }
                }
            });
            ui.add_space(3.0);
        });
        if let Some(command) = command {
            self.run_command(ctx, command);
        }
    }

    fn tab_bar(&mut self, root: &mut egui::Ui) {
        let mut activate = None;
        let mut close = None;
        egui::Panel::top("tabs").show(root, |ui| {
            egui::ScrollArea::horizontal()
                .id_salt("document_tabs")
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        if ui
                            .add_sized([52.0, 26.0], egui::Button::new("Open"))
                            .on_hover_text("Open file (Ctrl+O)")
                            .clicked()
                        {
                            self.open_dialog();
                        }
                        ui.add_space(4.0);
                        for (index, doc) in self.documents.iter().enumerate() {
                            let title =
                                format!("{}{}", if doc.dirty { "* " } else { "" }, doc.title());
                            let selected = self.active == Some(index);
                            let selection = ui.visuals().selection.bg_fill;
                            let tab = egui::Frame::new()
                                .fill(if selected {
                                    selection.gamma_multiply(0.28)
                                } else {
                                    Color32::TRANSPARENT
                                })
                                .stroke(if selected {
                                    egui::Stroke::new(1.0, selection.gamma_multiply(0.55))
                                } else {
                                    egui::Stroke::NONE
                                })
                                .corner_radius(4)
                                .inner_margin(egui::Margin::symmetric(8, 3))
                                .show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        let title_response = ui.add(
                                            egui::Label::new(RichText::new(title).color(
                                                if selected {
                                                    ui.visuals().strong_text_color()
                                                } else {
                                                    ui.visuals().text_color()
                                                },
                                            ))
                                            .selectable(false)
                                            .sense(egui::Sense::click()),
                                        );
                                        let (close_rect, close_response) = ui.allocate_exact_size(
                                            egui::vec2(18.0, 18.0),
                                            egui::Sense::click(),
                                        );
                                        if close_response.hovered() {
                                            ui.painter().rect_filled(
                                                close_rect,
                                                4.0,
                                                ui.visuals().selection.bg_fill.gamma_multiply(0.72),
                                            );
                                        }
                                        ui.painter().text(
                                            close_rect.center(),
                                            Align2::CENTER_CENTER,
                                            "×",
                                            egui::FontId::proportional(14.0),
                                            if close_response.hovered() {
                                                ui.visuals().selection.stroke.color
                                            } else {
                                                ui.visuals().strong_text_color()
                                            },
                                        );
                                        let close_response =
                                            close_response.on_hover_text("Close tab");
                                        (title_response, close_response)
                                    })
                                    .inner
                                });
                            let (title_response, close_response) = tab.inner;
                            if selected {
                                ui.painter().line_segment(
                                    [
                                        egui::pos2(
                                            tab.response.rect.left() + 4.0,
                                            tab.response.rect.bottom(),
                                        ),
                                        egui::pos2(
                                            tab.response.rect.right() - 4.0,
                                            tab.response.rect.bottom(),
                                        ),
                                    ],
                                    egui::Stroke::new(2.0, selection),
                                );
                            }
                            title_response.context_menu(|ui| {
                                if ui.button("Close tab").clicked() {
                                    close = Some(index);
                                    ui.close();
                                }
                            });
                            if title_response.middle_clicked() || close_response.middle_clicked() {
                                close = Some(index);
                            } else if title_response.clicked() {
                                activate = Some(index);
                            }
                            if close_response.clicked() {
                                close = Some(index);
                            }
                            ui.add_space(2.0);
                        }
                        if self.documents.is_empty() {
                            ui.label(RichText::new("No documents open").weak());
                        }
                    });
                });
        });
        if let Some(index) = activate {
            self.active = Some(index);
            self.sync_state();
        }
        if let Some(index) = close {
            self.request_close(index);
        }
    }

    fn document_toolbar(ui: &mut egui::Ui, doc: &mut Document) -> (bool, bool) {
        let mut add = false;
        let mut delete = false;
        ui.horizontal(|ui| {
            ui.label(RichText::new(doc.kind_name()).strong());
            ui.separator();
            ui.label("Find:");
            ui.add(
                TextEdit::singleline(&mut doc.search)
                    .hint_text("Filter rows…")
                    .desired_width(260.0),
            );
            if !doc.search.is_empty() && ui.small_button("Clear").clicked() {
                doc.search.clear();
            }
            ui.separator();
            if ui.button("Add row").clicked() {
                add = true;
            }
            if ui
                .add_enabled(
                    doc.selected_row.is_some(),
                    egui::Button::new(if doc.selected_rows.len() > 1 {
                        format!("Delete {} rows", doc.selected_rows.len())
                    } else {
                        "Delete row".to_owned()
                    }),
                )
                .clicked()
            {
                delete = true;
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if let Some(path) = &doc.path {
                    ui.label(RichText::new(path.display().to_string()).weak().small());
                }
            });
        });
        (add, delete)
    }

    fn tlk_editor(
        ui: &mut egui::Ui,
        doc: &mut Document,
        display_user_strref: bool,
        display_hex_strref: bool,
    ) {
        let table_id = (
            "tlk_table",
            doc.path.clone(),
            std::ptr::from_ref(&*doc).addr(),
        );
        let vertical_scroll_id = Id::new(("tlk_vertical_scroll", table_id.2));
        let horizontal_scroll_id = Id::new(("tlk_horizontal_scroll", table_id.2));
        let empty_menu_id = Id::new(("tlk_empty_menu", table_id.2));
        let (add, delete) = Self::document_toolbar(ui, doc);
        if delete && Self::delete_document_rows(doc) > 0 {
            doc.dirty = !doc.history.is_clean();
        }
        let DocumentData::Tlk(tlk) = &mut doc.data else {
            return;
        };
        if add {
            let position = doc
                .selected_row
                .map_or(tlk.entries.len(), |row| (row + 1).min(tlk.entries.len()));
            let inserted = TlkEntry::default();
            tlk.entries.insert(position, inserted.clone());
            Self::tlk_rows_inserted(&mut doc.tlk_modified, position, 1);
            doc.selected_row = Some(position);
            doc.selected_rows.clear();
            doc.selected_rows.insert(position);
            doc.selection_anchor = Some(position);
            doc.scroll_to_selected = true;
            doc.history.record(EditAction::TlkRows {
                index: position,
                removed: Vec::new(),
                inserted: vec![inserted],
            });
            doc.dirty = !doc.history.is_clean();
        }
        let settings_before = (tlk.language_id, tlk.encoding);
        ui.horizontal(|ui| {
            ui.label("Language ID");
            if ui
                .add(egui::DragValue::new(&mut tlk.language_id).range(0..=u32::MAX))
                .changed()
            {
                doc.dirty = true;
            }
            ui.separator();
            ui.label("Text encoding");
            let old = tlk.encoding;
            egui::ComboBox::from_id_salt("tlk_encoding")
                .selected_text(tlk.encoding.label())
                .show_ui(ui, |ui| {
                    for encoding in TlkEncoding::ALL {
                        ui.selectable_value(&mut tlk.encoding, encoding, encoding.label());
                    }
                });
            if old != tlk.encoding {
                doc.dirty = true;
            }
            ui.separator();
            ui.label(format!("{} entries", tlk.entries.len()));
        });
        let settings_after = (tlk.language_id, tlk.encoding);
        if settings_before != settings_after {
            doc.history.record(EditAction::TlkSettings {
                before: settings_before,
                after: settings_after,
            });
            doc.dirty = !doc.history.is_clean();
        }
        ui.separator();

        let format_strref = |index: usize| {
            let value = index as u32 + if display_user_strref { 1 << 24 } else { 0 };
            if display_hex_strref {
                format!("0x{value:x}")
            } else {
                value.to_string()
            }
        };
        let needle = doc.search.to_lowercase();
        let visible: Option<Vec<usize>> = (!needle.is_empty()).then(|| {
            tlk.entries
                .iter()
                .enumerate()
                .filter(|(i, entry)| {
                    format_strref(*i).to_lowercase().contains(&needle)
                        || entry.text.to_lowercase().contains(&needle)
                })
                .map(|(i, _)| i)
                .collect()
        });
        let visible_count = visible.as_ref().map_or(tlk.entries.len(), Vec::len);
        let navigation_rows = visible
            .clone()
            .unwrap_or_else(|| (0..tlk.entries.len()).collect());
        Self::keyboard_row_selection(
            ui,
            &mut doc.selected_row,
            &mut doc.selected_rows,
            &mut doc.selection_anchor,
            &mut doc.selected_column,
            &mut doc.scroll_to_selected,
            &navigation_rows,
        );
        let scroll_to = if std::mem::take(&mut doc.scroll_to_selected) {
            doc.selected_row.and_then(|selected| {
                visible.as_ref().map_or(Some(selected), |rows| {
                    rows.iter().position(|row| *row == selected)
                })
            })
        } else {
            None
        };
        let table_height = (ui.available_height() - 150.0).max(56.0);
        if let Some(row) = scroll_to {
            doc.table_first_row = row;
        }
        let row_step = 25.0 + ui.spacing().item_spacing.y;
        let window_size = (table_height / row_step).ceil() as usize + 2;
        if scroll_to.is_some() {
            doc.table_first_row = doc.table_first_row.saturating_sub(window_size / 2);
        }
        let max_first = visible_count.saturating_sub(window_size);
        doc.table_first_row = doc.table_first_row.min(max_first);
        let first_visible = doc.table_first_row;
        let window_count = window_size.min(visible_count.saturating_sub(first_visible));
        let mut row_menu_action = None;
        let builder = TableBuilder::new(ui)
            .id_salt(table_id)
            .vertical_scroll_offset(0.0)
            .horizontal_scroll_offset(doc.table_scroll_x)
            .native_vertical_input(false)
            .striped(true)
            .cell_layout(Layout::left_to_right(Align::Center))
            .resizable(true)
            .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
            .max_scroll_height(table_height)
            .auto_shrink([false, false])
            .column(Column::initial(105.0).at_least(70.0));
        let table_output = builder
            .column(Column::remainder().at_least(240.0))
            .header(24.0, |mut header| {
                header.col(|ui| {
                    ui.strong("StrRef");
                });
                header.col(|ui| {
                    ui.strong("Text");
                });
            })
            .body(|body| {
                body.rows(25.0, window_count, |mut row| {
                    let visible_index = first_visible + row.index();
                    let index = visible
                        .as_ref()
                        .map_or(visible_index, |rows| rows[visible_index]);
                    let entry = &mut tlk.entries[index];
                    let before = entry.clone();
                    let selected = doc.selected_rows.contains(&index);
                    row.set_selected(selected);
                    row.col(|ui| {
                        let (rect, response) =
                            ui.allocate_exact_size(ui.available_size(), egui::Sense::click());
                        if selected {
                            ui.painter()
                                .rect_filled(rect, 0.0, ui.visuals().selection.bg_fill);
                        }
                        ui.painter().text(
                            egui::pos2(rect.left() + 4.0, rect.center().y),
                            Align2::LEFT_CENTER,
                            format!(
                                "{}{}",
                                if doc.tlk_modified.contains(&index) {
                                    "*"
                                } else {
                                    ""
                                },
                                format_strref(index)
                            ),
                            egui::TextStyle::Body.resolve(ui.style()),
                            if selected {
                                ui.visuals().selection.stroke.color
                            } else {
                                ui.visuals().text_color()
                            },
                        );
                        if response.clicked() {
                            doc.selected_cells.clear();
                            doc.selected_columns.clear();
                            doc.cell_selection_anchor = None;
                            ui.memory_mut(|memory| {
                                if let Some(focused) = memory.focused() {
                                    memory.surrender_focus(focused);
                                }
                            });
                            let modifiers = ui.input(|input| input.modifiers);
                            Self::update_row_selection(
                                &mut doc.selected_row,
                                &mut doc.selected_rows,
                                &mut doc.selection_anchor,
                                &mut doc.selected_column,
                                index,
                                None,
                                modifiers,
                            );
                        }
                        Self::row_action_context_menu(
                            &response,
                            index,
                            doc.selected_rows.len().max(1),
                            &mut row_menu_action,
                        );
                    });
                    row.col(|ui| {
                        let r = ui.add(
                            TextEdit::singleline(&mut entry.text)
                                .frame(egui::Frame::NONE)
                                .desired_width(f32::INFINITY),
                        );
                        if r.changed() {
                            doc.dirty = true;
                        }
                        if r.clicked() {
                            doc.selected_rows.clear();
                            doc.selection_anchor = None;
                            doc.selected_cells.clear();
                            doc.cell_selection_anchor = None;
                            doc.selected_row = Some(index);
                            doc.selected_column = Some(4);
                        }
                        Self::row_action_context_menu(
                            &r,
                            index,
                            doc.selected_rows.len().max(1),
                            &mut row_menu_action,
                        );
                    });
                    if entry.text != before.text {
                        entry.flags = (entry.flags & !1) | u32::from(!entry.text.is_empty());
                    }
                    if entry.sound_resref != before.sound_resref {
                        entry.flags =
                            (entry.flags & !2) | if entry.sound_resref.is_empty() { 0 } else { 2 };
                    }
                    if entry.sound_length != before.sound_length {
                        entry.flags =
                            (entry.flags & !4) | if entry.sound_length == 0.0 { 0 } else { 4 };
                    }
                    if *entry != before {
                        doc.tlk_modified.insert(index);
                        doc.history.record(EditAction::TlkEntry {
                            row: index,
                            before,
                            after: entry.clone(),
                        });
                        doc.dirty = !doc.history.is_clean();
                    }
                    let response = row.response();
                    if response.secondary_clicked() && !doc.selected_rows.contains(&index) {
                        Self::update_row_selection(
                            &mut doc.selected_row,
                            &mut doc.selected_rows,
                            &mut doc.selection_anchor,
                            &mut doc.selected_column,
                            index,
                            None,
                            egui::Modifiers::NONE,
                        );
                    }
                });
            });
        doc.table_scroll_x = table_output.state.offset.x;
        Self::vertical_table_scrollbar(
            ui,
            vertical_scroll_id,
            table_output.inner_rect,
            visible_count,
            window_size,
            &mut doc.table_first_row,
        );
        Self::horizontal_table_scrollbar(
            ui,
            horizontal_scroll_id,
            table_output.inner_rect,
            table_output.content_size.x,
            &mut doc.table_scroll_x,
        );
        Self::middle_mouse_table_scroll(
            ui,
            table_output.inner_rect,
            visible_count,
            window_size,
            table_output.content_size.x,
            &mut doc.table_first_row,
            &mut doc.table_scroll_x,
        );
        Self::empty_table_context_menu(
            ui,
            empty_menu_id,
            table_output.inner_rect,
            window_count,
            row_step,
            &mut row_menu_action,
        );

        ui.separator();
        if let Some(index) = doc.selected_row.filter(|i| *i < tlk.entries.len()) {
            ui.horizontal(|ui| {
                ui.strong(format!("StrRef {}", format_strref(index)));
                ui.label("Full text (newlines supported)");
            });
            let before = tlk.entries[index].clone();
            if ui
                .add_sized(
                    [ui.available_width(), 120.0],
                    TextEdit::multiline(&mut tlk.entries[index].text),
                )
                .changed()
            {
                let entry = &mut tlk.entries[index];
                entry.flags = (entry.flags & !1) | u32::from(!entry.text.is_empty());
                doc.tlk_modified.insert(index);
                doc.history.record(EditAction::TlkEntry {
                    row: index,
                    before,
                    after: entry.clone(),
                });
                doc.dirty = !doc.history.is_clean();
            }
        } else {
            ui.centered_and_justified(|ui| {
                ui.label(RichText::new("Select an entry to edit its full text").weak());
            });
        }
        if let Some(action) = row_menu_action {
            Self::apply_row_menu_action(doc, action);
        }
    }

    fn twoda_editor(
        ui: &mut egui::Ui,
        doc: &mut Document,
        column_dialog: &mut Option<ColumnDialogState>,
    ) {
        let table_id = (
            "twoda_table",
            doc.path.clone(),
            std::ptr::from_ref(&*doc).addr(),
        );
        let vertical_scroll_id = Id::new(("twoda_vertical_scroll", table_id.2));
        let horizontal_scroll_id = Id::new(("twoda_horizontal_scroll", table_id.2));
        let empty_menu_id = Id::new(("twoda_empty_menu", table_id.2));
        let (add, delete) = Self::document_toolbar(ui, doc);
        if delete && Self::delete_document_rows(doc) > 0 {
            doc.dirty = !doc.history.is_clean();
        }
        let DocumentData::TwoDa(table) = &mut doc.data else {
            return;
        };
        if add {
            let mut row = vec!["****".to_owned(); table.columns.len()];
            let position = doc
                .selected_row
                .map_or(table.rows.len(), |row| (row + 1).min(table.rows.len()));
            if let Some(first) = row.first_mut() {
                *first = position.to_string();
            }
            table.rows.insert(position, row.clone());
            doc.selected_row = Some(position);
            doc.selected_rows.clear();
            doc.selected_rows.insert(position);
            doc.selection_anchor = Some(position);
            doc.scroll_to_selected = true;
            doc.history.record(EditAction::TwoDaRows {
                index: position,
                removed: Vec::new(),
                inserted: vec![row],
            });
            doc.dirty = !doc.history.is_clean();
        }
        let default_before = table.default_value.clone();
        let mut renumber = false;
        ui.horizontal(|ui| {
            ui.label("DEFAULT:");
            let default = table.default_value.get_or_insert_with(String::new);
            if ui
                .add(
                    TextEdit::singleline(default)
                        .hint_text("optional")
                        .desired_width(160.0),
                )
                .changed()
            {
                doc.dirty = true;
            }
            if default.is_empty() {
                table.default_value = None;
            }
            ui.separator();
            if ui
                .button("Renumber")
                .on_hover_text("Set stored Row labels to their physical zero-based indexes")
                .clicked()
            {
                renumber = true;
            }
            ui.separator();
            ui.label(format!(
                "{} rows × {} columns",
                table.rows.len(),
                table.columns.len()
            ));
        });
        if renumber {
            let before = table.clone();
            for (index, row) in table.rows.iter_mut().enumerate() {
                if let Some(label) = row.first_mut() {
                    *label = index.to_string();
                }
            }
            if *table != before {
                let after = table.clone();
                doc.history.record(EditAction::TwoDaTable { before, after });
                doc.dirty = !doc.history.is_clean();
            }
        }
        if default_before != table.default_value {
            doc.history.record(EditAction::TwoDaDefault {
                before: default_before,
                after: table.default_value.clone(),
            });
            doc.dirty = !doc.history.is_clean();
        }
        ui.separator();
        let needle = doc.search.to_lowercase();
        let visible: Option<Vec<usize>> = (!needle.is_empty()).then(|| {
            table
                .rows
                .iter()
                .enumerate()
                .filter(|(_, row)| {
                    row.iter()
                        .any(|value| value.to_lowercase().contains(&needle))
                })
                .map(|(i, _)| i)
                .collect()
        });
        let visible_count = visible.as_ref().map_or(table.rows.len(), Vec::len);
        let navigation_rows = visible
            .clone()
            .unwrap_or_else(|| (0..table.rows.len()).collect());
        Self::keyboard_row_selection(
            ui,
            &mut doc.selected_row,
            &mut doc.selected_rows,
            &mut doc.selection_anchor,
            &mut doc.selected_column,
            &mut doc.scroll_to_selected,
            &navigation_rows,
        );
        let scroll_to = if std::mem::take(&mut doc.scroll_to_selected) {
            doc.selected_row.and_then(|selected| {
                visible.as_ref().map_or(Some(selected), |rows| {
                    rows.iter().position(|row| *row == selected)
                })
            })
        } else {
            None
        };
        let headers = table.columns.clone();
        let table_height = ui.available_height().max(56.0);
        if let Some(row) = scroll_to {
            doc.table_first_row = row;
        }
        let row_step = 26.0 + ui.spacing().item_spacing.y;
        let window_size = (table_height / row_step).ceil() as usize + 2;
        if scroll_to.is_some() {
            doc.table_first_row = doc.table_first_row.saturating_sub(window_size / 2);
        }
        let max_first = visible_count.saturating_sub(window_size);
        doc.table_first_row = doc.table_first_row.min(max_first);
        let first_visible = doc.table_first_row;
        let window_count = window_size.min(visible_count.saturating_sub(first_visible));
        let mut row_menu_action = None;
        let mut delete_columns: Option<Vec<usize>> = None;
        let mut builder = TableBuilder::new(ui)
            .id_salt(table_id)
            .vertical_scroll_offset(0.0)
            .horizontal_scroll_offset(doc.table_scroll_x)
            .native_vertical_input(false)
            .striped(true)
            .cell_layout(Layout::left_to_right(Align::Center))
            .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
            .max_scroll_height(table_height)
            .auto_shrink([false, false])
            .resizable(true);
        builder = builder.column(Column::initial(70.0).at_least(55.0));
        for (index, _) in headers.iter().enumerate() {
            builder = builder.column(if index == 0 {
                Column::initial(75.0)
            } else {
                Column::initial(130.0).at_least(70.0)
            });
        }
        let table_output = builder
            .header(25.0, |mut header| {
                header.col(|ui| {
                    let (rect, response) =
                        ui.allocate_exact_size(ui.available_size(), egui::Sense::click());
                    ui.painter().text(
                        rect.center(),
                        Align2::CENTER_CENTER,
                        "Index",
                        egui::TextStyle::Body.resolve(ui.style()),
                        ui.visuals().strong_text_color(),
                    );
                    if response.clicked() {
                        doc.selected_cells.clear();
                        doc.selected_columns.clear();
                        doc.cell_selection_anchor = None;
                        doc.selected_rows = (0..table.rows.len()).collect();
                        doc.selected_row = (!table.rows.is_empty()).then_some(0);
                        doc.selection_anchor = doc.selected_row;
                        doc.selected_column = None;
                    }
                });
                for (column_index, name) in headers.iter().enumerate() {
                    header.col(|ui| {
                        let (rect, response) =
                            ui.allocate_exact_size(ui.available_size(), egui::Sense::click());
                        let column_selected = doc.selected_columns.contains(&column_index);
                        if column_selected {
                            ui.painter()
                                .rect_filled(rect, 0.0, ui.visuals().selection.bg_fill);
                        }
                        ui.painter().text(
                            rect.center(),
                            Align2::CENTER_CENTER,
                            name,
                            egui::TextStyle::Body.resolve(ui.style()),
                            if column_selected {
                                ui.visuals().selection.stroke.color
                            } else {
                                ui.visuals().strong_text_color()
                            },
                        );
                        if response.clicked() {
                            doc.selected_rows.clear();
                            doc.selection_anchor = None;
                            doc.selected_cells.clear();
                            let modifiers = ui.input(|input| input.modifiers);
                            if modifiers.shift {
                                let anchor = doc.selected_column.unwrap_or(column_index);
                                doc.selected_columns =
                                    (anchor.min(column_index)..=anchor.max(column_index)).collect();
                            } else if modifiers.ctrl || modifiers.command {
                                if column_selected {
                                    doc.selected_columns.remove(&column_index);
                                } else {
                                    doc.selected_columns.insert(column_index);
                                }
                            } else {
                                doc.selected_columns = BTreeSet::from([column_index]);
                            }
                            doc.selected_row = (!table.rows.is_empty()).then_some(0);
                            doc.selected_column = Some(column_index);
                            doc.cell_selection_anchor =
                                doc.selected_row.map(|row| (row, column_index));
                        }
                        if response.secondary_clicked() && !column_selected {
                            doc.selected_rows.clear();
                            doc.selected_cells.clear();
                            doc.selected_columns = BTreeSet::from([column_index]);
                            doc.selected_row = (!table.rows.is_empty()).then_some(0);
                            doc.selected_column = Some(column_index);
                            doc.cell_selection_anchor =
                                doc.selected_row.map(|row| (row, column_index));
                        }
                        response.context_menu(|ui| {
                            if ui.button("Insert column before…").clicked() {
                                *column_dialog = Some(ColumnDialogState {
                                    index: column_index.to_string(),
                                    name: String::new(),
                                    default_value: "****".to_owned(),
                                });
                                ui.close();
                            }
                            if ui.button("Insert column after…").clicked() {
                                *column_dialog = Some(ColumnDialogState {
                                    index: (column_index + 1).to_string(),
                                    name: String::new(),
                                    default_value: "****".to_owned(),
                                });
                                ui.close();
                            }
                            ui.separator();
                            if ui.button("Rename column…").clicked() {
                                *column_dialog = Some(ColumnDialogState {
                                    index: column_index.to_string(),
                                    name: name.clone(),
                                    default_value: "****".to_owned(),
                                });
                                ui.close();
                            }
                            ui.separator();
                            let selected: Vec<usize> =
                                doc.selected_columns.iter().copied().collect();
                            let selected = if selected.is_empty() {
                                vec![column_index]
                            } else {
                                selected
                            };
                            if ui
                                .add_enabled(
                                    headers.len() > selected.len(),
                                    egui::Button::new(if selected.len() == 1 {
                                        "Delete column".to_owned()
                                    } else {
                                        format!("Delete {} selected columns", selected.len())
                                    }),
                                )
                                .clicked()
                            {
                                delete_columns = Some(selected);
                                ui.close();
                            }
                        });
                    });
                }
            })
            .body(|body| {
                body.rows(26.0, window_count, |mut row| {
                    let visible_index = first_visible + row.index();
                    let row_index = visible
                        .as_ref()
                        .map_or(visible_index, |rows| rows[visible_index]);
                    let selected = doc.selected_rows.contains(&row_index);
                    row.set_selected(selected);
                    row.col(|ui| {
                        let (rect, response) =
                            ui.allocate_exact_size(ui.available_size(), egui::Sense::click());
                        ui.painter().text(
                            egui::pos2(rect.left() + 4.0, rect.center().y),
                            Align2::LEFT_CENTER,
                            row_index.to_string(),
                            egui::TextStyle::Body.resolve(ui.style()),
                            ui.visuals().text_color(),
                        );
                        if response.clicked() {
                            doc.selected_cells.clear();
                            doc.cell_selection_anchor = None;
                            ui.memory_mut(|memory| {
                                if let Some(focused) = memory.focused() {
                                    memory.surrender_focus(focused);
                                }
                            });
                            let modifiers = ui.input(|input| input.modifiers);
                            Self::update_row_selection(
                                &mut doc.selected_row,
                                &mut doc.selected_rows,
                                &mut doc.selection_anchor,
                                &mut doc.selected_column,
                                row_index,
                                None,
                                modifiers,
                            );
                        }
                        Self::row_action_context_menu(
                            &response,
                            row_index,
                            doc.selected_rows.len().max(1),
                            &mut row_menu_action,
                        );
                    });
                    for (column_index, cell) in table.rows[row_index].iter_mut().enumerate() {
                        let before = cell.clone();
                        let cell_selected = doc.selected_columns.contains(&column_index)
                            || doc.selected_cells.contains(&(row_index, column_index));
                        row.col(|ui| {
                            if cell_selected {
                                ui.painter().rect_filled(
                                    ui.max_rect(),
                                    0.0,
                                    ui.visuals().selection.bg_fill.gamma_multiply(0.72),
                                );
                            }
                            let response = ui.add(
                                TextEdit::singleline(cell)
                                    .frame(egui::Frame::NONE)
                                    .desired_width(f32::INFINITY),
                            );
                            if response.changed() {
                                doc.dirty = true;
                            }
                            if response.clicked() {
                                doc.selected_rows.clear();
                                doc.selection_anchor = None;
                                doc.selected_columns.clear();
                                let modifiers = ui.input(|input| input.modifiers);
                                Self::update_cell_selection(
                                    &mut doc.selected_cells,
                                    &mut doc.cell_selection_anchor,
                                    row_index,
                                    column_index,
                                    modifiers,
                                );
                                doc.selected_row = Some(row_index);
                                doc.selected_column = Some(column_index);
                            }
                            Self::row_action_context_menu(
                                &response,
                                row_index,
                                doc.selected_rows.len().max(1),
                                &mut row_menu_action,
                            );
                        });
                        if *cell != before {
                            doc.history.record(EditAction::TwoDaCell {
                                row: row_index,
                                column: column_index,
                                before,
                                after: cell.clone(),
                            });
                            doc.dirty = !doc.history.is_clean();
                        }
                    }
                    let response = row.response();
                    if response.secondary_clicked() && !doc.selected_rows.contains(&row_index) {
                        Self::update_row_selection(
                            &mut doc.selected_row,
                            &mut doc.selected_rows,
                            &mut doc.selection_anchor,
                            &mut doc.selected_column,
                            row_index,
                            None,
                            egui::Modifiers::NONE,
                        );
                    }
                });
            });
        doc.table_scroll_x = table_output.state.offset.x;
        Self::vertical_table_scrollbar(
            ui,
            vertical_scroll_id,
            table_output.inner_rect,
            visible_count,
            window_size,
            &mut doc.table_first_row,
        );
        Self::horizontal_table_scrollbar(
            ui,
            horizontal_scroll_id,
            table_output.inner_rect,
            table_output.content_size.x,
            &mut doc.table_scroll_x,
        );
        Self::middle_mouse_table_scroll(
            ui,
            table_output.inner_rect,
            visible_count,
            window_size,
            table_output.content_size.x,
            &mut doc.table_first_row,
            &mut doc.table_scroll_x,
        );
        Self::empty_table_context_menu(
            ui,
            empty_menu_id,
            table_output.inner_rect,
            window_count,
            row_step,
            &mut row_menu_action,
        );
        if let Some(columns) = delete_columns {
            Self::delete_twoda_columns(doc, columns);
        }
        if let Some(action) = row_menu_action {
            Self::apply_row_menu_action(doc, action);
        }
    }

    fn itp_node_label(node: &serde_json::Value) -> String {
        for key in ["DELETE_ME", "NAME", "RESREF"] {
            if let Some(value) = node
                .pointer(&format!("/{key}/value"))
                .and_then(|v| v.as_str())
                && !value.is_empty()
            {
                return value.to_owned();
            }
        }
        node.pointer("/STRREF/value")
            .and_then(|v| v.as_u64())
            .map(|v| format!("StrRef {v}"))
            .unwrap_or_else(|| "Palette node".to_owned())
    }

    fn itp_tree_ui(
        ui: &mut egui::Ui,
        nodes: &[serde_json::Value],
        path: &mut Vec<usize>,
        selected: &mut Vec<usize>,
        add_terminal_to: &mut Option<Vec<usize>>,
    ) {
        for (index, node) in nodes.iter().enumerate() {
            path.push(index);
            let label = Self::itp_node_label(node);
            let children = node.pointer("/LIST/value").and_then(|v| v.as_array());
            if let Some(children) = children {
                let response = egui::CollapsingHeader::new(label)
                    .id_salt(("itp_node", path.clone()))
                    .default_open(path.len() == 1)
                    .show(ui, |ui| {
                        Self::itp_tree_ui(ui, children, path, selected, add_terminal_to)
                    });
                if response.header_response.clicked() {
                    *selected = path.clone();
                }
                response.header_response.context_menu(|ui| {
                    if ui.button("New terminal category inside").clicked() {
                        *add_terminal_to = Some(path.clone());
                        ui.close();
                    }
                });
            } else if ui.selectable_label(*selected == *path, label).clicked() {
                *selected = path.clone();
            }
            path.pop();
        }
    }

    fn itp_node_mut<'a>(
        root: &'a mut serde_json::Value,
        path: &[usize],
    ) -> Option<&'a mut serde_json::Value> {
        fn descend<'a>(
            list: &'a mut serde_json::Value,
            path: &[usize],
        ) -> Option<&'a mut serde_json::Value> {
            let (index, rest) = path.split_first()?;
            let node = list.as_array_mut()?.get_mut(*index)?;
            if rest.is_empty() {
                Some(node)
            } else {
                descend(node.pointer_mut("/LIST/value")?, rest)
            }
        }
        descend(root.pointer_mut("/MAIN/value")?, path)
    }

    fn itp_selected_list_mut<'a>(
        root: &'a mut serde_json::Value,
        path: &[usize],
    ) -> Option<&'a mut Vec<serde_json::Value>> {
        if path.is_empty() {
            return root.pointer_mut("/MAIN/value")?.as_array_mut();
        }
        let node = Self::itp_node_mut(root, path)?;
        if node.pointer("/LIST/value").is_none() {
            node.as_object_mut()?.insert(
                "LIST".into(),
                serde_json::json!({"type":"list", "value":[]}),
            );
        }
        node.pointer_mut("/LIST/value")?.as_array_mut()
    }

    fn itp_next_palette_id(root: &serde_json::Value) -> u8 {
        fn mark_ids(nodes: &[serde_json::Value], used: &mut [bool; 256]) {
            for node in nodes {
                if let Some(id) = node.pointer("/ID/value").and_then(|value| value.as_u64())
                    && let Some(slot) = used.get_mut(id as usize)
                {
                    *slot = true;
                }
                if let Some(children) = node
                    .pointer("/LIST/value")
                    .and_then(|value| value.as_array())
                {
                    mark_ids(children, used);
                }
            }
        }

        let mut used = [false; 256];
        if let Some(nodes) = root
            .pointer("/MAIN/value")
            .and_then(|value| value.as_array())
        {
            mark_ids(nodes, &mut used);
        }
        used.iter().position(|used| !used).unwrap_or(0) as u8
    }

    fn itp_editor(ui: &mut egui::Ui, doc: &mut Document) {
        let before = match &doc.data {
            DocumentData::Itp(itp) => itp.clone(),
            _ => return,
        };
        let DocumentData::Itp(itp) = &mut doc.data else {
            return;
        };
        ui.horizontal(|ui| {
            if ui.button("Add category").clicked()
                && let Some(list) =
                    Self::itp_selected_list_mut(&mut itp.root, &doc.itp_selected_path)
            {
                list.push(serde_json::json!({
                    "__struct_id": 1,
                    "DELETE_ME": {"type":"cexostring", "value":"New category"},
                    "STRREF": {"type":"dword", "value":0},
                    "LIST": {"type":"list", "value":[]}
                }));
            }
            if ui.button("Add entry").clicked()
                && let Some(list) =
                    Self::itp_selected_list_mut(&mut itp.root, &doc.itp_selected_path)
            {
                list.push(serde_json::json!({
                    "__struct_id": 1,
                    "NAME": {"type":"cexostring", "value":"New entry"},
                    "RESREF": {"type":"resref", "value":"new_entry"}
                }));
            }
            if ui
                .add_enabled(
                    !doc.itp_selected_path.is_empty(),
                    egui::Button::new("Delete node"),
                )
                .clicked()
            {
                let index = *doc.itp_selected_path.last().unwrap();
                let parent = &doc.itp_selected_path[..doc.itp_selected_path.len() - 1];
                if let Some(list) = Self::itp_selected_list_mut(&mut itp.root, parent)
                    && index < list.len()
                {
                    list.remove(index);
                    doc.itp_selected_path.clear();
                }
            }
        });
        ui.separator();
        ui.columns(2, |columns| {
            columns[0].heading("Palette tree");
            let mut add_root_category = false;
            let mut add_root_terminal = false;
            let mut add_terminal_to = None;
            egui::ScrollArea::vertical().show(&mut columns[0], |ui| {
                if let Some(nodes) = itp.root.pointer("/MAIN/value").and_then(|v| v.as_array()) {
                    Self::itp_tree_ui(
                        ui,
                        nodes,
                        &mut Vec::new(),
                        &mut doc.itp_selected_path,
                        &mut add_terminal_to,
                    );
                }
                let empty_space = ui.allocate_response(ui.available_size(), egui::Sense::click());
                empty_space.context_menu(|ui| {
                    if ui.button("New category").clicked() {
                        add_root_category = true;
                        ui.close();
                    }
                    if ui.button("New terminal category").clicked() {
                        add_root_terminal = true;
                        ui.close();
                    }
                });
            });
            if add_root_category
                && let Some(list) = itp
                    .root
                    .pointer_mut("/MAIN/value")
                    .and_then(|v| v.as_array_mut())
            {
                list.push(serde_json::json!({
                    "__struct_id": 1,
                    "DELETE_ME": {"type":"cexostring", "value":"New category"},
                    "STRREF": {"type":"dword", "value":0},
                    "LIST": {"type":"list", "value":[]}
                }));
                doc.itp_selected_path = vec![list.len() - 1];
            }
            if add_root_terminal {
                let id = Self::itp_next_palette_id(&itp.root);
                if let Some(list) = itp
                    .root
                    .pointer_mut("/MAIN/value")
                    .and_then(|v| v.as_array_mut())
                {
                    list.push(serde_json::json!({
                        "__struct_id": 1,
                        "DELETE_ME": {"type":"cexostring", "value":"New terminal category"},
                        "ID": {"type":"byte", "value":id},
                        "STRREF": {"type":"dword", "value":0}
                    }));
                    doc.itp_selected_path = vec![list.len() - 1];
                }
            }
            if let Some(parent_path) = add_terminal_to {
                let id = Self::itp_next_palette_id(&itp.root);
                if let Some(list) = Self::itp_selected_list_mut(&mut itp.root, &parent_path) {
                    list.push(serde_json::json!({
                        "__struct_id": 1,
                        "DELETE_ME": {"type":"cexostring", "value":"New terminal category"},
                        "ID": {"type":"byte", "value":id},
                        "STRREF": {"type":"dword", "value":0}
                    }));
                    doc.itp_selected_path = parent_path;
                    doc.itp_selected_path.push(list.len() - 1);
                }
            }
            columns[1].heading("Node properties");
            if let Some(node) = Self::itp_node_mut(&mut itp.root, &doc.itp_selected_path)
                && let Some(fields) = node.as_object_mut()
            {
                egui::Grid::new("itp_properties")
                    .striped(true)
                    .show(&mut columns[1], |ui| {
                        for (name, field) in fields.iter_mut() {
                            if name == "__struct_id" || name == "LIST" {
                                continue;
                            }
                            ui.label(name);
                            let Some(value) = field.get_mut("value") else {
                                ui.label("—");
                                ui.end_row();
                                continue;
                            };
                            if let Some(text) = value.as_str() {
                                let mut edited = text.to_owned();
                                if ui
                                    .add_sized(
                                        [320.0, 24.0],
                                        egui::TextEdit::singleline(&mut edited),
                                    )
                                    .changed()
                                {
                                    *value = edited.into();
                                }
                            } else if let Some(number) = value.as_u64() {
                                let mut edited = number;
                                if ui
                                    .add_sized([320.0, 24.0], egui::DragValue::new(&mut edited))
                                    .changed()
                                {
                                    *value = edited.into();
                                }
                            } else if let Some(number) = value.as_i64() {
                                let mut edited = number;
                                if ui
                                    .add_sized([320.0, 24.0], egui::DragValue::new(&mut edited))
                                    .changed()
                                {
                                    *value = edited.into();
                                }
                            } else if let Some(number) = value.as_f64() {
                                let mut edited = number;
                                if ui
                                    .add_sized([320.0, 24.0], egui::DragValue::new(&mut edited))
                                    .changed()
                                {
                                    *value = edited.into();
                                }
                            } else {
                                ui.label(value.to_string());
                            }
                            ui.end_row();
                        }
                    });
            } else {
                columns[1].label("Select a node to edit its fields.");
            }
        });
        let after = match &doc.data {
            DocumentData::Itp(itp) => itp.clone(),
            _ => return,
        };
        if before != after {
            doc.record(EditAction::ItpTree { before, after });
        }
    }

    fn central(&mut self, root: &mut egui::Ui, ctx: &egui::Context) {
        egui::CentralPanel::default().show(root, |ui| {
            if let Some(index) = self.active.filter(|i| *i < self.documents.len()) {
                match self.documents[index].data {
                    DocumentData::Tlk(_) => Self::tlk_editor(
                        ui,
                        &mut self.documents[index],
                        self.state.display_user_strref,
                        self.state.display_hex_strref,
                    ),
                    DocumentData::TwoDa(_) => {
                        Self::twoda_editor(ui, &mut self.documents[index], &mut self.column_dialog)
                    }
                    DocumentData::Itp(_) => Self::itp_editor(ui, &mut self.documents[index]),
                }
            } else {
                ui.horizontal(|ui| {
                    if ui.button("Open").clicked() {
                        self.open_dialog();
                    }
                    if ui.button("New TLK").clicked() {
                        self.run_command(ctx, Command::NewTlk);
                    }
                    if ui.button("New 2DA").clicked() {
                        self.run_command(ctx, Command::NewTwoDa);
                    }
                });
                ui.vertical_centered(|ui| {
                    ui.add_space(80.0);
                    ui.heading("Aurora TLK Explorer");
                    ui.label(
                        RichText::new("A modern workspace for Neverwinter Nights data files")
                            .weak(),
                    );
                    ui.add_space(12.0);
                    ui.label(
                        RichText::new("You can also drop .tlk and .2da files into this window.")
                            .small()
                            .weak(),
                    );
                });
            }
        });
    }

    fn status_bar(&mut self, root: &mut egui::Ui) {
        egui::Panel::bottom("status").show(root, |ui| {
            ui.horizontal(|ui| {
                if let Some((message, error)) = &self.message {
                    ui.label(RichText::new(message).color(if *error {
                        Color32::from_rgb(245, 95, 95)
                    } else {
                        Color32::from_rgb(95, 200, 130)
                    }));
                    if ui.small_button("×").clicked() {
                        self.message = None;
                    }
                } else {
                    ui.label(RichText::new("Ready").weak());
                }
            });
        });
    }

    fn resize_dialog(&mut self, ctx: &egui::Context) {
        let Some(value) = &mut self.resize_value else {
            return;
        };
        let mut apply = false;
        let mut cancel = false;
        egui::Window::new("Resize table")
            .id(Id::new("resize_table"))
            .anchor(Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.label("New number of rows / TLK entries:");
                ui.add(TextEdit::singleline(value).desired_width(180.0));
                ui.horizontal(|ui| {
                    if ui.button("Apply").clicked() {
                        apply = true;
                    }
                    if ui.button("Cancel").clicked() {
                        cancel = true;
                    }
                });
            });
        if apply {
            match value.parse::<usize>() {
                Ok(size) => {
                    self.resize_active_table(size);
                    self.resize_value = None;
                }
                Err(_) => self.set_message("Enter a valid non-negative whole number", true),
            }
        } else if cancel {
            self.resize_value = None;
        }
    }

    fn alter_column(&mut self, action: ColumnAction, index: usize, name: String, default: String) {
        let Some(document) = self
            .active
            .and_then(|active| self.documents.get_mut(active))
        else {
            return;
        };
        let DocumentData::TwoDa(table) = &mut document.data else {
            return;
        };
        let before = table.clone();
        let result: Result<(), String> = match action {
            ColumnAction::Insert if index <= table.columns.len() => {
                if name.is_empty() || name.chars().any(char::is_whitespace) {
                    Err("Column names cannot be empty or contain spaces".to_owned())
                } else {
                    table.columns.insert(index, name);
                    for row in &mut table.rows {
                        row.insert(index, default.clone());
                    }
                    Ok(())
                }
            }
            ColumnAction::Insert => Err("The column position is outside the table".to_owned()),
            ColumnAction::Drop if index < table.columns.len() && table.columns.len() > 1 => {
                table.columns.remove(index);
                for row in &mut table.rows {
                    if index < row.len() {
                        row.remove(index);
                    }
                }
                Ok(())
            }
            ColumnAction::Drop => Err("That column cannot be removed".to_owned()),
            ColumnAction::Rename if index < table.columns.len() => {
                if name.is_empty() || name.chars().any(char::is_whitespace) {
                    Err("Column names cannot be empty or contain spaces".to_owned())
                } else {
                    table.columns[index] = name;
                    Ok(())
                }
            }
            ColumnAction::Rename => Err("The column position is outside the table".to_owned()),
        };
        match result {
            Ok(()) => {
                let after = table.clone();
                document.record(EditAction::TwoDaTable { before, after });
                self.column_dialog = None;
            }
            Err(message) => self.set_message(message, true),
        }
    }

    fn column_dialog(&mut self, ctx: &egui::Context) {
        let Some(dialog) = &mut self.column_dialog else {
            return;
        };
        let mut action = None;
        let mut cancel = false;
        egui::Window::new("Alter 2DA columns")
            .id(Id::new("alter_columns"))
            .anchor(Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .collapsible(false)
            .resizable(false)
            .fixed_size([540.0, 190.0])
            .show(ctx, |ui| {
                egui::Grid::new("column_fields").show(ui, |ui| {
                    ui.label("Column position");
                    ui.add_sized([360.0, 26.0], TextEdit::singleline(&mut dialog.index));
                    ui.end_row();
                    ui.label("Column name");
                    ui.add_sized([360.0, 26.0], TextEdit::singleline(&mut dialog.name));
                    ui.end_row();
                    ui.label("Default value");
                    ui.add_sized(
                        [360.0, 26.0],
                        TextEdit::singleline(&mut dialog.default_value),
                    );
                    ui.end_row();
                });
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.button("Insert").clicked() {
                        action = Some(ColumnAction::Insert);
                    }
                    if ui.button("Drop").clicked() {
                        action = Some(ColumnAction::Drop);
                    }
                    if ui.button("Rename").clicked() {
                        action = Some(ColumnAction::Rename);
                    }
                    if ui.button("Cancel").clicked() {
                        cancel = true;
                    }
                });
            });
        if cancel {
            self.column_dialog = None;
            return;
        }
        if let Some(action) = action {
            let index = dialog.index.parse::<usize>();
            let name = dialog.name.clone();
            let default = dialog.default_value.clone();
            match index {
                Ok(index) => self.alter_column(action, index, name, default),
                Err(_) => {
                    self.set_message("Column position must be a non-negative whole number", true)
                }
            }
        }
    }

    fn diff_overview(&mut self, ctx: &egui::Context) {
        if !self.show_diff_overview {
            return;
        }
        let rows: Vec<usize> = self
            .active
            .and_then(|index| self.documents.get(index))
            .map(|document| document.tlk_modified.iter().copied().collect())
            .unwrap_or_default();
        let mut open = self.show_diff_overview;
        let mut selected = None;
        egui::Window::new("TLK Diff Overview")
            .open(&mut open)
            .default_size([280.0, 380.0])
            .show(ctx, |ui| {
                ui.label(format!("{} modified entries", rows.len()));
                ui.separator();
                egui::ScrollArea::vertical().show(ui, |ui| {
                    for row in rows {
                        if ui.button(self.displayed_strref(row)).clicked() {
                            selected = Some(row);
                        }
                    }
                });
            });
        self.show_diff_overview = open;
        if let Some(row) = selected
            && let Some(document) = self.active.and_then(|index| self.documents.get_mut(index))
        {
            document.selected_row = Some(row);
            document.selected_rows.clear();
            document.selected_rows.insert(row);
            document.selection_anchor = Some(row);
            document.scroll_to_selected = true;
        }
    }

    fn search_window(&mut self, ctx: &egui::Context) {
        if !self.search_window_open {
            return;
        }
        let Some(index) = self.active.filter(|index| *index < self.documents.len()) else {
            self.search_window_open = false;
            return;
        };
        let mut open = self.search_window_open;
        let request_focus = std::mem::take(&mut self.focus_search_window);
        let mut action = None;
        let document = &mut self.documents[index];
        egui::Window::new(format!("Find — {}", document.title()))
            .id(Id::new("find_window"))
            .open(&mut open)
            .anchor(Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .collapsible(false)
            .resizable(false)
            .fixed_size([700.0, 210.0])
            .show(ctx, |ui| {
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    ui.add_sized([110.0, 28.0], egui::Label::new("Find:"));
                    let response = ui.add_sized(
                        [540.0, 28.0],
                        TextEdit::singleline(&mut document.find_query).id(Id::new("find_query")),
                    );
                    if request_focus {
                        response.request_focus();
                    }
                    if response.lost_focus()
                        && ui.input(|input| input.key_pressed(egui::Key::Enter))
                    {
                        action = Some(SearchAction::Next);
                    }
                });
                ui.horizontal(|ui| {
                    ui.add_sized([110.0, 28.0], egui::Label::new("Replace with:"));
                    ui.add_sized([540.0, 28.0], TextEdit::singleline(&mut document.replace));
                });
                ui.add_space(14.0);
                ui.horizontal(|ui| {
                    let spacing = ui.spacing().item_spacing.x;
                    let button_widths = [120.0, 120.0, 140.0, 120.0];
                    let total_width = button_widths.iter().sum::<f32>() + spacing * 3.0;
                    ui.add_space(((ui.available_width() - total_width) / 2.0).max(0.0));
                    let enabled = !document.find_query.is_empty();
                    if ui
                        .add_enabled(enabled, egui::Button::new("Find Next").min_size([button_widths[0], 28.0].into()))
                        .clicked()
                    {
                        action = Some(SearchAction::Next);
                    }
                    if ui
                        .add_enabled(enabled, egui::Button::new("Find Previous").min_size([button_widths[1], 28.0].into()))
                        .clicked()
                    {
                        action = Some(SearchAction::Previous);
                    }
                    if ui
                        .add_enabled(enabled, egui::Button::new("Replace Selected").min_size([button_widths[2], 28.0].into()))
                        .clicked()
                    {
                        action = Some(SearchAction::ReplaceSelected);
                    }
                    if ui
                        .add_enabled(enabled, egui::Button::new("Replace All").min_size([button_widths[3], 28.0].into()))
                        .clicked()
                    {
                        action = Some(SearchAction::ReplaceAll);
                    }
                });
                ui.add_space(8.0);
                ui.vertical_centered(|ui| {
                    ui.label(
                        RichText::new(
                            "Enter text or an exact StrRef. Ctrl+G finds the next match and wraps at the end.",
                        )
                        .small()
                        .weak(),
                    );
                });
            });
        self.search_window_open = open;
        match action {
            Some(SearchAction::Next) => self.find_next(),
            Some(SearchAction::Previous) => self.find_previous(),
            Some(SearchAction::ReplaceSelected) => self.replace_selected_match(),
            Some(SearchAction::ReplaceAll) => self.replace_all_matches(),
            None => {}
        }
    }

    fn close_dialog(&mut self, ctx: &egui::Context) {
        let Some(pending) = self.pending_close else {
            return;
        };
        let title = self
            .documents
            .get(pending.index)
            .map(Document::title)
            .unwrap_or_else(|| "document".into());
        let mut choice = None;
        egui::Window::new("Unsaved changes")
            .id(Id::new("unsaved_changes"))
            .anchor(Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .collapsible(false)
            .resizable(false)
            .fixed_size([500.0, 205.0])
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(12.0);
                    ui.label(
                        RichText::new(format!("Save changes to “{title}” before closing?"))
                            .size(18.0)
                            .strong(),
                    );
                    ui.add_space(8.0);
                    ui.label(
                        RichText::new(
                            "If you discard them, your changes will be permanently lost.",
                        )
                        .size(14.0)
                        .weak(),
                    );
                    ui.add_space(24.0);
                });
                ui.horizontal(|ui| {
                    let button_width = 110.0;
                    let total_width = button_width * 3.0 + ui.spacing().item_spacing.x * 2.0;
                    ui.add_space(((ui.available_width() - total_width) * 0.5).max(0.0));
                    if ui
                        .add_sized([button_width, 36.0], egui::Button::new("Save"))
                        .clicked()
                    {
                        choice = Some(CloseChoice::Save);
                    }
                    if ui
                        .add_sized([button_width, 36.0], egui::Button::new("Discard"))
                        .clicked()
                    {
                        choice = Some(CloseChoice::Discard);
                    }
                    if ui
                        .add_sized([button_width, 36.0], egui::Button::new("Cancel"))
                        .clicked()
                    {
                        choice = Some(CloseChoice::Cancel);
                    }
                });
            });
        if let Some(choice) = choice {
            self.handle_close_choice(ctx, choice);
        }
    }
}

impl eframe::App for AuroraApp {
    fn ui(&mut self, root: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = root.ctx().clone();
        if ctx.input(|i| i.viewport().close_requested()) && !self.allow_exit {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            if self.pending_close.is_none() {
                self.request_quit(&ctx);
            }
        }
        self.shortcuts(&ctx);
        let dropped: Vec<PathBuf> = ctx.input(|i| {
            i.raw
                .dropped_files
                .iter()
                .filter_map(|f| f.path.clone())
                .collect()
        });
        for path in dropped {
            self.open_path(&path, true);
        }

        let window_title = self
            .active
            .and_then(|i| self.documents.get(i))
            .map(|d| {
                format!(
                    "{}{} — {APP_NAME}",
                    if d.dirty { "* " } else { "" },
                    d.title()
                )
            })
            .unwrap_or_else(|| APP_NAME.to_owned());
        ctx.send_viewport_cmd(egui::ViewportCommand::Title(window_title));

        self.top_bar(root, &ctx);
        self.tab_bar(root);
        self.status_bar(root);
        self.central(root, &ctx);
        self.resize_dialog(&ctx);
        self.column_dialog(&ctx);
        self.diff_overview(&ctx);
        self.search_window(&ctx);
        self.close_dialog(&ctx);
    }

    fn save(&mut self, _storage: &mut dyn eframe::Storage) {
        self.sync_state();
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.sync_state();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deleting_fully_selected_twoda_columns_is_one_undoable_edit() {
        let mut document = Document::new_twoda();
        let DocumentData::TwoDa(table) = &mut document.data else {
            unreachable!();
        };
        table.rows = vec![
            vec!["0".into(), "a".into(), "b".into()],
            vec!["1".into(), "c".into(), "d".into()],
        ];
        document.selected_cells.extend([(0, 1), (1, 1)]);
        let columns = AuroraApp::fully_selected_twoda_columns(&document);
        assert_eq!(columns, vec![1]);
        assert_eq!(AuroraApp::delete_twoda_columns(&mut document, columns), 1);
        let DocumentData::TwoDa(table) = &document.data else {
            unreachable!();
        };
        assert_eq!(table.columns, ["Row", "Value"]);
        document.undo();
        let DocumentData::TwoDa(table) = &document.data else {
            unreachable!();
        };
        assert_eq!(table.columns, ["Row", "Label", "Value"]);
    }

    #[test]
    fn plain_row_click_replaces_selection_but_modifiers_preserve_multi_select() {
        let mut selected_row = None;
        let mut selected_rows = BTreeSet::new();
        let mut anchor = None;
        let mut column = None;

        AuroraApp::update_row_selection(
            &mut selected_row,
            &mut selected_rows,
            &mut anchor,
            &mut column,
            4,
            None,
            egui::Modifiers::NONE,
        );
        AuroraApp::update_row_selection(
            &mut selected_row,
            &mut selected_rows,
            &mut anchor,
            &mut column,
            7,
            None,
            egui::Modifiers::CTRL,
        );
        assert_eq!(selected_rows, BTreeSet::from([4, 7]));

        AuroraApp::update_row_selection(
            &mut selected_row,
            &mut selected_rows,
            &mut anchor,
            &mut column,
            9,
            None,
            egui::Modifiers::NONE,
        );
        assert_eq!(selected_rows, BTreeSet::from([9]));
        assert_eq!(selected_row, Some(9));
    }

    #[test]
    fn copied_index_selection_pastes_duplicate_below() {
        let mut document = Document::new_twoda();
        let DocumentData::TwoDa(table) = &mut document.data else {
            unreachable!();
        };
        table.columns = vec!["Row".into(), "Label".into()];
        table.rows = vec![
            vec!["0".into(), "first".into()],
            vec!["1".into(), "second".into()],
        ];
        document.selected_row = Some(0);
        document.selected_rows.insert(0);
        document.selection_anchor = Some(0);
        document.selected_column = None;

        let mut app = AuroraApp {
            documents: vec![document],
            active: Some(0),
            state: PersistentState::default(),
            pending_close: None,
            allow_exit: false,
            message: None,
            clipboard: None,
            clipboard_text: None,
            pending_paste_text: None,
            resize_value: None,
            column_dialog: None,
            show_diff_overview: false,
            search_window_open: false,
            focus_search_window: false,
        };

        assert!(app.copy_selected_row(&egui::Context::default()));
        app.paste_rows();

        let DocumentData::TwoDa(table) = &app.documents[0].data else {
            unreachable!();
        };
        assert_eq!(
            table.rows,
            vec![vec!["0", "first"], vec!["0", "first"], vec!["1", "second"]]
        );
        assert_eq!(app.documents[0].selected_row, Some(1));
    }

    #[test]
    fn copied_fields_fill_same_columns_on_selected_destination_row() {
        let mut document = Document::new_twoda();
        let DocumentData::TwoDa(table) = &mut document.data else {
            unreachable!();
        };
        table.columns = vec!["Row".into(), "Label".into(), "Value".into()];
        table.rows = vec![
            vec!["0".into(), "source".into(), "42".into()],
            vec!["1".into(), "destination".into(), "0".into()],
        ];
        document.selected_row = Some(0);
        document.selected_column = Some(2);
        document.selected_cells.extend([(0, 1), (0, 2)]);

        let mut app = AuroraApp {
            documents: vec![document],
            active: Some(0),
            state: PersistentState::default(),
            pending_close: None,
            allow_exit: false,
            message: None,
            clipboard: None,
            clipboard_text: None,
            pending_paste_text: None,
            resize_value: None,
            column_dialog: None,
            show_diff_overview: false,
            search_window_open: false,
            focus_search_window: false,
        };

        assert!(app.copy_selected_row(&egui::Context::default()));
        let document = &mut app.documents[0];
        document.selected_cells.clear();
        document.selected_column = None;
        document.selected_row = Some(1);
        document.selected_rows.insert(1);
        app.paste_rows();

        let DocumentData::TwoDa(table) = &app.documents[0].data else {
            unreachable!();
        };
        assert_eq!(table.rows[1], vec!["1", "source", "42"]);
        assert_eq!(
            app.documents[0].selected_cells,
            BTreeSet::from([(1, 1), (1, 2)])
        );
    }

    #[test]
    fn delete_removes_all_selected_rows_as_one_undoable_edit() {
        let mut document = Document::new_twoda();
        let DocumentData::TwoDa(table) = &mut document.data else {
            unreachable!();
        };
        table.columns = vec!["Row".into(), "Label".into()];
        table.rows = vec![
            vec!["0".into(), "first".into()],
            vec!["1".into(), "second".into()],
            vec!["2".into(), "third".into()],
        ];
        document.selected_row = Some(2);
        document.selected_rows.extend([0, 2]);
        document.selection_anchor = Some(0);
        document.selected_column = None;

        assert_eq!(AuroraApp::delete_document_rows(&mut document), 2);
        let DocumentData::TwoDa(table) = &document.data else {
            unreachable!();
        };
        assert_eq!(table.rows, vec![vec!["1", "second"]]);

        document.undo();
        let DocumentData::TwoDa(table) = &document.data else {
            unreachable!();
        };
        assert_eq!(
            table.rows,
            vec![vec!["0", "first"], vec!["1", "second"], vec!["2", "third"]]
        );
    }

    #[test]
    fn custom_vertical_scrollbar_handles_wheel_and_drag_without_table_input() {
        let ctx = egui::Context::default();
        let viewport = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(700.0, 400.0));
        let mut first_row = 0;
        let render = |ctx: &egui::Context, mut input: egui::RawInput, first_row: &mut usize| {
            input.screen_rect = Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(700.0, 400.0),
            ));
            let _ = ctx.run_ui(input, |ui| {
                AuroraApp::vertical_table_scrollbar(
                    ui,
                    Id::new("standalone_vertical_scroll"),
                    viewport,
                    15_180,
                    40,
                    first_row,
                );
            });
        };

        render(&ctx, egui::RawInput::default(), &mut first_row);
        let mut wheel = egui::RawInput::default();
        wheel
            .events
            .push(egui::Event::PointerMoved(egui::pos2(300.0, 200.0)));
        wheel.events.push(egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: egui::vec2(0.0, -120.0),
            modifiers: egui::Modifiers::NONE,
            phase: egui::TouchPhase::Move,
        });
        render(&ctx, wheel, &mut first_row);
        assert!(
            first_row > 0,
            "standalone wheel input did not move the table"
        );

        let handle = egui::pos2(694.0, 10.0);
        let mut press = egui::RawInput::default();
        press.events.push(egui::Event::PointerMoved(handle));
        press.events.push(egui::Event::PointerButton {
            pos: handle,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: egui::Modifiers::NONE,
        });
        render(&ctx, press, &mut first_row);
        let target = egui::pos2(694.0, 300.0);
        let mut drag = egui::RawInput::default();
        drag.events.push(egui::Event::PointerMoved(target));
        render(&ctx, drag, &mut first_row);
        assert!(
            first_row > 10_000,
            "standalone handle drag did not move proportionally"
        );
    }
}
