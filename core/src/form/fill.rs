// Ported from kkdoc (MIT): src/form/filler-hwpx.ts (byte-preserving splice fill)
//! HWPX form filling with 100% formatting preservation — locate value cells,
//! splice only the `<hp:t>` text bytes, re-pack the ZIP touching only changed
//! sections (all other entries stay byte-identical).

use std::collections::{HashMap, HashSet};
use std::io::{Cursor, Read};

use lazy_static::lazy_static;
use regex::Regex;
use zip::ZipArchive;

use super::matcher::{
    find_matching_key, normalize_label, normalize_values, resolve_unmatched, is_keyword_label,
    RawFillInput, ValueCursor,
};
use super::recognize::is_label_cell;
use super::scan::{scan_section, xml_escape_text, Para, Scan, TextRun};
use super::zip_patch::patch_zip_entries;
use super::{FormField};

lazy_static! {
    static ref SECTION_RE: Regex = Regex::new(r"(?i)section\d+\.xml$").unwrap();
    static ref INLINE_SINGLE_RE: Regex =
        Regex::new(r"^\s*([가-힣A-Za-z][가-힣A-Za-z0-9()\s]{0,20}?)\s*[:：]\s*(.*)$").unwrap();
}

/// Result of [`fill_hwpx`].
#[derive(Debug, Clone)]
pub struct FillResult {
    pub buffer: Vec<u8>,
    pub filled: Vec<FormField>,
    pub unmatched: Vec<String>,
    pub warnings: Vec<String>,
}

struct Splice {
    start: usize,
    end: usize,
    rep: String,
}

fn apply_splices(xml: &str, mut splices: Vec<Splice>) -> String {
    splices.sort_by(|a, b| a.start.cmp(&b.start).then(a.end.cmp(&b.end)));
    // drop overlaps — earlier (already-pushed) wins
    let mut merged: Vec<Splice> = Vec::new();
    for s in splices {
        if let Some(prev) = merged.last() {
            if s.start < prev.end {
                continue;
            }
        }
        merged.push(s);
    }
    let mut out = xml.to_string();
    for s in merged.into_iter().rev() {
        out.replace_range(s.start..s.end, &s.rep);
    }
    out
}

fn lineseg_removal_splices(xml: &str) -> Vec<Splice> {
    let mut v = Vec::new();
    let mut pos = 0;
    while let Some(i) = xml[pos..].find("<hp:linesegarray") {
        let s = pos + i;
        match xml[s..].find("</hp:linesegarray>") {
            Some(j) => {
                let e = s + j + "</hp:linesegarray>".len();
                v.push(Splice { start: s, end: e, rep: String::new() });
                pos = e;
            }
            None => break,
        }
    }
    v
}

/// Fill the value into the first run of `paras`, emptying every other run.
/// If no `<hp:t>` run exists, inject one after the first `<hp:run>` open tag.
/// Returns false if the cell has no place to write.
fn fill_paras_full(paras: &[Para], value: &str, splices: &mut Vec<Splice>) -> bool {
    let mut first: Option<&TextRun> = None;
    for p in paras {
        if let Some(r) = p.runs.first() {
            first = Some(r);
            break;
        }
    }
    if let Some(fr) = first {
        splices.push(Splice { start: fr.start, end: fr.end, rep: xml_escape_text(value) });
        for p in paras {
            for r in &p.runs {
                if r.start == fr.start && r.end == fr.end {
                    continue;
                }
                splices.push(Splice { start: r.start, end: r.end, rep: String::new() });
            }
        }
        return true;
    }
    for p in paras {
        if let Some(pos) = p.first_run_open_end {
            splices.push(Splice {
                start: pos,
                end: pos,
                rep: format!("<hp:t>{}</hp:t>", xml_escape_text(value)),
            });
            return true;
        }
    }
    false
}

fn clean_label(s: &str) -> String {
    s.trim().trim_end_matches([':', '：']).trim().to_string()
}

/// Run table + inline fill strategies over one section, collecting splices.
fn fill_section(
    scan: &Scan,
    cursor: &mut ValueCursor,
    matched: &mut HashSet<String>,
    filled: &mut Vec<FormField>,
) -> Vec<Splice> {
    let mut splices: Vec<Splice> = Vec::new();

    for table in &scan.tables {
        let rows = table.rows();

        // header-row skip guard (avoid "품명"→"규격" neighbor contamination)
        let skip_header = rows.len() >= 2 && {
            let first = &rows[0];
            let all_labels = !first.is_empty()
                && first.iter().all(|c| {
                    let t = c.text();
                    let tt = t.trim();
                    !tt.is_empty() && tt.chars().count() <= 20 && is_label_cell(tt)
                });
            all_labels && rows[1].first().is_some_and(|d0| !is_label_cell(d0.text().trim()))
        };

        // strategy 1: adjacent label-value cells
        for (ri, row) in rows.iter().enumerate() {
            if skip_header && ri == 0 {
                continue;
            }
            if row.len() < 2 {
                continue;
            }
            for ci in 0..row.len() - 1 {
                let label_text = row[ci].text();
                if !is_label_cell(label_text.trim()) {
                    continue;
                }
                let value_cell = row[ci + 1];
                if is_keyword_label(&value_cell.text()) {
                    continue;
                }
                let nlabel = normalize_label(&label_text);
                if nlabel.is_empty() {
                    continue;
                }
                let key = match find_matching_key(&nlabel, cursor) {
                    Some(k) => k,
                    None => continue,
                };
                let newval = match cursor.consume(&key) {
                    Some(v) => v,
                    None => continue,
                };
                if fill_paras_full(&value_cell.paras, &newval, &mut splices) {
                    matched.insert(key.clone());
                    filled.push(FormField {
                        label: clean_label(&label_text),
                        value: newval,
                        row: ri as i32,
                        col: ci as i32,
                    });
                }
            }
        }

        // strategy 2: header + data rows
        if rows.len() >= 2 {
            let header = &rows[0];
            let all_labels = !header.is_empty()
                && header.iter().all(|c| {
                    let t = c.text();
                    let tt = t.trim();
                    !tt.is_empty() && tt.chars().count() <= 20 && is_label_cell(tt)
                });
            if all_labels {
                for (ri, data) in rows.iter().enumerate().skip(1) {
                    for ci in 0..header.len().min(data.len()) {
                        let hl = normalize_label(&header[ci].text());
                        let key = match find_matching_key(&hl, cursor) {
                            Some(k) => k,
                            None => continue,
                        };
                        if !cursor.is_array(&key) && matched.contains(&key) {
                            continue;
                        }
                        let newval = match cursor.consume(&key) {
                            Some(v) => v,
                            None => continue,
                        };
                        if fill_paras_full(&data[ci].paras, &newval, &mut splices) {
                            matched.insert(key.clone());
                            filled.push(FormField {
                                label: header[ci].text().trim().to_string(),
                                value: newval,
                                row: ri as i32,
                                col: ci as i32,
                            });
                        }
                    }
                }
            }
        }
    }

    // strategy 3: inline single-label body paragraphs ("성명: 값" / "성명:")
    for para in &scan.body_paras {
        if para.runs.is_empty() {
            continue;
        }
        let caps = match INLINE_SINGLE_RE.captures(&para.text) {
            Some(c) => c,
            None => continue,
        };
        let label = caps.get(1).unwrap().as_str().trim();
        let nlabel = normalize_label(label);
        if nlabel.is_empty() {
            continue;
        }
        let key = match find_matching_key(&nlabel, cursor) {
            Some(k) => k,
            None => continue,
        };
        let newval = match cursor.consume(&key) {
            Some(v) => v,
            None => continue,
        };
        let text = &para.text;
        let new_text = match text.find(':').or_else(|| text.find('：')) {
            Some(cb) => {
                let colon = if text[cb..].starts_with('：') { '：' } else { ':' };
                let after = cb + colon.len_utf8();
                format!("{} {}", &text[..after], newval)
            }
            None => format!("{} {}", label, newval),
        };
        if fill_paras_full(std::slice::from_ref(para), &new_text, &mut splices) {
            matched.insert(key.clone());
            filled.push(FormField { label: label.to_string(), value: newval, row: -1, col: -1 });
        }
    }

    splices
}

/// Enumerate `Contents/section*.xml` entries (sorted), decoded to strings.
fn read_sections(bytes: &[u8]) -> Result<Vec<(String, String)>, String> {
    let mut zip = ZipArchive::new(Cursor::new(bytes.to_vec())).map_err(|e| e.to_string())?;
    let mut names: Vec<String> = (0..zip.len())
        .filter_map(|i| zip.by_index(i).ok().map(|f| f.name().to_string()))
        .filter(|n| SECTION_RE.is_match(n))
        .collect();
    names.sort();
    let mut out = Vec::new();
    for name in names {
        let mut s = String::new();
        zip.by_name(&name).map_err(|e| e.to_string())?.read_to_string(&mut s).map_err(|e| e.to_string())?;
        out.push((name, s));
    }
    Ok(out)
}

/// Fill an HWPX form, preserving formatting. `values` maps labels → values.
pub fn fill_hwpx(
    bytes: &[u8],
    values: &HashMap<String, RawFillInput>,
) -> Result<FillResult, String> {
    let mut warnings = Vec::new();
    let normalized = normalize_values(values, &mut warnings);
    let mut cursor = ValueCursor::new(normalized.clone());
    let mut matched: HashSet<String> = HashSet::new();
    let mut filled: Vec<FormField> = Vec::new();
    let mut replacements: HashMap<String, Vec<u8>> = HashMap::new();

    let sections = read_sections(bytes)?;
    if sections.is_empty() {
        return Err("HWPX에서 섹션 파일을 찾을 수 없습니다".into());
    }
    for (name, xml) in &sections {
        let scan = scan_section(xml);
        let mut splices = fill_section(&scan, &mut cursor, &mut matched, &mut filled);
        if !splices.is_empty() {
            splices.extend(lineseg_removal_splices(xml));
            replacements.insert(name.clone(), apply_splices(xml, splices).into_bytes());
        }
    }

    let unmatched = resolve_unmatched(&normalized, &matched, values);
    let buffer = if replacements.is_empty() {
        bytes.to_vec()
    } else {
        patch_zip_entries(bytes, &replacements)?
    };
    Ok(FillResult { buffer, filled, unmatched, warnings })
}

/// Result of [`patch_hwpx`].
#[derive(Debug, Clone)]
pub struct PatchResult {
    pub buffer: Vec<u8>,
    pub replaced: usize,
}

/// Lossless literal text patch — replace `find` → `replace` in every text run,
/// preserving all other bytes. Single-run matches only (a `find` string that
/// straddles run boundaries or lands in an entity-carrying run is skipped).
pub fn patch_hwpx(bytes: &[u8], replacements: &[(String, String)]) -> Result<PatchResult, String> {
    let sections = read_sections(bytes)?;
    let mut out_entries: HashMap<String, Vec<u8>> = HashMap::new();
    let mut replaced = 0usize;

    for (name, xml) in &sections {
        let scan = scan_section(xml);
        let mut splices: Vec<Splice> = Vec::new();
        // gather every run: body paragraphs + all table cell paragraphs
        let mut all_paras: Vec<&Para> = scan.body_paras.iter().collect();
        for table in &scan.tables {
            for cell in &table.cells {
                for p in &cell.paras {
                    all_paras.push(p);
                }
            }
        }
        for para in all_paras {
            for run in &para.runs {
                let raw = &xml[run.start..run.end];
                if raw.contains('&') {
                    continue; // entity-carrying run: decoded != raw, skip for safety
                }
                let mut new = raw.to_string();
                let mut hit = false;
                for (find, rep) in replacements {
                    if !find.is_empty() && new.contains(find.as_str()) {
                        new = new.replace(find.as_str(), &xml_escape_text(rep));
                        hit = true;
                    }
                }
                if hit {
                    replaced += 1;
                    splices.push(Splice { start: run.start, end: run.end, rep: new });
                }
            }
        }
        if !splices.is_empty() {
            splices.extend(lineseg_removal_splices(xml));
            out_entries.insert(name.clone(), apply_splices(xml, splices).into_bytes());
        }
    }

    let buffer = if out_entries.is_empty() {
        bytes.to_vec()
    } else {
        patch_zip_entries(bytes, &out_entries)?
    };
    Ok(PatchResult { buffer, replaced })
}
