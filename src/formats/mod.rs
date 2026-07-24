pub mod itp;
pub mod tlk;
pub mod twoda;

use anyhow::{Context, Result};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

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
        fs::rename(&temp, path)
            .with_context(|| format!("Could not atomically replace {}", path.display()))?;
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
}
