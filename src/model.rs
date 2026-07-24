use crate::formats::{
    itp::ItpFile,
    tlk::{TlkEncoding, TlkEntry, TlkFile},
    twoda::{TwoDaFile, TwoDaFormat},
};
use anyhow::{Context, Result, bail};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub enum DocumentData {
    Itp(ItpFile),
    Tlk(TlkFile),
    TwoDa(TwoDaFile),
}

#[derive(Clone, Debug)]
pub enum EditAction {
    Batch(Vec<EditAction>),
    ItpTree {
        before: ItpFile,
        after: ItpFile,
    },
    TlkEntry {
        row: usize,
        before: TlkEntry,
        after: TlkEntry,
    },
    TlkRows {
        index: usize,
        removed: Vec<TlkEntry>,
        inserted: Vec<TlkEntry>,
    },
    TlkSettings {
        before: (u32, TlkEncoding),
        after: (u32, TlkEncoding),
    },
    TwoDaCell {
        row: usize,
        column: usize,
        before: String,
        after: String,
    },
    TwoDaRows {
        index: usize,
        removed: Vec<Vec<String>>,
        inserted: Vec<Vec<String>>,
    },
    TwoDaDefault {
        before: Option<String>,
        after: Option<String>,
    },
    TwoDaTable {
        before: TwoDaFile,
        after: TwoDaFile,
    },
}

impl EditAction {
    fn apply(&self, data: &mut DocumentData, forward: bool) {
        if let Self::Batch(actions) = self {
            if forward {
                for action in actions {
                    action.apply(data, true);
                }
            } else {
                for action in actions.iter().rev() {
                    action.apply(data, false);
                }
            }
            return;
        }
        match (self, data) {
            (Self::ItpTree { before, after }, DocumentData::Itp(itp)) => {
                *itp = if forward { after } else { before }.clone();
            }
            (Self::TlkEntry { row, before, after }, DocumentData::Tlk(tlk)) => {
                if let Some(entry) = tlk.entries.get_mut(*row) {
                    *entry = if forward { after } else { before }.clone();
                }
            }
            (
                Self::TlkRows {
                    index,
                    removed,
                    inserted,
                },
                DocumentData::Tlk(tlk),
            ) => {
                let (take, put) = if forward {
                    (removed, inserted)
                } else {
                    (inserted, removed)
                };
                let end = (*index + take.len()).min(tlk.entries.len());
                tlk.entries.splice(*index..end, put.clone());
            }
            (Self::TlkSettings { before, after }, DocumentData::Tlk(tlk)) => {
                let value = if forward { after } else { before };
                tlk.language_id = value.0;
                tlk.encoding = value.1;
            }
            (
                Self::TwoDaCell {
                    row,
                    column,
                    before,
                    after,
                },
                DocumentData::TwoDa(table),
            ) => {
                if let Some(cell) = table
                    .rows
                    .get_mut(*row)
                    .and_then(|row| row.get_mut(*column))
                {
                    *cell = if forward { after } else { before }.clone();
                }
            }
            (
                Self::TwoDaRows {
                    index,
                    removed,
                    inserted,
                },
                DocumentData::TwoDa(table),
            ) => {
                let (take, put) = if forward {
                    (removed, inserted)
                } else {
                    (inserted, removed)
                };
                let end = (*index + take.len()).min(table.rows.len());
                table.rows.splice(*index..end, put.clone());
            }
            (Self::TwoDaDefault { before, after }, DocumentData::TwoDa(table)) => {
                table.default_value = if forward { after } else { before }.clone();
            }
            (Self::TwoDaTable { before, after }, DocumentData::TwoDa(table)) => {
                *table = if forward { after } else { before }.clone();
            }
            (Self::Batch(_), _) => unreachable!(),
            _ => {}
        }
    }

    fn merge(&mut self, newer: &Self) -> bool {
        match (self, newer) {
            (
                Self::TlkEntry { row, after, .. },
                Self::TlkEntry {
                    row: next,
                    after: value,
                    ..
                },
            ) if row == next => {
                *after = value.clone();
                true
            }
            (
                Self::TwoDaCell {
                    row, column, after, ..
                },
                Self::TwoDaCell {
                    row: next_row,
                    column: next_column,
                    after: value,
                    ..
                },
            ) if row == next_row && column == next_column => {
                *after = value.clone();
                true
            }
            (Self::TlkSettings { after, .. }, Self::TlkSettings { after: value, .. }) => {
                *after = *value;
                true
            }
            (Self::TwoDaDefault { after, .. }, Self::TwoDaDefault { after: value, .. }) => {
                *after = value.clone();
                true
            }
            _ => false,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct EditHistory {
    actions: Vec<EditAction>,
    cursor: usize,
    saved_cursor: Option<usize>,
}

impl EditHistory {
    const MAX_ACTIONS: usize = 512;

    pub fn new_saved() -> Self {
        Self {
            actions: Vec::new(),
            cursor: 0,
            saved_cursor: Some(0),
        }
    }

    pub fn record(&mut self, action: EditAction) {
        self.actions.truncate(self.cursor);
        let can_merge = self.cursor > 0 && self.saved_cursor != Some(self.cursor);
        if can_merge && self.actions[self.cursor - 1].merge(&action) {
            return;
        }
        self.actions.push(action);
        self.cursor += 1;
        if self.actions.len() > Self::MAX_ACTIONS {
            let remove = self.actions.len() - Self::MAX_ACTIONS;
            self.actions.drain(..remove);
            self.cursor = self.cursor.saturating_sub(remove);
            self.saved_cursor = self
                .saved_cursor
                .and_then(|cursor| cursor.checked_sub(remove));
        }
    }

    pub fn undo(&mut self, data: &mut DocumentData) -> bool {
        if self.cursor == 0 {
            return false;
        }
        self.cursor -= 1;
        self.actions[self.cursor].apply(data, false);
        true
    }

    pub fn redo(&mut self, data: &mut DocumentData) -> bool {
        if self.cursor >= self.actions.len() {
            return false;
        }
        self.actions[self.cursor].apply(data, true);
        self.cursor += 1;
        true
    }

    pub fn can_undo(&self) -> bool {
        self.cursor > 0
    }
    pub fn can_redo(&self) -> bool {
        self.cursor < self.actions.len()
    }
    pub fn mark_saved(&mut self) {
        self.saved_cursor = Some(self.cursor);
    }
    pub fn is_clean(&self) -> bool {
        self.saved_cursor == Some(self.cursor)
    }
}

#[derive(Clone, Debug)]
pub struct Document {
    pub path: Option<PathBuf>,
    pub data: DocumentData,
    pub dirty: bool,
    pub search: String,
    pub find_query: String,
    pub replace: String,
    pub selected_row: Option<usize>,
    pub selected_rows: BTreeSet<usize>,
    pub selection_anchor: Option<usize>,
    pub selected_column: Option<usize>,
    /// Whole-column selection is stored by column, not once per table row.
    pub selected_columns: BTreeSet<usize>,
    pub selected_cells: BTreeSet<(usize, usize)>,
    pub cell_selection_anchor: Option<(usize, usize)>,
    pub scroll_to_selected: bool,
    pub table_scroll_x: f32,
    pub table_first_row: usize,
    pub history: EditHistory,
    pub tlk_modified: BTreeSet<usize>,
    pub itp_selected_path: Vec<usize>,
}

impl Document {
    pub fn open(path: &Path) -> Result<Self> {
        let extension = path
            .extension()
            .and_then(|v| v.to_str())
            .unwrap_or_default();
        let data = if extension.eq_ignore_ascii_case("tlk") {
            DocumentData::Tlk(TlkFile::read(path)?)
        } else if extension.eq_ignore_ascii_case("2da") {
            DocumentData::TwoDa(TwoDaFile::read(path)?)
        } else if extension.eq_ignore_ascii_case("itp") {
            DocumentData::Itp(ItpFile::read(path)?)
        } else {
            bail!("Aurora currently opens .tlk, .2da, and .itp files");
        };
        Ok(Self {
            path: Some(path.to_path_buf()),
            data,
            dirty: false,
            search: String::new(),
            find_query: String::new(),
            replace: String::new(),
            selected_row: None,
            selected_rows: BTreeSet::new(),
            selection_anchor: None,
            selected_column: None,
            selected_columns: BTreeSet::new(),
            selected_cells: BTreeSet::new(),
            cell_selection_anchor: None,
            scroll_to_selected: false,
            table_scroll_x: 0.0,
            table_first_row: 0,
            history: EditHistory::new_saved(),
            tlk_modified: BTreeSet::new(),
            itp_selected_path: Vec::new(),
        })
    }

    pub fn new_tlk() -> Self {
        Self {
            path: None,
            data: DocumentData::Tlk(TlkFile::default()),
            dirty: true,
            search: String::new(),
            find_query: String::new(),
            replace: String::new(),
            selected_row: None,
            selected_rows: BTreeSet::new(),
            selection_anchor: None,
            selected_column: None,
            selected_columns: BTreeSet::new(),
            selected_cells: BTreeSet::new(),
            cell_selection_anchor: None,
            scroll_to_selected: false,
            table_scroll_x: 0.0,
            table_first_row: 0,
            history: EditHistory::default(),
            tlk_modified: BTreeSet::new(),
            itp_selected_path: Vec::new(),
        }
    }

    pub fn new_twoda() -> Self {
        Self {
            path: None,
            data: DocumentData::TwoDa(TwoDaFile {
                default_value: None,
                columns: vec!["Row".into(), "Label".into(), "Value".into()],
                rows: Vec::new(),
                format: TwoDaFormat::Text,
            }),
            dirty: true,
            search: String::new(),
            find_query: String::new(),
            replace: String::new(),
            selected_row: None,
            selected_rows: BTreeSet::new(),
            selection_anchor: None,
            selected_column: None,
            selected_columns: BTreeSet::new(),
            selected_cells: BTreeSet::new(),
            cell_selection_anchor: None,
            scroll_to_selected: false,
            table_scroll_x: 0.0,
            table_first_row: 0,
            history: EditHistory::default(),
            tlk_modified: BTreeSet::new(),
            itp_selected_path: Vec::new(),
        }
    }

    pub fn title(&self) -> String {
        self.path
            .as_ref()
            .and_then(|p| p.file_name())
            .and_then(|p| p.to_str())
            .unwrap_or(match self.data {
                DocumentData::Tlk(_) => "Untitled.tlk",
                DocumentData::TwoDa(_) => "Untitled.2da",
                DocumentData::Itp(_) => "Untitled.itp",
            })
            .to_owned()
    }

    pub fn kind_name(&self) -> &'static str {
        match self.data {
            DocumentData::Tlk(_) => "TLK",
            DocumentData::TwoDa(_) => "2DA",
            DocumentData::Itp(_) => "ITP",
        }
    }

    pub fn default_extension(&self) -> &'static str {
        match self.data {
            DocumentData::Tlk(_) => "tlk",
            DocumentData::TwoDa(_) => "2da",
            DocumentData::Itp(_) => "itp",
        }
    }

    pub fn save(&mut self) -> Result<()> {
        let path = self.path.as_deref().context("Choose a file name first")?;
        match &self.data {
            DocumentData::Tlk(file) => file.write(path)?,
            DocumentData::TwoDa(file) => file.write(path)?,
            DocumentData::Itp(file) => file.write(path)?,
        }
        self.dirty = false;
        self.history.mark_saved();
        Ok(())
    }

    pub fn save_as(&mut self, path: PathBuf) -> Result<()> {
        let previous = self.path.replace(path);
        if let Err(error) = self.save() {
            self.path = previous;
            return Err(error);
        }
        Ok(())
    }

    pub fn record(&mut self, action: EditAction) {
        self.history.record(action);
        self.dirty = !self.history.is_clean();
    }

    pub fn undo(&mut self) -> bool {
        let changed = self.history.undo(&mut self.data);
        self.dirty = !self.history.is_clean();
        changed
    }

    pub fn redo(&mut self) -> bool {
        let changed = self.history.redo(&mut self.data);
        self.dirty = !self.history.is_clean();
        changed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn undo_redo_tracks_saved_state() {
        let mut document = Document::new_tlk();
        let DocumentData::Tlk(tlk) = &mut document.data else {
            unreachable!()
        };
        tlk.entries.push(TlkEntry {
            text: "original".into(),
            ..Default::default()
        });
        document.history = EditHistory::new_saved();
        document.dirty = false;
        let DocumentData::Tlk(tlk) = &mut document.data else {
            unreachable!()
        };
        let before = tlk.entries[0].clone();
        tlk.entries[0].text = "changed".into();
        let after = tlk.entries[0].clone();
        document.record(EditAction::TlkEntry {
            row: 0,
            before: before.clone(),
            after,
        });
        assert!(document.dirty);
        assert!(document.undo());
        let DocumentData::Tlk(tlk) = &document.data else {
            unreachable!()
        };
        assert_eq!(tlk.entries[0], before);
        assert!(!document.dirty);
        assert!(document.redo());
        let DocumentData::Tlk(tlk) = &document.data else {
            unreachable!()
        };
        assert_eq!(tlk.entries[0].text, "changed");
    }

    #[test]
    fn failed_save_as_keeps_the_original_path_and_dirty_state() {
        let mut document = Document::new_tlk();
        let original = std::env::temp_dir().join("aurora-original.tlk");
        document.path = Some(original.clone());
        let impossible = std::env::temp_dir()
            .join(format!("aurora-missing-{}", std::process::id()))
            .join("document.tlk");
        assert!(document.save_as(impossible).is_err());
        assert_eq!(document.path, Some(original));
        assert!(document.dirty);
    }

    #[test]
    fn undo_history_is_bounded() {
        let mut history = EditHistory::default();
        for row in 0..(EditHistory::MAX_ACTIONS + 25) {
            history.record(EditAction::TlkRows {
                index: row,
                removed: Vec::new(),
                inserted: vec![TlkEntry::default()],
            });
        }
        assert_eq!(history.actions.len(), EditHistory::MAX_ACTIONS);
        assert_eq!(history.cursor, EditHistory::MAX_ACTIONS);
    }
}
