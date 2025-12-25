//! HWP Record parsing utilities
//! 
//! HWP 5.0 uses a tag-based record structure where each record has:
//! - Tag ID (10 bits): identifies the type of record
//! - Level (10 bits): nesting level
//! - Size (12 bits): data size (or 0xFFF for extended size)

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
            // Skip control characters and inline controls
            0..=8 | 0x10..=0x1F => {
                // Extended control characters take 8 bytes total (16 bytes including char)
                if char_code == CHAR_INLINE_CTRL_START 
                    || char_code == CHAR_SECTION_DEF
                    || char_code == CHAR_FIELD_START
                    || char_code == CHAR_TABLE
                    || char_code == CHAR_DRAWING
                {
                    // Skip 14 more bytes (16 total for inline control)
                    i += 14;
                }
            }
            CHAR_TAB => result.push('\t'),
            CHAR_LINE_BREAK => result.push('\n'),
            CHAR_PARA_BREAK => result.push('\n'),
            CHAR_SPACE => result.push(' '),
            CHAR_HYPHEN => result.push('-'),
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

/// Parse table structure from TABLE record
#[derive(Debug, Clone)]
pub struct TableInfo {
    pub rows: u16,
    pub cols: u16,
    pub cell_count: u16,
}

pub fn parse_table_info(data: &[u8]) -> Option<TableInfo> {
    if data.len() < 8 {
        return None;
    }

    // Table record structure (simplified):
    // - Flags: 4 bytes
    // - Row count: 2 bytes
    // - Col count: 2 bytes
    let rows = u16::from_le_bytes([data[4], data[5]]);
    let cols = u16::from_le_bytes([data[6], data[7]]);

    Some(TableInfo {
        rows,
        cols,
        cell_count: rows * cols,
    })
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
}
