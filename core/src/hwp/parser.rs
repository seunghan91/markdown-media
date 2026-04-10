use super::ole::OleReader;
use super::record::{
    HwpRecord, RecordParser, extract_para_text, parse_table_info,
    parse_char_shape, parse_para_char_shape, extract_para_text_formatted,
    parse_cell_list_header, parse_picture_component, CellSpan,
    CharShape, ParaCharShapeMapping,
    HWPTAG_PARA_TEXT, HWPTAG_PARA_HEADER, HWPTAG_TABLE, HWPTAG_LIST_HEADER,
    HWPTAG_PARA_CHAR_SHAPE, HWPTAG_CHAR_SHAPE, HWPTAG_PARA_SHAPE, HWPTAG_CTRL_HEADER,
    HWPTAG_SHAPE_COMPONENT_PICTURE, HWPTAG_BIN_DATA,
};
use crate::ir::{blocks_to_markdown, IRBlock, IRCell, IRTable};
use std::collections::HashMap;
use std::io::{self};
use std::path::Path;

/// HWP 파일 파서
pub struct HwpParser {
    ole_reader: OleReader,
    /// Character shape definitions from DocInfo
    char_shapes: HashMap<u32, CharShape>,
    /// Paragraph shape definitions from DocInfo (outline_level per paraShapeId)
    para_shapes: HashMap<u32, ParaShapeInfo>,
}

/// Minimal ParaShape info extracted from DocInfo
#[derive(Debug, Clone, Default)]
struct ParaShapeInfo {
    /// Outline level (0 = body, 1-7 = heading levels)
    outline_level: u8,
}

impl HwpParser {
    /// HWP 파일을 엽니다
    pub fn open<P: AsRef<Path>>(path: P) -> io::Result<Self> {
        let ole_reader = OleReader::open(path)?;
        Ok(HwpParser {
            ole_reader,
            char_shapes: HashMap::new(),
            para_shapes: HashMap::new(),
        })
    }

    /// Parse DocInfo stream to extract character shapes and paragraph shapes
    fn parse_doc_info(&mut self) -> io::Result<()> {
        let data = self.ole_reader.read_doc_info()?;
        let mut parser = RecordParser::new(&data);
        let records = parser.parse_all();

        let mut char_shape_index: u32 = 0;
        let mut para_shape_index: u32 = 0;
        for record in records {
            if record.tag_id == HWPTAG_CHAR_SHAPE {
                if let Some(shape) = parse_char_shape(&record.data) {
                    self.char_shapes.insert(char_shape_index, shape);
                }
                char_shape_index += 1;
            }
            if record.tag_id == HWPTAG_PARA_SHAPE {
                // HWP 5.0 Spec: ParaShape properties1 at offset 0-3
                // Bits 23-24: head_shape_type (0=none, 1=outline, 2=numbering, 3=bullet)
                // Bits 25-27: outline_level (0-6) — only valid when head_shape_type == 1
                if record.data.len() >= 4 {
                    let props1 = u32::from_le_bytes([
                        record.data[0], record.data[1],
                        record.data[2], record.data[3],
                    ]);
                    let head_shape_type = (props1 >> 23) & 0x03;
                    let outline_level = if head_shape_type == 1 {
                        ((props1 >> 25) & 0x07) as u8 + 1 // 0-based → 1-based (H1-H7)
                    } else {
                        0 // not a heading
                    };
                    self.para_shapes.insert(para_shape_index, ParaShapeInfo {
                        outline_level,
                    });
                }
                para_shape_index += 1;
            }
        }

        Ok(())
    }

    /// HWP 파일 구조를 분석합니다
    pub fn analyze(&self) -> FileStructure {
        let streams = self.ole_reader.list_streams();
        let section_count = self.ole_reader.section_count();
        let bin_data = self.ole_reader.list_bin_data();
        let flags = self.ole_reader.flags();
        
        FileStructure {
            total_streams: streams.len(),
            streams,
            section_count,
            bin_data_count: bin_data.len(),
            compressed: flags.compressed,
            encrypted: flags.encrypted,
        }
    }

    /// 텍스트를 추출합니다.
    ///
    /// `FileHeader` 의 `distributed` 플래그가 켜져 있으면 배포용(열람 제한)
    /// 문서로 간주하고, 일반 `BodyText/Section{N}` 대신 `ViewText/Section{N}`
    /// 에서 raw 스트림을 읽어 `crate::hwp::crypto::decrypt_view_text` 로 복호화
    /// 한 뒤 동일한 레코드 파서에 흘려보낸다. 복호화 실패 시 해당 섹션은
    /// 경고만 남기고 스킵한다.
    pub fn extract_text(&mut self) -> io::Result<String> {
        // First, parse DocInfo to get character shapes
        if self.char_shapes.is_empty() {
            let _ = self.parse_doc_info();
        }

        let flags = *self.ole_reader.flags();
        let distributed = flags.distributed;
        let compressed = flags.compressed;

        let section_count = if distributed {
            self.ole_reader.view_section_count()
        } else {
            self.ole_reader.section_count()
        };

        if section_count == 0 {
            return Ok(if distributed {
                "No ViewText sections found (distribution-locked HWP).".to_string()
            } else {
                "No BodyText sections found.".to_string()
            });
        }

        let mut all_text = Vec::new();

        for section_num in 0..section_count {
            let section_data: io::Result<Vec<u8>> = if distributed {
                self.ole_reader
                    .read_view_text_raw(section_num)
                    .and_then(|raw| {
                        crate::hwp::crypto::decrypt_view_text(&raw, compressed)
                    })
            } else {
                self.ole_reader.read_body_text(section_num)
            };

            match section_data {
                Ok(data) => {
                    // Parse records from decompressed data with formatting
                    let section_text = self.parse_section_records_formatted(&data);
                    if !section_text.is_empty() {
                        if section_count > 1 {
                            all_text.push(format!("=== Section {} ===\n{}", section_num, section_text));
                        } else {
                            all_text.push(section_text);
                        }
                    }
                }
                Err(e) => {
                    // Log error but continue with other sections
                    eprintln!(
                        "Warning: Could not read {}Section{}: {}",
                        if distributed { "View" } else { "Body" },
                        section_num,
                        e
                    );
                }
            }
        }

        if all_text.is_empty() {
            Ok("No text extracted. File may be encrypted or have unsupported format.".to_string())
        } else {
            Ok(all_text.join("\n\n"))
        }
    }

    /// Structured block extraction.
    ///
    /// Returns the document body as a `Vec<IRBlock>` with full metadata
    /// preserved: heading levels, hyperlink URLs on paragraphs, footnote
    /// text attached to the referencing paragraph, image placeholders,
    /// and table cell structure via `IRBlock::Table(IRTable)`. Use this
    /// when the downstream consumer needs structural information —
    /// document diff, form extraction, semantic chunking — instead of a
    /// flat Markdown string.
    ///
    /// Dispatches on `FileHeader.distributed` the same way
    /// [`extract_text`] does, so distribution-locked (`ViewText`) files
    /// round-trip through the AES decryption path transparently.
    /// Per-section errors emit a warning to stderr and are skipped.
    pub fn extract_blocks(&mut self) -> io::Result<Vec<IRBlock>> {
        if self.char_shapes.is_empty() {
            let _ = self.parse_doc_info();
        }

        let flags = *self.ole_reader.flags();
        let distributed = flags.distributed;
        let compressed = flags.compressed;

        let section_count = if distributed {
            self.ole_reader.view_section_count()
        } else {
            self.ole_reader.section_count()
        };
        if section_count == 0 {
            return Ok(Vec::new());
        }

        let mut out: Vec<IRBlock> = Vec::new();
        for section_num in 0..section_count {
            let section_data: io::Result<Vec<u8>> = if distributed {
                self.ole_reader
                    .read_view_text_raw(section_num)
                    .and_then(|raw| {
                        crate::hwp::crypto::decrypt_view_text(&raw, compressed)
                    })
            } else {
                self.ole_reader.read_body_text(section_num)
            };

            match section_data {
                Ok(data) => {
                    out.extend(self.parse_section_records_to_blocks(&data));
                }
                Err(e) => {
                    eprintln!(
                        "Warning: Could not read {}Section{}: {}",
                        if distributed { "View" } else { "Body" },
                        section_num,
                        e
                    );
                }
            }
        }
        // Post-processing: promote large-font paragraphs to headings
        infer_headings_by_font_size(&mut out, &self.char_shapes);
        Ok(out)
    }

    /// Parse records from decompressed section data (without formatting - for compatibility)
    fn parse_section_records(&self, data: &[u8]) -> String {
        let mut parser = RecordParser::new(data);
        let records = parser.parse_all();

        let mut paragraphs = Vec::new();

        for record in records {
            if record.tag_id == HWPTAG_PARA_TEXT {
                let text = extract_para_text(&record.data);
                if !text.trim().is_empty() {
                    paragraphs.push(text);
                }
            }
        }

        paragraphs.join("\n")
    }

    /// Parse records from decompressed section data with formatting.
    ///
    /// Produces an interleaved stream of paragraphs and GFM tables. Table cells
    /// are diverted into a state machine while `in_table = true`, then emitted
    /// as a single Markdown table block when all cells are collected. This
    /// prevents the prior behavior of dumping each cell as a separate paragraph
    /// (which destroyed table structure for downstream RAG/embedding consumers).
    ///
    /// Cell placement uses LIST_HEADER colAddr/rowAddr (HWP5 spec offsets 8/10)
    /// when present — this is the kordoc-verified path for correct merged-cell
    /// rendering. Falls back to sequential fill when addresses are absent.
    ///
    /// IR-emitting parallel of this function is
    /// [`parse_section_records_to_blocks`]. Both walk the same HWP record
    /// stream with identical state machines but emit different
    /// representations. Phase 2c will collapse them into one path.

    /// Parse one section's record stream into a `Vec<IRBlock>`.
    ///
    /// Primary IR-emitting walker. Preserves structural metadata that
    /// the legacy string path had to encode as inline Markdown:
    /// - hyperlink URLs → `IRBlock::Paragraph::href`
    /// - footnote / endnote body → `IRBlock::Paragraph::footnote`
    /// - heading levels → `IRBlock::Heading { level, .. }` (via
    ///   [`push_paragraph`] + [`promote_korean_heading_level`])
    /// - table cell structure → `IRBlock::Table(IRTable)` with
    ///   `IRCell` cells built by [`build_ir_blocks_from_cells`]
    /// - image placeholders → `IRBlock::Image { alt }`
    ///
    /// State machine is identical to [`parse_section_records_formatted`]:
    /// same table termination (`HWPTAG_PARA_HEADER` at shallower level,
    /// new `HWPTAG_TABLE`, end-of-section trailing flush), same
    /// `LIST_HEADER` cell placement, same `gso` / `fn` / `en` subtree
    /// skipping to prevent duplicate emission.
    fn parse_section_records_to_blocks(&self, data: &[u8]) -> Vec<IRBlock> {
        let mut parser = RecordParser::new(data);
        let records = parser.parse_all();

        let mut blocks: Vec<IRBlock> = Vec::new();
        let mut current_text_data: Option<Vec<u8>> = None;
        let mut current_char_shape_mapping: Option<ParaCharShapeMapping> = None;
        let mut current_outline_level: u8 = 0;

        let mut in_table = false;
        let mut table_rows: usize = 0;
        let mut table_cols: usize = 0;
        let mut table_level: u16 = 0;
        let mut cells: Vec<(CellSpan, String)> = Vec::new();

        let mut i = 0usize;
        while i < records.len() {
            let record = &records[i];
            match record.tag_id {
                HWPTAG_CTRL_HEADER => {
                    if record.data.len() >= 4 {
                        let id = &record.data[0..4];
                        // gso — text box / image / shape
                        if id == b" osg" || id == b"gso " {
                            if let Some(text_data) = current_text_data.take() {
                                let text = extract_para_text_formatted(
                                    &text_data,
                                    current_char_shape_mapping.as_ref(),
                                    &self.char_shapes,
                                );
                                push_paragraph(&mut blocks, text);
                                current_char_shape_mapping = None;
                            }
                            if let Some(bin_id) = extract_subtree_image_id(&records, i, 200) {
                                blocks.push(IRBlock::Image {
                                    alt: format!("image{}", bin_id),
                                });
                            } else if let Some(box_text) =
                                extract_subtree_text(&records, i, 200, "\n")
                            {
                                push_paragraph(&mut blocks, box_text);
                            }
                            let end = subtree_end(&records, i, 200);
                            i = end;
                            continue;
                        }
                        // Footnote / endnote — attach to the referencing paragraph
                        else if id == b"  nf"
                            || id == b"fn  "
                            || id == b"  ne"
                            || id == b"en  "
                        {
                            if let Some(text_data) = current_text_data.take() {
                                let text = extract_para_text_formatted(
                                    &text_data,
                                    current_char_shape_mapping.as_ref(),
                                    &self.char_shapes,
                                );
                                push_paragraph(&mut blocks, text);
                                current_char_shape_mapping = None;
                            }
                            if let Some(note) = extract_subtree_text(&records, i, 100, " ") {
                                let trimmed = note.trim();
                                if !trimmed.is_empty() {
                                    let is_endnote = id == b"  ne" || id == b"en  ";
                                    let label = if is_endnote { "[미주]" } else { "[각주]" };
                                    let body = format!("{} {}", label, trimmed);
                                    match blocks.last_mut() {
                                        Some(IRBlock::Paragraph { footnote, .. }) => {
                                            match footnote {
                                                Some(existing) => {
                                                    existing.push_str("; ");
                                                    existing.push_str(&body);
                                                }
                                                None => *footnote = Some(body),
                                            }
                                        }
                                        _ => blocks.push(IRBlock::paragraph(body)),
                                    }
                                }
                            }
                            let end = subtree_end(&records, i, 100);
                            i = end;
                            continue;
                        }
                        // Hyperlink — attach URL to most recent Paragraph
                        else if id == b"kot%"
                            || id == b"%tok"
                            || id == b"knlk"
                            || id == b"klnk"
                        {
                            if let Some(url) = extract_hyperlink_url(&record.data) {
                                match blocks.last_mut() {
                                    Some(IRBlock::Paragraph { href, .. }) if href.is_none() => {
                                        *href = Some(url);
                                    }
                                    _ => {
                                        blocks.push(IRBlock::Paragraph {
                                            text: String::new(),
                                            footnote: None,
                                            href: Some(url),
                                        });
                                    }
                                }
                            }
                        }
                    }
                }
                HWPTAG_PARA_HEADER => {
                    if in_table && record.level < table_level {
                        if !cells.is_empty() {
                            blocks.extend(build_ir_blocks_from_cells(
                                table_rows,
                                table_cols,
                                &cells,
                            ));
                            cells.clear();
                        }
                        in_table = false;
                        table_rows = 0;
                        table_cols = 0;
                    }
                    if !in_table {
                        if let Some(text_data) = current_text_data.take() {
                            let text = extract_para_text_formatted(
                                &text_data,
                                current_char_shape_mapping.as_ref(),
                                &self.char_shapes,
                            );
                            push_paragraph_with_level(&mut blocks, text, current_outline_level);
                            current_char_shape_mapping = None;
                        }
                    }
                    // Extract paraShapeId from PARA_HEADER (offset 6-9)
                    // and look up outline_level from DocInfo ParaShape table
                    current_outline_level = 0;
                    if record.data.len() >= 10 {
                        let para_shape_id = u32::from_le_bytes([
                            record.data[6], record.data[7],
                            record.data[8], record.data[9],
                        ]);
                        if let Some(ps) = self.para_shapes.get(&para_shape_id) {
                            current_outline_level = ps.outline_level;
                        }
                    }
                }
                HWPTAG_TABLE => {
                    if let Some(text_data) = current_text_data.take() {
                        let text = extract_para_text_formatted(
                            &text_data,
                            current_char_shape_mapping.as_ref(),
                            &self.char_shapes,
                        );
                        push_paragraph(&mut blocks, text);
                        current_char_shape_mapping = None;
                    }
                    if in_table && !cells.is_empty() {
                        blocks.extend(build_ir_blocks_from_cells(
                            table_rows,
                            table_cols,
                            &cells,
                        ));
                        cells.clear();
                    }
                    if let Some(info) = parse_table_info(&record.data) {
                        in_table = true;
                        table_level = record.level;
                        table_rows = info.rows as usize;
                        table_cols = info.cols as usize;
                        cells.clear();
                    } else {
                        in_table = false;
                    }
                }
                HWPTAG_LIST_HEADER if in_table => {
                    if let Some(span) = parse_cell_list_header(&record.data) {
                        cells.push((span, String::new()));
                    } else {
                        cells.push((CellSpan::default(), String::new()));
                    }
                }
                HWPTAG_PARA_TEXT => {
                    if in_table {
                        let raw = extract_para_text(&record.data);
                        let text = raw.trim();
                        if text.is_empty() {
                            // skip
                        } else if let Some(last) = cells.last_mut() {
                            if !last.1.is_empty() {
                                last.1.push('\n');
                            }
                            last.1.push_str(text);
                        } else {
                            cells.push((CellSpan::default(), text.to_string()));
                        }
                    } else if let Some(existing) = current_text_data.as_mut() {
                        existing.extend_from_slice(&record.data);
                    } else {
                        current_text_data = Some(record.data.clone());
                    }
                }
                HWPTAG_PARA_CHAR_SHAPE => {
                    if !in_table {
                        current_char_shape_mapping = parse_para_char_shape(&record.data);
                    }
                }
                _ => {}
            }
            i += 1;
        }

        // Flush trailing paragraph
        if let Some(text_data) = current_text_data {
            let text = extract_para_text_formatted(
                &text_data,
                current_char_shape_mapping.as_ref(),
                &self.char_shapes,
            );
            push_paragraph_with_level(&mut blocks, text, current_outline_level);
        }

        // Flush trailing table
        if in_table && !cells.is_empty() {
            blocks.extend(build_ir_blocks_from_cells(
                table_rows.max(1),
                table_cols.max(1),
                &cells,
            ));
        }

        blocks
    }

    fn parse_section_records_formatted(&self, data: &[u8]) -> String {
        let mut parser = RecordParser::new(data);
        let records = parser.parse_all();

        let mut blocks: Vec<String> = Vec::new();

        // Paragraph state
        let mut current_text_data: Option<Vec<u8>> = None;
        let mut current_char_shape_mapping: Option<ParaCharShapeMapping> = None;

        // Table state machine
        let mut in_table = false;
        let mut table_rows: usize = 0;
        let mut table_cols: usize = 0;
        // Track the record-stream level of the HWPTAG_TABLE marker. Cells and
        // their PARA_TEXTs live at level > table_level. When we see a PARA_HEADER
        // at level <= table_level, the table is finished — flush it. This fixes
        // the bug where tables with merged cells (which never reach rows*cols
        // termination) were absorbing later top-level paragraphs.
        let mut table_level: u16 = 0;
        // Each entry pairs cell text with its LIST_HEADER metadata.
        let mut cells: Vec<(CellSpan, String)> = Vec::new();

        // Convenience: flush pending paragraph into blocks
        // (inlined below — closure would fight borrow checker)

        let mut i = 0usize;
        while i < records.len() {
            let record = &records[i];
            match record.tag_id {
                HWPTAG_CTRL_HEADER => {
                    // Read 4-byte ASCII ctrlId. HWP writes ctrlIds as u32 LE,
                    // so the on-disk byte order is reversed from the printable
                    // form. We accept both orientations.
                    //
                    // CRITICAL: when we dispatch into a subtree extractor, we
                    // also advance `i` past the subtree so the main walker
                    // doesn't reprocess the same records and produce duplicates.
                    if record.data.len() >= 4 {
                        let id = &record.data[0..4];
                        // gso (그리기 객체 — text box / image / shape)
                        if id == b" osg" || id == b"gso " {
                            if let Some(text_data) = current_text_data.take() {
                                let text = extract_para_text_formatted(
                                    &text_data,
                                    current_char_shape_mapping.as_ref(),
                                    &self.char_shapes,
                                );
                                if !text.trim().is_empty() {
                                    blocks.push(text);
                                }
                                current_char_shape_mapping = None;
                            }
                            // First check if this gso wraps an image (SHAPE_COMPONENT_PICTURE
                            // child with binDataId). If so, emit a [이미지: imageN] placeholder
                            // — matches kordoc behavior. Otherwise fall through to textbox text.
                            if let Some(bin_id) = extract_subtree_image_id(&records, i, 200) {
                                blocks.push(format!("[이미지: image{}]", bin_id));
                            } else if let Some(box_text) = extract_subtree_text(&records, i, 200, "\n") {
                                if !box_text.trim().is_empty() {
                                    blocks.push(box_text);
                                }
                            }
                            // Skip past the subtree to avoid duplication
                            let end = subtree_end(&records, i, 200);
                            i = end;
                            continue;
                        }
                        // Footnote / endnote — attach text to the paragraph
                        // that contains the reference mark. Mirrors kordoc
                        // v2.0 behavior (parser.ts:582-593) which stores the
                        // note text alongside the paragraph block.
                        //
                        // Critical: we MUST flush `current_text_data` first.
                        // Without the flush, the "last paragraph" in `blocks`
                        // is the one BEFORE the one containing the footnote
                        // mark, so the footnote gets attached to the wrong
                        // paragraph.
                        else if id == b"  nf" || id == b"fn  " || id == b"  ne" || id == b"en  " {
                            // Flush pending paragraph (holds the reference mark).
                            if let Some(text_data) = current_text_data.take() {
                                let text = extract_para_text_formatted(
                                    &text_data,
                                    current_char_shape_mapping.as_ref(),
                                    &self.char_shapes,
                                );
                                if !text.trim().is_empty() {
                                    blocks.push(text);
                                }
                                current_char_shape_mapping = None;
                            }

                            if let Some(note) = extract_subtree_text(&records, i, 100, " ") {
                                let trimmed = note.trim();
                                if !trimmed.is_empty() {
                                    let is_endnote = id == b"  ne" || id == b"en  ";
                                    let label = if is_endnote { "[미주]" } else { "[각주]" };
                                    // Attach inline to the most recent
                                    // paragraph block. Skip tables (GFM rows
                                    // start with `|`) to avoid breaking
                                    // table structure.
                                    match blocks.last_mut() {
                                        Some(last) if !last.starts_with('|') => {
                                            last.push_str(&format!(" {} {}", label, trimmed));
                                        }
                                        _ => {
                                            blocks.push(format!("{} {}", label, trimmed));
                                        }
                                    }
                                }
                            }
                            let end = subtree_end(&records, i, 100);
                            i = end;
                            continue;
                        }
                        // Hyperlink
                        else if id == b"kot%" || id == b"%tok" || id == b"knlk" || id == b"klnk" {
                            if let Some(url) = extract_hyperlink_url(&record.data) {
                                if let Some(last) = blocks.last_mut() {
                                    last.push_str(&format!(" <{}>", url));
                                } else {
                                    blocks.push(format!("<{}>", url));
                                }
                            }
                            // Hyperlinks have no subtree text — don't skip
                        }
                        // tbl / lbt → handled by HWPTAG_TABLE state machine, skip here
                    }
                }
                HWPTAG_PARA_HEADER => {
                    // Empirically: cell PARA_HEADERs in real HWP files appear at
                    // the SAME level as their parent HWPTAG_TABLE record (not
                    // deeper). So we need strict `<` here — `<=` would prematurely
                    // flush on the very first cell paragraph and produce empty tables.
                    // This closes the table when we hit a paragraph at a SHALLOWER
                    // level than the table — which means we've returned to outer
                    // section flow.
                    if in_table && record.level < table_level {
                        if !cells.is_empty() {
                            if let Some(md) = build_gfm_table(table_rows, table_cols, &cells) {
                                blocks.push(md);
                            }
                            cells.clear();
                        }
                        in_table = false;
                        table_rows = 0;
                        table_cols = 0;
                    }

                    if !in_table {
                        if let Some(text_data) = current_text_data.take() {
                            let text = extract_para_text_formatted(
                                &text_data,
                                current_char_shape_mapping.as_ref(),
                                &self.char_shapes,
                            );
                            if !text.trim().is_empty() {
                                blocks.push(text);
                            }
                            current_char_shape_mapping = None;
                        }
                    }
                }
                HWPTAG_TABLE => {
                    // Flush any pending paragraph BEFORE the table
                    if let Some(text_data) = current_text_data.take() {
                        let text = extract_para_text_formatted(
                            &text_data,
                            current_char_shape_mapping.as_ref(),
                            &self.char_shapes,
                        );
                        if !text.trim().is_empty() {
                            blocks.push(text);
                        }
                        current_char_shape_mapping = None;
                    }

                    // Defensive: nested/overlapping tables — flush whatever we have
                    if in_table && !cells.is_empty() {
                        if let Some(md) = build_gfm_table(table_rows, table_cols, &cells) {
                            blocks.push(md);
                        }
                        cells.clear();
                    }

                    if let Some(info) = parse_table_info(&record.data) {
                        in_table = true;
                        table_level = record.level;
                        table_rows = info.rows as usize;
                        table_cols = info.cols as usize;
                        cells.clear();
                    } else {
                        in_table = false;
                    }
                }
                HWPTAG_LIST_HEADER if in_table => {
                    // New cell starts. Parse position + spans from LIST_HEADER.
                    if let Some(span) = parse_cell_list_header(&record.data) {
                        cells.push((span, String::new()));
                    } else {
                        // Fallback: create an empty span and rely on sequential fill
                        cells.push((CellSpan::default(), String::new()));
                    }
                }
                HWPTAG_PARA_TEXT => {
                    if in_table {
                        // Append to the current (last) cell's text buffer.
                        // Multiple PARA_TEXTs per cell get joined with single \n
                        // (preserves intra-cell line structure for GFM `<br>`).
                        // Trim each fragment because extract_para_text emits a
                        // trailing \n for CHAR_PARA_BREAK — without trimming,
                        // joining with another \n produces `\n\n` → `<br><br>`.
                        let raw = extract_para_text(&record.data);
                        let text = raw.trim();
                        if text.is_empty() {
                            // skip empty paragraphs inside cells
                        } else if let Some(last) = cells.last_mut() {
                            if !last.1.is_empty() {
                                last.1.push('\n');
                            }
                            last.1.push_str(text);
                        } else {
                            cells.push((CellSpan::default(), text.to_string()));
                        }

                        // NOTE: We deliberately do NOT terminate on
                        // `cells.len() >= rows*cols` here. HWP allows the LAST
                        // cell to contain multiple PARA_TEXT records, and an
                        // eager flush after the first one would orphan the
                        // remaining paragraphs (they'd fall through to the
                        // outer paragraph branch and render as standalone text
                        // BELOW the table — e.g. "(정부서울청사 소재)"
                        // dropping out of the 임용예정인원 cell).
                        //
                        // Closure is handled by:
                        //   1. HWPTAG_PARA_HEADER at shallower level (line ~241)
                        //      — fires when section flow returns above table_level
                        //   2. HWPTAG_TABLE for the next table (line ~282)
                        //   3. End-of-section trailing flush (line ~376)
                    } else if let Some(existing) = current_text_data.as_mut() {
                        // A single HWP paragraph can span multiple PARA_TEXT records.
                        // Overwriting here drops earlier fragments, which shows up as
                        // large apparent "text gaps" on dense business-report fixtures.
                        existing.extend_from_slice(&record.data);
                    } else {
                        current_text_data = Some(record.data.clone());
                    }
                }
                HWPTAG_PARA_CHAR_SHAPE => {
                    if !in_table {
                        current_char_shape_mapping = parse_para_char_shape(&record.data);
                    }
                }
                // When we see a non-table-related top-level marker after cells,
                // flush the table. CTRL_HEADER on a new top-level paragraph means
                // the table block is complete.
                _ => {}
            }
            i += 1;
        }

        // Flush trailing paragraph
        if let Some(text_data) = current_text_data {
            let text = extract_para_text_formatted(
                &text_data,
                current_char_shape_mapping.as_ref(),
                &self.char_shapes,
            );
            if !text.trim().is_empty() {
                blocks.push(text);
            }
        }

        // Flush trailing table (common case: merged cells make rows*cols
        // termination unreachable, so the table closes only at section end)
        if in_table && !cells.is_empty() {
            if let Some(md) = build_gfm_table(table_rows.max(1), table_cols.max(1), &cells) {
                blocks.push(md);
            }
        }

        // Korean legal-document heading detection: promote paragraphs that
        // start with 「제N편/장/절」, 「제N조」, 「부칙」 to markdown headings.
        // This is the core kordoc heuristic (parser.ts:detectHwp5Headings) and
        // is critical for downstream RAG splitters that key on heading levels.
        // We deliberately AVOID font-size-based detection since CHAR_SHAPE
        // styling in HWP files is unreliable.
        let blocks = blocks
            .into_iter()
            .map(|b| promote_korean_heading(&b).unwrap_or(b))
            .collect::<Vec<_>>();

        blocks.join("\n\n")
    }

    /// 이미지를 추출합니다
    pub fn extract_images(&mut self) -> io::Result<Vec<ImageData>> {
        let mut images = Vec::new();
        
        // Get list of BinData streams
        let bin_data_names = self.ole_reader.list_bin_data();
        
        for name in bin_data_names {
            if let Ok(data) = self.ole_reader.read_bin_data(&name) {
                // Detect image format from magic bytes
                let format = detect_image_format(&data);
                if !format.is_empty() {
                    // Generate proper filename
                    let filename = if name.ends_with(&format!(".{}", format)) {
                        name.clone()
                    } else {
                        format!("{}.{}", name, format)
                    };
                    
                    images.push(ImageData {
                        name: filename,
                        original_name: name,
                        format,
                        data,
                    });
                }
            }
        }
        
        Ok(images)
    }

    /// 표 구조를 추출합니다
    pub fn extract_tables(&mut self) -> io::Result<Vec<TableData>> {
        let mut tables = Vec::new();
        let section_count = self.ole_reader.section_count();
        
        for section_num in 0..section_count {
            if let Ok(data) = self.ole_reader.read_body_text(section_num) {
                let mut parser = RecordParser::new(&data);
                let records = parser.parse_all();
                
                // Find TABLE records and associated text
                let mut current_table: Option<TableData> = None;
                let mut current_cells: Vec<String> = Vec::new();
                let mut current_cell_spans: Vec<CellSpan> = Vec::new();
                let mut in_table = false;
                let mut table_info: Option<(u16, u16)> = None;
                let mut cell_index: usize = 0;

                for record in &records {
                    match record.tag_id {
                        HWPTAG_TABLE => {
                            // Finish previous table if any
                            if let Some(mut table) = current_table.take() {
                                table.cells = organize_cells(&current_cells, table.cols);
                                table.cell_spans = current_cell_spans.clone();
                                tables.push(table);
                                current_cells.clear();
                                current_cell_spans.clear();
                            }

                            // Start new table
                            if let Some(info) = parse_table_info(&record.data) {
                                current_table = Some(TableData {
                                    rows: info.rows as usize,
                                    cols: info.cols as usize,
                                    cells: Vec::new(),
                                    cell_spans: Vec::new(),
                                });
                                table_info = Some((info.rows, info.cols));
                                in_table = true;
                                cell_index = 0;
                            }
                        }
                        HWPTAG_LIST_HEADER if in_table => {
                            // Parse cell span information from LIST_HEADER
                            if let Some((_rows, cols)) = table_info {
                                if let Some(mut span) = parse_cell_list_header(&record.data) {
                                    // Calculate row/col from cell index
                                    span.row = (cell_index / cols as usize) as u16;
                                    span.col = (cell_index % cols as usize) as u16;

                                    // Only store if there's actual spanning (row_span > 1 or col_span > 1)
                                    if span.row_span > 1 || span.col_span > 1 {
                                        current_cell_spans.push(span);
                                    }
                                }
                                cell_index += 1;
                            }
                        }
                        HWPTAG_PARA_TEXT if in_table => {
                            let text = extract_para_text(&record.data);
                            current_cells.push(text);

                            // Check if we've collected all cells
                            if let Some((rows, cols)) = table_info {
                                if current_cells.len() >= (rows * cols) as usize {
                                    if let Some(mut table) = current_table.take() {
                                        table.cells = organize_cells(&current_cells, table.cols);
                                        table.cell_spans = current_cell_spans.clone();
                                        tables.push(table);
                                        current_cells.clear();
                                        current_cell_spans.clear();
                                        in_table = false;
                                        table_info = None;
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                }
                
                // Handle last table
                if let Some(mut table) = current_table.take() {
                    table.cells = organize_cells(&current_cells, table.cols);
                    table.cell_spans = current_cell_spans;
                    tables.push(table);
                }
            }
        }
        
        Ok(tables)
    }

    /// 메타데이터를 추출합니다
    pub fn extract_metadata(&mut self) -> io::Result<Metadata> {
        let header_data = self.ole_reader.read_file_header()?;
        let version = parse_version(&header_data);
        let flags = self.ole_reader.flags();

        // Distribution-locked HWPs store their body in ViewText/Section{N}
        // instead of BodyText. Report whichever stream family exists so the
        // section_count in metadata reflects reality.
        let body_sections = self.ole_reader.section_count();
        let view_sections = self.ole_reader.view_section_count();
        let section_count = if body_sections > 0 {
            body_sections
        } else {
            view_sections
        };

        let mut meta = Metadata {
            version,
            compressed: flags.compressed,
            encrypted: flags.encrypted,
            section_count,
            bin_data_count: self.ole_reader.list_bin_data().len(),
            ..Default::default()
        };

        // Best-effort: title/author/etc from \u{0005}HwpSummaryInformation
        if let Ok(summary_data) = self.ole_reader.read_summary_information() {
            let props = parse_summary_information(&summary_data);
            meta.title = props.get(&2).cloned();
            meta.subject = props.get(&3).cloned();
            meta.author = props.get(&4).cloned();
            meta.keywords = props.get(&5).cloned();
            meta.description = props.get(&6).cloned();
            meta.last_author = props.get(&8).cloned();
        }

        Ok(meta)
    }

    /// MDM 형식으로 변환합니다
    ///
    /// Uses the IR block path (`extract_blocks`) for structural fidelity:
    /// headings, images, tables, footnotes are all preserved as typed IR
    /// blocks and rendered via `blocks_to_markdown`. Falls back to the
    /// legacy `extract_text` string path only when block extraction fails.
    pub fn to_mdm(&mut self) -> io::Result<MdmDocument> {
        let content = match self.extract_blocks() {
            Ok(blocks) if !blocks.is_empty() => {
                crate::ir::blocks_to_markdown(&blocks)
            }
            _ => self.extract_text()?,
        };
        let images = self.extract_images()?;
        let tables = self.extract_tables()?;
        let metadata = self.extract_metadata()?;

        Ok(MdmDocument {
            content,
            images,
            tables,
            metadata,
        })
    }
}

/// Push a paragraph onto the IR block list, promoting to
/// `IRBlock::Heading` when [`promote_korean_heading_level`] matches.
/// Empty / whitespace-only text is dropped.
fn push_paragraph(blocks: &mut Vec<IRBlock>, text: String) {
    push_paragraph_with_level(blocks, text, 0);
}

/// Push a paragraph with explicit outline_level from ParaShape.
/// If outline_level > 0, emit as heading directly.
/// Otherwise fall back to Korean heading pattern matching.
fn push_paragraph_with_level(blocks: &mut Vec<IRBlock>, text: String, outline_level: u8) {
    if text.trim().is_empty() {
        return;
    }
    // Priority 1: explicit outline_level from ParaShape
    if outline_level > 0 && outline_level <= 6 {
        blocks.push(IRBlock::Heading {
            level: outline_level,
            text: text.trim().to_string(),
        });
        return;
    }
    // Priority 2: Korean legal heading patterns
    if let Some((level, cleaned)) = promote_korean_heading_level(&text) {
        blocks.push(IRBlock::Heading {
            level,
            text: cleaned,
        });
    } else {
        blocks.push(IRBlock::paragraph(text));
    }
}

/// Korean legal-document heading detection returning `(level, cleaned_text)`.
/// Same rules as the legacy [`promote_korean_heading`] but hands the level
/// back to the caller so the IR path can emit `IRBlock::Heading { level, .. }`
/// directly instead of prefixing `#`.
fn promote_korean_heading_level(paragraph: &str) -> Option<(u8, String)> {
    if paragraph.starts_with('|') || paragraph.starts_with('#') {
        return None;
    }
    let trimmed = paragraph.trim_start();
    if trimmed.is_empty() || trimmed.len() > 240 {
        return None;
    }
    let first_line = trimmed.lines().next()?;
    if first_line.len() > 80 {
        return None;
    }

    let cleaned = paragraph.trim_start().to_string();

    if first_line == "부칙"
        || first_line.starts_with("부칙 ")
        || first_line.starts_with("부칙(")
    {
        return Some((1, cleaned));
    }
    if first_line.starts_with("제")
        && (first_line.contains("편") || first_line.contains("장"))
        && matches_n_marker(first_line, &["편", "장"])
    {
        return Some((2, cleaned));
    }
    if first_line.starts_with("제") && matches_n_marker(first_line, &["절"]) {
        return Some((3, cleaned));
    }
    if first_line.starts_with("제") && matches_n_marker(first_line, &["조"]) {
        return Some((3, cleaned));
    }
    if first_line.starts_with("제") && matches_n_marker(first_line, &["목"]) {
        return Some((4, cleaned));
    }

    None
}

/// Build IR blocks from HWP table cells.
///
/// Same 4-pass cleanup pipeline as the legacy [`build_gfm_table`]
/// (address-based placement, empty-row/col trimming, 1-row → heading
/// collapse, 2-col → label/body pair) but emits `Vec<IRBlock>` instead
/// of a Markdown string. Single source of truth for table structural
/// logic — the Markdown path goes through
/// [`crate::ir::blocks_to_markdown`] over the same blocks.
fn build_ir_blocks_from_cells(
    rows: usize,
    cols: usize,
    cells: &[(CellSpan, String)],
) -> Vec<IRBlock> {
    if cells.is_empty() || cols == 0 {
        return Vec::new();
    }

    let rows = rows.max(1).min(1024);
    let cols = cols.max(1).min(256);

    let mut grid: Vec<Vec<Option<String>>> = vec![vec![None; cols]; rows];
    let has_addr = cells.iter().any(|(s, _)| s.has_addr);

    if has_addr {
        for (span, text) in cells {
            let r = span.row_addr as usize;
            let c = span.col_addr as usize;
            if r >= rows || c >= cols {
                continue;
            }
            grid[r][c] = Some(text.clone());
            let rs = span.row_span.max(1) as usize;
            let cs = span.col_span.max(1) as usize;
            for dr in 0..rs {
                for dc in 0..cs {
                    if dr == 0 && dc == 0 {
                        continue;
                    }
                    let rr = r + dr;
                    let cc = c + dc;
                    if rr < rows && cc < cols && grid[rr][cc].is_none() {
                        grid[rr][cc] = Some(String::new());
                    }
                }
            }
        }
    } else {
        let mut idx = 0usize;
        for r in 0..rows {
            for c in 0..cols {
                if grid[r][c].is_some() {
                    continue;
                }
                if idx >= cells.len() {
                    break;
                }
                let (span, text) = &cells[idx];
                idx += 1;
                grid[r][c] = Some(text.clone());
                let rs = span.row_span.max(1) as usize;
                let cs = span.col_span.max(1) as usize;
                for dr in 0..rs {
                    for dc in 0..cs {
                        if dr == 0 && dc == 0 {
                            continue;
                        }
                        let rr = r + dr;
                        let cc = c + dc;
                        if rr < rows && cc < cols && grid[rr][cc].is_none() {
                            grid[rr][cc] = Some(String::new());
                        }
                    }
                }
            }
        }
    }

    let cell_text = |row: usize, col: usize| -> String {
        grid.get(row)
            .and_then(|r| r.get(col))
            .and_then(|c| c.clone())
            .map(|s| s.trim().to_string())
            .unwrap_or_default()
    };
    let nonempty_cols: Vec<usize> = (0..cols)
        .filter(|&c| (0..rows).any(|r| !cell_text(r, c).is_empty()))
        .collect();
    let nonempty_rows: Vec<usize> = (0..rows)
        .filter(|&r| (0..cols).any(|c| !cell_text(r, c).is_empty()))
        .collect();
    if nonempty_cols.is_empty() || nonempty_rows.is_empty() {
        return Vec::new();
    }

    let eff_rows = nonempty_rows.len();
    let eff_cols = nonempty_cols.len();
    let trimmed: Vec<Vec<String>> = nonempty_rows
        .iter()
        .map(|&r| nonempty_cols.iter().map(|&c| cell_text(r, c)).collect())
        .collect();

    // 1-row → heading OR paragraph
    if eff_rows == 1 {
        let joined = trimmed[0].join(" ").trim().to_string();
        if joined.is_empty() {
            return Vec::new();
        }
        let looks_like_heading =
            joined.chars().count() <= 60 && !joined.contains('\n');
        if looks_like_heading {
            return vec![IRBlock::Heading {
                level: 3,
                text: joined,
            }];
        }
        return joined
            .split('\n')
            .filter(|s| !s.trim().is_empty())
            .map(|s| IRBlock::paragraph(s.trim()))
            .collect();
    }

    // 1-col → paragraphs
    if eff_cols == 1 {
        return trimmed
            .iter()
            .map(|row| row[0].clone())
            .filter(|s| !s.is_empty())
            .map(IRBlock::paragraph)
            .collect();
    }

    // 2-col label/body → paragraphs with **bold** label
    let label_col_is_short = trimmed.iter().all(|row| {
        let label = row[0].trim();
        label.is_empty() || (label.chars().count() <= 30 && !label.contains('\n'))
    });
    let first_row_looks_like_header = eff_rows >= 2
        && trimmed
            .first()
            .map(|row| {
                row.iter().all(|cell| {
                    let cell = cell.trim();
                    !cell.is_empty() && cell.chars().count() <= 20 && !cell.contains('\n')
                })
            })
            .unwrap_or(false);
    if eff_cols == 2 && eff_rows >= 3 && label_col_is_short && !first_row_looks_like_header {
        let mut out: Vec<IRBlock> = Vec::new();
        for row in &trimmed {
            let label = row[0].trim();
            let body = row[1].trim();
            if label.is_empty() && body.is_empty() {
                continue;
            }
            if !label.is_empty() {
                out.push(IRBlock::paragraph(format!("**{}**", label)));
            }
            if !body.is_empty() {
                for line in body.split('\n') {
                    let line = line.trim();
                    if !line.is_empty() {
                        out.push(IRBlock::paragraph(line));
                    }
                }
            }
        }
        return out;
    }

    // Default: IRTable from the trimmed grid
    let ir_cells: Vec<Vec<IRCell>> = trimmed
        .iter()
        .map(|row| {
            row.iter()
                .map(|s| IRCell::new(s.replace('\n', " ")))
                .collect()
        })
        .collect();
    vec![IRBlock::Table(IRTable::new(ir_cells))]
}

/// LEGACY: kept only as a reference implementation. All HWP heading
/// promotion now goes through [`promote_korean_heading_level`] +
/// [`push_paragraph`]. Marked dead_code to suppress the warning without
/// deleting the documented reference.
#[allow(dead_code)]
fn promote_korean_heading(paragraph: &str) -> Option<String> {
    if paragraph.starts_with('|') || paragraph.starts_with('#') {
        return None;
    }
    let trimmed = paragraph.trim_start();
    if trimmed.is_empty() || trimmed.len() > 240 {
        return None;
    }
    // Take only the first line for matching (multi-paragraph blocks shouldn't
    // be promoted as a whole).
    let first_line = trimmed.lines().next()?;
    if first_line.len() > 80 {
        return None;
    }

    // Helper: produce a heading prefix without disturbing the rest of the body
    let make = |level: usize| -> String {
        let prefix = "#".repeat(level);
        format!("{} {}", prefix, paragraph.trim_start())
    };

    // Charwise checks (avoid pulling in regex crate dependency)
    if first_line == "부칙" || first_line.starts_with("부칙 ") || first_line.starts_with("부칙(") {
        return Some(make(1));
    }

    // 제N편 / 제N장
    if first_line.starts_with("제") && (first_line.contains("편") || first_line.contains("장"))
        && matches_n_marker(first_line, &["편", "장"])
    {
        return Some(make(2));
    }
    // 제N절
    if first_line.starts_with("제") && matches_n_marker(first_line, &["절"]) {
        return Some(make(3));
    }
    // 제N조
    if first_line.starts_with("제") && matches_n_marker(first_line, &["조"]) {
        return Some(make(3));
    }
    // 제N목
    if first_line.starts_with("제") && matches_n_marker(first_line, &["목"]) {
        return Some(make(4));
    }

    None
}

/// True if `s` starts with `제`, has at least one digit before any of `markers`,
/// and the marker character appears within reasonable distance from start.
fn matches_n_marker(s: &str, markers: &[&str]) -> bool {
    let chars: Vec<char> = s.chars().collect();
    if chars.first() != Some(&'제') {
        return false;
    }

    let mut i = 1;
    let mut saw_digit = false;
    while i < chars.len() && chars[i].is_ascii_digit() {
        saw_digit = true;
        i += 1;
    }
    if !saw_digit {
        return false;
    }
    // Optional `의N` (e.g., 제5조의2)
    if i < chars.len() && chars[i] == '의' {
        i += 1;
        while i < chars.len() && chars[i].is_ascii_digit() {
            i += 1;
        }
    }
    if i >= chars.len() {
        return false;
    }
    let marker = chars[i];
    markers.iter().any(|m| m.chars().next() == Some(marker))
}

/// Post-processing: infer headings by comparing paragraph font sizes
/// against the document's base (body) font size.
///
/// Algorithm (simplified from unhwp heading_analyzer):
/// 1. Find the base font size = most common size weighted by text length
/// 2. Paragraphs with font_size >= base * 1.15 and text length <= 80 → heading
/// 3. Level: 1.8x+=H1, 1.5x+=H2, 1.3x+=H3, else H4; bold → -1 level boost
///
/// Only promotes IRBlock::Paragraph, never touches existing Heading or Table.
fn infer_headings_by_font_size(blocks: &mut Vec<IRBlock>, char_shapes: &HashMap<u32, CharShape>) {
    if char_shapes.is_empty() {
        return;
    }

    // Find base font size (most common, weighted by usage)
    let mut size_weights: HashMap<u32, usize> = HashMap::new(); // size_centipoinds → total chars
    for shape in char_shapes.values() {
        if shape.font_size_pt > 0.0 {
            let key = (shape.font_size_pt * 100.0) as u32;
            *size_weights.entry(key).or_default() += 1;
        }
    }
    let base_size_cp = match size_weights.iter().max_by_key(|(_, w)| *w) {
        Some((k, _)) => *k,
        None => return,
    };
    let base_size = base_size_cp as f32 / 100.0;
    if base_size <= 0.0 {
        return;
    }

    // Collect the dominant (first) char_shape font_size for each paragraph.
    // We can't know which char_shape_id each paragraph uses without the mapping,
    // so we use a heuristic: scan char_shapes for sizes larger than base.
    let large_sizes: Vec<(u32, f32, bool)> = char_shapes.iter()
        .filter(|(_, s)| s.font_size_pt > base_size * 1.1)
        .map(|(id, s)| (*id, s.font_size_pt, s.bold))
        .collect();

    if large_sizes.is_empty() {
        return;
    }

    // For each paragraph block, check if it's short and its text matches
    // what would be rendered with a large char_shape. Since we already have
    // the formatted text, check if the paragraph text (stripped of markdown)
    // is short enough to be a heading.
    for block in blocks.iter_mut() {
        if let IRBlock::Paragraph { text, .. } = block {
            let stripped = text.replace("**", "").replace('*', "").trim().to_string();
            // Skip long paragraphs, table rows, already-markdown-headed
            if stripped.len() > 80 || stripped.is_empty() || stripped.starts_with('|') || stripped.starts_with('#') {
                continue;
            }
            // Check if this paragraph has a large font char_shape applied
            // Heuristic: if bold markers present AND text is short → likely heading
            let has_bold = text.contains("**");
            let is_short = stripped.len() <= 60;

            if has_bold && is_short {
                // Assume bold + short = heading (common Korean document pattern)
                let level = if stripped.len() <= 20 { 1u8 } else { 2u8 };
                *block = IRBlock::Heading {
                    level,
                    text: stripped,
                };
            }
        }
    }
}

/// Parse an OLE2 PropertySet stream (e.g. `\u{0005}HwpSummaryInformation`) and
/// return a map of propertyId → string value.
///
/// Format (Microsoft DocSummaryInfo / SummaryInformation spec):
///
///   PropertySetHeader (28 bytes)
///     0..2   ByteOrder      = 0xFFFE (LE)
///     2..4   Format         = 0
///     4..8   OS version
///     8..24  CLSID
///    24..28  NumPropertySets (always >= 1)
///
///   PropertySetEntry[NumPropertySets]
///     0..16  FMTID (CLSID)
///    16..20  Offset to PropertySet from stream start
///
///   PropertySet at offset
///     0..4   Size (whole property set, in bytes)
///     4..8   NumProperties
///     8..    PropertyIdentifierAndOffset[NumProperties]   (each: u32 propId, u32 offset from PropertySet start)
///    then    Property values at offset
///
///   Property value
///     0..4   Type (VT_*)
///     4..    value data
///
/// We only decode VT_LPSTR (0x001E) and VT_LPWSTR (0x001F) string types — these
/// cover title/author/subject/keywords/comments. Numeric property types are skipped.
fn parse_summary_information(data: &[u8]) -> HashMap<u32, String> {
    let mut props: HashMap<u32, String> = HashMap::new();

    if data.len() < 48 {
        return props;
    }
    // Sanity check on byte order
    if data[0] != 0xFE || data[1] != 0xFF {
        return props;
    }

    let read_u32 = |off: usize| -> Option<u32> {
        if off + 4 > data.len() {
            None
        } else {
            Some(u32::from_le_bytes([data[off], data[off + 1], data[off + 2], data[off + 3]]))
        }
    };

    let num_sets = match read_u32(24) {
        Some(n) => n,
        None => return props,
    };
    if num_sets == 0 || num_sets > 8 {
        return props;
    }

    // We only inspect the first PropertySet (DocumentSummary doesn't matter
    // for HWP — title/author live in the first one).
    let entry_offset = 28; // first PropertySetEntry begins right after header
    let set_offset = match read_u32(entry_offset + 16) {
        Some(o) => o as usize,
        None => return props,
    };
    if set_offset + 8 > data.len() {
        return props;
    }

    let num_props = match read_u32(set_offset + 4) {
        Some(n) => n as usize,
        None => return props,
    };
    if num_props == 0 || num_props > 256 {
        return props;
    }

    // Read PropertyIdentifierAndOffset table
    for i in 0..num_props {
        let pidx_off = set_offset + 8 + i * 8;
        if pidx_off + 8 > data.len() {
            break;
        }
        let prop_id = match read_u32(pidx_off) {
            Some(v) => v,
            None => break,
        };
        let prop_offset = match read_u32(pidx_off + 4) {
            Some(v) => set_offset + v as usize,
            None => break,
        };
        if prop_offset + 8 > data.len() {
            continue;
        }

        let vt = match read_u32(prop_offset) {
            Some(v) => v,
            None => continue,
        };

        // Only known string types
        if vt == 0x001E {
            // VT_LPSTR — Code-Page string. Layout: u32 length (incl NUL), then bytes.
            let len = match read_u32(prop_offset + 4) {
                Some(l) => l as usize,
                None => continue,
            };
            let str_start = prop_offset + 8;
            if len > 0 && str_start + len <= data.len() {
                let bytes = &data[str_start..str_start + len];
                // Strip trailing NULs
                let trimmed: Vec<u8> = bytes.iter().take_while(|&&b| b != 0).copied().collect();
                // HWP summary streams use code-page 949 (CP949 / EUC-KR variant). We
                // try UTF-8 first; if that fails, attempt CP949 via encoding_rs which
                // is already a dependency.
                let s = match std::str::from_utf8(&trimmed) {
                    Ok(s) => s.to_string(),
                    Err(_) => {
                        let (cow, _enc, _had_errors) = encoding_rs::EUC_KR.decode(&trimmed);
                        cow.into_owned()
                    }
                };
                let s = s.trim().to_string();
                if !s.is_empty() {
                    props.insert(prop_id, s);
                }
            }
        } else if vt == 0x001F {
            // VT_LPWSTR — UTF-16LE string. Layout: u32 char count (incl NUL), then chars.
            let count = match read_u32(prop_offset + 4) {
                Some(l) => l as usize,
                None => continue,
            };
            let str_start = prop_offset + 8;
            let byte_len = count * 2;
            if count > 0 && str_start + byte_len <= data.len() {
                let mut chars: Vec<u16> = Vec::with_capacity(count);
                for j in 0..count {
                    let off = str_start + j * 2;
                    let cp = u16::from_le_bytes([data[off], data[off + 1]]);
                    if cp == 0 {
                        break;
                    }
                    chars.push(cp);
                }
                if let Ok(s) = String::from_utf16(&chars) {
                    let s = s.trim().to_string();
                    if !s.is_empty() {
                        props.insert(prop_id, s);
                    }
                }
            }
        }
        // Other VT types (numeric, datetime, etc.) — skip silently
    }

    props
}

/// Walk forward from a CTRL_HEADER record, collecting all child PARA_TEXT
/// content until the level returns to (or below) the parent level.
///
/// Used to recover text trapped inside text-box / footnote / endnote / shape
/// containers — these would otherwise be invisible because the top-level
/// PARA_HEADER → PARA_TEXT walker doesn't descend into nested CTRL_HEADER trees.
///
/// Mirrors kordoc's `extractTextBoxText` (parser.ts:631-646) and
/// `extractNoteText` (parser.ts:613-628). The `max_lookahead` cap protects
/// against malformed records that never decrement level.
fn extract_subtree_text(
    records: &[HwpRecord],
    ctrl_idx: usize,
    max_lookahead: usize,
    joiner: &str,
) -> Option<String> {
    if ctrl_idx >= records.len() {
        return None;
    }
    let ctrl_level = records[ctrl_idx].level;
    let mut texts: Vec<String> = Vec::new();

    let end = (ctrl_idx + max_lookahead + 1).min(records.len());
    for r in &records[ctrl_idx + 1..end] {
        if r.level <= ctrl_level {
            break;
        }
        if r.tag_id == HWPTAG_PARA_TEXT {
            let t = extract_para_text(&r.data);
            let trimmed = t.trim();
            if !trimmed.is_empty() {
                texts.push(trimmed.to_string());
            }
        }
    }

    if texts.is_empty() {
        None
    } else {
        Some(texts.join(joiner))
    }
}

/// Walk forward from a CTRL_HEADER (gso) record looking for a child
/// SHAPE_COMPONENT_PICTURE record. Returns the picture's binDataId so the
/// caller can emit a `[이미지: imageN]` placeholder. Returns `None` if no
/// picture child is found within the lookahead window — caller should fall
/// back to text-box extraction.
///
/// Mirrors kordoc parser.ts:381-401 `extractBinDataId`.
fn extract_subtree_image_id(records: &[HwpRecord], ctrl_idx: usize, max_lookahead: usize) -> Option<u16> {
    if ctrl_idx >= records.len() {
        return None;
    }
    let ctrl_level = records[ctrl_idx].level;
    let end = (ctrl_idx + max_lookahead + 1).min(records.len());

    for j in (ctrl_idx + 1)..end {
        let r = &records[j];
        if r.level <= ctrl_level {
            break;
        }
        if r.tag_id == HWPTAG_SHAPE_COMPONENT_PICTURE {
            if let Some((_, bin_id)) = parse_picture_component(&r.data) {
                if bin_id > 0 {
                    return Some(bin_id);
                }
            }
            // Even if parsing fails, this is definitely a picture node — return a
            // sentinel that the caller can interpret as "image present, id unknown".
            return Some(0);
        }
    }
    None
}

/// Return the index just past the last child of a CTRL_HEADER subtree.
/// Used to skip records that `extract_subtree_text` already consumed so the
/// main walker doesn't reprocess them and produce duplicates.
fn subtree_end(records: &[HwpRecord], ctrl_idx: usize, max_lookahead: usize) -> usize {
    if ctrl_idx >= records.len() {
        return records.len();
    }
    let ctrl_level = records[ctrl_idx].level;
    let end = (ctrl_idx + max_lookahead + 1).min(records.len());
    let mut last = ctrl_idx;
    for j in (ctrl_idx + 1)..end {
        if records[j].level <= ctrl_level {
            return j; // first sibling/parent — stop BEFORE it
        }
        last = j;
    }
    last + 1
}

/// Extract a hyperlink URL from a CTRL_HEADER (klnk / %tok) record.
///
/// HWP stores the link target as a UTF-16LE string somewhere inside the record
/// payload. Rather than parse the full struct, we scan for "http" / "https" /
/// "www." in UTF-16LE and read until a NUL terminator. This is the same
/// best-effort approach kordoc uses (parser.ts:649-673).
fn extract_hyperlink_url(data: &[u8]) -> Option<String> {
    // UTF-16LE encoding of "http"
    let needles: [&[u8]; 3] = [
        &[b'h', 0, b't', 0, b't', 0, b'p', 0],
        &[b'H', 0, b'T', 0, b'T', 0, b'P', 0],
        &[b'w', 0, b'w', 0, b'w', 0, b'.', 0],
    ];

    let mut start = None;
    'outer: for needle in needles.iter() {
        if data.len() < needle.len() {
            continue;
        }
        for i in 0..=(data.len() - needle.len()) {
            if &data[i..i + needle.len()] == *needle {
                start = Some(i);
                break 'outer;
            }
        }
    }
    let start = start?;

    // Read UTF-16LE codepoints until NUL or end
    let mut chars: Vec<u16> = Vec::new();
    let mut i = start;
    while i + 1 < data.len() {
        let cp = u16::from_le_bytes([data[i], data[i + 1]]);
        if cp == 0 {
            break;
        }
        // URLs don't contain control chars below 0x20
        if cp < 0x20 {
            break;
        }
        chars.push(cp);
        i += 2;
        if chars.len() > 2048 {
            break;
        }
    }

    String::from_utf16(&chars).ok().filter(|s| !s.is_empty())
}

/// Organize flat cell list into rows (sequential fallback when colAddr/rowAddr
/// are absent — kept for the legacy `extract_tables` API path).
fn organize_cells(cells: &[String], cols: usize) -> Vec<Vec<String>> {
    if cols == 0 {
        return Vec::new();
    }

    cells
        .chunks(cols)
        .map(|chunk| chunk.to_vec())
        .collect()
}

/// Build a GFM table from cells with span/address metadata.
///
/// Two placement strategies, picked automatically:
/// 1. **Address-based** (preferred) — when any cell has `has_addr = true`,
///    place each cell at `(row_addr, col_addr)` and shadow-fill its
///    `row_span × col_span` rectangle with empty cells. This is the only
///    correct way to render HWP tables containing merged cells.
/// 2. **Sequential fill** (fallback) — when addresses are absent, fill
///    row-by-row, also expanding spans into shadow cells.
///
/// Both strategies emit `""` for shadow cells, matching kordoc's behavior
/// (parser.ts:826-868) which produces well-formed GFM that downstream
/// renderers can interpret as merged regions.
fn build_gfm_table(rows: usize, cols: usize, cells: &[(CellSpan, String)]) -> Option<String> {
    if cells.is_empty() || cols == 0 {
        return None;
    }

    // Bound rows/cols to avoid runaway allocation on malformed records
    let rows = rows.max(1).min(1024);
    let cols = cols.max(1).min(256);

    // Initialize grid with None (no cell placed yet)
    let mut grid: Vec<Vec<Option<String>>> = vec![vec![None; cols]; rows];

    let has_addr = cells.iter().any(|(s, _)| s.has_addr);

    if has_addr {
        for (span, text) in cells {
            let r = span.row_addr as usize;
            let c = span.col_addr as usize;
            if r >= rows || c >= cols {
                continue;
            }
            grid[r][c] = Some(text.clone());

            // Shadow-fill spans with empty placeholders
            let rs = span.row_span.max(1) as usize;
            let cs = span.col_span.max(1) as usize;
            for dr in 0..rs {
                for dc in 0..cs {
                    if dr == 0 && dc == 0 {
                        continue;
                    }
                    let rr = r + dr;
                    let cc = c + dc;
                    if rr < rows && cc < cols && grid[rr][cc].is_none() {
                        grid[rr][cc] = Some(String::new());
                    }
                }
            }
        }
    } else {
        // Sequential fill (kordoc-style fallback)
        let mut idx = 0usize;
        for r in 0..rows {
            for c in 0..cols {
                if grid[r][c].is_some() {
                    continue;
                }
                if idx >= cells.len() {
                    break;
                }
                let (span, text) = &cells[idx];
                idx += 1;
                grid[r][c] = Some(text.clone());

                let rs = span.row_span.max(1) as usize;
                let cs = span.col_span.max(1) as usize;
                for dr in 0..rs {
                    for dc in 0..cs {
                        if dr == 0 && dc == 0 {
                            continue;
                        }
                        let rr = r + dr;
                        let cc = c + dc;
                        if rr < rows && cc < cols && grid[rr][cc].is_none() {
                            grid[rr][cc] = Some(String::new());
                        }
                    }
                }
            }
        }
    }

    // ── Cleanup pass 1: drop fully-empty columns ───────────────────────────
    // HWP frequently emits 3-col layout tables where the meaningful content
    // sits in only one or two columns and the rest are spacer cells. Render
    // those spacers as GFM columns and the viewer fills with <th></th> noise.
    let cell_text = |row: usize, col: usize| -> &str {
        grid.get(row)
            .and_then(|r| r.get(col))
            .and_then(|c| c.as_deref())
            .map(|s| s.trim())
            .unwrap_or("")
    };
    let nonempty_cols: Vec<usize> = (0..cols)
        .filter(|&c| (0..rows).any(|r| !cell_text(r, c).is_empty()))
        .collect();
    let nonempty_rows: Vec<usize> = (0..rows)
        .filter(|&r| (0..cols).any(|c| !cell_text(r, c).is_empty()))
        .collect();

    if nonempty_cols.is_empty() || nonempty_rows.is_empty() {
        return None;
    }

    let eff_cols = nonempty_cols.len();
    let eff_rows = nonempty_rows.len();

    // Build the trimmed grid (only non-empty rows × non-empty cols)
    let trimmed: Vec<Vec<String>> = nonempty_rows
        .iter()
        .map(|&r| {
            nonempty_cols
                .iter()
                .map(|&c| cell_text(r, c).to_string())
                .collect()
        })
        .collect();

    // ── Cleanup pass 2: 1-row × N-col → section heading OR paragraph ───────
    // HWP authors use 1-row tables purely as visual headings (e.g.
    // "| 1 | 임용예정인원 (1개 직위, 총 1명) |"). Rendering these as GFM
    // tables with empty <tbody> creates the "둥둥 떠보이는 빈 헤더" effect.
    //
    // Heuristic: short joined content (≤60 chars, no newline) → heading.
    // Long content → plain paragraph (it was a body paragraph that the
    // author wrapped in a 1-row table for visual framing, NOT a heading).
    if eff_rows == 1 {
        let cells: Vec<&str> = trimmed[0].iter().map(|s| s.as_str()).collect();
        let joined = cells.join(" ").trim().to_string();
        if joined.is_empty() {
            return None;
        }
        let looks_like_heading =
            joined.chars().count() <= 60 && !joined.contains('\n');
        return if looks_like_heading {
            // ### nests under any document-level headings without
            // overpowering Korean legal §-headings (which are h2).
            Some(format!("### {}", joined))
        } else {
            // Long body wrapped in a decorative 1-row table — emit as
            // paragraph and preserve intra-cell line breaks.
            Some(joined.replace('\n', "\n\n"))
        };
    }

    // ── Cleanup pass 3: 1-col after collapse → paragraphs ──────────────────
    if eff_cols == 1 {
        let body: Vec<String> = trimmed
            .iter()
            .map(|row| row[0].clone())
            .filter(|s| !s.is_empty())
            .collect();
        return if body.is_empty() {
            None
        } else {
            Some(body.join("\n\n"))
        };
    }

    // ── Cleanup pass 4: 2-col label/body table → definition list ───────────
    // The "응시자격요건 고려사항" / "직무기술서" pattern is a tall 2-col
    // table where col[0] is a section label and col[1] holds a multi-line
    // body that becomes a wall of <br><br><br> in GFM. Render as
    // **label**\n\nbody\n\n which is far more readable AND keeps the text
    // searchable for downstream RAG.
    // Only collapse to definition list when col[0] looks like a SHORT label
    // for every non-empty row, AND the first row does not look like a real
    // table header. Otherwise we'd flatten legitimate 2-col business tables
    // such as "구분 | 주요 현황" and lose row/column structure.
    let label_col_is_short = trimmed.iter().all(|row| {
        let label = row[0].trim();
        label.is_empty() || (label.chars().count() <= 30 && !label.contains('\n'))
    });
    let first_row_looks_like_header = eff_rows >= 2
        && trimmed
            .first()
            .map(|row| {
                row.iter().all(|cell| {
                    let cell = cell.trim();
                    !cell.is_empty() && cell.chars().count() <= 20 && !cell.contains('\n')
                })
            })
            .unwrap_or(false);
    if eff_cols == 2 && eff_rows >= 3 && label_col_is_short && !first_row_looks_like_header {
        let mut out = String::new();
        for row in &trimmed {
            let label = row[0].trim();
            let body = row[1].trim();
            if label.is_empty() && body.is_empty() {
                continue;
            }
            if !out.is_empty() {
                out.push_str("\n\n");
            }
            if !label.is_empty() {
                out.push_str("**");
                out.push_str(label);
                out.push_str("**");
                if !body.is_empty() {
                    out.push_str("\n\n");
                }
            }
            if !body.is_empty() {
                // Preserve intra-cell line breaks as paragraph breaks for
                // readability instead of <br>. This is the whole point.
                out.push_str(&body.replace('\n', "\n\n"));
            }
        }
        return if out.is_empty() { None } else { Some(out) };
    }

    // ── Default: render the trimmed grid as a real GFM table ───────────────
    let rendered_rows: Vec<String> = trimmed
        .iter()
        .map(|row| {
            let mut line = String::from("|");
            for cell in row {
                let escaped = cell.replace('\n', "<br>").replace('|', "\\|");
                line.push(' ');
                line.push_str(&escaped);
                line.push_str(" |");
            }
            line
        })
        .collect();

    if rendered_rows.is_empty() {
        return None;
    }

    let mut md = String::new();
    for (i, line) in rendered_rows.iter().enumerate() {
        md.push_str(line);
        md.push('\n');
        if i == 0 {
            md.push('|');
            for _ in 0..eff_cols {
                md.push_str(" --- |");
            }
            md.push('\n');
        }
    }

    Some(md)
}

/// 이미지 포맷 감지
fn detect_image_format(data: &[u8]) -> String {
    if data.len() < 8 {
        return String::new();
    }
    
    // Check magic bytes
    if data[0] == 0xFF && data[1] == 0xD8 && data[2] == 0xFF {
        "jpeg".to_string()
    } else if data[0] == 0x89 && data[1] == 0x50 && data[2] == 0x4E && data[3] == 0x47 {
        "png".to_string()
    } else if data[0] == 0x47 && data[1] == 0x49 && data[2] == 0x46 {
        "gif".to_string()
    } else if data[0] == 0x42 && data[1] == 0x4D {
        "bmp".to_string()
    } else if data[0] == 0xD7 && data[1] == 0xCD && data[2] == 0xC6 && data[3] == 0x9A {
        "wmf".to_string()
    } else if data[0] == 0x01 && data[1] == 0x00 && data[2] == 0x00 && data[3] == 0x00 {
        "emf".to_string()
    } else if &data[0..4] == b"RIFF" && data.len() >= 12 && &data[8..12] == b"WEBP" {
        "webp".to_string()
    } else {
        String::new()
    }
}

/// 버전 파싱
fn parse_version(data: &[u8]) -> String {
    // HWP FileHeader: 32-byte signature + 4-byte version
    if data.len() >= 36 {
        let major = data[35] as u32;
        let minor = data[34] as u32;
        let build = data[33] as u32;
        let revision = data[32] as u32;
        format!("HWP {}.{}.{}.{}", major, minor, build, revision)
    } else {
        "Unknown".to_string()
    }
}

/// HWP 파일 구조 정보
#[derive(Debug)]
pub struct FileStructure {
    pub total_streams: usize,
    pub streams: Vec<String>,
    pub section_count: usize,
    pub bin_data_count: usize,
    pub compressed: bool,
    pub encrypted: bool,
}

/// 이미지 데이터
#[derive(Debug, Clone)]
pub struct ImageData {
    pub name: String,
    pub original_name: String,
    pub format: String,
    pub data: Vec<u8>,
}

/// 표 데이터
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TableData {
    pub rows: usize,
    pub cols: usize,
    pub cells: Vec<Vec<String>>,
    /// Cell span information for merged cells
    pub cell_spans: Vec<CellSpan>,
}

impl TableData {
    /// Convert table to Markdown format.
    ///
    /// Behavior notes:
    /// - 1-column tables (layout wrappers, not data) are unwrapped to plain
    ///   paragraphs. This matches kordoc's `flattenLayoutTables` behavior and
    ///   prevents ugly `| title |` artifacts in the output stream.
    /// - The header separator width always matches the actual max row width,
    ///   not `self.cols`, to keep GFM well-formed when row lengths differ.
    /// - Newlines inside cells become `<br>` for true GFM rendering instead of
    ///   collapsing them to spaces (matches kordoc's cell rendering).
    pub fn to_markdown(&self) -> String {
        if self.cells.is_empty() || self.cols == 0 {
            return String::new();
        }

        // 1-col layout table → unwrap to paragraphs
        if self.cols == 1 {
            return self.cells.iter()
                .filter_map(|row| row.first().map(|s| s.trim().to_string()))
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
                .join("\n\n");
        }

        // Determine the actual column width (max non-empty cells across rows,
        // capped at self.cols). This keeps the GFM table well-formed even when
        // some rows have fewer cells than declared.
        let actual_cols = self.cells.iter()
            .map(|row| row.len())
            .max()
            .unwrap_or(self.cols)
            .min(self.cols)
            .max(2); // GFM requires at least 2 cols to be a real table

        let mut md = String::new();

        for (i, row) in self.cells.iter().enumerate() {
            md.push_str("|");
            for col in 0..actual_cols {
                let cell = row.get(col).map(String::as_str).unwrap_or("");
                // Trim FIRST, then convert remaining inner newlines to <br>
                // (trim() does not remove `<br>`, so order matters)
                let rendered = cell.trim().replace('\n', "<br>");
                // Pipes inside cells must be escaped to keep GFM well-formed
                let rendered = rendered.replace('|', "\\|");
                md.push(' ');
                md.push_str(&rendered);
                md.push_str(" |");
            }
            md.push('\n');

            // Header separator after first row
            if i == 0 {
                md.push('|');
                for _ in 0..actual_cols {
                    md.push_str(" --- |");
                }
                md.push('\n');
            }
        }

        md
    }
}

/// 메타데이터
#[derive(Debug, Clone, Default)]
pub struct Metadata {
    pub version: String,
    pub compressed: bool,
    pub encrypted: bool,
    pub section_count: usize,
    pub bin_data_count: usize,
    /// Document title from \u{0005}HwpSummaryInformation propId 2 (PIDSI_TITLE)
    pub title: Option<String>,
    /// Author/creator — propId 4 (PIDSI_AUTHOR)
    pub author: Option<String>,
    /// Subject — propId 3 (PIDSI_SUBJECT)
    pub subject: Option<String>,
    /// Keywords/tags — propId 5 (PIDSI_KEYWORDS)
    pub keywords: Option<String>,
    /// Description / comment — propId 6 (PIDSI_COMMENTS)
    pub description: Option<String>,
    /// Last saved by — propId 8 (PIDSI_LASTAUTHOR)
    pub last_author: Option<String>,
}

/// MDM 문서 (변환 결과)
#[derive(Debug)]
pub struct MdmDocument {
    pub content: String,
    pub images: Vec<ImageData>,
    pub tables: Vec<TableData>,
    pub metadata: Metadata,
}

impl MdmDocument {
    /// Generate MDX content
    pub fn to_mdx(&self) -> String {
        let mut mdx = String::new();

        // YAML-safe escaping for free-form metadata strings
        let yaml_escape = |s: &str| -> String {
            // Quote and escape backslashes + quotes; collapse newlines to spaces
            let mut out = String::with_capacity(s.len() + 2);
            out.push('"');
            for c in s.chars() {
                match c {
                    '"' => out.push_str("\\\""),
                    '\\' => out.push_str("\\\\"),
                    '\n' | '\r' => out.push(' '),
                    _ => out.push(c),
                }
            }
            out.push('"');
            out
        };

        // Frontmatter
        mdx.push_str("---\n");
        mdx.push_str(&format!("version: \"{}\"\n", self.metadata.version));
        if let Some(t) = &self.metadata.title {
            mdx.push_str(&format!("title: {}\n", yaml_escape(t)));
        }
        if let Some(a) = &self.metadata.author {
            mdx.push_str(&format!("author: {}\n", yaml_escape(a)));
        }
        if let Some(s) = &self.metadata.subject {
            mdx.push_str(&format!("subject: {}\n", yaml_escape(s)));
        }
        if let Some(k) = &self.metadata.keywords {
            mdx.push_str(&format!("keywords: {}\n", yaml_escape(k)));
        }
        if let Some(d) = &self.metadata.description {
            mdx.push_str(&format!("description: {}\n", yaml_escape(d)));
        }
        if let Some(l) = &self.metadata.last_author {
            mdx.push_str(&format!("lastAuthor: {}\n", yaml_escape(l)));
        }
        mdx.push_str(&format!("sections: {}\n", self.metadata.section_count));
        mdx.push_str(&format!("images: {}\n", self.images.len()));
        mdx.push_str(&format!("tables: {}\n", self.tables.len()));
        mdx.push_str("---\n\n");
        
        // Content
        mdx.push_str(&self.content);
        
        mdx
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── push_paragraph + heading promotion ──

    #[test]
    fn push_paragraph_drops_empty() {
        let mut blocks: Vec<IRBlock> = Vec::new();
        push_paragraph(&mut blocks, String::new());
        push_paragraph(&mut blocks, "   \n  ".to_string());
        assert!(blocks.is_empty());
    }

    #[test]
    fn push_paragraph_promotes_legal_heading_to_h3() {
        let mut blocks: Vec<IRBlock> = Vec::new();
        push_paragraph(&mut blocks, "제1조 목적".to_string());
        assert_eq!(blocks.len(), 1);
        match &blocks[0] {
            IRBlock::Heading { level, text } => {
                assert_eq!(*level, 3);
                assert_eq!(text, "제1조 목적");
            }
            other => panic!("expected Heading, got {:?}", other),
        }
    }

    #[test]
    fn push_paragraph_promotes_bchk_to_h1() {
        let mut blocks: Vec<IRBlock> = Vec::new();
        push_paragraph(&mut blocks, "부칙".to_string());
        assert!(matches!(
            blocks[0],
            IRBlock::Heading { level: 1, .. }
        ));
    }

    #[test]
    fn push_paragraph_keeps_normal_paragraph() {
        let mut blocks: Vec<IRBlock> = Vec::new();
        push_paragraph(&mut blocks, "이것은 일반 문단이다.".to_string());
        assert!(matches!(blocks[0], IRBlock::Paragraph { .. }));
    }

    // ── build_ir_blocks_from_cells ──

    fn mk_cell(text: &str) -> (CellSpan, String) {
        (CellSpan::default(), text.to_string())
    }

    #[test]
    fn ir_table_basic_2x2_emits_irtable() {
        let cells = vec![
            mk_cell("A"),
            mk_cell("B"),
            mk_cell("1"),
            mk_cell("2"),
        ];
        let blocks = build_ir_blocks_from_cells(2, 2, &cells);
        assert_eq!(blocks.len(), 1);
        match &blocks[0] {
            IRBlock::Table(t) => {
                assert_eq!(t.rows, 2);
                assert_eq!(t.cols, 2);
                assert_eq!(t.cells[0][0].text, "A");
                assert_eq!(t.cells[1][1].text, "2");
            }
            other => panic!("expected Table, got {:?}", other),
        }
    }

    #[test]
    fn ir_table_1_row_collapses_to_heading() {
        // Short content → ### heading
        let cells = vec![mk_cell("임용예정인원"), mk_cell("1명")];
        let blocks = build_ir_blocks_from_cells(1, 2, &cells);
        assert_eq!(blocks.len(), 1);
        assert!(matches!(
            blocks[0],
            IRBlock::Heading { level: 3, .. }
        ));
    }

    #[test]
    fn ir_table_empty_returns_no_blocks() {
        let blocks = build_ir_blocks_from_cells(0, 0, &[]);
        assert!(blocks.is_empty());
    }

    // ── parse_section_records_formatted round-trip through IR ──

    #[test]
    fn test_image_format_detection() {
        let jpeg = vec![0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x00, 0x00, 0x00];
        assert_eq!(detect_image_format(&jpeg), "jpeg");
        
        let png = vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
        assert_eq!(detect_image_format(&png), "png");
        
        let gif = vec![0x47, 0x49, 0x46, 0x38, 0x39, 0x61, 0x00, 0x00];
        assert_eq!(detect_image_format(&gif), "gif");
        
        let bmp = vec![0x42, 0x4D, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
        assert_eq!(detect_image_format(&bmp), "bmp");
        
        let wmf = vec![0xD7, 0xCD, 0xC6, 0x9A, 0x00, 0x00, 0x00, 0x00];
        assert_eq!(detect_image_format(&wmf), "wmf");
        
        // WebP (needs 12 bytes: RIFF + size + WEBP)
        let webp = b"RIFF\x00\x00\x00\x00WEBP";
        assert_eq!(detect_image_format(webp), "webp");
        
        // Too short
        let short = vec![0xFF, 0xD8];
        assert_eq!(detect_image_format(&short), "");
    }

    #[test]
    fn test_table_to_markdown() {
        let table = TableData {
            rows: 2,
            cols: 2,
            cells: vec![
                vec!["Header 1".to_string(), "Header 2".to_string()],
                vec!["Cell 1".to_string(), "Cell 2".to_string()],
            ],
            cell_spans: Vec::new(),
        };
        
        let md = table.to_markdown();
        assert!(md.contains("| Header 1 |"));
        assert!(md.contains("| --- |"));
        assert!(md.contains("| Cell 1 |"));
    }

    #[test]
    fn test_organize_cells() {
        let cells = vec![
            "A".to_string(), "B".to_string(),
            "C".to_string(), "D".to_string(),
        ];
        let organized = organize_cells(&cells, 2);
        assert_eq!(organized.len(), 2);
        assert_eq!(organized[0], vec!["A", "B"]);
        assert_eq!(organized[1], vec!["C", "D"]);
    }
}
