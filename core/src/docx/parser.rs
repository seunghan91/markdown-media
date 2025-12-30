use std::fs::File;
use std::io::{self, Read, BufReader, Write};
use std::path::{Path, PathBuf};
use std::collections::HashMap;
use quick_xml::Reader;
use quick_xml::events::Event;
use serde::{Serialize, Deserialize};

// Word XML namespaces
const WORD_NS: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";
const REL_NS: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const DC_NS: &str = "http://purl.org/dc/elements/1.1/";

/// Text run with formatting
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextRun {
    pub text: String,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strike: bool,
    pub font_size: Option<u32>,
    pub font_name: Option<String>,
    pub color: Option<String>,
}

impl Default for TextRun {
    fn default() -> Self {
        TextRun {
            text: String::new(),
            bold: false,
            italic: false,
            underline: false,
            strike: false,
            font_size: None,
            font_name: None,
            color: None,
        }
    }
}

impl TextRun {
    /// Convert to markdown formatted text
    pub fn to_markdown(&self) -> String {
        let mut result = self.text.clone();

        if self.bold && self.italic {
            result = format!("***{}***", result);
        } else if self.bold {
            result = format!("**{}**", result);
        } else if self.italic {
            result = format!("*{}*", result);
        }

        if self.strike {
            result = format!("~~{}~~", result);
        }

        result
    }
}

/// Paragraph with style information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Paragraph {
    pub runs: Vec<TextRun>,
    pub style: Option<String>,
    pub alignment: Option<String>,
    pub indent_level: u32,
    pub is_list_item: bool,
    pub list_type: Option<String>, // "bullet" or "number"
}

impl Default for Paragraph {
    fn default() -> Self {
        Paragraph {
            runs: Vec::new(),
            style: None,
            alignment: None,
            indent_level: 0,
            is_list_item: false,
            list_type: None,
        }
    }
}

impl Paragraph {
    /// Get plain text content
    pub fn text(&self) -> String {
        self.runs.iter().map(|r| r.text.as_str()).collect()
    }

    /// Convert to markdown
    pub fn to_markdown(&self) -> String {
        let content: String = self.runs.iter().map(|r| r.to_markdown()).collect();

        if content.trim().is_empty() {
            return String::new();
        }

        // Handle headings
        if let Some(ref style) = self.style {
            match style.as_str() {
                "Heading1" | "1" => return format!("# {}", content),
                "Heading2" | "2" => return format!("## {}", content),
                "Heading3" | "3" => return format!("### {}", content),
                "Heading4" | "4" => return format!("#### {}", content),
                "Title" => return format!("# {}", content),
                "Subtitle" => return format!("## {}", content),
                _ => {}
            }
        }

        // Handle list items
        if self.is_list_item {
            let indent = "  ".repeat(self.indent_level as usize);
            return match self.list_type.as_deref() {
                Some("number") => format!("{}1. {}", indent, content),
                _ => format!("{}- {}", indent, content),
            };
        }

        content
    }
}

/// Table cell
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableCell {
    pub content: String,
    pub col_span: u32,
    pub row_span: u32,
}

impl Default for TableCell {
    fn default() -> Self {
        TableCell {
            content: String::new(),
            col_span: 1,
            row_span: 1,
        }
    }
}

/// Table structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocxTable {
    pub rows: Vec<Vec<TableCell>>,
    pub has_header: bool,
}

impl DocxTable {
    /// Convert to markdown table
    pub fn to_markdown(&self) -> String {
        if self.rows.is_empty() {
            return String::new();
        }

        let mut lines = Vec::new();

        for (i, row) in self.rows.iter().enumerate() {
            let cells: Vec<String> = row.iter()
                .map(|c| c.content.replace("|", "\\|").replace("\n", " "))
                .collect();

            lines.push(format!("| {} |", cells.join(" | ")));

            // Add separator after header row
            if i == 0 {
                let sep: Vec<String> = row.iter().map(|_| "---".to_string()).collect();
                lines.push(format!("| {} |", sep.join(" | ")));
            }
        }

        lines.join("\n")
    }
}

/// Image reference
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocxImage {
    pub id: String,
    pub filename: String,
    pub path: String,
    pub alt_text: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub data: Option<Vec<u8>>,
}

/// Document metadata
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DocxMetadata {
    pub title: Option<String>,
    pub author: Option<String>,
    pub subject: Option<String>,
    pub created: Option<String>,
    pub modified: Option<String>,
    pub revision: Option<u32>,
    pub word_count: Option<u32>,
    pub page_count: Option<u32>,
}

/// Complete DOCX document
#[derive(Debug, Serialize, Deserialize)]
pub struct DocxDocument {
    pub paragraphs: Vec<Paragraph>,
    pub tables: Vec<DocxTable>,
    pub images: Vec<DocxImage>,
    pub metadata: DocxMetadata,
}

impl DocxDocument {
    /// Get plain text content
    pub fn text(&self) -> String {
        self.paragraphs.iter()
            .map(|p| p.text())
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Convert to markdown
    pub fn to_markdown(&self) -> String {
        let mut parts = Vec::new();

        for para in &self.paragraphs {
            let md = para.to_markdown();
            if !md.is_empty() {
                parts.push(md);
            }
        }

        parts.join("\n\n")
    }

    /// Convert to MDX format with frontmatter
    pub fn to_mdx(&self, source_filename: &str) -> String {
        let mut output = String::new();

        // Frontmatter
        output.push_str("---\n");
        if let Some(ref title) = self.metadata.title {
            output.push_str(&format!("title: \"{}\"\n", title.replace("\"", "\\\"")));
        }
        if let Some(ref author) = self.metadata.author {
            output.push_str(&format!("author: \"{}\"\n", author.replace("\"", "\\\"")));
        }
        output.push_str(&format!("source: \"{}\"\n", source_filename));
        output.push_str(&format!("format: docx\n"));
        output.push_str("---\n\n");

        // Content
        output.push_str(&self.to_markdown());

        // Tables
        for (i, table) in self.tables.iter().enumerate() {
            output.push_str(&format!("\n\n<!-- Table {} -->\n", i + 1));
            output.push_str(&table.to_markdown());
        }

        output
    }
}

/// DOCX Parser
pub struct DocxParser {
    archive: zip::ZipArchive<BufReader<File>>,
    path: PathBuf,
    relationships: HashMap<String, String>,
    styles: HashMap<String, String>,
    numbering: HashMap<String, String>,
}

impl DocxParser {
    /// Open a DOCX file
    pub fn open<P: AsRef<Path>>(path: P) -> io::Result<Self> {
        let path_buf = path.as_ref().to_path_buf();
        let file = File::open(&path_buf)?;
        let reader = BufReader::new(file);

        let archive = zip::ZipArchive::new(reader)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, format!("Invalid DOCX: {}", e)))?;

        let mut parser = DocxParser {
            archive,
            path: path_buf,
            relationships: HashMap::new(),
            styles: HashMap::new(),
            numbering: HashMap::new(),
        };

        parser.load_relationships()?;
        parser.load_styles()?;

        Ok(parser)
    }

    /// Load document relationships
    fn load_relationships(&mut self) -> io::Result<()> {
        let content = match self.read_archive_file("word/_rels/document.xml.rels") {
            Ok(c) => c,
            Err(_) => return Ok(()), // No relationships file
        };

        let mut reader = Reader::from_str(&content);
        reader.trim_text(true);

        loop {
            match reader.read_event() {
                Ok(Event::Empty(ref e)) | Ok(Event::Start(ref e)) => {
                    if e.local_name().as_ref() == b"Relationship" {
                        let mut id = String::new();
                        let mut target = String::new();

                        for attr in e.attributes().flatten() {
                            match attr.key.local_name().as_ref() {
                                b"Id" => id = String::from_utf8_lossy(&attr.value).to_string(),
                                b"Target" => target = String::from_utf8_lossy(&attr.value).to_string(),
                                _ => {}
                            }
                        }

                        if !id.is_empty() && !target.is_empty() {
                            self.relationships.insert(id, target);
                        }
                    }
                }
                Ok(Event::Eof) => break,
                Err(e) => return Err(io::Error::new(io::ErrorKind::InvalidData, e.to_string())),
                _ => {}
            }
        }

        Ok(())
    }

    /// Load document styles
    fn load_styles(&mut self) -> io::Result<()> {
        let content = match self.read_archive_file("word/styles.xml") {
            Ok(c) => c,
            Err(_) => return Ok(()),
        };

        let mut reader = Reader::from_str(&content);
        reader.trim_text(true);

        let mut current_style_id = String::new();
        let mut current_style_name = String::new();
        let mut in_style = false;

        loop {
            match reader.read_event() {
                Ok(Event::Start(ref e)) => {
                    if e.local_name().as_ref() == b"style" {
                        in_style = true;
                        for attr in e.attributes().flatten() {
                            if attr.key.local_name().as_ref() == b"styleId" {
                                current_style_id = String::from_utf8_lossy(&attr.value).to_string();
                            }
                        }
                    } else if in_style && e.local_name().as_ref() == b"name" {
                        for attr in e.attributes().flatten() {
                            if attr.key.local_name().as_ref() == b"val" {
                                current_style_name = String::from_utf8_lossy(&attr.value).to_string();
                            }
                        }
                    }
                }
                Ok(Event::End(ref e)) => {
                    if e.local_name().as_ref() == b"style" {
                        if !current_style_id.is_empty() {
                            self.styles.insert(current_style_id.clone(), current_style_name.clone());
                        }
                        current_style_id.clear();
                        current_style_name.clear();
                        in_style = false;
                    }
                }
                Ok(Event::Eof) => break,
                Err(_) => break,
                _ => {}
            }
        }

        Ok(())
    }

    /// Read a file from the archive
    fn read_archive_file(&mut self, name: &str) -> io::Result<String> {
        let mut file = self.archive.by_name(name)
            .map_err(|e| io::Error::new(io::ErrorKind::NotFound, e.to_string()))?;

        let mut content = String::new();
        file.read_to_string(&mut content)?;
        Ok(content)
    }

    /// Parse the document
    pub fn parse(&mut self) -> io::Result<DocxDocument> {
        let content = self.read_archive_file("word/document.xml")?;

        let mut paragraphs = Vec::new();
        let mut tables = Vec::new();

        let mut reader = Reader::from_str(&content);
        reader.trim_text(true);

        let mut current_para = Paragraph::default();
        let mut current_run = TextRun::default();
        let mut current_table: Option<DocxTable> = None;
        let mut current_row: Vec<TableCell> = Vec::new();
        let mut current_cell: Option<TableCell> = None;

        let mut in_paragraph = false;
        let mut in_run = false;
        let mut in_text = false;
        let mut in_table = false;
        let mut in_table_row = false;
        let mut in_table_cell = false;
        let mut in_num_pr = false;  // Inside numbering properties

        loop {
            match reader.read_event() {
                Ok(Event::Start(ref e)) => {
                    match e.local_name().as_ref() {
                        b"p" => {
                            in_paragraph = true;
                            current_para = Paragraph::default();
                        }
                        b"r" if in_paragraph => {
                            in_run = true;
                            current_run = TextRun::default();
                        }
                        b"t" if in_run => {
                            in_text = true;
                        }
                        b"tbl" => {
                            in_table = true;
                            current_table = Some(DocxTable {
                                rows: Vec::new(),
                                has_header: true,
                            });
                        }
                        b"tr" if in_table => {
                            in_table_row = true;
                            current_row = Vec::new();
                        }
                        b"tc" if in_table_row => {
                            in_table_cell = true;
                            current_cell = Some(TableCell::default());
                        }
                        b"pStyle" if in_paragraph => {
                            for attr in e.attributes().flatten() {
                                if attr.key.local_name().as_ref() == b"val" {
                                    let style_id = String::from_utf8_lossy(&attr.value).to_string();
                                    current_para.style = self.styles.get(&style_id).cloned()
                                        .or(Some(style_id));
                                }
                            }
                        }
                        b"b" if in_run => {
                            // Bold - check for val="0" or val="false"
                            let mut is_off = false;
                            for attr in e.attributes().flatten() {
                                if attr.key.local_name().as_ref() == b"val" {
                                    let val = String::from_utf8_lossy(&attr.value);
                                    is_off = val == "0" || val == "false";
                                }
                            }
                            if !is_off {
                                current_run.bold = true;
                            }
                        }
                        b"i" if in_run => {
                            let mut is_off = false;
                            for attr in e.attributes().flatten() {
                                if attr.key.local_name().as_ref() == b"val" {
                                    let val = String::from_utf8_lossy(&attr.value);
                                    is_off = val == "0" || val == "false";
                                }
                            }
                            if !is_off {
                                current_run.italic = true;
                            }
                        }
                        b"u" if in_run => {
                            current_run.underline = true;
                        }
                        b"strike" if in_run => {
                            current_run.strike = true;
                        }
                        b"sz" if in_run => {
                            for attr in e.attributes().flatten() {
                                if attr.key.local_name().as_ref() == b"val" {
                                    if let Ok(size) = String::from_utf8_lossy(&attr.value).parse::<u32>() {
                                        current_run.font_size = Some(size / 2); // Half-points to points
                                    }
                                }
                            }
                        }
                        b"gridSpan" if in_table_cell => {
                            for attr in e.attributes().flatten() {
                                if attr.key.local_name().as_ref() == b"val" {
                                    if let Ok(span) = String::from_utf8_lossy(&attr.value).parse::<u32>() {
                                        if let Some(ref mut cell) = current_cell {
                                            cell.col_span = span;
                                        }
                                    }
                                }
                            }
                        }
                        b"numPr" if in_paragraph => {
                            // Start of numbering properties - this paragraph is a list item
                            in_num_pr = true;
                            current_para.is_list_item = true;
                        }
                        b"ilvl" if in_num_pr => {
                            // Indent level for list
                            for attr in e.attributes().flatten() {
                                if attr.key.local_name().as_ref() == b"val" {
                                    if let Ok(level) = String::from_utf8_lossy(&attr.value).parse::<u32>() {
                                        current_para.indent_level = level;
                                    }
                                }
                            }
                        }
                        b"numId" if in_num_pr => {
                            // Numbering ID - determines bullet vs numbered list
                            for attr in e.attributes().flatten() {
                                if attr.key.local_name().as_ref() == b"val" {
                                    let num_id = String::from_utf8_lossy(&attr.value).to_string();
                                    // numId "0" means no numbering, "1" is typically bullet, "2+" is numbered
                                    // We'll use a simple heuristic: even IDs are bullets, odd are numbered
                                    // More accurate would require parsing numbering.xml
                                    if num_id != "0" {
                                        // Check if we have numbering definition
                                        if let Some(list_type) = self.numbering.get(&num_id) {
                                            current_para.list_type = Some(list_type.clone());
                                        } else {
                                            // Default heuristic: numId "1" is typically bullet
                                            current_para.list_type = Some(
                                                if num_id == "1" { "bullet".to_string() }
                                                else { "number".to_string() }
                                            );
                                        }
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                }
                Ok(Event::Empty(ref e)) => {
                    match e.local_name().as_ref() {
                        b"b" if in_run => {
                            let mut is_off = false;
                            for attr in e.attributes().flatten() {
                                if attr.key.local_name().as_ref() == b"val" {
                                    let val = String::from_utf8_lossy(&attr.value);
                                    is_off = val == "0" || val == "false";
                                }
                            }
                            if !is_off {
                                current_run.bold = true;
                            }
                        }
                        b"i" if in_run => {
                            let mut is_off = false;
                            for attr in e.attributes().flatten() {
                                if attr.key.local_name().as_ref() == b"val" {
                                    let val = String::from_utf8_lossy(&attr.value);
                                    is_off = val == "0" || val == "false";
                                }
                            }
                            if !is_off {
                                current_run.italic = true;
                            }
                        }
                        b"u" if in_run => {
                            current_run.underline = true;
                        }
                        b"strike" if in_run => {
                            current_run.strike = true;
                        }
                        b"pStyle" if in_paragraph => {
                            for attr in e.attributes().flatten() {
                                if attr.key.local_name().as_ref() == b"val" {
                                    let style_id = String::from_utf8_lossy(&attr.value).to_string();
                                    current_para.style = self.styles.get(&style_id).cloned()
                                        .or(Some(style_id));
                                }
                            }
                        }
                        b"ilvl" if in_num_pr => {
                            // Indent level (self-closing tag)
                            for attr in e.attributes().flatten() {
                                if attr.key.local_name().as_ref() == b"val" {
                                    if let Ok(level) = String::from_utf8_lossy(&attr.value).parse::<u32>() {
                                        current_para.indent_level = level;
                                    }
                                }
                            }
                        }
                        b"numId" if in_num_pr => {
                            // Numbering ID (self-closing tag)
                            for attr in e.attributes().flatten() {
                                if attr.key.local_name().as_ref() == b"val" {
                                    let num_id = String::from_utf8_lossy(&attr.value).to_string();
                                    if num_id != "0" {
                                        if let Some(list_type) = self.numbering.get(&num_id) {
                                            current_para.list_type = Some(list_type.clone());
                                        } else {
                                            current_para.list_type = Some(
                                                if num_id == "1" { "bullet".to_string() }
                                                else { "number".to_string() }
                                            );
                                        }
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                }
                Ok(Event::Text(ref e)) => {
                    if in_text {
                        let text = e.unescape().unwrap_or_default().to_string();
                        current_run.text.push_str(&text);

                        // Also add to cell content if in table
                        if in_table_cell {
                            if let Some(ref mut cell) = current_cell {
                                cell.content.push_str(&text);
                            }
                        }
                    }
                }
                Ok(Event::End(ref e)) => {
                    match e.local_name().as_ref() {
                        b"t" => {
                            in_text = false;
                        }
                        b"r" => {
                            if !current_run.text.is_empty() {
                                current_para.runs.push(current_run.clone());
                            }
                            current_run = TextRun::default();
                            in_run = false;
                        }
                        b"p" => {
                            if !in_table_cell {
                                paragraphs.push(current_para.clone());
                            }
                            current_para = Paragraph::default();
                            in_paragraph = false;
                            in_num_pr = false;  // Reset numPr state when paragraph ends
                        }
                        b"numPr" => {
                            in_num_pr = false;
                        }
                        b"tc" => {
                            if let Some(cell) = current_cell.take() {
                                current_row.push(cell);
                            }
                            in_table_cell = false;
                        }
                        b"tr" => {
                            if let Some(ref mut table) = current_table {
                                table.rows.push(current_row.clone());
                            }
                            current_row = Vec::new();
                            in_table_row = false;
                        }
                        b"tbl" => {
                            if let Some(table) = current_table.take() {
                                tables.push(table);
                            }
                            in_table = false;
                        }
                        _ => {}
                    }
                }
                Ok(Event::Eof) => break,
                Err(e) => {
                    return Err(io::Error::new(io::ErrorKind::InvalidData, e.to_string()));
                }
                _ => {}
            }
        }

        // Extract images
        let images = self.extract_images()?;

        // Extract metadata
        let metadata = self.extract_metadata()?;

        Ok(DocxDocument {
            paragraphs,
            tables,
            images,
            metadata,
        })
    }

    /// Extract images from the document
    pub fn extract_images(&mut self) -> io::Result<Vec<DocxImage>> {
        let mut images = Vec::new();

        for (id, target) in &self.relationships.clone() {
            let lower_target = target.to_lowercase();
            if lower_target.ends_with(".png") ||
               lower_target.ends_with(".jpg") ||
               lower_target.ends_with(".jpeg") ||
               lower_target.ends_with(".gif") ||
               lower_target.ends_with(".bmp") {

                let path = if target.starts_with('/') {
                    target[1..].to_string()
                } else {
                    format!("word/{}", target)
                };

                let filename = Path::new(target)
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| format!("{}.png", id));

                // Try to read image data
                let data = self.read_archive_file_bytes(&path).ok();

                images.push(DocxImage {
                    id: id.clone(),
                    filename: filename.clone(),
                    path: path.clone(),
                    alt_text: None,
                    width: None,
                    height: None,
                    data,
                });
            }
        }

        Ok(images)
    }

    /// Read binary file from archive
    fn read_archive_file_bytes(&mut self, name: &str) -> io::Result<Vec<u8>> {
        let mut file = self.archive.by_name(name)
            .map_err(|e| io::Error::new(io::ErrorKind::NotFound, e.to_string()))?;

        let mut data = Vec::new();
        file.read_to_end(&mut data)?;
        Ok(data)
    }

    /// Extract metadata from core.xml
    pub fn extract_metadata(&mut self) -> io::Result<DocxMetadata> {
        let mut metadata = DocxMetadata::default();

        // Try to read core.xml
        let content = match self.read_archive_file("docProps/core.xml") {
            Ok(c) => c,
            Err(_) => return Ok(metadata),
        };

        let mut reader = Reader::from_str(&content);
        reader.trim_text(true);

        let mut current_element = String::new();

        loop {
            match reader.read_event() {
                Ok(Event::Start(ref e)) => {
                    current_element = String::from_utf8_lossy(e.local_name().as_ref()).to_string();
                }
                Ok(Event::Text(ref e)) => {
                    let text = e.unescape().unwrap_or_default().to_string();
                    match current_element.as_str() {
                        "title" => metadata.title = Some(text),
                        "creator" => metadata.author = Some(text),
                        "subject" => metadata.subject = Some(text),
                        "created" => metadata.created = Some(text),
                        "modified" => metadata.modified = Some(text),
                        "revision" => metadata.revision = text.parse().ok(),
                        _ => {}
                    }
                }
                Ok(Event::End(_)) => {
                    current_element.clear();
                }
                Ok(Event::Eof) => break,
                Err(_) => break,
                _ => {}
            }
        }

        // Try to read app.xml for word count
        if let Ok(app_content) = self.read_archive_file("docProps/app.xml") {
            let mut app_reader = Reader::from_str(&app_content);
            app_reader.trim_text(true);

            let mut current_el = String::new();

            loop {
                match app_reader.read_event() {
                    Ok(Event::Start(ref e)) => {
                        current_el = String::from_utf8_lossy(e.local_name().as_ref()).to_string();
                    }
                    Ok(Event::Text(ref e)) => {
                        let text = e.unescape().unwrap_or_default().to_string();
                        match current_el.as_str() {
                            "Words" => metadata.word_count = text.parse().ok(),
                            "Pages" => metadata.page_count = text.parse().ok(),
                            _ => {}
                        }
                    }
                    Ok(Event::End(_)) => current_el.clear(),
                    Ok(Event::Eof) => break,
                    Err(_) => break,
                    _ => {}
                }
            }
        }

        Ok(metadata)
    }

    /// Extract text only
    pub fn extract_text(&mut self) -> io::Result<String> {
        let doc = self.parse()?;
        Ok(doc.text())
    }

    /// Convert to MDX and save
    pub fn to_mdx(&mut self, output_dir: &Path) -> io::Result<PathBuf> {
        let doc = self.parse()?;

        // Create output directory
        std::fs::create_dir_all(output_dir)?;

        // Create assets directory
        let assets_dir = output_dir.join("assets");
        std::fs::create_dir_all(&assets_dir)?;

        // Get source filename
        let source_name = self.path.file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "document.docx".to_string());

        let stem = self.path.file_stem()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "document".to_string());

        // Save images
        for image in &doc.images {
            if let Some(ref data) = image.data {
                let img_path = assets_dir.join(&image.filename);
                let mut file = File::create(&img_path)?;
                file.write_all(data)?;
            }
        }

        // Generate MDX content
        let mdx_content = doc.to_mdx(&source_name);

        // Save MDX file
        let mdx_path = output_dir.join(format!("{}.mdx", stem));
        let mut mdx_file = File::create(&mdx_path)?;
        mdx_file.write_all(mdx_content.as_bytes())?;

        // Generate MDM manifest
        let mdm_content = self.generate_mdm_manifest(&doc, &source_name);
        let mdm_path = output_dir.join(format!("{}.mdm", stem));
        let mut mdm_file = File::create(&mdm_path)?;
        mdm_file.write_all(mdm_content.as_bytes())?;

        Ok(mdx_path)
    }

    /// Generate MDM manifest JSON
    fn generate_mdm_manifest(&self, doc: &DocxDocument, source_name: &str) -> String {
        let mut resources = serde_json::Map::new();

        for image in &doc.images {
            let mut resource = serde_json::Map::new();
            resource.insert("type".to_string(), serde_json::Value::String("image".to_string()));
            resource.insert("src".to_string(), serde_json::Value::String(format!("assets/{}", image.filename)));
            if let Some(ref alt) = image.alt_text {
                resource.insert("alt".to_string(), serde_json::Value::String(alt.clone()));
            }
            resources.insert(image.id.clone(), serde_json::Value::Object(resource));
        }

        let manifest = serde_json::json!({
            "version": "1.0",
            "format": "docx",
            "source": source_name,
            "resources": resources,
            "metadata": {
                "title": doc.metadata.title,
                "author": doc.metadata.author,
                "wordCount": doc.metadata.word_count,
                "pageCount": doc.metadata.page_count,
            }
        });

        serde_json::to_string_pretty(&manifest).unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_text_run_markdown() {
        let run = TextRun {
            text: "hello".to_string(),
            bold: true,
            italic: false,
            ..Default::default()
        };
        assert_eq!(run.to_markdown(), "**hello**");

        let run2 = TextRun {
            text: "world".to_string(),
            bold: true,
            italic: true,
            ..Default::default()
        };
        assert_eq!(run2.to_markdown(), "***world***");
    }

    #[test]
    fn test_paragraph_heading() {
        let para = Paragraph {
            runs: vec![TextRun {
                text: "Title".to_string(),
                ..Default::default()
            }],
            style: Some("Heading1".to_string()),
            ..Default::default()
        };
        assert_eq!(para.to_markdown(), "# Title");
    }

    #[test]
    fn test_table_markdown() {
        let table = DocxTable {
            rows: vec![
                vec![
                    TableCell { content: "A".to_string(), col_span: 1, row_span: 1 },
                    TableCell { content: "B".to_string(), col_span: 1, row_span: 1 },
                ],
                vec![
                    TableCell { content: "1".to_string(), col_span: 1, row_span: 1 },
                    TableCell { content: "2".to_string(), col_span: 1, row_span: 1 },
                ],
            ],
            has_header: true,
        };

        let md = table.to_markdown();
        assert!(md.contains("| A | B |"));
        assert!(md.contains("| --- | --- |"));
        assert!(md.contains("| 1 | 2 |"));
    }
}
