#![no_main]

use aurora_tlk_explorer::formats::tlk::TlkFile;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(file) = TlkFile::from_bytes(data) {
        let _ = file.to_bytes();
    }
});
