//! HWPX parser implementation with table and character formatting support

use std::collections::HashMap;
use std::fs::File;
use std::io::{self, Read};
use std::path::Path;
use zip::ZipArchive;

/// Character style properties
#[derive(Debug, Clone, Default)]
pub struct CharStyle {
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strikeout: bool,
}

/// Image information from HWPX file
#[derive(Debug, Clone)]
pub struct ImageInfo {
    pub id: String,           // image1, image2, ...
    pub path: String,         // BinData/image1.bmp
    pub media_type: String,   // image/bmp, image/png
    pub data: Vec<u8>,        // actual binary data
}

/// HWPX document parser
pub struct HwpxParser {
    archive: ZipArchive<File>,
    char_styles: HashMap<u32, CharStyle>,
}

/// Parsed HWPX document
#[derive(Debug, Clone)]
pub struct HwpxDocument {
    pub version: String,
    pub sections: Vec<String>,
    pub images: Vec<String>,
    pub image_info: Vec<ImageInfo>,
    pub preview_text: String,
    pub tables: Vec<Table>,
}

/// Table structure
#[derive(Debug, Clone)]
pub struct Table {
    pub rows: usize,
    pub cols: usize,
    pub cells: Vec<Vec<String>>,
    pub has_header: bool,
}

impl Table {
    /// Convert table to Markdown format
    pub fn to_markdown(&self) -> String {
        if self.cells.is_empty() || self.cols == 0 {
            return String::new();
        }

        let mut result = String::new();

        for (row_idx, row) in self.cells.iter().enumerate() {
            // Build row
            result.push('|');
            for cell in row {
                let cell_text = cell.trim().replace('\n', " ");
                result.push_str(&format!(" {} |", cell_text));
            }
            result.push('\n');

            // Add separator after header row
            if row_idx == 0 && self.has_header {
                result.push('|');
                for _ in 0..self.cols {
                    result.push_str("---|");
                }
                result.push('\n');
            }
        }

        result
    }
}

impl HwpxParser {
    /// Open an HWPX file
    pub fn open<P: AsRef<Path>>(path: P) -> io::Result<Self> {
        let file = File::open(path)?;
        let archive = ZipArchive::new(file)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        Ok(Self {
            archive,
            char_styles: HashMap::new(),
        })
    }

    /// Parse the HWPX document
    pub fn parse(&mut self) -> io::Result<HwpxDocument> {
        let version = self.read_version()?;
        let preview_text = self.read_preview_text().unwrap_or_default();

        // Parse header.xml for character styles
        self.parse_header_styles()?;

        let (sections, tables) = self.extract_sections_with_tables()?;
        let images = self.list_images();
        
        // Parse manifest and extract image info
        let image_info = self.extract_images_with_data()?;

        Ok(HwpxDocument {
            version,
            sections,
            images,
            image_info,
            preview_text,
            tables,
        })
    }

    /// Parse header.xml to extract character style definitions
    fn parse_header_styles(&mut self) -> io::Result<()> {
        if let Ok(mut file) = self.archive.by_name("Contents/header.xml") {
            let mut content = String::new();
            file.read_to_string(&mut content)?;
            self.char_styles = parse_char_properties(&content);
        }
        Ok(())
    }

    /// Read version info
    fn read_version(&mut self) -> io::Result<String> {
        if let Ok(mut file) = self.archive.by_name("version.xml") {
            let mut content = String::new();
            file.read_to_string(&mut content)?;
            if let Some(start) = content.find("version=\"") {
                let start = start + 9;
                if let Some(end) = content[start..].find('"') {
                    return Ok(content[start..start + end].to_string());
                }
            }
        }
        Ok("unknown".to_string())
    }

    /// Read preview text (fast method)
    fn read_preview_text(&mut self) -> io::Result<String> {
        let mut file = self.archive.by_name("Preview/PrvText.txt")?;
        let mut content = String::new();
        file.read_to_string(&mut content)?;
        Ok(content)
    }

    /// Extract text and tables from all sections
    fn extract_sections_with_tables(&mut self) -> io::Result<(Vec<String>, Vec<Table>)> {
        let mut sections = Vec::new();
        let mut all_tables = Vec::new();
        let mut section_idx = 0;

        loop {
            let section_name = format!("Contents/section{}.xml", section_idx);
            match self.archive.by_name(&section_name) {
                Ok(mut file) => {
                    let mut content = String::new();
                    file.read_to_string(&mut content)?;

                    let (text, tables) = parse_section_xml(&content, &self.char_styles);
                    sections.push(text);
                    all_tables.extend(tables);
                    section_idx += 1;
                }
                Err(_) => break,
            }
        }

        Ok((sections, all_tables))
    }

    /// List all images in BinData
    fn list_images(&self) -> Vec<String> {
        self.archive
            .file_names()
            .filter(|name| name.starts_with("BinData/"))
            .map(|s| s.to_string())
            .collect()
    }

    /// Get section count
    pub fn section_count(&self) -> usize {
        self.archive
            .file_names()
            .filter(|name| name.starts_with("Contents/section") && name.ends_with(".xml"))
            .count()
    }

    /// Check if compressed
    pub fn is_compressed(&self) -> bool {
        true
    }

    /// Check if encrypted
    pub fn is_encrypted(&self) -> bool {
        false
    }

    /// Parse manifest (content.hpf) and extract images with binary data
    fn extract_images_with_data(&mut self) -> io::Result<Vec<ImageInfo>> {
        let mut image_list = Vec::new();
        
        // First, parse the manifest to get image metadata
        if let Ok(mut file) = self.archive.by_name("Contents/content.hpf") {
            let mut content = String::new();
            file.read_to_string(&mut content)?;
            
            // Parse manifest for image items
            // Format: <opf:item id="image1" href="BinData/image1.bmp" media-type="image/bmp" .../>
            let mut pos = 0;
            while let Some(item_start) = content[pos..].find("<opf:item ") {
                let item_pos = pos + item_start;
                if let Some(item_end) = content[item_pos..].find("/>") {
                    let item_xml = &content[item_pos..item_pos + item_end + 2];
                    
                    // Check if this is an image
                    if let Some(href) = extract_attr(item_xml, "href") {
                        if href.starts_with("BinData/") {
                            let id = extract_attr(item_xml, "id").unwrap_or_default();
                            let media_type = extract_attr(item_xml, "media-type").unwrap_or_default();
                            
                            image_list.push((id, href, media_type));
                        }
                    }
                    pos = item_pos + item_end + 2;
                } else {
                    break;
                }
            }
        }
        
        // Now extract the actual image data
        let mut result = Vec::new();
        for (id, path, media_type) in image_list {
            if let Ok(mut file) = self.archive.by_name(&path) {
                let mut data = Vec::new();
                file.read_to_end(&mut data)?;
                result.push(ImageInfo {
                    id,
                    path,
                    media_type,
                    data,
                });
            }
        }
        
        Ok(result)
    }
}

/// Parse character properties from header.xml
fn parse_char_properties(header_xml: &str) -> HashMap<u32, CharStyle> {
    let mut styles = HashMap::new();

    // Find each charPr element
    let mut pos = 0;
    while let Some(start) = header_xml[pos..].find("<hh:charPr ") {
        let char_pr_start = pos + start;

        // Find the end of this charPr element
        if let Some(end) = header_xml[char_pr_start..].find("</hh:charPr>") {
            let char_pr_xml = &header_xml[char_pr_start..char_pr_start + end + 12];

            // Extract id
            if let Some(id) = extract_attr(char_pr_xml, "id") {
                if let Ok(id_num) = id.parse::<u32>() {
                    let mut style = CharStyle::default();

                    // Check for bold
                    style.bold = char_pr_xml.contains("<hh:bold") || char_pr_xml.contains("<hh:bold/>");

                    // Check for italic
                    style.italic = char_pr_xml.contains("<hh:italic") || char_pr_xml.contains("<hh:italic/>");

                    // Check for underline (type != NONE)
                    if let Some(underline_pos) = char_pr_xml.find("<hh:underline ") {
                        let underline_xml = &char_pr_xml[underline_pos..];
                        if let Some(type_val) = extract_attr(underline_xml, "type") {
                            style.underline = type_val != "NONE";
                        }
                    }

                    // Check for strikeout (shape != NONE)
                    if let Some(strike_pos) = char_pr_xml.find("<hh:strikeout ") {
                        let strike_xml = &char_pr_xml[strike_pos..];
                        if let Some(shape_val) = extract_attr(strike_xml, "shape") {
                            style.strikeout = shape_val != "NONE";
                        }
                    }

                    styles.insert(id_num, style);
                }
            }
            pos = char_pr_start + end + 12;
        } else {
            break;
        }
    }

    styles
}

/// Parse section XML and extract text with tables
fn parse_section_xml(xml: &str, char_styles: &HashMap<u32, CharStyle>) -> (String, Vec<Table>) {
    let mut result = String::new();
    let mut tables = Vec::new();
    let mut pos = 0;

    while pos < xml.len() {
        // Look for table start
        if let Some(tbl_start) = xml[pos..].find("<hp:tbl ") {
            let tbl_pos = pos + tbl_start;

            // Extract text before table
            let before_table = &xml[pos..tbl_pos];
            result.push_str(&extract_text_with_formatting(before_table, char_styles));

            // Find table end
            if let Some(tbl_end) = xml[tbl_pos..].find("</hp:tbl>") {
                let table_xml = &xml[tbl_pos..tbl_pos + tbl_end + 9];

                // Parse table
                if let Some(table) = parse_table(table_xml) {
                    // Add table markdown
                    result.push_str("\n\n");
                    result.push_str(&table.to_markdown());
                    result.push('\n');
                    tables.push(table);
                }

                pos = tbl_pos + tbl_end + 9;
            } else {
                pos = tbl_pos + 1;
            }
        } else {
            // No more tables, extract remaining text
            result.push_str(&extract_text_with_formatting(&xml[pos..], char_styles));
            break;
        }
    }

    // Clean up result
    let cleaned = clean_text(&result);
    (cleaned, tables)
}

/// Parse a single table from XML
fn parse_table(xml: &str) -> Option<Table> {
    // Extract row and column count
    let rows = extract_attr(xml, "rowCnt").and_then(|s| s.parse().ok()).unwrap_or(0);
    let cols = extract_attr(xml, "colCnt").and_then(|s| s.parse().ok()).unwrap_or(0);

    if rows == 0 || cols == 0 {
        return None;
    }

    let mut cells: Vec<Vec<String>> = Vec::new();
    let mut has_header = false;
    let mut pos = 0;

    // Parse each row
    while let Some(tr_start) = xml[pos..].find("<hp:tr>") {
        let tr_pos = pos + tr_start;

        if let Some(tr_end) = xml[tr_pos..].find("</hp:tr>") {
            let row_xml = &xml[tr_pos..tr_pos + tr_end];
            let mut row_cells = Vec::new();
            let mut cell_pos = 0;

            // Parse each cell in row
            while let Some(tc_start) = row_xml[cell_pos..].find("<hp:tc ") {
                let tc_pos = cell_pos + tc_start;

                // Check if header cell
                if let Some(header_attr) = extract_attr(&row_xml[tc_pos..], "header") {
                    if header_attr == "1" {
                        has_header = true;
                    }
                }

                if let Some(tc_end) = row_xml[tc_pos..].find("</hp:tc>") {
                    let cell_xml = &row_xml[tc_pos..tc_pos + tc_end];
                    let cell_text = extract_cell_text(cell_xml);
                    row_cells.push(cell_text);
                    cell_pos = tc_pos + tc_end + 8;
                } else {
                    cell_pos = tc_pos + 1;
                }
            }

            if !row_cells.is_empty() {
                // Pad row to match column count
                while row_cells.len() < cols {
                    row_cells.push(String::new());
                }
                cells.push(row_cells);
            }

            pos = tr_pos + tr_end + 8;
        } else {
            break;
        }
    }

    if cells.is_empty() {
        return None;
    }

    Some(Table {
        rows: cells.len(),
        cols,
        cells,
        has_header,
    })
}

/// Extract cell text from cell XML
fn extract_cell_text(xml: &str) -> String {
    let mut result = String::new();
    let mut pos = 0;

    while let Some(t_start) = xml[pos..].find("<hp:t>") {
        let start = pos + t_start + 6;
        if let Some(t_end) = xml[start..].find("</hp:t>") {
            let text = &xml[start..start + t_end];
            if !result.is_empty() && !text.is_empty() {
                result.push(' ');
            }
            result.push_str(text);
            pos = start + t_end + 7;
        } else {
            break;
        }
    }

    // Also handle self-closing <hp:t/>
    if result.is_empty() {
        // Try alternative text extraction
        if let Some(t_start) = xml.find("<hp:t ") {
            let start = t_start;
            if let Some(close) = xml[start..].find('>') {
                let after_tag = start + close + 1;
                if let Some(end) = xml[after_tag..].find("</hp:t>") {
                    result = xml[after_tag..after_tag + end].to_string();
                }
            }
        }
    }

    result.trim().to_string()
}

/// Extract attribute value from XML tag
fn extract_attr(xml: &str, attr: &str) -> Option<String> {
    let pattern = format!("{}=\"", attr);
    if let Some(start) = xml.find(&pattern) {
        let value_start = start + pattern.len();
        if let Some(end) = xml[value_start..].find('"') {
            return Some(xml[value_start..value_start + end].to_string());
        }
    }
    None
}

/// Extract text with formatting from section XML
fn extract_text_with_formatting(xml: &str, char_styles: &HashMap<u32, CharStyle>) -> String {
    let mut result = String::new();
    let mut pos = 0;

    // Process paragraph by paragraph
    while let Some(p_start) = xml[pos..].find("<hp:p") {
        let p_pos = pos + p_start;

        // Find the end of paragraph
        if let Some(p_end) = xml[p_pos..].find("</hp:p>") {
            let para_xml = &xml[p_pos..p_pos + p_end + 7];

            // Extract runs from this paragraph
            let para_text = extract_runs_with_formatting(para_xml, char_styles);
            if !para_text.is_empty() {
                result.push_str(&para_text);
                result.push('\n');
            }

            pos = p_pos + p_end + 7;
        } else {
            break;
        }
    }

    result
}

/// Extract runs with formatting applied
fn extract_runs_with_formatting(para_xml: &str, char_styles: &HashMap<u32, CharStyle>) -> String {
    let mut result = String::new();
    let mut pos = 0;

    while let Some(run_start) = para_xml[pos..].find("<hp:run ") {
        let run_pos = pos + run_start;

        // Get charPrIDRef attribute
        let run_tag_end = para_xml[run_pos..].find('>').unwrap_or(0);
        let run_tag = &para_xml[run_pos..run_pos + run_tag_end];
        let char_pr_id = extract_attr(run_tag, "charPrIDRef")
            .and_then(|s| s.parse::<u32>().ok());

        // Find run content and end
        if let Some(run_end) = para_xml[run_pos..].find("</hp:run>") {
            let run_content = &para_xml[run_pos..run_pos + run_end];

            // Extract text from <hp:t> tags within this run
            let mut text_content = String::new();
            let mut t_pos = 0;

            while let Some(t_start) = run_content[t_pos..].find("<hp:t>") {
                let t_start_pos = t_pos + t_start + 6;
                if let Some(t_end) = run_content[t_start_pos..].find("</hp:t>") {
                    let text = &run_content[t_start_pos..t_start_pos + t_end];
                    text_content.push_str(text);
                    t_pos = t_start_pos + t_end + 7;
                } else {
                    break;
                }
            }

            // Handle <hp:fwSpace/> (fixed-width space)
            if run_content.contains("<hp:fwSpace/>") && text_content.is_empty() {
                text_content.push(' ');
            }

            // Apply formatting if we have text and a valid charPrIDRef
            if !text_content.is_empty() {
                let formatted = if let Some(id) = char_pr_id {
                    if let Some(style) = char_styles.get(&id) {
                        apply_markdown_formatting(&text_content, style)
                    } else {
                        text_content
                    }
                } else {
                    text_content
                };
                result.push_str(&formatted);
            }

            pos = run_pos + run_end + 9;
        } else {
            // Self-closing run or malformed
            pos = run_pos + 1;
        }
    }

    result
}

/// Apply Markdown formatting based on CharStyle
fn apply_markdown_formatting(text: &str, style: &CharStyle) -> String {
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

/// Simple text extraction (for non-table content) - fallback without formatting
fn extract_text_simple(xml: &str) -> String {
    let mut result = String::new();
    let mut in_text_tag = false;
    let mut chars = xml.chars().peekable();
    let mut skip_table = false;

    while let Some(c) = chars.next() {
        if c == '<' {
            let mut tag = String::new();
            while let Some(&next) = chars.peek() {
                if next == '>' {
                    chars.next();
                    break;
                }
                tag.push(chars.next().unwrap());
            }

            // Skip table content (handled separately)
            if tag.starts_with("hp:tbl ") {
                skip_table = true;
            } else if tag == "/hp:tbl" {
                skip_table = false;
            } else if !skip_table {
                if tag == "hp:t" || tag.starts_with("hp:t ") {
                    in_text_tag = true;
                } else if tag == "/hp:t" {
                    in_text_tag = false;
                } else if tag == "/hp:p" {
                    result.push('\n');
                }
            }
        } else if in_text_tag && !skip_table {
            result.push(c);
        }
    }

    result
}

/// Remove XML tags of a given name from text
fn remove_xml_tags(text: &str, tag_name: &str) -> String {
    let mut result = text.to_string();
    let open_pattern = format!("<{} ", tag_name);
    let self_close = "/>";
    let close_pattern = format!("</{}>", tag_name);

    // Remove self-closing tags like <hp:tab ... />
    loop {
        if let Some(start) = result.find(&open_pattern) {
            if let Some(end) = result[start..].find(self_close) {
                result = format!("{} {}", &result[..start], &result[start + end + 2..]);
                continue;
            } else if let Some(end) = result[start..].find('>') {
                // Tag ends with > but may have closing tag later
                if let Some(close) = result[start + end..].find(&close_pattern) {
                    result = format!(
                        "{} {}",
                        &result[..start],
                        &result[start + end + 1 + close + close_pattern.len()..]
                    );
                    continue;
                }
            }
        }
        break;
    }

    result
}

/// Clean up extracted text
fn clean_text(text: &str) -> String {
    let mut cleaned = String::new();
    let mut prev_newline_count = 0;

    // Replace any remaining XML tags
    let text = text.replace("<hp:fwSpace/>", " ");
    // Remove tab tags (but preserve their presence as a space)
    let text = remove_xml_tags(&text, "hp:tab");

    for c in text.chars() {
        if c == '\n' {
            prev_newline_count += 1;
            if prev_newline_count <= 2 {
                cleaned.push(c);
            }
        } else if c.is_whitespace() {
            if !cleaned.ends_with(' ') && !cleaned.ends_with('\n') {
                cleaned.push(' ');
            }
            prev_newline_count = 0;
        } else {
            cleaned.push(c);
            prev_newline_count = 0;
        }
    }

    // Clean up redundant formatting markers
    let cleaned = cleaned.replace("******", "");
    let cleaned = cleaned.replace("****", "");
    let cleaned = cleaned.replace("** **", " ");
    let cleaned = cleaned.replace("**  **", " ");

    cleaned.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_table_to_markdown() {
        let table = Table {
            rows: 2,
            cols: 3,
            cells: vec![
                vec!["헤더1".to_string(), "헤더2".to_string(), "헤더3".to_string()],
                vec!["데이터1".to_string(), "데이터2".to_string(), "데이터3".to_string()],
            ],
            has_header: true,
        };

        let md = table.to_markdown();
        assert!(md.contains("| 헤더1 |"));
        assert!(md.contains("|---|---|---|"));
        assert!(md.contains("| 데이터1 |"));
    }

    #[test]
    fn test_extract_cell_text() {
        let xml = r#"<hp:tc><hp:subList><hp:p><hp:run><hp:t>테스트</hp:t></hp:run></hp:p></hp:subList></hp:tc>"#;
        let text = extract_cell_text(xml);
        assert_eq!(text, "테스트");
    }

    #[test]
    fn test_extract_attr() {
        let xml = r#"<hp:tbl rowCnt="5" colCnt="3">"#;
        assert_eq!(extract_attr(xml, "rowCnt"), Some("5".to_string()));
        assert_eq!(extract_attr(xml, "colCnt"), Some("3".to_string()));
    }

    #[test]
    fn test_apply_markdown_formatting() {
        // Test bold
        let style = CharStyle {
            bold: true,
            italic: false,
            underline: false,
            strikeout: false,
        };
        assert_eq!(apply_markdown_formatting("테스트", &style), "**테스트**");

        // Test italic
        let style = CharStyle {
            bold: false,
            italic: true,
            underline: false,
            strikeout: false,
        };
        assert_eq!(apply_markdown_formatting("테스트", &style), "*테스트*");

        // Test bold+italic
        let style = CharStyle {
            bold: true,
            italic: true,
            underline: false,
            strikeout: false,
        };
        assert_eq!(apply_markdown_formatting("테스트", &style), "***테스트***");

        // Test underline
        let style = CharStyle {
            bold: false,
            italic: false,
            underline: true,
            strikeout: false,
        };
        assert_eq!(apply_markdown_formatting("테스트", &style), "<u>테스트</u>");

        // Test strikeout
        let style = CharStyle {
            bold: false,
            italic: false,
            underline: false,
            strikeout: true,
        };
        assert_eq!(apply_markdown_formatting("테스트", &style), "~~테스트~~");

        // Test combined: bold + underline + strikeout
        let style = CharStyle {
            bold: true,
            italic: false,
            underline: true,
            strikeout: true,
        };
        assert_eq!(
            apply_markdown_formatting("테스트", &style),
            "<u>**~~테스트~~**</u>"
        );
    }

    #[test]
    fn test_parse_char_properties() {
        let header_xml = r#"
            <hh:charPr id="0" height="1000">
                <hh:underline type="NONE"/>
                <hh:strikeout shape="NONE"/>
            </hh:charPr>
            <hh:charPr id="7" height="1100">
                <hh:bold/>
                <hh:underline type="NONE"/>
                <hh:strikeout shape="NONE"/>
            </hh:charPr>
            <hh:charPr id="28" height="2200">
                <hh:bold/>
                <hh:underline type="BOTTOM"/>
                <hh:strikeout shape="NONE"/>
            </hh:charPr>
            <hh:charPr id="99" height="1000">
                <hh:underline type="NONE"/>
                <hh:strikeout shape="3D"/>
            </hh:charPr>
        "#;

        let styles = parse_char_properties(header_xml);

        // id 0: no formatting
        let style0 = styles.get(&0).unwrap();
        assert!(!style0.bold);
        assert!(!style0.underline);
        assert!(!style0.strikeout);

        // id 7: bold only
        let style7 = styles.get(&7).unwrap();
        assert!(style7.bold);
        assert!(!style7.underline);
        assert!(!style7.strikeout);

        // id 28: bold + underline
        let style28 = styles.get(&28).unwrap();
        assert!(style28.bold);
        assert!(style28.underline);
        assert!(!style28.strikeout);

        // id 99: strikeout only
        let style99 = styles.get(&99).unwrap();
        assert!(!style99.bold);
        assert!(!style99.underline);
        assert!(style99.strikeout);
    }

    #[test]
    fn test_remove_xml_tags() {
        let text = "앞<hp:tab width=\"100\" type=\"1\"/>뒤";
        let result = remove_xml_tags(text, "hp:tab");
        assert!(result.contains("앞"));
        assert!(result.contains("뒤"));
        assert!(!result.contains("<hp:tab"));
    }
}
