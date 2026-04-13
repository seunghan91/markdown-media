use std::fs::File;
use std::io::{self, Read, Seek, BufReader, Write, Cursor};
use std::path::{Path, PathBuf};
use std::collections::HashMap;
use quick_xml::Reader;
use quick_xml::events::Event;
use serde::{Serialize, Deserialize};

// Word XML namespaces (kept for reference; suppress unused warnings)
#[allow(dead_code)]
const WORD_NS: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";
#[allow(dead_code)]
const REL_NS: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
#[allow(dead_code)]
const DC_NS: &str = "http://purl.org/dc/elements/1.1/";

/// Numbering level definition parsed from numbering.xml
#[derive(Debug, Clone)]
struct NumberingLevel {
    /// "bullet", "decimal", "lowerLetter", "upperLetter", "lowerRoman", "upperRoman",
    /// "ganada", "chosung", etc.
    num_fmt: String,
    /// Level text pattern e.g. "%1." or "%1)"
    lvl_text: Option<String>,
}

/// Abstract numbering definition (abstractNum)
#[derive(Debug, Clone)]
struct AbstractNumDef {
    /// ilvl -> NumberingLevel
    levels: HashMap<u32, NumberingLevel>,
}

/// Style definition with optional outline level for heading detection
#[derive(Debug, Clone)]
struct StyleDef {
    name: String,
    /// outline level from w:outlineLvl, 0-based (0 = Heading 1)
    outline_level: Option<u32>,
}

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

        if result.is_empty() {
            return result;
        }

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

/// Inline element: either a text run or a hyperlink wrapping runs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum InlineElement {
    Run(TextRun),
    Hyperlink {
        url: String,
        runs: Vec<TextRun>,
    },
    FootnoteRef {
        id: String,
    },
    EndnoteRef {
        id: String,
    },
}

impl InlineElement {
    fn to_markdown(&self) -> String {
        match self {
            InlineElement::Run(run) => run.to_markdown(),
            InlineElement::Hyperlink { url, runs } => {
                let text: String = runs.iter().map(|r| r.to_markdown()).collect();
                if text.is_empty() {
                    return String::new();
                }
                format!("[{}]({})", text, url)
            }
            InlineElement::FootnoteRef { id } => format!("[^{}]", id),
            InlineElement::EndnoteRef { id } => format!("[^en{}]", id),
        }
    }
}

/// Paragraph with style information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Paragraph {
    pub runs: Vec<TextRun>,
    pub style: Option<String>,
    pub style_id: Option<String>,
    pub alignment: Option<String>,
    pub indent_level: u32,
    pub is_list_item: bool,
    pub list_type: Option<String>, // "bullet", "number", "lowerLetter", "upperLetter", "lowerRoman", "upperRoman", "ganada", "chosung"
    /// Inline elements including hyperlinks and footnote refs
    #[serde(skip)]
    pub inlines: Vec<InlineElement>,
    /// Outline level for heading detection (from styles.xml outlineLvl)
    #[serde(skip)]
    pub outline_level: Option<u32>,
    /// Whether this paragraph is a blockquote
    #[serde(skip)]
    pub is_blockquote: bool,
}

impl Default for Paragraph {
    fn default() -> Self {
        Paragraph {
            runs: Vec::new(),
            style: None,
            style_id: None,
            alignment: None,
            indent_level: 0,
            is_list_item: false,
            list_type: None,
            inlines: Vec::new(),
            outline_level: None,
            is_blockquote: false,
        }
    }
}

impl Paragraph {
    /// Get plain text content
    pub fn text(&self) -> String {
        self.runs.iter().map(|r| r.text.as_str()).collect()
    }

    /// Convert to markdown using inline elements (hyperlinks, footnotes) when available,
    /// falling back to runs for backward compatibility.
    pub fn to_markdown(&self) -> String {
        let content = if !self.inlines.is_empty() {
            // Build content with smart spacing between inline elements
            let mut result = String::new();
            for (idx, inline) in self.inlines.iter().enumerate() {
                let rendered = inline.to_markdown();
                if rendered.is_empty() { continue; }

                // Ensure space between adjacent elements when needed
                if idx > 0 && !result.is_empty() {
                    let last_char = result.chars().last().unwrap_or(' ');
                    let first_char = rendered.chars().next().unwrap_or(' ');

                    // Footnote/endnote refs should attach directly to preceding text (no space)
                    let is_note_ref = matches!(&self.inlines[idx],
                        InlineElement::FootnoteRef { .. } | InlineElement::EndnoteRef { .. });

                    // Need space if: prev ends with word char and next starts with markdown/word char
                    let needs_space = !is_note_ref &&
                        (last_char.is_alphanumeric() || last_char == '*' || last_char == '~' || last_char == ']' || last_char == ')') &&
                        (first_char == '[' || first_char == '*' || first_char == '~' ||
                         first_char.is_alphanumeric());

                    // But don't add space if prev already ends with space or next starts with punctuation
                    let already_spaced = last_char == ' ' || first_char == ' ';
                    let next_is_punct = first_char == ',' || first_char == '.' || first_char == ';' ||
                                        first_char == ':' || first_char == '!' || first_char == '?';

                    if needs_space && !already_spaced && !next_is_punct {
                        result.push(' ');
                    }
                }

                result.push_str(&rendered);
            }
            result
        } else {
            self.runs.iter().map(|r| r.to_markdown()).collect::<String>()
        };

        if content.trim().is_empty() {
            return String::new();
        }

        // Handle headings via outline_level (from styles.xml outlineLvl)
        if let Some(level) = self.outline_level {
            let hashes = "#".repeat((level + 1).min(6) as usize);
            return format!("{} {}", hashes, content);
        }

        // Fallback heading detection via style name
        if let Some(ref style) = self.style {
            match style.as_str() {
                "Heading1" | "heading 1" => return format!("# {}", content),
                "Heading2" | "heading 2" => return format!("## {}", content),
                "Heading3" | "heading 3" => return format!("### {}", content),
                "Heading4" | "heading 4" => return format!("#### {}", content),
                "Title" => return format!("# {}", content),
                "Subtitle" => return format!("## {}", content),
                _ => {}
            }
        }
        // Also check style_id for common patterns like "1", "2", etc.
        if let Some(ref sid) = self.style_id {
            match sid.as_str() {
                "Heading1" | "1" => return format!("# {}", content),
                "Heading2" | "2" => return format!("## {}", content),
                "Heading3" | "3" => return format!("### {}", content),
                "Heading4" | "4" => return format!("#### {}", content),
                _ => {}
            }
        }

        // Handle blockquotes
        if self.is_blockquote {
            return format!("> {}", content);
        }

        // Handle list items
        if self.is_list_item {
            let indent = "  ".repeat(self.indent_level as usize);
            return match self.list_type.as_deref() {
                Some("number") | Some("decimal") => format!("{}1. {}", indent, content),
                Some("lowerLetter") => format!("{}a) {}", indent, content),
                Some("upperLetter") => format!("{}A) {}", indent, content),
                Some("lowerRoman") => format!("{}i. {}", indent, content),
                Some("upperRoman") => format!("{}I. {}", indent, content),
                Some("ganada") => {
                    let marker = ganada_marker(self.indent_level);
                    format!("{}{}) {}", indent, marker, content)
                }
                Some("chosung") => {
                    let marker = chosung_marker(self.indent_level);
                    format!("{}{}) {}", indent, marker, content)
                }
                _ => format!("{}- {}", indent, content),
            };
        }

        content
    }
}

/// Get Korean ganada marker: 가, 나, 다, 라, 마, 바, 사, 아, 자, 차, 카, 타, 파, 하
fn ganada_marker(index: u32) -> char {
    const GANADA: &[char] = &[
        '\u{AC00}', '\u{B098}', '\u{B2E4}', '\u{B77C}', '\u{B9C8}',
        '\u{BC14}', '\u{C0AC}', '\u{C544}', '\u{C790}', '\u{CC28}',
        '\u{CE74}', '\u{D0C0}', '\u{D30C}', '\u{D558}',
    ];
    GANADA.get(index as usize).copied().unwrap_or('\u{AC00}')
}

/// Get Korean chosung marker: ㄱ, ㄴ, ㄷ, ㄹ, ㅁ, ㅂ, ㅅ, ㅇ, ㅈ, ㅊ, ㅋ, ㅌ, ㅍ, ㅎ
fn chosung_marker(index: u32) -> char {
    const CHOSUNG: &[char] = &[
        '\u{3131}', '\u{3134}', '\u{3137}', '\u{3139}', '\u{3141}',
        '\u{3142}', '\u{3145}', '\u{3147}', '\u{3148}', '\u{314A}',
        '\u{314B}', '\u{314C}', '\u{314D}', '\u{314E}',
    ];
    CHOSUNG.get(index as usize).copied().unwrap_or('\u{3131}')
}

/// Table cell
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableCell {
    pub content: String,
    pub col_span: u32,
    pub row_span: u32,
    /// true if this cell is a vMerge continuation (should be empty in rendering)
    #[serde(skip)]
    pub v_merge_continue: bool,
}

impl Default for TableCell {
    fn default() -> Self {
        TableCell {
            content: String::new(),
            col_span: 1,
            row_span: 1,
            v_merge_continue: false,
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
                .map(|c| {
                    if c.v_merge_continue {
                        String::new() // empty for vertically merged continuation cells
                    } else {
                        c.content.replace('|', "\\|").replace('\n', " ")
                    }
                })
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
    /// Footnote definitions: id -> markdown content
    #[serde(skip)]
    pub footnotes: Vec<(String, String)>,
    /// Endnote definitions: id -> markdown content
    #[serde(skip)]
    pub endnotes: Vec<(String, String)>,
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

        let mut result = parts.join("\n\n");

        // Append footnote definitions
        if !self.footnotes.is_empty() {
            result.push_str("\n\n");
            for (id, content) in &self.footnotes {
                result.push_str(&format!("[^{}]: {}\n", id, content));
            }
        }

        // Append endnote definitions
        if !self.endnotes.is_empty() {
            result.push_str("\n\n");
            for (id, content) in &self.endnotes {
                result.push_str(&format!("[^en{}]: {}\n", id, content));
            }
        }

        result
    }

    /// Convert to MDX format with frontmatter
    pub fn to_mdx(&self, source_filename: &str) -> String {
        let mut output = String::new();

        // Frontmatter
        output.push_str("---\n");
        if let Some(ref title) = self.metadata.title {
            output.push_str(&format!("title: \"{}\"\n", title.replace('"', "\\\"")));
        }
        if let Some(ref author) = self.metadata.author {
            output.push_str(&format!("author: \"{}\"\n", author.replace('"', "\\\"")));
        }
        output.push_str(&format!("source: \"{}\"\n", source_filename));
        output.push_str("format: docx\n");
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

/// DOCX Parser, generic over the underlying reader type.
///
/// The default type parameter `BufReader<File>` preserves backward
/// compatibility: existing code that writes `DocxParser` without a
/// type argument continues to work unchanged.
pub struct DocxParser<R: Read + Seek = BufReader<File>> {
    archive: zip::ZipArchive<R>,
    path: PathBuf,
    relationships: HashMap<String, String>,
    /// styleId -> StyleDef (name + optional outline_level)
    styles: HashMap<String, StyleDef>,
    /// numId -> (ilvl -> resolved NumberingLevel from abstractNum)
    numbering: HashMap<String, HashMap<u32, NumberingLevel>>,
    /// Footnote id -> plain text content
    footnotes: HashMap<String, String>,
    /// Endnote id -> plain text content
    endnotes: HashMap<String, String>,
}

impl DocxParser<BufReader<File>> {
    /// Open a DOCX file from disk.
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
            footnotes: HashMap::new(),
            endnotes: HashMap::new(),
        };

        parser.load_relationships()?;
        parser.load_styles()?;
        parser.load_numbering()?;
        parser.load_footnotes()?;
        parser.load_endnotes()?;

        Ok(parser)
    }
}

impl DocxParser<Cursor<Vec<u8>>> {
    /// Create a DOCX parser from in-memory data.
    ///
    /// This constructor is used for WASM and other environments
    /// where file system access is unavailable.
    pub fn from_bytes(data: Vec<u8>) -> io::Result<Self> {
        let cursor = Cursor::new(data);
        let archive = zip::ZipArchive::new(cursor)
            .map_err(|e| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("Invalid DOCX: {}", e),
                )
            })?;

        let mut parser = DocxParser {
            archive,
            path: PathBuf::from("<memory>"),
            relationships: HashMap::new(),
            styles: HashMap::new(),
            numbering: HashMap::new(),
            footnotes: HashMap::new(),
            endnotes: HashMap::new(),
        };

        parser.load_relationships()?;
        parser.load_styles()?;
        parser.load_numbering()?;
        parser.load_footnotes()?;
        parser.load_endnotes()?;

        Ok(parser)
    }
}

impl<R: Read + Seek> DocxParser<R> {
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

    /// Load document styles, including outlineLvl for heading detection
    fn load_styles(&mut self) -> io::Result<()> {
        let content = match self.read_archive_file("word/styles.xml") {
            Ok(c) => c,
            Err(_) => return Ok(()),
        };

        let mut reader = Reader::from_str(&content);
        reader.trim_text(true);

        let mut current_style_id = String::new();
        let mut current_style_name = String::new();
        let mut current_outline_level: Option<u32> = None;
        let mut in_style = false;
        let mut in_ppr = false; // inside w:pPr (paragraph properties of a style)

        loop {
            match reader.read_event() {
                Ok(Event::Start(ref e)) => {
                    match e.local_name().as_ref() {
                        b"style" => {
                            in_style = true;
                            current_style_id.clear();
                            current_style_name.clear();
                            current_outline_level = None;
                            for attr in e.attributes().flatten() {
                                if attr.key.local_name().as_ref() == b"styleId" {
                                    current_style_id = String::from_utf8_lossy(&attr.value).to_string();
                                }
                            }
                        }
                        b"name" if in_style => {
                            for attr in e.attributes().flatten() {
                                if attr.key.local_name().as_ref() == b"val" {
                                    current_style_name = String::from_utf8_lossy(&attr.value).to_string();
                                }
                            }
                        }
                        b"pPr" if in_style => {
                            in_ppr = true;
                        }
                        b"outlineLvl" if in_ppr => {
                            for attr in e.attributes().flatten() {
                                if attr.key.local_name().as_ref() == b"val" {
                                    if let Ok(lvl) = String::from_utf8_lossy(&attr.value).parse::<u32>() {
                                        current_outline_level = Some(lvl);
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                }
                Ok(Event::Empty(ref e)) => {
                    match e.local_name().as_ref() {
                        b"name" if in_style => {
                            for attr in e.attributes().flatten() {
                                if attr.key.local_name().as_ref() == b"val" {
                                    current_style_name = String::from_utf8_lossy(&attr.value).to_string();
                                }
                            }
                        }
                        b"outlineLvl" if in_ppr => {
                            for attr in e.attributes().flatten() {
                                if attr.key.local_name().as_ref() == b"val" {
                                    if let Ok(lvl) = String::from_utf8_lossy(&attr.value).parse::<u32>() {
                                        current_outline_level = Some(lvl);
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                }
                Ok(Event::End(ref e)) => {
                    match e.local_name().as_ref() {
                        b"style" => {
                            if !current_style_id.is_empty() {
                                self.styles.insert(current_style_id.clone(), StyleDef {
                                    name: current_style_name.clone(),
                                    outline_level: current_outline_level,
                                });
                            }
                            in_style = false;
                            in_ppr = false;
                        }
                        b"pPr" => {
                            in_ppr = false;
                        }
                        _ => {}
                    }
                }
                Ok(Event::Eof) => break,
                Err(_) => break,
                _ => {}
            }
        }

        Ok(())
    }

    /// Load numbering definitions from word/numbering.xml.
    /// Builds a lookup: numId -> (ilvl -> NumberingLevel) by resolving abstractNumId references.
    fn load_numbering(&mut self) -> io::Result<()> {
        let content = match self.read_archive_file("word/numbering.xml") {
            Ok(c) => c,
            Err(_) => return Ok(()), // No numbering definitions
        };

        let mut reader = Reader::from_str(&content);
        reader.trim_text(true);

        // Phase 1: collect abstractNum definitions
        let mut abstract_nums: HashMap<String, AbstractNumDef> = HashMap::new();
        // Phase 2: collect num -> abstractNumId mappings
        let mut num_to_abstract: HashMap<String, String> = HashMap::new();

        let mut current_abstract_id = String::new();
        let mut current_levels: HashMap<u32, NumberingLevel> = HashMap::new();
        let mut current_lvl_ilvl: Option<u32> = None;
        let mut current_lvl_fmt = String::new();
        let mut current_lvl_text: Option<String> = None;

        let mut in_abstract_num = false;
        let mut in_lvl = false;

        let mut current_num_id = String::new();
        let mut in_num = false;

        loop {
            match reader.read_event() {
                Ok(Event::Start(ref e)) => {
                    match e.local_name().as_ref() {
                        b"abstractNum" => {
                            in_abstract_num = true;
                            current_levels.clear();
                            for attr in e.attributes().flatten() {
                                if attr.key.local_name().as_ref() == b"abstractNumId" {
                                    current_abstract_id = String::from_utf8_lossy(&attr.value).to_string();
                                }
                            }
                        }
                        b"lvl" if in_abstract_num => {
                            in_lvl = true;
                            current_lvl_fmt.clear();
                            current_lvl_text = None;
                            current_lvl_ilvl = None;
                            for attr in e.attributes().flatten() {
                                if attr.key.local_name().as_ref() == b"ilvl" {
                                    current_lvl_ilvl = String::from_utf8_lossy(&attr.value).parse().ok();
                                }
                            }
                        }
                        b"numFmt" if in_lvl => {
                            for attr in e.attributes().flatten() {
                                if attr.key.local_name().as_ref() == b"val" {
                                    current_lvl_fmt = String::from_utf8_lossy(&attr.value).to_string();
                                }
                            }
                        }
                        b"lvlText" if in_lvl => {
                            for attr in e.attributes().flatten() {
                                if attr.key.local_name().as_ref() == b"val" {
                                    current_lvl_text = Some(String::from_utf8_lossy(&attr.value).to_string());
                                }
                            }
                        }
                        b"num" if !in_abstract_num => {
                            in_num = true;
                            for attr in e.attributes().flatten() {
                                if attr.key.local_name().as_ref() == b"numId" {
                                    current_num_id = String::from_utf8_lossy(&attr.value).to_string();
                                }
                            }
                        }
                        b"abstractNumId" if in_num => {
                            for attr in e.attributes().flatten() {
                                if attr.key.local_name().as_ref() == b"val" {
                                    let abs_id = String::from_utf8_lossy(&attr.value).to_string();
                                    num_to_abstract.insert(current_num_id.clone(), abs_id);
                                }
                            }
                        }
                        _ => {}
                    }
                }
                Ok(Event::Empty(ref e)) => {
                    match e.local_name().as_ref() {
                        b"numFmt" if in_lvl => {
                            for attr in e.attributes().flatten() {
                                if attr.key.local_name().as_ref() == b"val" {
                                    current_lvl_fmt = String::from_utf8_lossy(&attr.value).to_string();
                                }
                            }
                        }
                        b"lvlText" if in_lvl => {
                            for attr in e.attributes().flatten() {
                                if attr.key.local_name().as_ref() == b"val" {
                                    current_lvl_text = Some(String::from_utf8_lossy(&attr.value).to_string());
                                }
                            }
                        }
                        b"abstractNumId" if in_num => {
                            for attr in e.attributes().flatten() {
                                if attr.key.local_name().as_ref() == b"val" {
                                    let abs_id = String::from_utf8_lossy(&attr.value).to_string();
                                    num_to_abstract.insert(current_num_id.clone(), abs_id);
                                }
                            }
                        }
                        _ => {}
                    }
                }
                Ok(Event::End(ref e)) => {
                    match e.local_name().as_ref() {
                        b"lvl" if in_abstract_num => {
                            if let Some(ilvl) = current_lvl_ilvl {
                                current_levels.insert(ilvl, NumberingLevel {
                                    num_fmt: current_lvl_fmt.clone(),
                                    lvl_text: current_lvl_text.clone(),
                                });
                            }
                            in_lvl = false;
                        }
                        b"abstractNum" => {
                            if !current_abstract_id.is_empty() {
                                abstract_nums.insert(current_abstract_id.clone(), AbstractNumDef {
                                    levels: current_levels.clone(),
                                });
                            }
                            in_abstract_num = false;
                        }
                        b"num" => {
                            in_num = false;
                        }
                        _ => {}
                    }
                }
                Ok(Event::Eof) => break,
                Err(_) => break,
                _ => {}
            }
        }

        // Resolve: numId -> abstractNumId -> levels
        for (num_id, abs_id) in &num_to_abstract {
            if let Some(abs_def) = abstract_nums.get(abs_id) {
                self.numbering.insert(num_id.clone(), abs_def.levels.clone());
            }
        }

        Ok(())
    }

    /// Load footnotes from word/footnotes.xml
    fn load_footnotes(&mut self) -> io::Result<()> {
        let content = match self.read_archive_file("word/footnotes.xml") {
            Ok(c) => c,
            Err(_) => return Ok(()),
        };
        self.footnotes = Self::parse_notes_xml(&content);
        Ok(())
    }

    /// Load endnotes from word/endnotes.xml
    fn load_endnotes(&mut self) -> io::Result<()> {
        let content = match self.read_archive_file("word/endnotes.xml") {
            Ok(c) => c,
            Err(_) => return Ok(()),
        };
        self.endnotes = Self::parse_notes_xml(&content);
        Ok(())
    }

    /// Parse footnotes.xml or endnotes.xml into id -> text mapping.
    /// Skips separator/continuationSeparator notes (ids "0" and "-1").
    fn parse_notes_xml(content: &str) -> HashMap<String, String> {
        let mut notes: HashMap<String, String> = HashMap::new();
        let mut reader = Reader::from_str(content);
        reader.trim_text(true);

        // The root element is <w:footnotes> or <w:endnotes>.
        // Each child is <w:footnote w:id="N"> or <w:endnote w:id="N">.
        let mut current_id: Option<String> = None;
        let mut current_text = String::new();
        let mut in_note = false;
        let mut in_run = false;
        let mut in_text = false;
        // Track bold/italic for formatting within footnotes
        let mut run_bold = false;
        let mut run_italic = false;
        let mut run_text = String::new();

        loop {
            match reader.read_event() {
                Ok(Event::Start(ref e)) => {
                    match e.local_name().as_ref() {
                        b"footnote" | b"endnote" => {
                            in_note = true;
                            current_text.clear();
                            let mut note_id = None;
                            let mut note_type = None;
                            for attr in e.attributes().flatten() {
                                match attr.key.local_name().as_ref() {
                                    b"id" => note_id = Some(String::from_utf8_lossy(&attr.value).to_string()),
                                    b"type" => note_type = Some(String::from_utf8_lossy(&attr.value).to_string()),
                                    _ => {}
                                }
                            }
                            // Skip separator types
                            if let Some(ref t) = note_type {
                                if t == "separator" || t == "continuationSeparator" {
                                    current_id = None;
                                } else {
                                    current_id = note_id;
                                }
                            } else {
                                current_id = note_id;
                            }
                        }
                        b"r" if in_note => {
                            in_run = true;
                            run_bold = false;
                            run_italic = false;
                            run_text.clear();
                        }
                        b"t" if in_run => {
                            in_text = true;
                        }
                        b"b" if in_run => run_bold = true,
                        b"i" if in_run => run_italic = true,
                        _ => {}
                    }
                }
                Ok(Event::Empty(ref e)) => {
                    match e.local_name().as_ref() {
                        b"b" if in_run => run_bold = true,
                        b"i" if in_run => run_italic = true,
                        _ => {}
                    }
                }
                Ok(Event::Text(ref e)) => {
                    if in_text {
                        let text = e.unescape().unwrap_or_default().to_string();
                        run_text.push_str(&text);
                    }
                }
                Ok(Event::End(ref e)) => {
                    match e.local_name().as_ref() {
                        b"t" => in_text = false,
                        b"r" => {
                            // Apply formatting
                            let formatted = if run_bold && run_italic {
                                format!("***{}***", run_text)
                            } else if run_bold {
                                format!("**{}**", run_text)
                            } else if run_italic {
                                format!("*{}*", run_text)
                            } else {
                                run_text.clone()
                            };
                            current_text.push_str(&formatted);
                            in_run = false;
                        }
                        b"footnote" | b"endnote" => {
                            if let Some(ref id) = current_id {
                                // Skip ids 0 and -1 (separators)
                                if id != "0" && id != "-1" {
                                    let trimmed = current_text.trim().to_string();
                                    if !trimmed.is_empty() {
                                        notes.insert(id.clone(), trimmed);
                                    }
                                }
                            }
                            in_note = false;
                            current_id = None;
                        }
                        _ => {}
                    }
                }
                Ok(Event::Eof) => break,
                Err(_) => break,
                _ => {}
            }
        }

        notes
    }

    /// Resolve list type for a given numId and ilvl using parsed numbering.xml data.
    /// Returns a list_type string like "bullet", "decimal", "lowerLetter", "ganada", etc.
    fn resolve_list_type(&self, num_id: &str, ilvl: u32) -> Option<String> {
        if let Some(levels) = self.numbering.get(num_id) {
            if let Some(level) = levels.get(&ilvl) {
                let list_type = match level.num_fmt.as_str() {
                    "bullet" => "bullet",
                    "decimal" | "decimalZero" => "decimal",
                    "lowerLetter" => "lowerLetter",
                    "upperLetter" => "upperLetter",
                    "lowerRoman" => "lowerRoman",
                    "upperRoman" => "upperRoman",
                    "ganada" => "ganada",
                    "chosung" => "chosung",
                    "none" => return None,
                    _ => "bullet", // fallback unknown formats to bullet
                };
                return Some(list_type.to_string());
            }
        }
        None
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
        // For collecting formatted cell content (runs converted to markdown)
        let mut cell_inlines: Vec<InlineElement> = Vec::new();

        let mut in_paragraph = false;
        let mut in_run = false;
        let mut in_text = false;
        let mut in_table = false;
        let mut in_table_row = false;
        let mut in_table_cell = false;
        let mut in_num_pr = false;
        let mut in_hyperlink = false;
        let mut hyperlink_url = String::new();
        let mut hyperlink_runs: Vec<TextRun> = Vec::new();

        // Track numId for deferred list type resolution
        let mut current_num_id: Option<String> = None;

        // vMerge tracking
        let mut current_cell_v_merge_continue = false;

        loop {
            match reader.read_event() {
                Ok(Event::Start(ref e)) => {
                    match e.local_name().as_ref() {
                        b"p" => {
                            in_paragraph = true;
                            current_para = Paragraph::default();
                            current_num_id = None;
                        }
                        b"hyperlink" if in_paragraph => {
                            in_hyperlink = true;
                            hyperlink_url.clear();
                            hyperlink_runs.clear();
                            // Check for r:id (external) or w:anchor (internal)
                            for attr in e.attributes().flatten() {
                                match attr.key.local_name().as_ref() {
                                    b"id" => {
                                        let rid = String::from_utf8_lossy(&attr.value).to_string();
                                        if let Some(target) = self.relationships.get(&rid) {
                                            hyperlink_url = target.clone();
                                        }
                                    }
                                    b"anchor" => {
                                        let anchor = String::from_utf8_lossy(&attr.value).to_string();
                                        hyperlink_url = format!("#{}", anchor);
                                    }
                                    _ => {}
                                }
                            }
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
                            cell_inlines.clear();
                            current_cell_v_merge_continue = false;
                        }
                        b"pStyle" if in_paragraph => {
                            for attr in e.attributes().flatten() {
                                if attr.key.local_name().as_ref() == b"val" {
                                    let style_id = String::from_utf8_lossy(&attr.value).to_string();
                                    // Look up style definition
                                    if let Some(style_def) = self.styles.get(&style_id) {
                                        current_para.style = Some(style_def.name.clone());
                                        current_para.outline_level = style_def.outline_level;
                                        // Check for blockquote styles
                                        let name_lower = style_def.name.to_lowercase();
                                        if name_lower.contains("quote") {
                                            current_para.is_blockquote = true;
                                        }
                                    } else {
                                        current_para.style = Some(style_id.clone());
                                    }
                                    current_para.style_id = Some(style_id.clone());

                                    // Style-based list detection (when numPr is absent)
                                    // Many documents use pStyle like "ListBullet", "ListNumber" etc.
                                    let sid_lower = style_id.to_lowercase();
                                    let name_lower = current_para.style.as_deref().unwrap_or("").to_lowercase();
                                    if sid_lower.contains("listbullet") || name_lower.contains("list bullet") {
                                        current_para.is_list_item = true;
                                        current_para.list_type = Some("bullet".to_string());
                                        // Detect nesting level from style name suffix
                                        if sid_lower.ends_with('2') || name_lower.ends_with('2') {
                                            current_para.indent_level = 1;
                                        } else if sid_lower.ends_with('3') || name_lower.ends_with('3') {
                                            current_para.indent_level = 2;
                                        }
                                    } else if sid_lower.contains("listnumber") || name_lower.contains("list number") {
                                        current_para.is_list_item = true;
                                        current_para.list_type = Some("decimal".to_string());
                                        if sid_lower.ends_with('2') || name_lower.ends_with('2') {
                                            current_para.indent_level = 1;
                                        } else if sid_lower.ends_with('3') || name_lower.ends_with('3') {
                                            current_para.indent_level = 2;
                                        }
                                    }
                                }
                            }
                        }
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
                        b"sz" if in_run => {
                            for attr in e.attributes().flatten() {
                                if attr.key.local_name().as_ref() == b"val" {
                                    if let Ok(size) = String::from_utf8_lossy(&attr.value).parse::<u32>() {
                                        current_run.font_size = Some(size / 2);
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
                        b"vMerge" if in_table_cell => {
                            // vMerge with no val or val="continue" means continuation cell
                            // vMerge with val="restart" means start of vertical merge
                            let mut is_restart = false;
                            for attr in e.attributes().flatten() {
                                if attr.key.local_name().as_ref() == b"val" {
                                    let val = String::from_utf8_lossy(&attr.value);
                                    if val == "restart" {
                                        is_restart = true;
                                    }
                                }
                            }
                            if !is_restart {
                                current_cell_v_merge_continue = true;
                            }
                        }
                        b"numPr" if in_paragraph => {
                            in_num_pr = true;
                            current_para.is_list_item = true;
                        }
                        b"ilvl" if in_num_pr => {
                            for attr in e.attributes().flatten() {
                                if attr.key.local_name().as_ref() == b"val" {
                                    if let Ok(level) = String::from_utf8_lossy(&attr.value).parse::<u32>() {
                                        current_para.indent_level = level;
                                    }
                                }
                            }
                        }
                        b"numId" if in_num_pr => {
                            for attr in e.attributes().flatten() {
                                if attr.key.local_name().as_ref() == b"val" {
                                    let num_id = String::from_utf8_lossy(&attr.value).to_string();
                                    if num_id != "0" {
                                        current_num_id = Some(num_id);
                                    } else {
                                        current_para.is_list_item = false;
                                    }
                                }
                            }
                        }
                        b"footnoteReference" if in_run => {
                            for attr in e.attributes().flatten() {
                                if attr.key.local_name().as_ref() == b"id" {
                                    let fid = String::from_utf8_lossy(&attr.value).to_string();
                                    if fid != "0" && fid != "-1" {
                                        let elem = InlineElement::FootnoteRef { id: fid };
                                        if in_hyperlink {
                                            // unusual but handle gracefully
                                        } else {
                                            current_para.inlines.push(elem);
                                        }
                                    }
                                }
                            }
                        }
                        b"endnoteReference" if in_run => {
                            for attr in e.attributes().flatten() {
                                if attr.key.local_name().as_ref() == b"id" {
                                    let eid = String::from_utf8_lossy(&attr.value).to_string();
                                    if eid != "0" && eid != "-1" {
                                        let elem = InlineElement::EndnoteRef { id: eid };
                                        current_para.inlines.push(elem);
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
                                    if let Some(style_def) = self.styles.get(&style_id) {
                                        current_para.style = Some(style_def.name.clone());
                                        current_para.outline_level = style_def.outline_level;
                                        let name_lower = style_def.name.to_lowercase();
                                        if name_lower.contains("quote") {
                                            current_para.is_blockquote = true;
                                        }
                                    } else {
                                        current_para.style = Some(style_id.clone());
                                    }
                                    current_para.style_id = Some(style_id.clone());

                                    // Style-based list detection (Empty tag variant)
                                    let sid_lower = style_id.to_lowercase();
                                    let name_lower = current_para.style.as_deref().unwrap_or("").to_lowercase();
                                    if sid_lower.contains("listbullet") || name_lower.contains("list bullet") {
                                        current_para.is_list_item = true;
                                        current_para.list_type = Some("bullet".to_string());
                                        if sid_lower.ends_with('2') || name_lower.ends_with('2') {
                                            current_para.indent_level = 1;
                                        } else if sid_lower.ends_with('3') || name_lower.ends_with('3') {
                                            current_para.indent_level = 2;
                                        }
                                    } else if sid_lower.contains("listnumber") || name_lower.contains("list number") {
                                        current_para.is_list_item = true;
                                        current_para.list_type = Some("decimal".to_string());
                                        if sid_lower.ends_with('2') || name_lower.ends_with('2') {
                                            current_para.indent_level = 1;
                                        } else if sid_lower.ends_with('3') || name_lower.ends_with('3') {
                                            current_para.indent_level = 2;
                                        }
                                    }
                                }
                            }
                        }
                        b"ilvl" if in_num_pr => {
                            for attr in e.attributes().flatten() {
                                if attr.key.local_name().as_ref() == b"val" {
                                    if let Ok(level) = String::from_utf8_lossy(&attr.value).parse::<u32>() {
                                        current_para.indent_level = level;
                                    }
                                }
                            }
                        }
                        b"numId" if in_num_pr => {
                            for attr in e.attributes().flatten() {
                                if attr.key.local_name().as_ref() == b"val" {
                                    let num_id = String::from_utf8_lossy(&attr.value).to_string();
                                    if num_id != "0" {
                                        current_num_id = Some(num_id);
                                    } else {
                                        current_para.is_list_item = false;
                                    }
                                }
                            }
                        }
                        b"vMerge" if in_table_cell => {
                            let mut is_restart = false;
                            for attr in e.attributes().flatten() {
                                if attr.key.local_name().as_ref() == b"val" {
                                    let val = String::from_utf8_lossy(&attr.value);
                                    if val == "restart" {
                                        is_restart = true;
                                    }
                                }
                            }
                            if !is_restart {
                                current_cell_v_merge_continue = true;
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
                        b"footnoteReference" if in_run => {
                            for attr in e.attributes().flatten() {
                                if attr.key.local_name().as_ref() == b"id" {
                                    let fid = String::from_utf8_lossy(&attr.value).to_string();
                                    if fid != "0" && fid != "-1" {
                                        current_para.inlines.push(InlineElement::FootnoteRef { id: fid });
                                    }
                                }
                            }
                        }
                        b"endnoteReference" if in_run => {
                            for attr in e.attributes().flatten() {
                                if attr.key.local_name().as_ref() == b"id" {
                                    let eid = String::from_utf8_lossy(&attr.value).to_string();
                                    if eid != "0" && eid != "-1" {
                                        current_para.inlines.push(InlineElement::EndnoteRef { id: eid });
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
                    }
                }
                Ok(Event::End(ref e)) => {
                    match e.local_name().as_ref() {
                        b"t" => {
                            in_text = false;
                        }
                        b"r" => {
                            if !current_run.text.is_empty() {
                                if in_hyperlink {
                                    hyperlink_runs.push(current_run.clone());
                                } else {
                                    current_para.runs.push(current_run.clone());
                                    current_para.inlines.push(InlineElement::Run(current_run.clone()));
                                    // Also add formatted content to cell
                                    if in_table_cell {
                                        cell_inlines.push(InlineElement::Run(current_run.clone()));
                                    }
                                }
                            }
                            current_run = TextRun::default();
                            in_run = false;
                        }
                        b"hyperlink" => {
                            if !hyperlink_runs.is_empty() && !hyperlink_url.is_empty() {
                                // Add runs to paragraph runs for backward compat (plain text)
                                for r in &hyperlink_runs {
                                    current_para.runs.push(r.clone());
                                }
                                let link = InlineElement::Hyperlink {
                                    url: hyperlink_url.clone(),
                                    runs: hyperlink_runs.clone(),
                                };
                                current_para.inlines.push(link.clone());
                                if in_table_cell {
                                    cell_inlines.push(link);
                                }
                            } else if !hyperlink_runs.is_empty() {
                                // No URL resolved, just add runs as plain text
                                for r in &hyperlink_runs {
                                    current_para.runs.push(r.clone());
                                    current_para.inlines.push(InlineElement::Run(r.clone()));
                                    if in_table_cell {
                                        cell_inlines.push(InlineElement::Run(r.clone()));
                                    }
                                }
                            }
                            in_hyperlink = false;
                        }
                        b"p" => {
                            // Resolve list type from numbering.xml before finalizing
                            if current_para.is_list_item {
                                if let Some(ref nid) = current_num_id {
                                    if let Some(lt) = self.resolve_list_type(nid, current_para.indent_level) {
                                        current_para.list_type = Some(lt);
                                    } else {
                                        // Fallback: default to bullet
                                        current_para.list_type = Some("bullet".to_string());
                                    }
                                }
                            }

                            if !in_table_cell {
                                paragraphs.push(current_para.clone());
                            } else {
                                // For table cells, store formatted markdown content
                                if let Some(ref mut cell) = current_cell {
                                    if !cell.content.is_empty() {
                                        cell.content.push(' ');
                                    }
                                    // Convert cell inlines to markdown
                                    let cell_md: String = cell_inlines.iter().map(|i| i.to_markdown()).collect();
                                    if cell_md.is_empty() {
                                        // Fallback to plain text from runs
                                        let plain: String = current_para.runs.iter().map(|r| r.to_markdown()).collect();
                                        cell.content.push_str(&plain);
                                    } else {
                                        cell.content.push_str(&cell_md);
                                    }
                                }
                                cell_inlines.clear();
                            }
                            current_para = Paragraph::default();
                            in_paragraph = false;
                            in_num_pr = false;
                            current_num_id = None;
                        }
                        b"numPr" => {
                            in_num_pr = false;
                        }
                        b"tc" => {
                            if let Some(mut cell) = current_cell.take() {
                                cell.v_merge_continue = current_cell_v_merge_continue;
                                current_row.push(cell);
                            }
                            in_table_cell = false;
                            cell_inlines.clear();
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

        // Collect footnote/endnote definitions
        let footnotes: Vec<(String, String)> = self.footnotes.iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        let endnotes: Vec<(String, String)> = self.endnotes.iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();

        Ok(DocxDocument {
            paragraphs,
            tables,
            images,
            metadata,
            footnotes,
            endnotes,
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
    fn test_paragraph_heading_by_outline_level() {
        let para = Paragraph {
            runs: vec![TextRun {
                text: "My Heading".to_string(),
                ..Default::default()
            }],
            inlines: vec![InlineElement::Run(TextRun {
                text: "My Heading".to_string(),
                ..Default::default()
            })],
            outline_level: Some(0), // outline level 0 = Heading 1
            ..Default::default()
        };
        assert_eq!(para.to_markdown(), "# My Heading");

        let para2 = Paragraph {
            runs: vec![TextRun {
                text: "Sub Heading".to_string(),
                ..Default::default()
            }],
            inlines: vec![InlineElement::Run(TextRun {
                text: "Sub Heading".to_string(),
                ..Default::default()
            })],
            outline_level: Some(2), // outline level 2 = Heading 3
            ..Default::default()
        };
        assert_eq!(para2.to_markdown(), "### Sub Heading");
    }

    #[test]
    fn test_paragraph_heading_fallback() {
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
    fn test_paragraph_blockquote() {
        let para = Paragraph {
            runs: vec![TextRun {
                text: "A quote".to_string(),
                ..Default::default()
            }],
            is_blockquote: true,
            ..Default::default()
        };
        assert_eq!(para.to_markdown(), "> A quote");
    }

    #[test]
    fn test_hyperlink_inline() {
        let para = Paragraph {
            runs: vec![TextRun {
                text: "Click here".to_string(),
                ..Default::default()
            }],
            inlines: vec![InlineElement::Hyperlink {
                url: "https://example.com".to_string(),
                runs: vec![TextRun {
                    text: "Click here".to_string(),
                    ..Default::default()
                }],
            }],
            ..Default::default()
        };
        assert_eq!(para.to_markdown(), "[Click here](https://example.com)");
    }

    #[test]
    fn test_footnote_ref_inline() {
        let para = Paragraph {
            runs: vec![TextRun {
                text: "Some text".to_string(),
                ..Default::default()
            }],
            inlines: vec![
                InlineElement::Run(TextRun {
                    text: "Some text".to_string(),
                    ..Default::default()
                }),
                InlineElement::FootnoteRef { id: "1".to_string() },
            ],
            ..Default::default()
        };
        assert_eq!(para.to_markdown(), "Some text[^1]");
    }

    #[test]
    fn test_list_types() {
        let bullet = Paragraph {
            runs: vec![TextRun { text: "item".to_string(), ..Default::default() }],
            is_list_item: true,
            list_type: Some("bullet".to_string()),
            indent_level: 0,
            ..Default::default()
        };
        assert_eq!(bullet.to_markdown(), "- item");

        let decimal = Paragraph {
            runs: vec![TextRun { text: "item".to_string(), ..Default::default() }],
            is_list_item: true,
            list_type: Some("decimal".to_string()),
            indent_level: 0,
            ..Default::default()
        };
        assert_eq!(decimal.to_markdown(), "1. item");

        let lower = Paragraph {
            runs: vec![TextRun { text: "item".to_string(), ..Default::default() }],
            is_list_item: true,
            list_type: Some("lowerLetter".to_string()),
            indent_level: 1,
            ..Default::default()
        };
        assert_eq!(lower.to_markdown(), "  a) item");
    }

    #[test]
    fn test_korean_numbering() {
        assert_eq!(ganada_marker(0), '\u{AC00}'); // 가
        assert_eq!(ganada_marker(1), '\u{B098}'); // 나
        assert_eq!(chosung_marker(0), '\u{3131}'); // ㄱ
        assert_eq!(chosung_marker(1), '\u{3134}'); // ㄴ
    }

    #[test]
    fn test_table_vmerge() {
        let table = DocxTable {
            rows: vec![
                vec![
                    TableCell { content: "Merged".to_string(), col_span: 1, row_span: 1, v_merge_continue: false },
                    TableCell { content: "B".to_string(), col_span: 1, row_span: 1, v_merge_continue: false },
                ],
                vec![
                    TableCell { content: String::new(), col_span: 1, row_span: 1, v_merge_continue: true },
                    TableCell { content: "D".to_string(), col_span: 1, row_span: 1, v_merge_continue: false },
                ],
            ],
            has_header: true,
        };

        let md = table.to_markdown();
        assert!(md.contains("| Merged | B |"));
        assert!(md.contains("|  | D |")); // vMerge continuation is empty
    }

    #[test]
    fn test_table_markdown() {
        let table = DocxTable {
            rows: vec![
                vec![
                    TableCell { content: "A".to_string(), col_span: 1, row_span: 1, v_merge_continue: false },
                    TableCell { content: "B".to_string(), col_span: 1, row_span: 1, v_merge_continue: false },
                ],
                vec![
                    TableCell { content: "1".to_string(), col_span: 1, row_span: 1, v_merge_continue: false },
                    TableCell { content: "2".to_string(), col_span: 1, row_span: 1, v_merge_continue: false },
                ],
            ],
            has_header: true,
        };

        let md = table.to_markdown();
        assert!(md.contains("| A | B |"));
        assert!(md.contains("| --- | --- |"));
        assert!(md.contains("| 1 | 2 |"));
    }

    #[test]
    fn test_footnote_definitions_in_document() {
        let doc = DocxDocument {
            paragraphs: vec![Paragraph {
                runs: vec![TextRun { text: "Hello".to_string(), ..Default::default() }],
                inlines: vec![
                    InlineElement::Run(TextRun { text: "Hello".to_string(), ..Default::default() }),
                    InlineElement::FootnoteRef { id: "1".to_string() },
                ],
                ..Default::default()
            }],
            tables: vec![],
            images: vec![],
            metadata: DocxMetadata::default(),
            footnotes: vec![("1".to_string(), "This is a footnote.".to_string())],
            endnotes: vec![],
        };

        let md = doc.to_markdown();
        assert!(md.contains("Hello[^1]"));
        assert!(md.contains("[^1]: This is a footnote."));
    }

    #[test]
    fn test_parse_notes_xml() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:footnotes xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:footnote w:type="separator" w:id="0">
    <w:p><w:r><w:t>sep</w:t></w:r></w:p>
  </w:footnote>
  <w:footnote w:id="1">
    <w:p><w:r><w:t>First footnote text</w:t></w:r></w:p>
  </w:footnote>
  <w:footnote w:id="2">
    <w:p><w:r><w:rPr><w:b/></w:rPr><w:t>Bold note</w:t></w:r></w:p>
  </w:footnote>
</w:footnotes>"#;

        let notes = DocxParser::<std::io::BufReader<std::fs::File>>::parse_notes_xml(xml);
        assert_eq!(notes.get("1").unwrap(), "First footnote text");
        assert_eq!(notes.get("2").unwrap(), "**Bold note**");
        assert!(notes.get("0").is_none()); // separator skipped
    }
}
