//! Form module tests — build a minimal HWPX in code, then verify
//! recognition, format-preserving fill, byte-lossless repackaging, and patch.

use super::*;
use std::io::Write;
use zip::write::SimpleFileOptions;

/// A `<hp:tc>` cell: subList → p → run → t, then this cell's own addr/span.
fn cell(row: u32, col: u32, text: &str) -> String {
    format!(
        concat!(
            r#"<hp:tc name="" borderFillIDRef="1">"#,
            r#"<hp:subList id="" vertAlign="CENTER">"#,
            r#"<hp:p id="0" paraPrIDRef="0" styleIDRef="0"><hp:run charPrIDRef="0">"#,
            "<hp:t>{}</hp:t></hp:run>",
            "<hp:linesegarray><hp:lineseg textpos=\"0\" vertpos=\"0\"/></hp:linesegarray>",
            "</hp:p></hp:subList>",
            r#"<hp:cellAddr colAddr="{}" rowAddr="{}"/><hp:cellSpan colSpan="1" rowSpan="1"/>"#,
            r#"<hp:cellSz width="4000" height="1000"/>"#,
            "</hp:tc>"
        ),
        scan_escape(text),
        col,
        row
    )
}

fn scan_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

fn row(cells: &[String]) -> String {
    format!("<hp:tr>{}</hp:tr>", cells.concat())
}

/// A section XML with a 2x2 label|value form table plus one inline body field.
fn sample_section() -> String {
    let table = format!(
        r#"<hp:p id="1" paraPrIDRef="0"><hp:run charPrIDRef="0"><hp:tbl id="9" rowCnt="2" colCnt="2" borderFillIDRef="1">{}{}</hp:tbl></hp:run></hp:p>"#,
        row(&[cell(0, 0, "성명"), cell(0, 1, "")]),
        row(&[cell(1, 0, "생년월일"), cell(1, 1, "")]),
    );
    let inline = r#"<hp:p id="2" paraPrIDRef="0"><hp:run charPrIDRef="0"><hp:t>주소: </hp:t></hp:run><hp:linesegarray><hp:lineseg textpos="0"/></hp:linesegarray></hp:p>"#;
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><hp:sec xmlns:hp="http://www.hancom.co.kr/hwpml/2011/paragraph">{table}{inline}</hp:sec>"#
    )
}

/// Assemble a minimal but valid HWPX (mimetype STORED first, section deflated).
fn build_hwpx(section: &str, extra: Option<(&str, &[u8])>) -> Vec<u8> {
    let mut buf = Vec::new();
    {
        let mut zw = zip::ZipWriter::new(std::io::Cursor::new(&mut buf));
        let stored = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
        let deflated = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

        zw.start_file("mimetype", stored).unwrap();
        zw.write_all(b"application/hwp+zip").unwrap();

        zw.start_file("version.xml", deflated).unwrap();
        zw.write_all(br#"<?xml version="1.0"?><hv:HCFVersion/>"#).unwrap();

        zw.start_file("Contents/header.xml", deflated).unwrap();
        zw.write_all(br#"<?xml version="1.0"?><hh:head/>"#).unwrap();

        if let Some((name, data)) = extra {
            zw.start_file(name, stored).unwrap();
            zw.write_all(data).unwrap();
        }

        zw.start_file("Contents/section0.xml", deflated).unwrap();
        zw.write_all(section.as_bytes()).unwrap();

        zw.finish().unwrap();
    }
    buf
}

fn read_section0(bytes: &[u8]) -> String {
    let mut zip = ZipArchive::new(std::io::Cursor::new(bytes.to_vec())).unwrap();
    let mut s = String::new();
    zip.by_name("Contents/section0.xml").unwrap().read_to_string(&mut s).unwrap();
    s
}

#[test]
fn form_scan_reconstructs_cells() {
    let xml = sample_section();
    let scan = scan_section(&xml);
    assert_eq!(scan.tables.len(), 1, "one table expected");
    let rows = scan.tables[0].rows();
    assert_eq!(rows.len(), 2, "two rows");
    assert_eq!(rows[0][0].text(), "성명");
    assert_eq!(rows[0][1].text(), "");
    assert_eq!(rows[1][0].text(), "생년월일");
    // inline body paragraph captured
    assert!(scan.body_paras.iter().any(|p| p.text.starts_with("주소")));
}

#[test]
fn form_extract_fields_and_schema() {
    let bytes = build_hwpx(&sample_section(), None);
    let result = extract_form_fields(&bytes).unwrap();
    let labels: Vec<&str> = result.fields.iter().map(|f| f.label.as_str()).collect();
    assert!(labels.contains(&"성명"), "성명 field, got {labels:?}");
    assert!(labels.contains(&"생년월일"), "생년월일 field");
    assert!(result.confidence > 0.0);

    let schema = extract_form_schema(&bytes).unwrap();
    let birth = schema.fields.iter().find(|f| f.label == "생년월일").unwrap();
    assert_eq!(birth.field_type, FormFieldType::Date);
    assert!(birth.empty);
    // empty inline label surfaced as fill target
    assert!(schema.fields.iter().any(|f| f.label == "주소" && f.empty));

    // JSON schema serializes
    let json = form_schema_json(&bytes).unwrap();
    assert!(json.contains("\"type\""));
}

#[test]
fn form_fill_writes_values_and_preserves_format() {
    let bytes = build_hwpx(&sample_section(), None);
    let values = values_from_pairs([
        ("성명", "홍길동"),
        ("생년월일", "1990-01-01"),
        ("주소", "서울시 강남구"),
    ]);
    let res = fill_hwpx(&bytes, &values).unwrap();

    let filled: Vec<&str> = res.filled.iter().map(|f| f.label.as_str()).collect();
    assert!(filled.contains(&"성명"), "성명 filled, got {filled:?}");
    assert!(res.unmatched.is_empty(), "all matched, unmatched={:?}", res.unmatched);

    let section = read_section0(&res.buffer);
    assert!(section.contains("홍길동"), "value written into cell");
    assert!(section.contains("1990-01-01"));
    assert!(section.contains("서울시 강남구"), "inline value written");
    // formatting-bearing attributes untouched
    assert!(section.contains(r#"charPrIDRef="0""#));
    assert!(section.contains(r#"borderFillIDRef="1""#));
    // linesegarray removed so the viewer recomputes layout
    assert!(!section.contains("<hp:linesegarray"), "lineseg cache cleared");

    // output ZIP re-opens; mimetype still first & stored
    let entries = read_zip_entries(&res.buffer).unwrap();
    assert!(entries.contains_key("mimetype"));
}

#[test]
fn form_fill_is_byte_lossless_for_unchanged_entries() {
    let extra = b"\x00\x01\x02BINARY-SEAL-IMAGE\xff\xfe";
    let bytes = build_hwpx(&sample_section(), Some(("BinData/seal.bin", extra)));
    let values = values_from_pairs([("성명", "홍길동")]);
    let res = fill_hwpx(&bytes, &values).unwrap();

    let before = read_zip_entries(&bytes).unwrap();
    let after = read_zip_entries(&res.buffer).unwrap();
    for (name, (method, data)) in &before {
        if name.ends_with("section0.xml") {
            continue; // the only entry we intend to change
        }
        let (m2, d2) = after.get(name).expect("entry survives");
        assert_eq!(method, m2, "method preserved for {name}");
        assert_eq!(data, d2, "bytes preserved for {name}");
    }
    // the seal binary is byte-identical
    assert_eq!(after.get("BinData/seal.bin").unwrap().1, extra);
}

#[test]
fn form_patch_hwpx_literal_replace() {
    let section = r#"<?xml version="1.0"?><hp:sec xmlns:hp="x"><hp:p id="0"><hp:run charPrIDRef="0"><hp:t>계약 상대방: 구주식회사</hp:t></hp:run></hp:p></hp:sec>"#;
    let bytes = build_hwpx(section, None);
    let res = patch_hwpx(&bytes, &[("구주식회사".into(), "신주식회사".into())]).unwrap();
    assert_eq!(res.replaced, 1);
    let out = read_section0(&res.buffer);
    assert!(out.contains("신주식회사"));
    assert!(!out.contains("구주식회사"));
}

/// Set the encryption bit (general purpose flag bit 0) on both the local header
/// and the central-directory record of the named entry.
fn set_encrypted_flag(zip: &mut [u8], name: &str) {
    let needle = name.as_bytes();
    let mut occurrences = Vec::new();
    let mut i = 0;
    while i + needle.len() <= zip.len() {
        if &zip[i..i + needle.len()] == needle {
            occurrences.push(i);
            i += needle.len();
        } else {
            i += 1;
        }
    }
    // first occurrence = local header (flags at name-24), second = CD (flags at name-38)
    assert!(occurrences.len() >= 2, "expected local + CD name occurrences");
    zip[occurrences[0] - 24] |= 0x01;
    zip[occurrences[1] - 38] |= 0x01;
}

#[test]
fn form_patch_zip_rejects_encrypted_target_entry() {
    let mut bytes = build_hwpx(&sample_section(), None);
    set_encrypted_flag(&mut bytes, "Contents/section0.xml");

    let mut repl = std::collections::HashMap::new();
    repl.insert("Contents/section0.xml".to_string(), b"<x/>".to_vec());
    let err = patch_zip_entries(&bytes, &repl).unwrap_err();
    assert!(err.contains("암호화"), "clear rejection, got: {err}");

    // an untouched encrypted entry must NOT block patching a different entry
    let mut repl2 = std::collections::HashMap::new();
    repl2.insert("Contents/header.xml".to_string(), b"<hh:head/>".to_vec());
    assert!(patch_zip_entries(&bytes, &repl2).is_ok(), "non-target encrypted entry passes through");
}

#[test]
fn form_realworld_fixture_roundtrips() {
    let path = format!(
        "{}/../tests/realworld/mois/보도_125103_0.hwpx",
        env!("CARGO_MANIFEST_DIR")
    );
    let bytes = match std::fs::read(&path) {
        Ok(b) => b,
        Err(_) => return, // fixture not present in this checkout — skip
    };

    // recognition must not panic and returns a well-formed result
    let fields = extract_form_fields(&bytes).unwrap();
    assert!(fields.confidence >= 0.0 && fields.confidence <= 1.0);

    // literal patch on real content exercises deflate re-pack + ZIP surgery
    let res = patch_hwpx(&bytes, &[("행정안전부".into(), "행정안전부(테스트)".into())]).unwrap();
    // output re-opens as a valid archive
    let after = read_zip_entries(&res.buffer).unwrap();
    let before = read_zip_entries(&bytes).unwrap();
    // every non-section entry is byte-identical
    for (name, (_m, data)) in &before {
        if name.contains("section") && name.ends_with(".xml") {
            continue;
        }
        assert_eq!(&after.get(name).unwrap().1, data, "lossless for {name}");
    }
    if res.replaced > 0 {
        let section = {
            let mut zip = ZipArchive::new(std::io::Cursor::new(res.buffer.clone())).unwrap();
            let mut s = String::new();
            zip.by_name("Contents/section0.xml").unwrap().read_to_string(&mut s).unwrap();
            s
        };
        assert!(section.contains("행정안전부(테스트)"));
    }
}
