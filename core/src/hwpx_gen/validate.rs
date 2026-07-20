// Ported from kkdoc (MIT): src/validate.ts (which ports claw-hwp validate.py, MIT)
//! HWPX container validation — catches defects that make Hangul refuse to open.
//!
//! Checks: valid ZIP; mimetype is the first entry with the right content;
//! required files present; XML/HPF/RDF entries well-formed; header.xml secCnt
//! matches the actual section file count; content.hpf manifest hrefs all exist.

use std::io::{Cursor, Read};

use quick_xml::events::Event;
use quick_xml::Reader;

const REQUIRED_FILES: [&str; 5] = [
    "mimetype",
    "META-INF/container.xml",
    "Contents/content.hpf",
    "Contents/header.xml",
    "Contents/section0.xml",
];
const EXPECTED_MIMETYPE: &str = "application/hwp+zip";
const XML_SUFFIXES: [&str; 3] = [".xml", ".hpf", ".rdf"];

/// One validation problem.
#[derive(Debug, Clone)]
pub struct ValidateIssue {
    /// ZIP-internal path (None for container-wide issues).
    pub path: Option<String>,
    pub message: String,
}

/// Validation result.
#[derive(Debug, Clone)]
pub struct ValidateResult {
    pub ok: bool,
    pub issues: Vec<ValidateIssue>,
    /// Number of non-directory entries examined.
    pub entry_count: usize,
}

fn issue(path: Option<&str>, message: String) -> ValidateIssue {
    ValidateIssue {
        path: path.map(|s| s.to_string()),
        message,
    }
}

/// Validate an HWPX container buffer. `ok == true` when no problems found.
pub fn validate_hwpx(buffer: &[u8]) -> ValidateResult {
    let mut archive = match zip::ZipArchive::new(Cursor::new(buffer)) {
        Ok(a) => a,
        Err(e) => {
            return ValidateResult {
                ok: false,
                issues: vec![issue(None, format!("유효한 ZIP이 아님: {e}"))],
                entry_count: 0,
            }
        }
    };

    let mut issues: Vec<ValidateIssue> = Vec::new();

    // Central-directory order for the first-entry check.
    let mut ordered_names: Vec<String> = Vec::new();
    let mut names: Vec<String> = Vec::new();
    let mut contents: std::collections::HashMap<String, Vec<u8>> = std::collections::HashMap::new();
    for i in 0..archive.len() {
        let mut file = match archive.by_index(i) {
            Ok(f) => f,
            Err(_) => continue,
        };
        let name = file.name().to_string();
        ordered_names.push(name.clone());
        if file.is_dir() {
            continue;
        }
        names.push(name.clone());
        let mut buf = Vec::new();
        // bounded read to avoid pathological entries
        let mut limited = file.by_ref().take(64 * 1024 * 1024);
        let _ = limited.read_to_end(&mut buf);
        contents.insert(name, buf);
    }

    if names.is_empty() {
        return ValidateResult {
            ok: false,
            issues: vec![issue(None, "빈 ZIP".to_string())],
            entry_count: 0,
        };
    }

    if ordered_names.first().map(|s| s.as_str()) != Some("mimetype") {
        issues.push(issue(
            None,
            format!(
                "첫 zip 엔트리가 '{}' — 'mimetype'이어야 함",
                ordered_names.first().cloned().unwrap_or_default()
            ),
        ));
    }

    let nameset: std::collections::HashSet<&String> = names.iter().collect();

    if let Some(data) = contents.get("mimetype") {
        let mt = String::from_utf8_lossy(data).trim().to_string();
        if mt != EXPECTED_MIMETYPE {
            issues.push(issue(
                Some("mimetype"),
                format!("내용이 '{mt}' — '{EXPECTED_MIMETYPE}'이어야 함"),
            ));
        }
    }

    for req in REQUIRED_FILES {
        if !names.iter().any(|n| n == req) {
            issues.push(issue(None, format!("필수 파일 누락: {req}")));
        }
    }

    // XML well-formedness (errors only).
    for name in &names {
        if !XML_SUFFIXES.iter().any(|s| name.ends_with(s)) {
            continue;
        }
        if let Some(data) = contents.get(name) {
            let text = String::from_utf8_lossy(data);
            if let Err(e) = check_well_formed(&text) {
                issues.push(issue(Some(name), format!("XML 웰폼드 위반: {e}")));
            }
        }
    }

    // secCnt ↔ actual section count.
    if let Some(data) = contents.get("Contents/header.xml") {
        let header = String::from_utf8_lossy(data);
        if let Some(declared) = extract_sec_cnt(&header) {
            let actual = names
                .iter()
                .filter(|n| is_section_file(n))
                .count();
            if declared != actual {
                issues.push(issue(
                    Some("Contents/header.xml"),
                    format!("secCnt={declared}인데 실제 sectionN.xml은 {actual}개 — 한컴독스가 열기를 거부함"),
                ));
            }
        }
    }

    // manifest hrefs exist.
    if let Some(data) = contents.get("Contents/content.hpf") {
        let hpf = String::from_utf8_lossy(data);
        for href in extract_hrefs(&hpf) {
            let full = format!("Contents/{href}");
            if !nameset.contains(&href) && !nameset.contains(&full) {
                issues.push(issue(
                    Some("Contents/content.hpf"),
                    format!("manifest가 없는 파일을 참조: {href}"),
                ));
            }
        }
    }

    ValidateResult {
        ok: issues.is_empty(),
        issues,
        entry_count: names.len(),
    }
}

fn check_well_formed(text: &str) -> Result<(), String> {
    // quick-xml 0.31: check_end_names (mismatched close tags) is on by default.
    let mut reader = Reader::from_str(text);
    let mut buf_depth = 0i64;
    loop {
        match reader.read_event() {
            Ok(Event::Eof) => break,
            Ok(Event::Start(_)) => buf_depth += 1,
            Ok(Event::End(_)) => buf_depth -= 1,
            Ok(_) => {}
            Err(e) => return Err(e.to_string().lines().next().unwrap_or("").to_string()),
        }
    }
    let _ = buf_depth;
    Ok(())
}

fn extract_sec_cnt(header: &str) -> Option<usize> {
    // find `head` element opening then secCnt="N"
    let idx = header.find(":head")?;
    let slice = &header[idx..];
    let key = "secCnt=\"";
    let pos = slice.find(key)? + key.len();
    let num: String = slice[pos..].chars().take_while(|c| c.is_ascii_digit()).collect();
    num.parse().ok()
}

fn is_section_file(name: &str) -> bool {
    if let Some(rest) = name.strip_prefix("Contents/section") {
        if let Some(digits) = rest.strip_suffix(".xml") {
            return !digits.is_empty() && digits.chars().all(|c| c.is_ascii_digit());
        }
    }
    false
}

fn extract_hrefs(hpf: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = hpf;
    while let Some(pos) = rest.find("<opf:item") {
        rest = &rest[pos..];
        if let Some(end) = rest.find('>') {
            let tag = &rest[..end];
            if let Some(hpos) = tag.find("href=\"") {
                let after = &tag[hpos + 6..];
                if let Some(q) = after.find('"') {
                    out.push(after[..q].to_string());
                }
            }
            rest = &rest[end..];
        } else {
            break;
        }
    }
    out
}
