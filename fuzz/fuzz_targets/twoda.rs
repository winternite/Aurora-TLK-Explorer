#![no_main]

use aurora_tlk_explorer::formats::twoda::TwoDaFile;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if data.starts_with(b"2DA V2.b\n") {
        if let Ok(file) = TwoDaFile::parse_binary(data) {
            let _ = file.to_binary();
        }
    } else if let Ok(text) = std::str::from_utf8(data)
        && let Ok(file) = TwoDaFile::parse(text)
    {
        let _ = file.to_text();
    }
});
