pub mod itp;
pub mod tlk;
pub mod twoda;

use anyhow::{Context, Result};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, SystemTime},
};

static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);
pub(crate) const MAX_DOCUMENT_BYTES: u64 = 512 * 1024 * 1024;
const STALE_TEMP_AGE: Duration = Duration::from_secs(24 * 60 * 60);

/// Remove only Aurora's abandoned save/conversion files for this document.
///
/// Files younger than a day are preserved so another running Aurora instance
/// cannot lose an in-progress atomic save.
pub(crate) fn cleanup_stale_temp_files(path: &Path) {
    let Some(parent) = path.parent() else {
        return;
    };
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("document");
    let prefix = format!(".{name}.aurora-tmp-");
    let cutoff = SystemTime::now().checked_sub(STALE_TEMP_AGE);
    let Ok(entries) = fs::read_dir(parent) else {
        return;
    };
    for entry in entries.flatten() {
        let file_name = entry.file_name();
        let Some(file_name) = file_name.to_str() else {
            continue;
        };
        if !file_name.starts_with(&prefix) {
            continue;
        }
        let Ok(metadata) = entry.metadata() else {
            continue;
        };
        if !metadata.is_file()
            || cutoff
                .is_some_and(|cutoff| metadata.modified().is_ok_and(|modified| modified > cutoff))
        {
            continue;
        }
        let _ = fs::remove_file(entry.path());
    }
}

#[cfg(windows)]
fn replace_file(temp: &Path, destination: &Path) -> Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::ReplaceFileW;

    if !destination.exists() {
        fs::rename(temp, destination)?;
        return Ok(());
    }
    let destination_path = destination.to_path_buf();
    let destination: Vec<u16> = destination
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect();
    let temp: Vec<u16> = temp.as_os_str().encode_wide().chain(Some(0)).collect();
    // ReplaceFile preserves the existing destination until the replacement is
    // complete, unlike delete-then-rename.
    if unsafe {
        ReplaceFileW(
            destination.as_ptr(),
            temp.as_ptr(),
            std::ptr::null(),
            0,
            std::ptr::null(),
            std::ptr::null(),
        )
    } == 0
    {
        return Err(std::io::Error::last_os_error()).with_context(|| {
            format!(
                "Could not atomically replace {}",
                destination_path.display()
            )
        });
    }
    Ok(())
}

#[cfg(not(windows))]
fn replace_file(temp: &Path, destination: &Path) -> Result<()> {
    fs::rename(temp, destination)
        .with_context(|| format!("Could not atomically replace {}", destination.display()))
}

pub(crate) fn ensure_file_size(path: &Path, kind: &str) -> Result<()> {
    let metadata =
        fs::metadata(path).with_context(|| format!("Could not inspect {}", path.display()))?;
    if metadata.len() > MAX_DOCUMENT_BYTES {
        anyhow::bail!(
            "{kind} exceeds the supported size limit of {} MiB",
            MAX_DOCUMENT_BYTES / (1024 * 1024)
        );
    }
    Ok(())
}

pub(crate) fn read_file_limited(path: &Path, kind: &str) -> Result<Vec<u8>> {
    cleanup_stale_temp_files(path);
    ensure_file_size(path, kind)?;
    let bytes = fs::read(path).with_context(|| format!("Could not read {}", path.display()))?;
    if bytes.len() as u64 > MAX_DOCUMENT_BYTES {
        anyhow::bail!(
            "{kind} exceeds the supported size limit of {} MiB",
            MAX_DOCUMENT_BYTES / (1024 * 1024)
        );
    }
    Ok(bytes)
}

pub(crate) fn unique_temp_path(path: &Path) -> PathBuf {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("document");
    let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    parent.join(format!(
        ".{name}.aurora-tmp-{}-{sequence}",
        std::process::id()
    ))
}

pub(crate) fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    cleanup_stale_temp_files(path);
    #[cfg(unix)]
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let (temp, mut file) = (0..32)
        .find_map(|_| {
            let temp = unique_temp_path(path);
            match OpenOptions::new().write(true).create_new(true).open(&temp) {
                Ok(file) => Some(Ok((temp, file))),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => None,
                Err(error) => Some(Err(error)),
            }
        })
        .transpose()
        .with_context(|| {
            format!(
                "Could not create a temporary file beside {}",
                path.display()
            )
        })?
        .context("Could not allocate a unique temporary file")?;

    let result = (|| -> Result<()> {
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        replace_file(&temp, path)?;
        #[cfg(unix)]
        fs::File::open(parent)
            .and_then(|directory| directory.sync_all())
            .with_context(|| format!("Could not sync {}", parent.display()))?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atomic_write_replaces_an_existing_file() {
        let path = std::env::temp_dir().join(format!(
            "aurora-atomic-write-{}-{}",
            std::process::id(),
            TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        atomic_write(&path, b"first").unwrap();
        atomic_write(&path, b"second").unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"second");
        let _ = fs::remove_file(path);
    }

    #[test]
    fn oversized_files_are_rejected_before_reading() {
        let path = std::env::temp_dir().join(format!(
            "aurora-oversized-{}-{}",
            std::process::id(),
            TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        let file = fs::File::create(&path).unwrap();
        file.set_len(MAX_DOCUMENT_BYTES + 1).unwrap();
        drop(file);
        assert!(read_file_limited(&path, "test file").is_err());
        let _ = fs::remove_file(path);
    }
}
