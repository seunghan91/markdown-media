//! Tauri bridge over the vendored rhwp crate.
//!
//! This is Phase 2 of the rhwp integration (see commit 2a1bacf for Phase 1).
//! We expose just enough of rhwp's parse/serialize/edit surface to let the
//! desktop frontend load an HWP or HWPX file, enumerate its paragraphs,
//! optionally mutate paragraph text, and write the result back as an HWP.
//!
//! Round-trip stability is the core contract: loading then immediately
//! serializing must produce a byte-stable HWP that rhwp itself can parse
//! again. Edits are layered on top of that guarantee.
//!
//! Design choices
//! --------------
//! 1. **Simple text replacement for v1**: paragraph edits directly mutate
//!    `Paragraph.text` and recompute `char_count` / minimal `char_offsets`
//!    / `char_shapes`. We lose per-character formatting on edited
//!    paragraphs; the next iteration should route through rhwp's
//!    `DocumentCore::insert_text_native` + `delete_text_native` to preserve
//!    runs, but that requires constructing a full DocumentCore and
//!    reading the document back out — rhwp currently makes the latter
//!    `pub(crate)`. A follow-up patch to the vendored crate will expose
//!    an accessor and we'll swap the implementation.
//!
//! 2. **Cell paragraphs ignored for v1**: only top-level section paragraphs
//!    are enumerated and editable. Paragraphs inside table cells or
//!    text-boxes are reachable via rhwp's `insert_text_in_cell_native`
//!    but we'll wire those once the top-level edit path is solid.

use std::path::PathBuf;

use rhwp::{parse_document, serializer::serialize_document};
use serde::{Deserialize, Serialize};

/// A single paragraph returned to the frontend for display/edit.
///
/// Stable identity is `(section, index)` — any edit command must reference
/// the same pair we returned here. Position is 0-based in both dimensions.
#[derive(Serialize, Debug)]
pub struct RhwpParagraph {
    pub section: usize,
    pub index: usize,
    pub text: String,
    /// Original character count (useful for the UI to flag "edited").
    pub char_count: u32,
}

/// An edit the frontend wants applied before serialization.
#[derive(Deserialize, Debug)]
pub struct RhwpParagraphEdit {
    pub section: usize,
    pub index: usize,
    pub new_text: String,
}

/// Summary returned after a round-trip or edit-and-save call.
#[derive(Serialize, Debug)]
pub struct RhwpSaveSummary {
    pub source_bytes: usize,
    pub output_bytes: usize,
    pub paragraphs_edited: usize,
    pub output_path: String,
}

fn read_file(path: &str) -> Result<Vec<u8>, String> {
    std::fs::read(PathBuf::from(path))
        .map_err(|e| format!("원본 파일을 읽지 못했습니다: {} ({})", path, e))
}

/// Parse a HWP / HWPX file and return a flat list of its top-level
/// paragraphs. Opening the list is separate from saving so the frontend
/// can let the user review and batch up edits before committing.
#[tauri::command]
pub async fn rhwp_list_paragraphs(path: String) -> Result<Vec<RhwpParagraph>, String> {
    let data = read_file(&path)?;
    let doc = parse_document(&data)
        .map_err(|e| format!("HWP 파싱 실패: {}", e))?;

    let mut out = Vec::new();
    for (section_idx, section) in doc.sections.iter().enumerate() {
        for (para_idx, para) in section.paragraphs.iter().enumerate() {
            out.push(RhwpParagraph {
                section: section_idx,
                index: para_idx,
                text: para.text.clone(),
                char_count: para.char_count,
            });
        }
    }
    Ok(out)
}

/// Parse, apply `edits` if any, and re-serialize to `target_path`.
///
/// With an empty `edits` vector this is a pure round-trip — useful as a
/// smoke test ("does the vendored rhwp survive parse+serialize on this
/// document?") and as a "Save a copy" action in the frontend.
#[tauri::command]
pub async fn rhwp_save_with_edits(
    source_path: String,
    target_path: String,
    edits: Vec<RhwpParagraphEdit>,
) -> Result<RhwpSaveSummary, String> {
    let data = read_file(&source_path)?;
    let source_bytes = data.len();
    let mut doc = parse_document(&data)
        .map_err(|e| format!("HWP 파싱 실패: {}", e))?;

    let mut applied = 0usize;
    for edit in &edits {
        let Some(section) = doc.sections.get_mut(edit.section) else {
            return Err(format!(
                "구역 {}은(는) 존재하지 않습니다 (총 {}개)",
                edit.section,
                doc.sections.len()
            ));
        };
        let Some(para) = section.paragraphs.get_mut(edit.index) else {
            return Err(format!(
                "구역 {}에 문단 {}이 없습니다 (총 {}개)",
                edit.section,
                edit.index,
                section.paragraphs.len()
            ));
        };

        // Preserve the first char_shape if present so the paragraph keeps
        // its default font/size when text length changes. Offsets collapse
        // to one run covering the whole new text — per-character runs are
        // reconstructed later when rhwp's DocumentCore accessor lands.
        let first_shape = para.char_shapes.first().cloned();
        let new_count = edit.new_text.chars().count() as u32;
        para.text = edit.new_text.clone();
        para.char_count = new_count;
        para.char_offsets = if new_count == 0 { vec![] } else { vec![0] };
        para.char_shapes = match first_shape {
            Some(mut s) => {
                // `start_pos` anchors this char-shape run to the first char
                // of the edited paragraph; there's only one run in v1.
                s.start_pos = 0;
                vec![s]
            }
            None => vec![],
        };
        applied += 1;
    }

    let out = serialize_document(&doc)
        .map_err(|e| format!("HWP 직렬화 실패: {}", e))?;
    std::fs::write(&target_path, &out)
        .map_err(|e| format!("저장 실패: {} ({})", target_path, e))?;

    Ok(RhwpSaveSummary {
        source_bytes,
        output_bytes: out.len(),
        paragraphs_edited: applied,
        output_path: target_path,
    })
}
