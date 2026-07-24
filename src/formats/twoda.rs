use anyhow::{Context, Result, bail};
use std::{collections::HashMap, path::Path};

const MAX_COLUMNS: usize = 4096;
const MAX_ROWS: usize = 250_000;
const MAX_CELLS: usize = 2_000_000;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TwoDaFormat {
    #[default]
    Text,
    Binary,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct TwoDaFile {
    pub default_value: Option<String>,
    /// Includes the synthetic first "Row" column.
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
    pub format: TwoDaFormat,
}

fn tokenize(line: &str) -> Result<Vec<String>> {
    let mut fields = Vec::new();
    let mut chars = line.chars().peekable();
    loop {
        while chars
            .next_if(|character| character.is_whitespace())
            .is_some()
        {}
        let Some(first) = chars.next() else { break };
        let mut value = String::new();
        if first == '"' {
            let mut closed = false;
            for character in chars.by_ref() {
                if character == '"' {
                    closed = true;
                    break;
                }
                value.push(character);
            }
            if !closed {
                bail!("Unterminated quoted value");
            }
        } else {
            value.push(first);
            while let Some(character) = chars.next_if(|character| !character.is_whitespace()) {
                value.push(character);
            }
        }
        fields.push(value);
    }
    Ok(fields)
}

fn quoted(value: &str) -> Result<String> {
    if value.contains('"') {
        bail!("2DA values containing double quotes cannot be saved without data loss");
    }
    if value.chars().any(char::is_whitespace) {
        Ok(format!("\"{value}\""))
    } else if value.is_empty() {
        Ok("****".to_owned())
    } else {
        Ok(value.to_owned())
    }
}

impl TwoDaFile {
    pub fn read(path: &Path) -> Result<Self> {
        let bytes = super::read_file_limited(path, "2DA file")?;
        if bytes.starts_with(b"2DA V2.b\n") {
            return Self::parse_binary(&bytes);
        }
        let text = std::str::from_utf8(&bytes).context("2DA contains invalid UTF-8")?;
        Self::parse(text)
    }

    pub fn parse_binary(data: &[u8]) -> Result<Self> {
        if !data.starts_with(b"2DA V2.b\n") {
            bail!("Not a binary 2DA V2.b file");
        }
        let mut cursor = 9;
        let read_tab_string = |cursor: &mut usize| -> Result<String> {
            let end = data
                .get(*cursor..)
                .context("Truncated binary 2DA")?
                .iter()
                .position(|byte| *byte == b'\t')
                .context("Truncated binary 2DA string")?
                + *cursor;
            let value: String = data[*cursor..end]
                .iter()
                .map(|byte| char::from(*byte))
                .collect();
            *cursor = end + 1;
            Ok(value)
        };
        let mut columns = vec!["Row".to_owned()];
        loop {
            columns.push(read_tab_string(&mut cursor)?);
            if data.get(cursor) == Some(&0) {
                cursor += 1;
                break;
            }
        }
        let data_columns = columns.len() - 1;
        if data_columns > MAX_COLUMNS {
            bail!("Binary 2DA contains too many columns");
        }
        let row_count = u32::from_le_bytes(
            data.get(cursor..cursor + 4)
                .context("Truncated binary 2DA row count")?
                .try_into()
                .unwrap(),
        ) as usize;
        cursor += 4;
        if row_count > MAX_ROWS {
            bail!("Binary 2DA contains too many rows");
        }
        let minimum_index_bytes = row_count
            .checked_mul(data_columns)
            .and_then(|count| count.checked_mul(2))
            .context("Binary 2DA is too large")?;
        let minimum_remaining = row_count
            .checked_add(minimum_index_bytes)
            .and_then(|count| count.checked_add(2))
            .context("Binary 2DA is too large")?;
        if minimum_remaining > data.len().saturating_sub(cursor) {
            bail!("Binary 2DA row count exceeds the available data");
        }
        if row_count.saturating_mul(columns.len()) > MAX_CELLS {
            bail!("Binary 2DA expands to too many cells");
        }
        let mut row_headers = Vec::with_capacity(row_count);
        for _ in 0..row_count {
            row_headers.push(read_tab_string(&mut cursor)?);
        }
        let index_count = row_count
            .checked_mul(data_columns)
            .context("Binary 2DA is too large")?;
        let index_end = cursor
            .checked_add(index_count * 2)
            .context("Binary 2DA is too large")?;
        let index = data
            .get(cursor..index_end)
            .context("Truncated binary 2DA index")?;
        cursor = index_end;
        let block_size = u16::from_le_bytes(
            data.get(cursor..cursor + 2)
                .context("Truncated binary 2DA data size")?
                .try_into()
                .unwrap(),
        ) as usize;
        cursor += 2;
        let block = data
            .get(cursor..cursor + block_size)
            .context("Truncated binary 2DA data")?;
        let mut rows = Vec::with_capacity(row_count);
        for (row_index, row_header) in row_headers.into_iter().enumerate() {
            let mut row = Vec::with_capacity(columns.len());
            row.push(row_header);
            for column in 0..data_columns {
                let pos = (row_index * data_columns + column) * 2;
                let offset = u16::from_le_bytes(index[pos..pos + 2].try_into().unwrap()) as usize;
                let rest = block
                    .get(offset..)
                    .context("Binary 2DA value offset is out of range")?;
                let end = rest
                    .iter()
                    .position(|byte| *byte == 0)
                    .context("Unterminated binary 2DA value")?;
                let value: String = rest[..end].iter().map(|byte| char::from(*byte)).collect();
                row.push(if value.trim().is_empty() {
                    "****".to_owned()
                } else {
                    value.trim().to_owned()
                });
            }
            rows.push(row);
        }
        Ok(Self {
            default_value: None,
            columns,
            rows,
            format: TwoDaFormat::Binary,
        })
    }

    pub fn parse(text: &str) -> Result<Self> {
        let mut lines = text
            .lines()
            .map(str::trim_end)
            .filter(|line| !line.trim().is_empty());
        let signature = lines.next().context("The 2DA file is empty")?.trim();
        if !signature.eq_ignore_ascii_case("2DA V2.0") {
            if signature.eq_ignore_ascii_case("2DA V2.b") {
                bail!("Binary 2DA V2.b is not supported yet");
            }
            bail!("Not a 2DA V2.0 file");
        }
        let mut next = lines.next().context("The 2DA file has no column header")?;
        let default_value = if next
            .trim_start()
            .to_ascii_lowercase()
            .starts_with("default:")
        {
            let value = next
                .split_once(':')
                .map(|(_, v)| v.trim().to_owned())
                .unwrap_or_default();
            next = lines.next().context("The 2DA file has no column header")?;
            Some(value)
        } else {
            None
        };

        let mut columns = vec!["Row".to_owned()];
        columns.extend(tokenize(next)?);
        if columns.len() < 2 {
            bail!("The 2DA file has no data columns");
        }
        if columns.len() - 1 > MAX_COLUMNS {
            bail!("2DA contains too many columns");
        }
        let mut rows = Vec::new();
        for (line_no, line) in lines.enumerate() {
            let row = tokenize(line).with_context(|| format!("Invalid 2DA row {}", line_no + 1))?;
            if row.len() != columns.len() {
                bail!(
                    "2DA row {} has {} values; expected {}",
                    line_no + 1,
                    row.len(),
                    columns.len()
                );
            }
            if rows.len() >= MAX_ROWS || rows.len().saturating_add(1) * columns.len() > MAX_CELLS {
                bail!("2DA contains too many rows");
            }
            rows.push(row);
        }
        Ok(Self {
            default_value,
            columns,
            rows,
            format: TwoDaFormat::Text,
        })
    }

    fn validate(&self) -> Result<()> {
        if self.columns.len() < 2 || self.columns.first().map(String::as_str) != Some("Row") {
            bail!("2DA must contain the synthetic Row column and at least one data column");
        }
        for name in self.columns.iter().skip(1) {
            if name.is_empty() || name.chars().any(char::is_whitespace) {
                bail!("2DA column names cannot be empty or contain whitespace");
            }
        }
        for (index, row) in self.rows.iter().enumerate() {
            if row.len() != self.columns.len() {
                bail!(
                    "2DA row {index} has {} values; expected {}",
                    row.len(),
                    self.columns.len()
                );
            }
        }
        Ok(())
    }

    pub fn to_text(&self) -> Result<String> {
        self.validate()?;
        let mut widths: Vec<usize> = self.columns.iter().map(|v| v.len() + 2).collect();
        for row in &self.rows {
            for (i, value) in row.iter().enumerate() {
                if i < widths.len() {
                    widths[i] = widths[i].max(quoted(value)?.len() + 2);
                }
            }
        }
        let mut out = String::from("2DA V2.0\r\n\r\n");
        if let Some(default) = &self.default_value {
            out.push_str("DEFAULT: ");
            out.push_str(default);
            out.push_str("\r\n\r\n");
        }
        out.push_str(&" ".repeat(widths.first().copied().unwrap_or(4)));
        for (i, name) in self.columns.iter().enumerate().skip(1) {
            out.push_str(name);
            out.push_str(&" ".repeat(widths[i].saturating_sub(name.len())));
        }
        out.push_str("\r\n");
        for row in &self.rows {
            for (i, value) in row.iter().enumerate() {
                let value = quoted(value)?;
                out.push_str(&value);
                out.push_str(
                    &" ".repeat(
                        widths
                            .get(i)
                            .copied()
                            .unwrap_or(2)
                            .saturating_sub(value.len()),
                    ),
                );
            }
            out.push_str("\r\n");
        }
        Ok(out)
    }

    pub fn to_binary(&self) -> Result<Vec<u8>> {
        self.validate()?;
        if self.default_value.is_some() {
            bail!("Binary 2DA V2.b cannot represent a DEFAULT value");
        }
        fn single_byte(value: &str) -> Result<Vec<u8>> {
            value
                .chars()
                .map(|character| {
                    u8::try_from(character as u32)
                        .context("Binary 2DA V2.b supports only single-byte characters")
                })
                .collect()
        }

        let mut out = b"2DA V2.b\n".to_vec();
        for column in self.columns.iter().skip(1) {
            out.extend(single_byte(column)?);
            out.push(b'\t');
        }
        out.push(0);
        out.extend_from_slice(
            &u32::try_from(self.rows.len())
                .context("Too many binary 2DA rows")?
                .to_le_bytes(),
        );
        for row in &self.rows {
            out.extend(single_byte(&row[0])?);
            out.push(b'\t');
        }

        let mut block = Vec::new();
        let mut offsets = HashMap::<String, u16>::new();
        let mut index = Vec::new();
        for row in &self.rows {
            for value in row.iter().skip(1) {
                let offset = if let Some(offset) = offsets.get(value) {
                    *offset
                } else {
                    let offset = u16::try_from(block.len())
                        .context("Binary 2DA data block exceeds 65535 bytes")?;
                    block.extend(single_byte(value)?);
                    block.push(0);
                    offsets.insert(value.clone(), offset);
                    offset
                };
                index.extend_from_slice(&offset.to_le_bytes());
            }
        }
        let block_size =
            u16::try_from(block.len()).context("Binary 2DA data block exceeds 65535 bytes")?;
        out.extend(index);
        out.extend_from_slice(&block_size.to_le_bytes());
        out.extend(block);
        Ok(out)
    }

    pub fn write(&self, path: &Path) -> Result<()> {
        let bytes = match self.format {
            TwoDaFormat::Text => self.to_text()?.into_bytes(),
            TwoDaFormat::Binary => self.to_binary()?,
        };
        super::atomic_write(path, &bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_da_round_trip() {
        let input = "2DA V2.0\n\n  Label Name\n0 FIGHTER \"A fighter\"\n1 **** Rogue\n";
        let first = TwoDaFile::parse(input).unwrap();
        assert_eq!(first.columns, ["Row", "Label", "Name"]);
        assert_eq!(first.rows[0][2], "A fighter");
        let second = TwoDaFile::parse(&first.to_text().unwrap()).unwrap();
        assert_eq!(first, second);
    }

    #[test]
    fn reads_binary_two_da() {
        let mut data = b"2DA V2.b\nLabel\tName\t\0".to_vec();
        data.extend_from_slice(&1u32.to_le_bytes());
        data.extend_from_slice(b"7\t");
        data.extend_from_slice(&0u16.to_le_bytes());
        data.extend_from_slice(&8u16.to_le_bytes());
        let block = b"Fighter\0Warrior\0";
        data.extend_from_slice(&(block.len() as u16).to_le_bytes());
        data.extend_from_slice(block);
        let table = TwoDaFile::parse_binary(&data).unwrap();
        assert_eq!(table.columns, ["Row", "Label", "Name"]);
        assert_eq!(table.rows[0], ["7", "Fighter", "Warrior"]);
        let encoded = table.to_binary().unwrap();
        assert_eq!(TwoDaFile::parse_binary(&encoded).unwrap(), table);
    }

    #[test]
    fn rejects_malformed_and_lossy_tables() {
        assert!(TwoDaFile::parse("2DA V2.0\n\nLabel\n0 \"unterminated").is_err());
        let mut table = TwoDaFile::parse("2DA V2.0\n\nLabel\n0 ok\n").unwrap();
        table.rows[0][1] = "cannot \" round trip".into();
        assert!(table.to_text().is_err());

        let mut impossible_rows = b"2DA V2.b\nLabel\t\0".to_vec();
        impossible_rows.extend_from_slice(&u32::MAX.to_le_bytes());
        assert!(TwoDaFile::parse_binary(&impossible_rows).is_err());
    }
}
