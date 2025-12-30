use super::ole::OleReader;
use super::record::{
    HwpRecord, RecordParser, extract_para_text, parse_table_info,
    parse_char_shape, parse_para_char_shape, extract_para_text_formatted,
    parse_cell_list_header, CellSpan,
    CharShape, ParaCharShapeMapping,
    HWPTAG_PARA_TEXT, HWPTAG_PARA_HEADER, HWPTAG_TABLE, HWPTAG_LIST_HEADER,
    HWPTAG_PARA_CHAR_SHAPE, HWPTAG_CHAR_SHAPE,
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

    /// Parse records from decompressed section data with formatting
    fn parse_section_records_formatted(&self, data: &[u8]) -> String {
        let mut parser = RecordParser::new(data);
        let records = parser.parse_all();

        let mut paragraphs = Vec::new();

        // Group records by paragraph: PARA_HEADER -> PARA_TEXT, PARA_CHAR_SHAPE, etc.
        let mut current_text_data: Option<Vec<u8>> = None;
        let mut current_char_shape_mapping: Option<ParaCharShapeMapping> = None;

        for record in &records {
            match record.tag_id {
                HWPTAG_PARA_HEADER => {
                    // New paragraph starts - flush previous if any
                    if let Some(text_data) = current_text_data.take() {
                        let text = extract_para_text_formatted(
                            &text_data,
                            current_char_shape_mapping.as_ref(),
                            &self.char_shapes,
                        );
                        if !text.trim().is_empty() {
                            paragraphs.push(text);
                        }
                        current_char_shape_mapping = None;
                    }
                }
                HWPTAG_PARA_TEXT => {
                    current_text_data = Some(record.data.clone());
                }
                HWPTAG_PARA_CHAR_SHAPE => {
                    current_char_shape_mapping = parse_para_char_shape(&record.data);
                }
                _ => {}
            }
        }

        // Flush last paragraph
        if let Some(text_data) = current_text_data {
            let text = extract_para_text_formatted(
                &text_data,
                current_char_shape_mapping.as_ref(),
                &self.char_shapes,
            );
            if !text.trim().is_empty() {
                paragraphs.push(text);
            }
        }

        paragraphs.join("\n")
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

/// Organize flat cell list into rows
fn organize_cells(cells: &[String], cols: usize) -> Vec<Vec<String>> {
    if cols == 0 {
        return Vec::new();
    }
    
    cells
        .chunks(cols)
        .map(|chunk| chunk.to_vec())
        .collect()
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
#[derive(Debug, Clone)]
pub struct TableData {
    pub rows: usize,
    pub cols: usize,
    pub cells: Vec<Vec<String>>,
    /// Cell span information for merged cells
    pub cell_spans: Vec<CellSpan>,
}

impl TableData {
    /// Convert table to Markdown format
    pub fn to_markdown(&self) -> String {
        if self.cells.is_empty() || self.cols == 0 {
            return String::new();
        }
        
        let mut md = String::new();
        
        for (i, row) in self.cells.iter().enumerate() {
            md.push_str("| ");
            for cell in row {
                md.push_str(&cell.replace('\n', " ").trim().to_string());
                md.push_str(" | ");
            }
            md.push('\n');
            
            // Header separator after first row
            if i == 0 {
                md.push_str("| ");
                for _ in 0..self.cols {
                    md.push_str("--- | ");
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
