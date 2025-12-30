//! HWP Record parsing utilities
//!
//! HWP 5.0 uses a tag-based record structure where each record has:
//! - Tag ID (10 bits): identifies the type of record
//! - Level (10 bits): nesting level
//! - Size (12 bits): data size (or 0xFFF for extended size)

use std::collections::HashMap;
use std::io::{self, Read, Cursor};

/// HWPTAG constants - Document Info section
pub const HWPTAG_DOCUMENT_PROPERTIES: u16 = 0x00;
pub const HWPTAG_ID_MAPPINGS: u16 = 0x01;
pub const HWPTAG_BIN_DATA: u16 = 0x02;
pub const HWPTAG_FACE_NAME: u16 = 0x03;
pub const HWPTAG_BORDER_FILL: u16 = 0x04;
pub const HWPTAG_CHAR_SHAPE: u16 = 0x05;
pub const HWPTAG_TAB_DEF: u16 = 0x06;
pub const HWPTAG_NUMBERING: u16 = 0x07;
pub const HWPTAG_BULLET: u16 = 0x08;
pub const HWPTAG_PARA_SHAPE: u16 = 0x09;
pub const HWPTAG_STYLE: u16 = 0x0A;

/// HWPTAG constants - Body section (offset 0x42 = 66)
pub const HWPTAG_PARA_HEADER: u16 = 0x42;
pub const HWPTAG_PARA_TEXT: u16 = 0x43;
pub const HWPTAG_PARA_CHAR_SHAPE: u16 = 0x44;
pub const HWPTAG_PARA_LINE_SEG: u16 = 0x45;
pub const HWPTAG_PARA_RANGE_TAG: u16 = 0x46;
pub const HWPTAG_CTRL_HEADER: u16 = 0x47;
pub const HWPTAG_LIST_HEADER: u16 = 0x48;
pub const HWPTAG_PAGE_DEF: u16 = 0x49;
pub const HWPTAG_FOOTNOTE_SHAPE: u16 = 0x4A;
pub const HWPTAG_PAGE_BORDER_FILL: u16 = 0x4B;
pub const HWPTAG_SHAPE_COMPONENT: u16 = 0x4C;
pub const HWPTAG_TABLE: u16 = 0x4D;
pub const HWPTAG_SHAPE_COMPONENT_LINE: u16 = 0x4E;
pub const HWPTAG_SHAPE_COMPONENT_RECTANGLE: u16 = 0x4F;
pub const HWPTAG_SHAPE_COMPONENT_ELLIPSE: u16 = 0x50;
pub const HWPTAG_SHAPE_COMPONENT_POLYGON: u16 = 0x52;
pub const HWPTAG_SHAPE_COMPONENT_CURVE: u16 = 0x53;
pub const HWPTAG_SHAPE_COMPONENT_OLE: u16 = 0x54;
pub const HWPTAG_SHAPE_COMPONENT_PICTURE: u16 = 0x55;
pub const HWPTAG_SHAPE_COMPONENT_CONTAINER: u16 = 0x56;
pub const HWPTAG_CTRL_DATA: u16 = 0x57;
pub const HWPTAG_EQEDIT: u16 = 0x58;

/// Special characters in HWP text
pub const CHAR_LINE_BREAK: u16 = 0x0A;
pub const CHAR_PARA_BREAK: u16 = 0x0D;
pub const CHAR_TAB: u16 = 0x09;
pub const CHAR_HYPHEN: u16 = 0x1E;
pub const CHAR_SPACE: u16 = 0x20;
pub const CHAR_INLINE_CTRL_START: u16 = 0x01; // Extended control start
pub const CHAR_INLINE_CTRL_END: u16 = 0x02;   // Extended control end
pub const CHAR_SECTION_DEF: u16 = 0x03;
pub const CHAR_FIELD_START: u16 = 0x04;
pub const CHAR_FIELD_END: u16 = 0x05;
pub const CHAR_BOOKMARK: u16 = 0x06;
pub const CHAR_TABLE: u16 = 0x0B;
pub const CHAR_DRAWING: u16 = 0x0C;
pub const CHAR_FOOTNOTE_ENDNOTE: u16 = 0x0E;
pub const CHAR_HIDDEN_COMMENT: u16 = 0x0F;

/// Parsed HWP record
#[derive(Debug, Clone)]
pub struct HwpRecord {
    pub tag_id: u16,
    pub level: u16,
    pub size: u32,
    pub data: Vec<u8>,
}

impl HwpRecord {
    /// Get tag name for debugging
    pub fn tag_name(&self) -> &'static str {
        match self.tag_id {
            HWPTAG_DOCUMENT_PROPERTIES => "DOCUMENT_PROPERTIES",
            HWPTAG_ID_MAPPINGS => "ID_MAPPINGS",
            HWPTAG_BIN_DATA => "BIN_DATA",
            HWPTAG_FACE_NAME => "FACE_NAME",
            HWPTAG_CHAR_SHAPE => "CHAR_SHAPE",
            HWPTAG_PARA_SHAPE => "PARA_SHAPE",
            HWPTAG_STYLE => "STYLE",
            HWPTAG_PARA_HEADER => "PARA_HEADER",
            HWPTAG_PARA_TEXT => "PARA_TEXT",
            HWPTAG_PARA_CHAR_SHAPE => "PARA_CHAR_SHAPE",
            HWPTAG_PARA_LINE_SEG => "PARA_LINE_SEG",
            HWPTAG_CTRL_HEADER => "CTRL_HEADER",
            HWPTAG_TABLE => "TABLE",
            HWPTAG_SHAPE_COMPONENT_PICTURE => "PICTURE",
            _ => "UNKNOWN",
        }
    }
}

/// Record parser for decompressed HWP data
pub struct RecordParser<'a> {
    data: &'a [u8],
    position: usize,
}

impl<'a> RecordParser<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        RecordParser { data, position: 0 }
    }

    /// Parse all records from the data
    pub fn parse_all(&mut self) -> Vec<HwpRecord> {
        let mut records = Vec::new();
        while let Some(record) = self.parse_next() {
            records.push(record);
        }
        records
    }

    /// Parse next record
    pub fn parse_next(&mut self) -> Option<HwpRecord> {
        if self.position + 4 > self.data.len() {
            return None;
        }

        // Read 4-byte header
        let header = u32::from_le_bytes([
            self.data[self.position],
            self.data[self.position + 1],
            self.data[self.position + 2],
            self.data[self.position + 3],
        ]);
        self.position += 4;

        // Parse header fields
        // Tag ID: bits 0-9 (10 bits)
        let tag_id = (header & 0x3FF) as u16;
        // Level: bits 10-19 (10 bits)
        let level = ((header >> 10) & 0x3FF) as u16;
        // Size: bits 20-31 (12 bits)
        let size_field = (header >> 20) & 0xFFF;

        // Extended size if size_field == 0xFFF
        let size = if size_field == 0xFFF {
            if self.position + 4 > self.data.len() {
                return None;
            }
            let extended = u32::from_le_bytes([
                self.data[self.position],
                self.data[self.position + 1],
                self.data[self.position + 2],
                self.data[self.position + 3],
            ]);
            self.position += 4;
            extended
        } else {
            size_field
        };

        // Read data
        if self.position + size as usize > self.data.len() {
            return None;
        }

        let data = self.data[self.position..self.position + size as usize].to_vec();
        self.position += size as usize;

        Some(HwpRecord {
            tag_id,
            level,
            size,
            data,
        })
    }
}

/// Extract text from PARA_TEXT record data
pub fn extract_para_text(data: &[u8]) -> String {
    let mut result = String::new();
    let mut i = 0;

    while i + 1 < data.len() {
        // Read UTF-16LE character
        let char_code = u16::from_le_bytes([data[i], data[i + 1]]);
        i += 2;

        match char_code {
            // Whitespace / visible specials
            CHAR_TAB => result.push('\t'),
            CHAR_LINE_BREAK => result.push('\n'),
            CHAR_PARA_BREAK => result.push('\n'),
            CHAR_SPACE => result.push(' '),
            CHAR_HYPHEN => result.push('-'),

            // Control characters and inline controls
            0..=8 | 0x10..=0x1F => {
                // Extended control characters take 16 bytes total (2 bytes char + 14 bytes payload)
                if char_code == CHAR_INLINE_CTRL_START
                    || char_code == CHAR_SECTION_DEF
                    || char_code == CHAR_FIELD_START
                    || char_code == CHAR_TABLE
                    || char_code == CHAR_DRAWING
                {
                    i += 14;
                }
            }

            // Normal character
            code => {
                if let Some(c) = char::from_u32(code as u32) {
                    result.push(c);
                }
            }
        }
    }

    result
}

/// Cell span information for merged cells
#[derive(Debug, Clone, Default)]
pub struct CellSpan {
    pub row: u16,
    pub col: u16,
    pub row_span: u16,
    pub col_span: u16,
}

/// Table cell with content and merge info
#[derive(Debug, Clone)]
pub struct TableCell {
    pub row: u16,
    pub col: u16,
    pub row_span: u16,
    pub col_span: u16,
    pub content: String,
    pub is_header: bool,
    pub background_color: Option<u32>,
    pub border_style: Option<u8>,
}

impl Default for TableCell {
    fn default() -> Self {
        TableCell {
            row: 0,
            col: 0,
            row_span: 1,
            col_span: 1,
            content: String::new(),
            is_header: false,
            background_color: None,
            border_style: None,
        }
    }
}

/// Parse table structure from TABLE record
#[derive(Debug, Clone)]
pub struct TableInfo {
    pub rows: u16,
    pub cols: u16,
    pub cell_count: u16,
    pub cell_spans: Vec<CellSpan>,
    pub row_heights: Vec<u16>,
    pub col_widths: Vec<u16>,
}

impl Default for TableInfo {
    fn default() -> Self {
        TableInfo {
            rows: 0,
            cols: 0,
            cell_count: 0,
            cell_spans: Vec::new(),
            row_heights: Vec::new(),
            col_widths: Vec::new(),
        }
    }
}

pub fn parse_table_info(data: &[u8]) -> Option<TableInfo> {
    if data.len() < 8 {
        return None;
    }

    // Table record structure:
    // - Flags: 4 bytes
    // - Row count: 2 bytes
    // - Col count: 2 bytes
    // - Cell spacing: 2 bytes
    // - Left/Right/Top/Bottom margins: 2 bytes each (8 bytes total)
    // - Row sizes array: rows * 2 bytes
    // - Border fill ID: 2 bytes
    // - Zone info count: 2 bytes
    // - Zone infos: ...

    let rows = u16::from_le_bytes([data[4], data[5]]);
    let cols = u16::from_le_bytes([data[6], data[7]]);

    let mut info = TableInfo {
        rows,
        cols,
        cell_count: rows * cols,
        cell_spans: Vec::new(),
        row_heights: Vec::new(),
        col_widths: Vec::new(),
    };

    // Parse row heights if available (after margins at offset 18)
    let row_heights_offset = 18;
    if data.len() >= row_heights_offset + (rows as usize * 2) {
        for i in 0..rows as usize {
            let offset = row_heights_offset + i * 2;
            if offset + 2 <= data.len() {
                let height = u16::from_le_bytes([data[offset], data[offset + 1]]);
                info.row_heights.push(height);
            }
        }
    }

    Some(info)
}

/// Parse LIST_HEADER record to extract cell properties (including merge info)
///
/// LIST_HEADER structure for table cells:
/// - Para count: 2 bytes
/// - Flags: 4 bytes (bits 0-1: text direction, bit 2-3: page break type)
/// - Width: 2 bytes
/// - Height: 2 bytes
/// - Left margin: 2 bytes
/// - Right margin: 2 bytes
/// - Top margin: 2 bytes
/// - Bottom margin: 2 bytes
/// - Border fill ID: 2 bytes
/// - Col span: 2 bytes (at offset 22)
/// - Row span: 2 bytes (at offset 24)
/// - Cell width: 2 bytes (at offset 26)
/// - Cell height: 2 bytes (at offset 28)
pub fn parse_cell_list_header(data: &[u8]) -> Option<CellSpan> {
    if data.len() < 26 {
        return None;
    }

    // Read col_span and row_span
    let col_span = u16::from_le_bytes([data[22], data[23]]);
    let row_span = u16::from_le_bytes([data[24], data[25]]);

    Some(CellSpan {
        row: 0,  // Set by caller
        col: 0,  // Set by caller
        row_span: row_span.max(1),
        col_span: col_span.max(1),
    })
}

/// Shape component types
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ShapeType {
    Line,
    Rectangle,
    Ellipse,
    Arc,
    Polygon,
    Curve,
    Picture,
    Ole,
    Container,
    Unknown(u16),
}

impl From<u16> for ShapeType {
    fn from(tag: u16) -> Self {
        match tag {
            HWPTAG_SHAPE_COMPONENT_LINE => ShapeType::Line,
            HWPTAG_SHAPE_COMPONENT_RECTANGLE => ShapeType::Rectangle,
            HWPTAG_SHAPE_COMPONENT_ELLIPSE => ShapeType::Ellipse,
            HWPTAG_SHAPE_COMPONENT_POLYGON => ShapeType::Polygon,
            HWPTAG_SHAPE_COMPONENT_CURVE => ShapeType::Curve,
            HWPTAG_SHAPE_COMPONENT_PICTURE => ShapeType::Picture,
            HWPTAG_SHAPE_COMPONENT_OLE => ShapeType::Ole,
            HWPTAG_SHAPE_COMPONENT_CONTAINER => ShapeType::Container,
            other => ShapeType::Unknown(other),
        }
    }
}

/// Shape component structure for drawings and pictures
#[derive(Debug, Clone)]
pub struct ShapeComponent {
    pub shape_type: ShapeType,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub rotation: i16,
    pub x_flip: bool,
    pub y_flip: bool,
    pub bin_data_id: Option<u16>,  // For pictures, reference to BinData
    pub alt_text: Option<String>,
}

impl Default for ShapeComponent {
    fn default() -> Self {
        ShapeComponent {
            shape_type: ShapeType::Unknown(0),
            x: 0,
            y: 0,
            width: 0,
            height: 0,
            rotation: 0,
            x_flip: false,
            y_flip: false,
            bin_data_id: None,
            alt_text: None,
        }
    }
}

/// Parse SHAPE_COMPONENT record
///
/// SHAPE_COMPONENT structure:
/// - Flags: 4 bytes
/// - Rotation: 2 bytes (degrees * 10)
/// - X coord: 4 bytes (signed)
/// - Y coord: 4 bytes (signed)
/// - Width: 4 bytes
/// - Height: 4 bytes
/// - X flip: 1 byte
/// - Y flip: 1 byte
/// - ...
pub fn parse_shape_component(data: &[u8], shape_type: ShapeType) -> Option<ShapeComponent> {
    if data.len() < 24 {
        return None;
    }

    let _flags = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
    let rotation = i16::from_le_bytes([data[4], data[5]]);
    let x = i32::from_le_bytes([data[6], data[7], data[8], data[9]]);
    let y = i32::from_le_bytes([data[10], data[11], data[12], data[13]]);
    let width = u32::from_le_bytes([data[14], data[15], data[16], data[17]]);
    let height = u32::from_le_bytes([data[18], data[19], data[20], data[21]]);
    let x_flip = data.get(22).map(|&b| b != 0).unwrap_or(false);
    let y_flip = data.get(23).map(|&b| b != 0).unwrap_or(false);

    Some(ShapeComponent {
        shape_type,
        x,
        y,
        width,
        height,
        rotation: rotation / 10,  // Convert from degrees * 10 to degrees
        x_flip,
        y_flip,
        bin_data_id: None,
        alt_text: None,
    })
}

/// Parse SHAPE_COMPONENT_PICTURE record
///
/// Additional picture-specific data after SHAPE_COMPONENT:
/// - Border color: 4 bytes
/// - Border thickness: 4 bytes
/// - Border properties: 4 bytes
/// - Image clip left: 4 bytes
/// - Image clip top: 4 bytes
/// - Image clip right: 4 bytes
/// - Image clip bottom: 4 bytes
/// - Brightness: 1 byte
/// - Contrast: 1 byte
/// - Effect: 1 byte
/// - BinData ID: 2 bytes (reference to BinData record)
pub fn parse_picture_component(data: &[u8]) -> Option<(ShapeComponent, u16)> {
    // First parse the shape component base
    let mut shape = parse_shape_component(data, ShapeType::Picture)?;

    // Picture-specific data starts at offset 24
    // BinData ID is typically at offset 24 + 28 = 52
    let bin_data_offset = 52;
    if data.len() >= bin_data_offset + 2 {
        let bin_data_id = u16::from_le_bytes([data[bin_data_offset], data[bin_data_offset + 1]]);
        shape.bin_data_id = Some(bin_data_id);
        return Some((shape, bin_data_id));
    }

    // Try alternative offset (some HWP versions use different layouts)
    if data.len() >= 26 {
        let bin_data_id = u16::from_le_bytes([data[24], data[25]]);
        shape.bin_data_id = Some(bin_data_id);
        return Some((shape, bin_data_id));
    }

    Some((shape, 0))
}

/// Complete table structure with cells and formatting
#[derive(Debug, Clone)]
pub struct HwpTable {
    pub info: TableInfo,
    pub cells: Vec<Vec<TableCell>>,
}

impl HwpTable {
    pub fn new(rows: u16, cols: u16) -> Self {
        let mut cells = Vec::with_capacity(rows as usize);
        for r in 0..rows {
            let mut row = Vec::with_capacity(cols as usize);
            for c in 0..cols {
                row.push(TableCell {
                    row: r,
                    col: c,
                    ..Default::default()
                });
            }
            cells.push(row);
        }

        HwpTable {
            info: TableInfo {
                rows,
                cols,
                cell_count: rows * cols,
                ..Default::default()
            },
            cells,
        }
    }

    /// Convert table to markdown
    pub fn to_markdown(&self) -> String {
        if self.cells.is_empty() {
            return String::new();
        }

        let mut lines = Vec::new();
        let mut skip_cells: std::collections::HashSet<(u16, u16)> = std::collections::HashSet::new();

        for (row_idx, row) in self.cells.iter().enumerate() {
            let mut cell_contents = Vec::new();

            for (col_idx, cell) in row.iter().enumerate() {
                // Skip cells that are covered by merged cells
                if skip_cells.contains(&(row_idx as u16, col_idx as u16)) {
                    continue;
                }

                // Mark cells covered by this cell's span
                if cell.row_span > 1 || cell.col_span > 1 {
                    for r in 0..cell.row_span {
                        for c in 0..cell.col_span {
                            if r != 0 || c != 0 {
                                skip_cells.insert((row_idx as u16 + r, col_idx as u16 + c));
                            }
                        }
                    }
                }

                // Clean content for markdown table
                let content = cell.content
                    .replace("|", "\\|")
                    .replace("\n", " ")
                    .trim()
                    .to_string();

                cell_contents.push(content);
            }

            lines.push(format!("| {} |", cell_contents.join(" | ")));

            // Add separator after header row
            if row_idx == 0 {
                let sep: Vec<&str> = (0..cell_contents.len()).map(|_| "---").collect();
                lines.push(format!("| {} |", sep.join(" | ")));
            }
        }

        lines.join("\n")
    }
}

/// Character style properties (matching HWPX CharStyle)
#[derive(Debug, Clone, Default)]
pub struct CharShape {
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strikeout: bool,
}

/// Parse HWPTAG_CHAR_SHAPE record to extract character formatting
///
/// HWP 5.0 CHAR_SHAPE structure (simplified):
/// - FaceNameId[7]: 14 bytes (7 x WORD for each language)
/// - Width[7]: 14 bytes (7 x BYTE ratios)
/// - Spacing[7]: 14 bytes (7 x BYTE)
/// - RelSize[7]: 14 bytes (7 x BYTE)
/// - Position[7]: 14 bytes (7 x BYTE)
/// - BaseSize: 4 bytes (INT32, in HWP units)
/// - Attr: 4 bytes (UINT32, formatting flags)
///   - Bit 0: Italic
///   - Bit 1: Bold
///   - Bit 2: Underline type (bits 2-3)
///   - Bit 4-5: Outline type
///   - Bit 6-8: Shadow type
///   - Bit 9: Emboss
///   - Bit 10: Engrave
///   - Bit 11: Superscript
///   - Bit 12: Subscript
///   - Bits 18-21: Strikeout type
/// - ShadowGap: 2 bytes
/// - ... more fields
pub fn parse_char_shape(data: &[u8]) -> Option<CharShape> {
    // Minimum size: 7*2 + 7*1*4 + 4 + 4 = 14 + 28 + 8 = 50 bytes for basic fields
    // But we need at least up to Attr field
    // Offset calculation:
    // - FaceNameId: 7 * 2 = 14 bytes (offset 0-13)
    // - Width ratios: 7 * 1 = 7 bytes (offset 14-20) - but spec says UINT8[7]
    // Actually, let's use simpler offset based on observed data

    // HWP 5.0 spec:
    // Offset 0-13: FaceNameId (7 WORDs)
    // Offset 14-20: Width ratio (7 BYTEs)
    // Offset 21-27: Spacing (7 BYTEs)
    // Offset 28-34: RelSize (7 BYTEs)
    // Offset 35-41: Position (7 BYTEs)
    // Offset 42-45: BaseSize (INT32)
    // Offset 46-49: Attr (UINT32) <- formatting flags here

    if data.len() < 50 {
        return None;
    }

    // Read Attr field at offset 46
    let attr = u32::from_le_bytes([data[46], data[47], data[48], data[49]]);

    // Parse formatting flags
    let italic = (attr & 0x01) != 0;      // Bit 0
    let bold = (attr & 0x02) != 0;         // Bit 1
    let underline_type = (attr >> 2) & 0x03; // Bits 2-3
    let strikeout_type = (attr >> 18) & 0x0F; // Bits 18-21

    Some(CharShape {
        bold,
        italic,
        underline: underline_type != 0,
        strikeout: strikeout_type != 0,
    })
}

/// Character shape mapping for a paragraph
/// Maps text positions to character shape IDs
#[derive(Debug, Clone)]
pub struct ParaCharShapeMapping {
    pub mappings: Vec<(u32, u32)>, // (position, char_shape_id)
}

/// Parse HWPTAG_PARA_CHAR_SHAPE record
///
/// Structure: Array of (Position: UINT32, CharShapeID: UINT32) pairs
/// Position is the character position in the paragraph text
/// CharShapeID references the DocInfo CHAR_SHAPE records
pub fn parse_para_char_shape(data: &[u8]) -> Option<ParaCharShapeMapping> {
    if data.len() < 8 {
        return None;
    }

    let mut mappings = Vec::new();
    let mut pos = 0;

    while pos + 8 <= data.len() {
        let text_pos = u32::from_le_bytes([data[pos], data[pos+1], data[pos+2], data[pos+3]]);
        let shape_id = u32::from_le_bytes([data[pos+4], data[pos+5], data[pos+6], data[pos+7]]);
        mappings.push((text_pos, shape_id));
        pos += 8;
    }

    Some(ParaCharShapeMapping { mappings })
}

/// Apply Markdown formatting based on CharShape
pub fn apply_markdown_formatting(text: &str, style: &CharShape) -> String {
    if text.is_empty() {
        return String::new();
    }

    let mut result = text.to_string();

    // Apply formatting in order: strikeout, bold, italic, underline
    if style.strikeout {
        result = format!("~~{}~~", result);
    }
    if style.bold && style.italic {
        result = format!("***{}***", result);
    } else if style.bold {
        result = format!("**{}**", result);
    } else if style.italic {
        result = format!("*{}*", result);
    }
    if style.underline {
        // Markdown doesn't have native underline, use HTML
        result = format!("<u>{}</u>", result);
    }

    result
}

/// Extract text from PARA_TEXT with formatting applied
pub fn extract_para_text_formatted(
    text_data: &[u8],
    char_shape_mapping: Option<&ParaCharShapeMapping>,
    char_shapes: &HashMap<u32, CharShape>,
) -> String {
    // First, extract raw text with positions
    let text_with_positions = extract_para_text_with_positions(text_data);

    if text_with_positions.is_empty() {
        return String::new();
    }

    // If no char shape mapping, return plain text
    let mapping = match char_shape_mapping {
        Some(m) if !m.mappings.is_empty() => m,
        _ => return text_with_positions.iter().map(|(_, c)| c).collect(),
    };

    // Build formatted text by applying styles to text runs
    let mut result = String::new();
    let mut current_style_id: Option<u32> = None;
    let mut current_run = String::new();

    for (pos, ch) in &text_with_positions {
        // Find the style for this position
        let style_id = find_style_for_position(*pos, mapping);

        // If style changed, flush current run
        if current_style_id != style_id && !current_run.is_empty() {
            let formatted = if let Some(id) = current_style_id {
                if let Some(style) = char_shapes.get(&id) {
                    apply_markdown_formatting(&current_run, style)
                } else {
                    current_run.clone()
                }
            } else {
                current_run.clone()
            };
            result.push_str(&formatted);
            current_run.clear();
        }

        current_style_id = style_id;
        current_run.push(*ch);
    }

    // Flush final run
    if !current_run.is_empty() {
        let formatted = if let Some(id) = current_style_id {
            if let Some(style) = char_shapes.get(&id) {
                apply_markdown_formatting(&current_run, style)
            } else {
                current_run.clone()
            }
        } else {
            current_run.clone()
        };
        result.push_str(&formatted);
    }

    result
}

/// Find the style ID for a given text position
fn find_style_for_position(pos: u32, mapping: &ParaCharShapeMapping) -> Option<u32> {
    // The mappings are sorted by position
    // Find the last mapping where position <= pos
    let mut current_id = None;
    for (map_pos, shape_id) in &mapping.mappings {
        if *map_pos <= pos {
            current_id = Some(*shape_id);
        } else {
            break;
        }
    }
    current_id
}

/// Extract text with character positions from PARA_TEXT record
fn extract_para_text_with_positions(data: &[u8]) -> Vec<(u32, char)> {
    let mut result = Vec::new();
    let mut i = 0;
    let mut char_pos: u32 = 0;

    while i + 1 < data.len() {
        // Read UTF-16LE character
        let char_code = u16::from_le_bytes([data[i], data[i + 1]]);
        i += 2;

        match char_code {
            // Whitespace / visible specials
            CHAR_TAB => {
                result.push((char_pos, '\t'));
                char_pos += 1;
            }
            CHAR_LINE_BREAK => {
                result.push((char_pos, '\n'));
                char_pos += 1;
            }
            CHAR_PARA_BREAK => {
                result.push((char_pos, '\n'));
                char_pos += 1;
            }
            CHAR_SPACE => {
                result.push((char_pos, ' '));
                char_pos += 1;
            }
            CHAR_HYPHEN => {
                result.push((char_pos, '-'));
                char_pos += 1;
            }

            // Control characters and inline controls
            0..=8 | 0x10..=0x1F => {
                if char_code == CHAR_INLINE_CTRL_START
                    || char_code == CHAR_SECTION_DEF
                    || char_code == CHAR_FIELD_START
                    || char_code == CHAR_TABLE
                    || char_code == CHAR_DRAWING
                {
                    i += 14;
                }
                char_pos += 1;
            }

            // Normal character
            code => {
                if let Some(c) = char::from_u32(code as u32) {
                    result.push((char_pos, c));
                }
                char_pos += 1;
            }
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_record_header_parsing() {
        // Create a simple record: tag=0x43 (PARA_TEXT), level=0, size=4
        // Header: tag_id | (level << 10) | (size << 20)
        // = 0x43 | 0 | (4 << 20) = 0x00400043
        let header: u32 = 0x43 | (0 << 10) | (4 << 20);
        let mut data = header.to_le_bytes().to_vec();
        data.extend_from_slice(&[b'T', b'e', b's', b't']);

        let mut parser = RecordParser::new(&data);
        let record = parser.parse_next().unwrap();

        assert_eq!(record.tag_id, HWPTAG_PARA_TEXT);
        assert_eq!(record.level, 0);
        assert_eq!(record.size, 4);
    }

    #[test]
    fn test_extract_para_text() {
        // UTF-16LE "Hello"
        let data = vec![
            b'H', 0, b'e', 0, b'l', 0, b'l', 0, b'o', 0,
        ];
        let text = extract_para_text(&data);
        assert_eq!(text, "Hello");
    }

    #[test]
    fn test_korean_text() {
        // UTF-16LE "안녕" (U+C548, U+B155)
        let data = vec![
            0x48, 0xC5, // 안
            0x55, 0xB1, // 녕
        ];
        let text = extract_para_text(&data);
        assert_eq!(text, "안녕");
    }

    #[test]
    fn test_parse_char_shape() {
        // Create a minimal CHAR_SHAPE record data (50 bytes)
        let mut data = vec![0u8; 50];
        // Set Bold + Italic flags at offset 46-49
        // Bold = bit 1 (0x02), Italic = bit 0 (0x01)
        data[46] = 0x03; // Bold + Italic
        data[47] = 0x00;
        data[48] = 0x00;
        data[49] = 0x00;

        let shape = parse_char_shape(&data).unwrap();
        assert!(shape.bold);
        assert!(shape.italic);
        assert!(!shape.underline);
        assert!(!shape.strikeout);
    }

    #[test]
    fn test_parse_char_shape_underline() {
        let mut data = vec![0u8; 50];
        // Underline type 1 at bits 2-3 (0x04)
        data[46] = 0x04;
        data[47] = 0x00;
        data[48] = 0x00;
        data[49] = 0x00;

        let shape = parse_char_shape(&data).unwrap();
        assert!(!shape.bold);
        assert!(!shape.italic);
        assert!(shape.underline);
        assert!(!shape.strikeout);
    }

    #[test]
    fn test_parse_char_shape_strikeout() {
        let mut data = vec![0u8; 50];
        // Strikeout type at bits 18-21 (0x00040000 = bit 18 set)
        data[46] = 0x00;
        data[47] = 0x00;
        data[48] = 0x04; // 0x04 << 16 = 0x00040000
        data[49] = 0x00;

        let shape = parse_char_shape(&data).unwrap();
        assert!(!shape.bold);
        assert!(!shape.italic);
        assert!(!shape.underline);
        assert!(shape.strikeout);
    }

    #[test]
    fn test_parse_para_char_shape() {
        // Create mapping: position 0 -> shape 0, position 5 -> shape 1
        let data = vec![
            0x00, 0x00, 0x00, 0x00, // position 0
            0x00, 0x00, 0x00, 0x00, // shape_id 0
            0x05, 0x00, 0x00, 0x00, // position 5
            0x01, 0x00, 0x00, 0x00, // shape_id 1
        ];

        let mapping = parse_para_char_shape(&data).unwrap();
        assert_eq!(mapping.mappings.len(), 2);
        assert_eq!(mapping.mappings[0], (0, 0));
        assert_eq!(mapping.mappings[1], (5, 1));
    }

    #[test]
    fn test_apply_markdown_formatting() {
        // Test bold
        let style = CharShape { bold: true, italic: false, underline: false, strikeout: false };
        assert_eq!(apply_markdown_formatting("테스트", &style), "**테스트**");

        // Test italic
        let style = CharShape { bold: false, italic: true, underline: false, strikeout: false };
        assert_eq!(apply_markdown_formatting("테스트", &style), "*테스트*");

        // Test bold+italic
        let style = CharShape { bold: true, italic: true, underline: false, strikeout: false };
        assert_eq!(apply_markdown_formatting("테스트", &style), "***테스트***");

        // Test underline
        let style = CharShape { bold: false, italic: false, underline: true, strikeout: false };
        assert_eq!(apply_markdown_formatting("테스트", &style), "<u>테스트</u>");

        // Test strikeout
        let style = CharShape { bold: false, italic: false, underline: false, strikeout: true };
        assert_eq!(apply_markdown_formatting("테스트", &style), "~~테스트~~");

        // Test combined: bold + strikeout
        let style = CharShape { bold: true, italic: false, underline: false, strikeout: true };
        assert_eq!(apply_markdown_formatting("테스트", &style), "**~~테스트~~**");
    }

    #[test]
    fn test_extract_para_text_formatted() {
        // UTF-16LE "Hello World"
        let text_data = vec![
            b'H', 0, b'e', 0, b'l', 0, b'l', 0, b'o', 0,
            b' ', 0,
            b'W', 0, b'o', 0, b'r', 0, b'l', 0, b'd', 0,
        ];

        // Create char shapes: 0 = normal, 1 = bold
        let mut char_shapes = HashMap::new();
        char_shapes.insert(0, CharShape::default());
        char_shapes.insert(1, CharShape { bold: true, italic: false, underline: false, strikeout: false });

        // Mapping: position 0-5 = shape 0 (normal), position 6+ = shape 1 (bold)
        let mapping = ParaCharShapeMapping {
            mappings: vec![(0, 0), (6, 1)],
        };

        let result = extract_para_text_formatted(&text_data, Some(&mapping), &char_shapes);
        assert_eq!(result, "Hello **World**");
    }
}
