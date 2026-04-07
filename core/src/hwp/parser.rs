use super::ole::OleReader;
use super::record::{
    HwpRecord, RecordParser, extract_para_text, parse_table_info,
    parse_char_shape, parse_para_char_shape, extract_para_text_formatted,
    parse_cell_list_header, CellSpan,
    CharShape, ParaCharShapeMapping,
    HWPTAG_PARA_TEXT, HWPTAG_PARA_HEADER, HWPTAG_TABLE, HWPTAG_LIST_HEADER,
    HWPTAG_PARA_CHAR_SHAPE, HWPTAG_CHAR_SHAPE, HWPTAG_CTRL_HEADER,
    HWPTAG_SHAPE_COMPONENT_PICTURE, HWPTAG_BIN_DATA,
};
use std::collections::HashMap;
use std::io::{self};
use std::path::Path;

/// HWP 파일 파서
pub struct HwpParser {
    ole_reader: OleReader,
    /// Character shape definitions from DocInfo
    char_shapes: HashMap<u32, CharShape>,
}

impl HwpParser {
    /// HWP 파일을 엽니다
    pub fn open<P: AsRef<Path>>(path: P) -> io::Result<Self> {
        let ole_reader = OleReader::open(path)?;
        Ok(HwpParser {
            ole_reader,
            char_shapes: HashMap::new(),
        })
    }

    /// Parse DocInfo stream to extract character shapes
    fn parse_doc_info(&mut self) -> io::Result<()> {
        let data = self.ole_reader.read_doc_info()?;
        let mut parser = RecordParser::new(&data);
        let records = parser.parse_all();

        let mut shape_index: u32 = 0;
        for record in records {
            if record.tag_id == HWPTAG_CHAR_SHAPE {
                if let Some(shape) = parse_char_shape(&record.data) {
                    self.char_shapes.insert(shape_index, shape);
                }
                shape_index += 1;
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

    /// 텍스트를 추출합니다
    pub fn extract_text(&mut self) -> io::Result<String> {
        // First, parse DocInfo to get character shapes
        if self.char_shapes.is_empty() {
            let _ = self.parse_doc_info();
        }

        let mut all_text = Vec::new();
        let section_count = self.ole_reader.section_count();

        if section_count == 0 {
            return Ok("No BodyText sections found.".to_string());
        }

        for section_num in 0..section_count {
            match self.ole_reader.read_body_text(section_num) {
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
                    eprintln!("Warning: Could not read Section{}: {}", section_num, e);
                }
            }
        }

        if all_text.is_empty() {
            Ok("No text extracted. File may be encrypted or have unsupported format.".to_string())
        } else {
            Ok(all_text.join("\n\n"))
        }
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
        // Each entry pairs cell text with its LIST_HEADER metadata.
        // The active cell (last entry) accumulates PARA_TEXT until a new
        // LIST_HEADER appears or the table ends.
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
                            if let Some(box_text) = extract_subtree_text(&records, i, 200, "\n") {
                                if !box_text.trim().is_empty() {
                                    blocks.push(box_text);
                                }
                            }
                            // Skip past the subtree to avoid duplication
                            let end = subtree_end(&records, i, 200);
                            i = end;
                            continue;
                        }
                        // Footnote / endnote
                        else if id == b"  nf" || id == b"fn  " || id == b"  ne" || id == b"en  " {
                            if let Some(note) = extract_subtree_text(&records, i, 100, " ") {
                                let trimmed = note.trim();
                                if !trimmed.is_empty() {
                                    blocks.push(format!("[각주] {}", trimmed));
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

                        // Termination check: did we collect rows*cols cells?
                        // Note: with merged cells the actual cell count is LESS than
                        // rows*cols, so this check may never trip — that's why we
                        // also flush on the next HWPTAG_TABLE / end-of-section.
                        if table_cols > 0
                            && table_rows > 0
                            && cells.len() >= table_rows * table_cols
                        {
                            if let Some(md) = build_gfm_table(table_rows, table_cols, &cells) {
                                blocks.push(md);
                            }
                            cells.clear();
                            in_table = false;
                            table_rows = 0;
                            table_cols = 0;
                        }
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
        match self.ole_reader.read_file_header() {
            Ok(header_data) => {
                let version = parse_version(&header_data);
                let flags = self.ole_reader.flags();
                
                Ok(Metadata {
                    version,
                    compressed: flags.compressed,
                    encrypted: flags.encrypted,
                    section_count: self.ole_reader.section_count(),
                    bin_data_count: self.ole_reader.list_bin_data().len(),
                })
            }
            Err(e) => Err(e),
        }
    }

    /// MDM 형식으로 변환합니다
    pub fn to_mdm(&mut self) -> io::Result<MdmDocument> {
        let text = self.extract_text()?;
        let images = self.extract_images()?;
        let tables = self.extract_tables()?;
        let metadata = self.extract_metadata()?;
        
        Ok(MdmDocument {
            content: text,
            images,
            tables,
            metadata,
        })
    }
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

    // Render to GFM (mirrors TableData::to_markdown but skips the 1-col unwrap
    // because by this point we know the table has structural meaning).
    if cols == 1 {
        // 1-column wrapper → unwrap to paragraphs
        let body: Vec<String> = grid
            .iter()
            .filter_map(|row| row[0].as_ref())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        return if body.is_empty() {
            None
        } else {
            Some(body.join("\n\n"))
        };
    }

    let mut md = String::new();
    for (i, row) in grid.iter().enumerate() {
        md.push('|');
        for cell in row {
            let raw = cell.as_deref().unwrap_or("");
            let escaped = raw.trim().replace('\n', "<br>").replace('|', "\\|");
            md.push(' ');
            md.push_str(&escaped);
            md.push_str(" |");
        }
        md.push('\n');

        if i == 0 {
            md.push('|');
            for _ in 0..cols {
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
#[derive(Debug, Clone)]
pub struct Metadata {
    pub version: String,
    pub compressed: bool,
    pub encrypted: bool,
    pub section_count: usize,
    pub bin_data_count: usize,
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
        
        // Frontmatter
        mdx.push_str("---\n");
        mdx.push_str(&format!("version: \"{}\"\n", self.metadata.version));
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
