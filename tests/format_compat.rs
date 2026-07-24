use aurora_tlk_explorer::formats::{
    tlk::{TlkEncoding, TlkEntry, TlkFile},
    twoda::{TwoDaFile, TwoDaFormat},
};

#[test]
fn representative_tlk_encodings_round_trip() {
    for (encoding, language_id, text) in [
        (TlkEncoding::Utf8, 0, "Dvořák — 日本語"),
        (TlkEncoding::Windows1252, 0, "Café déjà vu"),
        (TlkEncoding::Windows1250, 5, "Příliš žluťoučký kůň"),
        (TlkEncoding::Korean, 128, "한국어"),
        (TlkEncoding::Big5, 129, "中文"),
        (TlkEncoding::Gbk, 130, "简体中文"),
        (TlkEncoding::ShiftJis, 131, "日本語"),
    ] {
        let original = TlkFile {
            language_id,
            encoding,
            entries: vec![TlkEntry {
                flags: 1,
                sound_resref: "voice_01".into(),
                text: text.into(),
                ..Default::default()
            }],
        };
        let decoded = TlkFile::from_bytes(&original.to_bytes().unwrap()).unwrap();
        assert_eq!(decoded.entries, original.entries, "{encoding:?}");
    }
}

#[test]
fn representative_text_and_binary_twoda_round_trip() {
    let text = "2DA V2.0\n\nDEFAULT: ****\n\nLabel Name Value\n0 Fighter \"Two words\" 42\n1 Rogue **** -1\n";
    let original = TwoDaFile::parse(text).unwrap();
    let decoded = TwoDaFile::parse(&original.to_text().unwrap()).unwrap();
    assert_eq!(decoded, original);

    let mut binary = TwoDaFile::parse("2DA V2.0\n\nLabel Name\n7 Fighter Warrior\n").unwrap();
    binary.format = TwoDaFormat::Binary;
    let encoded = binary.to_binary().unwrap();
    assert_eq!(TwoDaFile::parse_binary(&encoded).unwrap(), binary);
}

#[test]
fn malformed_headers_offsets_and_rows_are_rejected() {
    assert!(TlkFile::from_bytes(b"not a tlk").is_err());
    let mut tlk = b"TLK V3.0".to_vec();
    tlk.extend_from_slice(&0u32.to_le_bytes());
    tlk.extend_from_slice(&u32::MAX.to_le_bytes());
    tlk.extend_from_slice(&20u32.to_le_bytes());
    assert!(TlkFile::from_bytes(&tlk).is_err());

    assert!(TwoDaFile::parse("2DA V2.0\n\nLabel Name\n0 only-one\n").is_err());
    assert!(TwoDaFile::parse_binary(b"2DA V2.b\ntruncated").is_err());
}
