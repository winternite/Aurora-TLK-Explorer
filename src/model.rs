use crate::formats::{
    itp::ItpFile,
    tlk::{TlkEncoding, TlkEntry, TlkFile},
    twoda::{TwoDaFile, TwoDaFormat},
};
use anyhow::{Context, Result, bail};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_DOCUMENT_ID: AtomicU64 = AtomicU64::new(1);

fn next_document_id() -> u64 {
    NEXT_DOCUMENT_ID.fetch_add(1, Ordering::Relaxed)
}

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
    fn estimated_bytes(&self) -> usize {
        fn string_bytes(value: &String) -> usize {
            std::mem::size_of::<String>() + value.capacity()
        }
        fn entry_bytes(entry: &TlkEntry) -> usize {
            std::mem::size_of::<TlkEntry>() + entry.sound_resref.capacity() + entry.text.capacity()
        }
        fn table_bytes(table: &TwoDaFile) -> usize {
            std::mem::size_of::<TwoDaFile>()
                + table.columns.iter().map(string_bytes).sum::<usize>()
                + table
                    .rows
                    .iter()
                    .map(|row| {
                        std::mem::size_of::<Vec<String>>()
                            + row.iter().map(string_bytes).sum::<usize>()
                    })
                    .sum::<usize>()
                + table.default_value.as_ref().map_or(0, string_bytes)
        }
        fn json_bytes(value: &serde_json::Value) -> usize {
            use serde_json::Value;
            match value {
                Value::Null | Value::Bool(_) | Value::Number(_) => std::mem::size_of::<Value>(),
                Value::String(value) => std::mem::size_of::<Value>() + value.capacity(),
                Value::Array(values) => {
                    std::mem::size_of::<Value>() + values.iter().map(json_bytes).sum::<usize>()
                }
                Value::Object(values) => {
                    std::mem::size_of::<Value>()
                        + values
                            .iter()
                            .map(|(key, value)| key.capacity() + json_bytes(value))
                            .sum::<usize>()
                }
            }
        }

        match self {
            Self::Batch(actions) => {
                std::mem::size_of::<Self>()
                    + actions.iter().map(Self::estimated_bytes).sum::<usize>()
            }
            Self::ItpTree { before, after } => {
                std::mem::size_of::<Self>() + json_bytes(&before.root) + json_bytes(&after.root)
            }
            Self::TlkEntry { before, after, .. } => {
                std::mem::size_of::<Self>() + entry_bytes(before) + entry_bytes(after)
            }
            Self::TlkRows {
                removed, inserted, ..
            } => {
                std::mem::size_of::<Self>()
                    + removed.iter().map(entry_bytes).sum::<usize>()
                    + inserted.iter().map(entry_bytes).sum::<usize>()
            }
            Self::TlkSettings { .. } => std::mem::size_of::<Self>(),
            Self::TwoDaDefault { before, after } => {
                std::mem::size_of::<Self>()
                    + before.as_ref().map_or(0, string_bytes)
                    + after.as_ref().map_or(0, string_bytes)
            }
            Self::TwoDaCell { before, after, .. } => {
                std::mem::size_of::<Self>() + string_bytes(before) + string_bytes(after)
            }
            Self::TwoDaRows {
                removed, inserted, ..
            } => {
                let rows = removed.iter().chain(inserted);
                std::mem::size_of::<Self>()
                    + rows
                        .map(|row| {
                            std::mem::size_of::<Vec<String>>()
                                + row.iter().map(string_bytes).sum::<usize>()
                        })
                        .sum::<usize>()
            }
            Self::TwoDaTable { before, after } => {
                std::mem::size_of::<Self>() + table_bytes(before) + table_bytes(after)
            }
        }
    }

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
    action_bytes: Vec<usize>,
    total_bytes: usize,
    cursor: usize,
    saved_cursor: Option<usize>,
    revision: u64,
}

impl EditHistory {
    const MAX_ACTIONS: usize = 512;
    const MAX_BYTES: usize = 256 * 1024 * 1024;

    pub fn new_saved() -> Self {
        Self {
            actions: Vec::new(),
            action_bytes: Vec::new(),
            total_bytes: 0,
            cursor: 0,
            saved_cursor: Some(0),
            revision: 0,
        }
    }

    pub fn record(&mut self, action: EditAction) {
        self.revision = self.revision.wrapping_add(1);
        let discarded = self.action_bytes[self.cursor..].iter().sum::<usize>();
        self.total_bytes = self.total_bytes.saturating_sub(discarded);
        self.actions.truncate(self.cursor);
        self.action_bytes.truncate(self.cursor);
        let can_merge = self.cursor > 0 && self.saved_cursor != Some(self.cursor);
        if can_merge && self.actions[self.cursor - 1].merge(&action) {
            let old_size = self.action_bytes[self.cursor - 1];
            let new_size = self.actions[self.cursor - 1].estimated_bytes();
            self.action_bytes[self.cursor - 1] = new_size;
            self.total_bytes = self
                .total_bytes
                .saturating_sub(old_size)
                .saturating_add(new_size);
            return;
        }
        let action_bytes = action.estimated_bytes();
        self.actions.push(action);
        self.action_bytes.push(action_bytes);
        self.total_bytes = self.total_bytes.saturating_add(action_bytes);
        self.cursor += 1;
        while (self.actions.len() > Self::MAX_ACTIONS || self.total_bytes > Self::MAX_BYTES)
            && self.actions.len() > 1
        {
            self.actions.remove(0);
            self.total_bytes = self.total_bytes.saturating_sub(self.action_bytes.remove(0));
            self.cursor = self.cursor.saturating_sub(1);
            self.saved_cursor = self.saved_cursor.and_then(|cursor| cursor.checked_sub(1));
        }
    }

    pub fn undo(&mut self, data: &mut DocumentData) -> bool {
        if self.cursor == 0 {
            return false;
        }
        self.cursor -= 1;
        self.actions[self.cursor].apply(data, false);
        self.revision = self.revision.wrapping_add(1);
        true
    }

    pub fn redo(&mut self, data: &mut DocumentData) -> bool {
        if self.cursor >= self.actions.len() {
            return false;
        }
        self.actions[self.cursor].apply(data, true);
        self.cursor += 1;
        self.revision = self.revision.wrapping_add(1);
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
    pub fn revision(&self) -> u64 {
        self.revision
    }
}

#[derive(Clone, Debug)]
pub struct Document {
    pub id: u64,
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
            id: next_document_id(),
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
            id: next_document_id(),
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
            id: next_document_id(),
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
