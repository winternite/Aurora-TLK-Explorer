use super::{atomic_write, unique_temp_path};
use anyhow::{Context, Result, bail};
use serde_json::Value;
use std::{
    io::{Read, Write},
    path::Path,
    process::{Command, Output, Stdio},
    thread,
    time::{Duration, Instant},
};

#[derive(Clone, Debug, PartialEq)]
pub struct ItpFile {
    pub root: Value,
}

#[cfg(target_os = "windows")]
fn converter() -> std::path::PathBuf {
    const WINDOWS_CONVERTER: &[u8] = include_bytes!("../../assets/nwn_gff-x86_64-windows.exe");
    const WINDOWS_SQLITE: &[u8] = include_bytes!("../../assets/sqlite3_64-windows.dll");
    fn install(path: &Path, bytes: &[u8]) {
        if std::fs::read(path).is_ok_and(|installed| installed == bytes) {
            return;
        }
        let _ = atomic_write(path, bytes);
    }

    let directory = std::env::temp_dir().join("AuroraTlkExplorer");
    let path = directory.join("nwn_gff-2.1.2.exe");
    let _ = std::fs::create_dir_all(&directory);
    install(&path, WINDOWS_CONVERTER);
    install(&directory.join("sqlite3_64.dll"), WINDOWS_SQLITE);
    path
}

#[cfg(not(target_os = "windows"))]
fn converter() -> std::path::PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.join("nwn_gff")))
        .filter(|path| path.is_file())
        .unwrap_or_else(|| "nwn_gff".into())
}

fn run_converter(mut command: Command, input: Option<&[u8]>) -> Result<Output> {
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    if input.is_some() {
        command.stdin(Stdio::piped());
    }
    let mut child = command
        .spawn()
        .with_context(|| "Could not start bundled NWN GFF converter")?;
    let mut stdout = child
        .stdout
        .take()
        .context("Could not capture converter output")?;
    let mut stderr = child
        .stderr
        .take()
        .context("Could not capture converter errors")?;
    let stdout_reader = thread::spawn(move || {
        let mut bytes = Vec::new();
        stdout.read_to_end(&mut bytes).map(|_| bytes)
    });
    let stderr_reader = thread::spawn(move || {
        let mut bytes = Vec::new();
        stderr.read_to_end(&mut bytes).map(|_| bytes)
    });
    if let Some(bytes) = input {
        child
            .stdin
            .take()
            .context("Could not open converter input")?
            .write_all(bytes)?;
    }

    let deadline = Instant::now() + Duration::from_secs(60);
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            bail!("NWN GFF converter timed out after 60 seconds");
        }
        thread::sleep(Duration::from_millis(20));
    };
    let stdout = stdout_reader
        .join()
        .map_err(|_| anyhow::anyhow!("Converter output reader failed"))??;
    let stderr = stderr_reader
        .join()
        .map_err(|_| anyhow::anyhow!("Converter error reader failed"))??;
    Ok(Output {
        status,
        stdout,
        stderr,
    })
}

fn validate_field(value: &Value, depth: usize, nodes: &mut usize) -> Result<()> {
    if depth > 128 {
        bail!("ITP structure exceeds the maximum nesting depth");
    }
    *nodes = nodes.checked_add(1).context("ITP structure is too large")?;
    if *nodes > 1_000_000 {
        bail!("ITP structure contains too many values");
    }
    match value {
        Value::Array(values) => {
            for value in values {
                validate_field(value, depth + 1, nodes)?;
            }
        }
        Value::Object(object) => {
            if let (Some(kind), Some(field_value)) = (
                object.get("type").and_then(Value::as_str),
                object.get("value"),
            ) {
                match kind {
                    "byte"
                        if field_value
                            .as_u64()
                            .is_none_or(|value| value > u8::MAX.into()) =>
                    {
                        bail!("ITP byte field is outside 0..=255")
                    }
                    "word"
                        if field_value
                            .as_u64()
                            .is_none_or(|value| value > u16::MAX.into()) =>
                    {
                        bail!("ITP word field is outside 0..=65535")
                    }
                    "dword"
                        if field_value
                            .as_u64()
                            .is_none_or(|value| value > u32::MAX.into()) =>
                    {
                        bail!("ITP dword field is outside the unsigned 32-bit range")
                    }
                    "char"
                        if field_value
                            .as_i64()
                            .is_none_or(|value| !(-128..=127).contains(&value)) =>
                    {
                        bail!("ITP char field is outside the signed 8-bit range")
                    }
                    "short"
                        if field_value
                            .as_i64()
                            .is_none_or(|value| !(-32768..=32767).contains(&value)) =>
                    {
                        bail!("ITP short field is outside the signed 16-bit range")
                    }
                    "int"
                        if field_value
                            .as_i64()
                            .is_none_or(|value| i32::try_from(value).is_err()) =>
                    {
                        bail!("ITP int field is outside the signed 32-bit range")
                    }
                    "list" if !field_value.is_array() => bail!("ITP list field is not a list"),
                    "cexostring" | "resref" if !field_value.is_string() => {
                        bail!("ITP text field is not a string")
                    }
                    _ => {}
                }
            }
            for child in object.values() {
                validate_field(child, depth + 1, nodes)?;
            }
        }
        _ => {}
    }
    Ok(())
}

impl ItpFile {
    pub fn validate(&self) -> Result<()> {
        if self.root.get("__data_type").and_then(Value::as_str) != Some("ITP ") {
            bail!("The GFF resource is not an ITP palette");
        }
        if self
            .root
            .pointer("/MAIN/value")
            .and_then(Value::as_array)
            .is_none()
        {
            bail!("ITP palette has no MAIN tree");
        }
        validate_field(&self.root, 0, &mut 0)
    }

    pub fn read(path: &Path) -> Result<Self> {
        let mut command = Command::new(converter());
        command.args([
            "-i",
            &path.to_string_lossy(),
            "-k",
            "json",
            "--other-encoding",
            "UTF-8",
        ]);
        let output = run_converter(command, None)?;
        if !output.status.success() {
            bail!(
                "Could not decode ITP: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
        let root: Value =
            serde_json::from_slice(&output.stdout).context("Invalid ITP structure")?;
        let file = Self { root };
        file.validate()?;
        Ok(file)
    }

    pub fn write(&self, path: &Path) -> Result<()> {
        self.validate()?;
        let encoded = unique_temp_path(path).with_extension("itp.aurora-encoded");
        let mut command = Command::new(converter());
        command.args([
            "-l",
            "json",
            "-k",
            "gff",
            "-o",
            &encoded.to_string_lossy(),
            "--other-encoding",
            "UTF-8",
        ]);
        let json = serde_json::to_vec(&self.root)?;
        let output = run_converter(command, Some(&json))?;
        if !output.status.success() {
            let _ = std::fs::remove_file(&encoded);
            bail!(
                "Could not encode ITP: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
        let bytes = std::fs::read(&encoded)?;
        let _ = std::fs::remove_file(encoded);
        atomic_write(path, &bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "set ATE_ITP_FIXTURE to run the external converter integration test"]
    fn reads_and_round_trips_item_palette() {
        let fixture = std::env::var_os("ATE_ITP_FIXTURE")
            .expect("ATE_ITP_FIXTURE must name a real ITP palette");
        let source = Path::new(&fixture);
        let itp = ItpFile::read(source).unwrap();
        assert!(
            !itp.root
                .pointer("/MAIN/value")
                .unwrap()
                .as_array()
                .unwrap()
                .is_empty()
        );
        let output = std::env::temp_dir().join("aurora-itp-roundtrip.itp");
        itp.write(&output).unwrap();
        let reread = ItpFile::read(&output).unwrap();
        assert_eq!(itp, reread);
        let _ = std::fs::remove_file(output);
    }

    #[test]
    fn validates_required_structure_and_ranges() {
        let missing_main = ItpFile {
            root: serde_json::json!({"__data_type": "ITP "}),
        };
        assert!(missing_main.validate().is_err());
        let invalid_byte = ItpFile {
            root: serde_json::json!({
                "__data_type": "ITP ",
                "MAIN": {"type": "list", "value": [{
                    "__struct_id": 1,
                    "ID": {"type": "byte", "value": 256}
                }]}
            }),
        };
        assert!(invalid_byte.validate().is_err());
    }
}
