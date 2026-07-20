// Ported from kkdoc (MIT): src/form/{recognize,match,filler-hwpx}.ts + src/roundtrip/{source-map,zip-patch}.ts
//! Form (서식) recognition and format-preserving filling for HWPX documents.
//!
//! - [`extract_form_fields`] — recognize label/value form fields from a HWPX file.
//! - [`extract_form_schema`] — the same, enriched with inferred type/required/empty.
//! - [`form_schema_json`] — the schema as a JSON string.
//! - [`fill_hwpx`] / [`fill_form`] — write values into an HWPX form, preserving all
//!   formatting (only `<hp:t>` text bytes change; unchanged ZIP entries stay identical).
//! - [`patch_hwpx`] — lossless literal text replacement (roundtrip patch).

mod fill;
mod matcher;
mod recognize;
mod scan;
mod seal;
mod zip_patch;

pub use fill::{fill_hwpx, patch_hwpx, FillResult, PatchResult};
pub use matcher::{format_fill_value, normalize_label, FillValue, RawFillInput};
pub use recognize::{infer_field_type, is_label_cell, FormFieldType};
// Source map — the section-XML byte-position map plus precise-edit primitives
// (byte-range splicing that preserves formatting). Reusable outside form filling.
pub use scan::{
    apply_splices, build_range_splices, para_t_text, scan_section, Cell, Para, Scan, SpliceEdit,
    Table, TextRun,
};
pub use seal::{place_seal_hwpx, SealAnchor, SealOptions};
pub use zip_patch::{patch_zip_entries, read_zip_entries};

use std::collections::HashMap;
use std::io::{Cursor, Read};

use lazy_static::lazy_static;
use regex::Regex;
use zip::ZipArchive;

use matcher::normalize_label as norm;
use recognize::{infer_field_type as infer, is_empty_value, is_required_label};

lazy_static! {
    static ref SECTION_RE: Regex = Regex::new(r"(?i)section\d+\.xml$").unwrap();
    static ref INLINE_FIELD_RE: Regex =
        Regex::new(r"^\s*([가-힣A-Za-z][가-힣A-Za-z0-9()\s]{0,20}?)\s*[:：]\s*(.*)$").unwrap();
}

/// A recognized form field (label/value pair with grid position; row/col = -1 for inline).
#[derive(Debug, Clone, serde::Serialize)]
pub struct FormField {
    pub label: String,
    pub value: String,
    pub row: i32,
    pub col: i32,
}

/// Result of [`extract_form_fields`].
#[derive(Debug, Clone, serde::Serialize)]
pub struct FormResult {
    pub fields: Vec<FormField>,
    pub confidence: f64,
}

/// A form field enriched with inferred schema info.
#[derive(Debug, Clone, serde::Serialize)]
pub struct FormFieldSchema {
    pub label: String,
    pub value: String,
    pub row: i32,
    pub col: i32,
    #[serde(rename = "type")]
    pub field_type: FormFieldType,
    #[serde(skip_serializing_if = "is_false")]
    pub required: bool,
    pub empty: bool,
}

fn is_false(b: &bool) -> bool {
    !*b
}

/// Result of [`extract_form_schema`].
#[derive(Debug, Clone, serde::Serialize)]
pub struct FormSchemaResult {
    pub fields: Vec<FormFieldSchema>,
    pub confidence: f64,
}

/// `fill_form` is an alias for [`fill_hwpx`] (HWPX is the only supported form target).
pub use fill::fill_hwpx as fill_form;

fn read_section_xmls(bytes: &[u8]) -> Result<Vec<String>, String> {
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
        out.push(s);
    }
    Ok(out)
}

fn extract_from_table(table: &Table) -> Vec<FormField> {
    let rows = table.rows();
    let mut fields = Vec::new();

    // strategy 1: adjacent label-value cells
    for (ri, row) in rows.iter().enumerate() {
        if row.len() < 2 {
            continue;
        }
        for ci in 0..row.len() - 1 {
            let label = row[ci].text();
            if is_label_cell(label.trim()) {
                fields.push(FormField {
                    label: label.trim().trim_end_matches([':', '：']).trim().to_string(),
                    value: row[ci + 1].text().trim().to_string(),
                    row: ri as i32,
                    col: ci as i32,
                });
            }
        }
    }

    // strategy 2: header + data rows (only if strategy 1 found nothing)
    if fields.is_empty() && rows.len() >= 2 {
        let header = &rows[0];
        let all_labels = !header.is_empty()
            && header.iter().all(|c| {
                let t = c.text();
                let tt = t.trim();
                !tt.is_empty() && tt.chars().count() <= 20
            });
        if all_labels {
            for ri in 1..rows.len() {
                let data = &rows[ri];
                for ci in 0..header.len().min(data.len()) {
                    let label = header[ci].text().trim().to_string();
                    let value = data[ci].text().trim().to_string();
                    if !label.is_empty() && !value.is_empty() {
                        fields.push(FormField { label, value, row: ri as i32, col: ci as i32 });
                    }
                }
            }
        }
    }

    fields
}

/// Recognize form fields from an HWPX file.
pub fn extract_form_fields(bytes: &[u8]) -> Result<FormResult, String> {
    let sections = read_section_xmls(bytes)?;
    let mut fields = Vec::new();
    let mut total_tables = 0usize;
    let mut form_tables = 0usize;

    for xml in &sections {
        let scan = scan_section(xml);
        for table in &scan.tables {
            total_tables += 1;
            let tf = extract_from_table(table);
            if !tf.is_empty() {
                form_tables += 1;
                fields.extend(tf);
            }
        }
        // inline "라벨: 값" (value present)
        for para in &scan.body_paras {
            if let Some(caps) = INLINE_FIELD_RE.captures(&para.text) {
                let value = caps.get(2).map(|m| m.as_str().trim()).unwrap_or("");
                if !value.is_empty() {
                    let label = caps.get(1).unwrap().as_str().trim();
                    fields.push(FormField {
                        label: label.to_string(),
                        value: value.to_string(),
                        row: -1,
                        col: -1,
                    });
                }
            }
        }
    }

    let confidence = if total_tables > 0 {
        (form_tables as f64 / total_tables as f64).min(1.0)
    } else if !fields.is_empty() {
        0.3
    } else {
        0.0
    };
    Ok(FormResult { fields, confidence })
}

/// Recognize form fields enriched with inferred type / required / empty flags.
pub fn extract_form_schema(bytes: &[u8]) -> Result<FormSchemaResult, String> {
    let base = extract_form_fields(bytes)?;
    let mut fields: Vec<FormFieldSchema> = base
        .fields
        .iter()
        .map(|f| FormFieldSchema {
            label: f.label.clone(),
            value: f.value.clone(),
            row: f.row,
            col: f.col,
            field_type: infer(&f.label, &f.value),
            required: is_required_label(&f.label),
            empty: is_empty_value(&f.value),
        })
        .collect();

    // surface empty inline labels ("작성일자:") as fill targets too
    let mut seen: std::collections::HashSet<String> = fields.iter().map(|f| norm(&f.label)).collect();
    let sections = read_section_xmls(bytes)?;
    for xml in &sections {
        let scan = scan_section(xml);
        for para in &scan.body_paras {
            if let Some(caps) = INLINE_FIELD_RE.captures(&para.text) {
                let value = caps.get(2).map(|m| m.as_str().trim()).unwrap_or("");
                if !value.is_empty() {
                    continue;
                }
                let label = caps.get(1).unwrap().as_str().trim();
                let key = norm(label);
                if key.is_empty() || seen.contains(&key) {
                    continue;
                }
                seen.insert(key);
                fields.push(FormFieldSchema {
                    label: label.to_string(),
                    value: String::new(),
                    row: -1,
                    col: -1,
                    field_type: infer(label, ""),
                    required: is_required_label(label),
                    empty: true,
                });
            }
        }
    }

    Ok(FormSchemaResult { fields, confidence: base.confidence })
}

/// The schema as a pretty JSON string.
pub fn form_schema_json(bytes: &[u8]) -> Result<String, String> {
    let schema = extract_form_schema(bytes)?;
    serde_json::to_string_pretty(&schema).map_err(|e| e.to_string())
}

/// Convenience: build the `values` map for [`fill_hwpx`] from string pairs.
pub fn values_from_pairs<I, K, V>(pairs: I) -> HashMap<String, RawFillInput>
where
    I: IntoIterator<Item = (K, V)>,
    K: Into<String>,
    V: Into<String>,
{
    pairs
        .into_iter()
        .map(|(k, v)| (k.into(), RawFillInput { value: FillValue::Scalar(v.into()), format: None }))
        .collect()
}

#[cfg(test)]
mod tests;
