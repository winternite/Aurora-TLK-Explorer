use anyhow::{Context, Result, bail};
use encoding_rs::{BIG5, EUC_KR, GBK, SHIFT_JIS, WINDOWS_1250, WINDOWS_1252};
use serde::{Deserialize, Serialize};
use std::{fs, path::Path};

const MAGIC: &[u8; 8] = b"TLK V3.0";
const HEADER_SIZE: usize = 20;
const ENTRY_SIZE: usize = 40;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum TlkEncoding {
    #[default]
    Utf8,
    Windows1252,
    Windows1250,
    Korean,
    Big5,
    Gbk,
    ShiftJis,
}

impl TlkEncoding {
    pub const ALL: [Self; 7] = [
        Self::Utf8,
        Self::Windows1252,
        Self::Windows1250,
        Self::Korean,
        Self::Big5,
        Self::Gbk,
        Self::ShiftJis,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Utf8 => "UTF-8 (NWN2 / modern)",
            Self::Windows1252 => "Windows-1252 (Western NWN1)",
            Self::Windows1250 => "Windows-1250 (Polish NWN1)",
            Self::Korean => "Windows-949 (Korean)",
            Self::Big5 => "Big5 (Traditional Chinese)",
            Self::Gbk => "GBK (Simplified Chinese)",
            Self::ShiftJis => "Shift-JIS (Japanese)",
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct TlkEntry {
    pub flags: u32,
    pub sound_resref: String,
    pub volume_variance: u32,
    pub pitch_variance: u32,
    pub sound_length: f32,
    pub text: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TlkFile {
    pub language_id: u32,
    pub encoding: TlkEncoding,
    pub entries: Vec<TlkEntry>,
}

impl Default for TlkFile {
    fn default() -> Self {
        Self {
            language_id: 0,
            encoding: TlkEncoding::Windows1252,
            entries: Vec::new(),
        }
    }
}

fn u32_at(data: &[u8], pos: usize) -> Result<u32> {
    let bytes: [u8; 4] = data
        .get(pos..pos + 4)
        .context("unexpected end of TLK file")?
        .try_into()
        .unwrap();
    Ok(u32::from_le_bytes(bytes))
}

fn decode(bytes: &[u8], encoding: TlkEncoding) -> Result<String> {
    if encoding == TlkEncoding::Utf8 {
        Ok(std::str::from_utf8(bytes)
            .context("TLK contains invalid UTF-8")?
            .to_owned())
    } else {
        let codec = match encoding {
            TlkEncoding::Windows1252 => WINDOWS_1252,
            TlkEncoding::Windows1250 => WINDOWS_1250,
            TlkEncoding::Korean => EUC_KR,
            TlkEncoding::Big5 => BIG5,
            TlkEncoding::Gbk => GBK,
            TlkEncoding::ShiftJis => SHIFT_JIS,
            TlkEncoding::Utf8 => unreachable!(),
        };
        let (text, _, had_errors) = codec.decode(bytes);
        if had_errors {
            bail!("TLK contains bytes invalid for {}", encoding.label());
        }
        Ok(text.into_owned())
    }
}

fn encode(text: &str, encoding: TlkEncoding) -> Result<Vec<u8>> {
    if encoding == TlkEncoding::Utf8 {
        Ok(text.as_bytes().to_vec())
    } else {
        let codec = match encoding {
            TlkEncoding::Windows1252 => WINDOWS_1252,
            TlkEncoding::Windows1250 => WINDOWS_1250,
            TlkEncoding::Korean => EUC_KR,
            TlkEncoding::Big5 => BIG5,
            TlkEncoding::Gbk => GBK,
            TlkEncoding::ShiftJis => SHIFT_JIS,
            TlkEncoding::Utf8 => unreachable!(),
        };
        let (bytes, _, had_errors) = codec.encode(text);
        if had_errors {
            bail!(
                "Text contains characters that cannot be represented as {}",
                encoding.label()
            );
        }
        Ok(bytes.into_owned())
    }
}

fn likely_encoding(language_id: u32, strings: &[&[u8]]) -> TlkEncoding {
    let non_ascii: Vec<u8> = strings
        .iter()
        .flat_map(|s| s.iter().copied())
        .filter(|byte| !byte.is_ascii())
        .collect();
    if !non_ascii.is_empty() && std::str::from_utf8(&non_ascii).is_ok() {
        return TlkEncoding::Utf8;
    }
    match language_id {
        5 => TlkEncoding::Windows1250,
        128 => TlkEncoding::Korean,
        129 => TlkEncoding::Big5,
        130 => TlkEncoding::Gbk,
        131 => TlkEncoding::ShiftJis,
        _ => TlkEncoding::Windows1252,
    }
}

impl TlkFile {
    pub fn read(path: &Path) -> Result<Self> {
        let data = fs::read(path).with_context(|| format!("Could not read {}", path.display()))?;
        Self::from_bytes(&data)
    }

    pub fn from_bytes(data: &[u8]) -> Result<Self> {
        if data.get(..8) != Some(MAGIC) {
            bail!("Not a TLK V3.0 file");
        }
        let language_id = u32_at(data, 8)?;
        let count = u32_at(data, 12)? as usize;
        let strings_start = u32_at(data, 16)? as usize;
        let index_end = HEADER_SIZE
            .checked_add(count.checked_mul(ENTRY_SIZE).context("TLK is too large")?)
            .context("TLK is too large")?;
        if index_end > data.len() || strings_start > data.len() {
            bail!("TLK index points beyond the end of the file");
        }

        let mut ranges = Vec::with_capacity(count);
        for i in 0..count {
            let base = HEADER_SIZE + i * ENTRY_SIZE;
            let offset = u32_at(data, base + 28)? as usize;
            let length = u32_at(data, base + 32)? as usize;
            let start = strings_start
                .checked_add(offset)
                .context("Invalid TLK string offset")?;
            let end = start
                .checked_add(length)
                .context("Invalid TLK string length")?;
            if end > data.len() {
                bail!("TLK string {i} points beyond the end of the file");
            }
            ranges.push(start..end);
        }
        let raw_strings: Vec<&[u8]> = ranges.iter().map(|range| &data[range.clone()]).collect();
        let encoding = likely_encoding(language_id, &raw_strings);
        let mut entries = Vec::with_capacity(count);
        for (i, range) in ranges.into_iter().enumerate() {
            let base = HEADER_SIZE + i * ENTRY_SIZE;
            let resref_bytes = &data[base + 4..base + 20];
            let resref_end = resref_bytes.iter().position(|b| *b == 0).unwrap_or(16);
            entries.push(TlkEntry {
                flags: u32_at(data, base)?,
                sound_resref: std::str::from_utf8(&resref_bytes[..resref_end])
                    .context("TLK contains a non-UTF-8 sound ResRef")?
                    .trim()
                    .to_owned(),
                volume_variance: u32_at(data, base + 20)?,
                pitch_variance: u32_at(data, base + 24)?,
                sound_length: f32::from_bits(u32_at(data, base + 36)?),
                text: decode(&data[range], encoding)
                    .with_context(|| format!("Could not decode TLK string {i}"))?,
            });
        }
        Ok(Self {
            language_id,
            encoding,
            entries,
        })
    }

    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        let strings: Vec<Vec<u8>> = self
            .entries
            .iter()
            .enumerate()
            .map(|(index, entry)| {
                encode(&entry.text, self.encoding)
                    .with_context(|| format!("Could not encode TLK string {index}"))
            })
            .collect::<Result<_>>()?;
        let strings_start = HEADER_SIZE
            .checked_add(
                self.entries
                    .len()
                    .checked_mul(ENTRY_SIZE)
                    .context("TLK index is too large")?,
            )
            .context("TLK index is too large")?;
        let total_strings = strings.iter().try_fold(0usize, |total, value| {
            total
                .checked_add(value.len())
                .context("TLK strings are too large")
        })?;
        let capacity = strings_start
            .checked_add(total_strings)
            .context("TLK is too large")?;
        let mut out = Vec::with_capacity(capacity);
        out.extend_from_slice(MAGIC);
        out.extend_from_slice(&self.language_id.to_le_bytes());
        out.extend_from_slice(
            &(u32::try_from(self.entries.len()).context("Too many TLK entries")?).to_le_bytes(),
        );
        out.extend_from_slice(
            &(u32::try_from(strings_start).context("TLK index is too large")?).to_le_bytes(),
        );

        let mut offset = 0usize;
        for (entry, text) in self.entries.iter().zip(&strings) {
            out.extend_from_slice(&entry.flags.to_le_bytes());
            let resref = entry.sound_resref.as_bytes();
            if resref.len() > 16 {
                bail!("Sound ResRef is longer than the TLK limit of 16 bytes");
            }
            if !resref.is_ascii() {
                bail!("Sound ResRef must contain ASCII characters");
            }
            out.extend_from_slice(resref);
            out.resize(out.len() + 16 - resref.len(), 0);
            out.extend_from_slice(&entry.volume_variance.to_le_bytes());
            out.extend_from_slice(&entry.pitch_variance.to_le_bytes());
            out.extend_from_slice(
                &(u32::try_from(offset).context("TLK strings are too large")?).to_le_bytes(),
            );
            out.extend_from_slice(
                &(u32::try_from(text.len()).context("TLK string is too large")?).to_le_bytes(),
            );
            out.extend_from_slice(&entry.sound_length.to_bits().to_le_bytes());
            offset = offset
                .checked_add(text.len())
                .context("TLK strings are too large")?;
        }
        for text in strings {
            out.extend_from_slice(&text);
        }
        Ok(out)
    }

    pub fn write(&self, path: &Path) -> Result<()> {
        super::atomic_write(path, &self.to_bytes()?)
    }

    pub fn write_diff(path: &Path, entries: &[(usize, TlkEntry)]) -> Result<()> {
        let mut out = Vec::new();
        out.extend_from_slice(
            &(u32::try_from(entries.len()).context("Too many diff entries")?).to_be_bytes(),
        );
        for (position, entry) in entries {
            out.extend_from_slice(
                &(u32::try_from(*position).context("StrRef is too large")?).to_be_bytes(),
            );
            out.push(entry.flags as u8);
            out.extend_from_slice(&entry.sound_length.to_bits().to_be_bytes());
            let sound = entry.sound_resref.as_bytes();
            let sound_len = sound.len().min(u8::MAX as usize);
            out.push(sound_len as u8);
            out.extend_from_slice(&sound[..sound_len]);
            let text = entry.text.as_bytes();
            out.extend_from_slice(
                &(u32::try_from(text.len()).context("Diff text is too large")?).to_be_bytes(),
            );
            out.extend_from_slice(text);
        }
        super::atomic_write(path, &out)
    }

    pub fn read_diff(path: &Path) -> Result<Vec<(usize, TlkEntry)>> {
        let data = fs::read(path)?;
        let mut cursor = 0;
        let read_u32 = |cursor: &mut usize| -> Result<u32> {
            let bytes: [u8; 4] = data
                .get(*cursor..*cursor + 4)
                .context("Truncated TLK diff")?
                .try_into()
                .unwrap();
            *cursor += 4;
            Ok(u32::from_be_bytes(bytes))
        };
        let count = read_u32(&mut cursor)? as usize;
        let mut entries = Vec::with_capacity(count);
        for _ in 0..count {
            let position = read_u32(&mut cursor)? as usize;
            let flags = *data.get(cursor).context("Truncated TLK diff")? as u32;
            cursor += 1;
            let sound_length = f32::from_bits(read_u32(&mut cursor)?);
            let sound_len = *data.get(cursor).context("Truncated TLK diff")? as usize;
            cursor += 1;
            let sound = data
                .get(cursor..cursor + sound_len)
                .context("Truncated TLK diff")?;
            cursor += sound_len;
            let text_len = read_u32(&mut cursor)? as usize;
            let text = data
                .get(cursor..cursor + text_len)
                .context("Truncated TLK diff")?;
            cursor += text_len;
            entries.push((
                position,
                TlkEntry {
                    flags,
                    sound_resref: std::str::from_utf8(sound)
                        .context("Diff contains an invalid sound ResRef")?
                        .to_owned(),
                    sound_length,
                    text: std::str::from_utf8(text)
                        .context("Diff contains invalid UTF-8 text")?
                        .to_owned(),
                    ..Default::default()
                },
            ));
        }
        Ok(entries)
    }

    pub fn read_dtu(path: &Path) -> Result<Vec<(usize, TlkEntry)>> {
        let data = fs::read(path)?;
        let mut cursor = 0;
        let read_le_u32 = |cursor: &mut usize| -> Result<u32> {
            let bytes: [u8; 4] = data
                .get(*cursor..*cursor + 4)
                .context("Truncated DTU file")?
                .try_into()
                .unwrap();
            *cursor += 4;
            Ok(u32::from_le_bytes(bytes))
        };
        let count = read_le_u32(&mut cursor)? as usize;
        let mut entries = Vec::with_capacity(count);
        for _ in 0..count {
            let position = read_le_u32(&mut cursor)? as usize;
            let sound_len = *data.get(cursor).context("Truncated DTU file")? as usize;
            cursor += 1;
            let sound = data
                .get(cursor..cursor + sound_len)
                .context("Truncated DTU file")?;
            cursor += sound_len;
            let first = *data.get(cursor).context("Truncated DTU file")? as usize;
            cursor += 1;
            let text_len = if first == 255 {
                let low = *data.get(cursor).context("Truncated DTU file")? as usize;
                let high = *data.get(cursor + 1).context("Truncated DTU file")? as usize;
                cursor += 2;
                low | (high << 8)
            } else {
                first
            };
            let text = data
                .get(cursor..cursor + text_len)
                .context("Truncated DTU file")?;
            cursor += text_len;
            let sound_resref = std::str::from_utf8(sound)
                .context("DTU contains an invalid sound ResRef")?
                .to_owned();
            let text = std::str::from_utf8(text)
                .context("DTU contains invalid UTF-8 text")?
                .to_owned();
            let mut flags = 0;
            if !text.is_empty() {
                flags |= 1;
            }
            if !sound_resref.is_empty() {
                flags |= 2;
            }
            entries.push((
                position,
                TlkEntry {
                    flags,
                    sound_resref,
                    text,
                    ..Default::default()
                },
            ));
        }
        Ok(entries)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn tlk_round_trip() {
        let original = TlkFile {
            language_id: 0,
            encoding: TlkEncoding::Windows1252,
            entries: vec![
                TlkEntry {
                    flags: 1,
                    text: "Café".into(),
                    ..Default::default()
                },
                TlkEntry {
                    flags: 7,
                    sound_resref: "hello".into(),
                    sound_length: 1.25,
                    text: "Line\ntwo".into(),
                    ..Default::default()
                },
            ],
        };
        let decoded = TlkFile::from_bytes(&original.to_bytes().unwrap()).unwrap();
        assert_eq!(decoded.entries, original.entries);
        assert_eq!(decoded.language_id, 0);
    }

    #[test]
    fn legacy_diff_round_trip() {
        let path = std::env::temp_dir().join(format!(
            "aurora-diff-{}.bin",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let entries = vec![(
            42,
            TlkEntry {
                flags: 7,
                sound_resref: "voice".into(),
                sound_length: 2.5,
                text: "A changed line".into(),
                ..Default::default()
            },
        )];
        TlkFile::write_diff(&path, &entries).unwrap();
        assert_eq!(TlkFile::read_diff(&path).unwrap(), entries);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn rejects_lossy_text_and_overlong_resrefs() {
        let mut file = TlkFile {
            encoding: TlkEncoding::Windows1252,
            entries: vec![TlkEntry {
                text: "not representable: Ж".into(),
                ..Default::default()
            }],
            ..Default::default()
        };
        assert!(file.to_bytes().is_err());
        file.entries[0].text.clear();
        file.entries[0].sound_resref = "this_resref_is_too_long".into();
        assert!(file.to_bytes().is_err());
    }
}
