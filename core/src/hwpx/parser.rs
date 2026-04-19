//! HWPX parser implementation with table and character formatting support

use crate::utils::bounded_io::{
    read_limited, read_limited_to_string, MAX_HWPX_BINDATA, MAX_HWPX_XML,
};
use std::collections::HashMap;
use std::fs::File;
use std::io::{self, Read, Seek, Cursor};
use std::path::Path;
use zip::ZipArchive;

/// Character style properties
#[derive(Debug, Clone, Default)]
pub struct CharStyle {
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strikeout: bool,
    /// 강조점 (방점) — 한국 공공 문서에서 핵심 용어 강조에 쓰인다.
    /// OWPML symMark 속성이 NONE 이외(DOT/CIRCLE/TICK/TILDE/MIDDLE_DOT/COLON)
    /// 면 true. 마크다운 출력은 `<mark>` 태그로 감싸 의미 정보를 보존한다.
    pub emphasis_dot: bool,
}

/// Image information from HWPX file
#[derive(Debug, Clone)]
pub struct ImageInfo {
    pub id: String,           // image1, image2, ...
    pub path: String,         // BinData/image1.bmp
    pub media_type: String,   // image/bmp, image/png
    pub data: Vec<u8>,        // actual binary data
}

/// HWPX document parser, generic over the underlying reader type.
///
/// The default type parameter `File` preserves backward compatibility.
pub struct HwpxParser<R: Read + Seek = File> {
    archive: ZipArchive<R>,
    char_styles: HashMap<u32, CharStyle>,
    heading_styles: HashMap<u32, u8>,
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
///
/// `spans` is a parallel grid to `cells`: `spans[r][c] = (col_span, row_span)`.
/// When populated, rendering switches to HTML `<table>` so `rowspan`/`colspan`
/// survive (GFM pipe tables cannot express merged cells). Empty = no merge
/// info captured (markdown-only renderer).
///
/// Ported from chrisryugj/kordoc `src/table/builder.ts:tableToHtml` (2026-04-09,
/// commit f68e825). Shadow span cells carry `(0, 0)` so the HTML renderer
/// skips them — the origin cell owns the visible content and the span attrs.
#[derive(Debug, Clone)]
pub struct Table {
    pub rows: usize,
    pub cols: usize,
    pub cells: Vec<Vec<String>>,
    pub has_header: bool,
    pub spans: Vec<Vec<(u16, u16)>>,
}

impl Table {
    /// True if any cell has `colSpan > 1` or `rowSpan > 1`.
    pub fn has_merged_cells(&self) -> bool {
        self.spans
            .iter()
            .any(|row| row.iter().any(|&(cs, rs)| cs > 1 || rs > 1))
    }

    /// Convert table to Markdown, or HTML `<table>` when merged cells exist.
    ///
    /// Same rendering rules as the HWP 5.x `build_gfm_table`:
    /// - 1-column wrapper tables → unwrap to plain paragraphs
    /// - Header separator always emitted after row 0 (GFM requires it)
    /// - Newlines inside cells become `<br>`
    /// - Pipes inside cells are escaped as `\|`
    /// - Header separator width matches actual column count
    ///
    /// Merged cells (colspan/rowspan) cannot be expressed in GFM, so the
    /// renderer emits HTML `<table>` instead. Markdown viewers pass HTML
    /// through, so this is a strict upgrade for span fidelity.
    pub fn to_markdown(&self) -> String {
        if self.cells.is_empty() || self.cols == 0 {
            return String::new();
        }

        // Merged cells → HTML (markdown cannot express rowspan/colspan)
        if self.has_merged_cells() {
            return self.to_html();
        }

        // 1-column layout wrapper → unwrap to paragraphs
        if self.cols == 1 {
            return self.cells.iter()
                .filter_map(|row| row.first().map(|s| s.trim().to_string()))
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
                .join("\n\n");
        }

        let mut md = String::new();
        for (row_idx, row) in self.cells.iter().enumerate() {
            md.push('|');
            for cell in row {
                let escaped = cell.trim().replace('\n', "<br>").replace('|', "\\|");
                md.push(' ');
                md.push_str(&escaped);
                md.push_str(" |");
            }
            md.push('\n');

            // GFM header separator after the first row (always, not just when has_header)
            if row_idx == 0 {
                md.push('|');
                for _ in 0..self.cols {
                    md.push_str(" --- |");
                }
                md.push('\n');
            }
        }

        md
    }

    /// Emit HTML `<table>` preserving `rowspan`/`colspan`.
    ///
    /// Shadow span cells — where `spans[r][c] == (0, 0)` — are skipped; the
    /// origin cell owns the visible content and the span attrs. First row is
    /// rendered as `<th>`, subsequent rows as `<td>`. Newlines inside cells
    /// become `<br>`. Text is HTML-escaped (`<`, `>`, `&`).
    pub fn to_html(&self) -> String {
        let mut out = String::from("<table>\n");
        for (r, row) in self.cells.iter().enumerate() {
            let tag = if r == 0 && self.has_header { "th" } else if r == 0 { "th" } else { "td" };
            let mut row_html = String::new();
            for (c, text) in row.iter().enumerate() {
                // Default to (1,1) if spans grid is short (defensive)
                let (cs, rs) = self
                    .spans
                    .get(r)
                    .and_then(|sr| sr.get(c))
                    .copied()
                    .unwrap_or((1, 1));
                // Shadow cell → skip
                if cs == 0 && rs == 0 {
                    continue;
                }
                let escaped = html_escape(text.trim()).replace('\n', "<br>");
                row_html.push('<');
                row_html.push_str(tag);
                if cs > 1 {
                    row_html.push_str(&format!(" colspan=\"{}\"", cs));
                }
                if rs > 1 {
                    row_html.push_str(&format!(" rowspan=\"{}\"", rs));
                }
                row_html.push('>');
                row_html.push_str(&escaped);
                row_html.push_str("</");
                row_html.push_str(tag);
                row_html.push('>');
            }
            if !row_html.is_empty() {
                out.push_str("<tr>");
                out.push_str(&row_html);
                out.push_str("</tr>\n");
            }
        }
        out.push_str("</table>");
        out
    }
}

/// Minimal HTML escaper for cell text (`&`, `<`, `>`).
fn html_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            _ => out.push(ch),
        }
    }
    out
}

impl HwpxParser<File> {
    /// Open an HWPX file from disk.
    pub fn open<P: AsRef<Path>>(path: P) -> io::Result<Self> {
        let file = File::open(path)?;
        let archive = ZipArchive::new(file)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        Ok(Self {
            archive,
            char_styles: HashMap::new(),
            heading_styles: HashMap::new(),
        })
    }
}

impl HwpxParser<Cursor<Vec<u8>>> {
    /// Create an HWPX parser from in-memory data.
    ///
    /// This constructor is used for WASM and other environments
    /// where file system access is unavailable.
    pub fn from_bytes(data: Vec<u8>) -> io::Result<Self> {
        let cursor = Cursor::new(data);
        let archive = ZipArchive::new(cursor)
            .map_err(|e| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("Invalid HWPX: {}", e),
                )
            })?;
        Ok(Self {
            archive,
            char_styles: HashMap::new(),
            heading_styles: HashMap::new(),
        })
    }
}

impl<R: Read + Seek> HwpxParser<R> {
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

    /// Parse header.xml to extract character style definitions and heading styles
    fn parse_header_styles(&mut self) -> io::Result<()> {
        if let Ok(mut file) = self.archive.by_name("Contents/header.xml") {
            let content = read_limited_to_string(&mut file, MAX_HWPX_XML)?;
            self.char_styles = parse_char_properties(&content);
            self.heading_styles = parse_heading_styles(&content);
        }
        Ok(())
    }

    /// Read version info
    fn read_version(&mut self) -> io::Result<String> {
        if let Ok(mut file) = self.archive.by_name("version.xml") {
            let content = read_limited_to_string(&mut file, MAX_HWPX_XML)?;
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
        read_limited_to_string(&mut file, MAX_HWPX_XML)
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
                    let content = read_limited_to_string(&mut file, MAX_HWPX_XML)?;

                    let (text, tables) = parse_section_xml(&content, &self.char_styles, &self.heading_styles);
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
            let content = read_limited_to_string(&mut file, MAX_HWPX_XML)?;

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
                let data = read_limited(&mut file, MAX_HWPX_BINDATA)?;
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

/// Determine whether a `<hh:strikeout shape="...">` value represents a real
/// rendered strikethrough.
///
/// Whitelist of OWPML LineSym2 strike shapes that Hancom actually renders.
/// Values like `3D` are emitted by Hancom's exporter as defaults on body-text
/// charPr definitions and must NOT be treated as strikethrough — otherwise
/// large portions of normal text get wrapped in `~~...~~` in the markdown
/// output. Unknown values are treated as no-strike (fail-closed).
fn is_real_strikeout_shape(shape: &str) -> bool {
    matches!(
        shape,
        "CONT"
            | "SOLID"
            | "DOT"
            | "DASH"
            | "DASH_DOT"
            | "DASH_DOT_DOT"
            | "LONG_DASH"
            | "DOUBLE"
            | "DOUBLE_SLIM"
            | "SLIM_THICK"
            | "THICK_SLIM"
            | "SLIM_THICK_SLIM"
            | "WAVE"
            | "DOUBLE_WAVE"
            | "CIRCLE"
    )
}

/// Determine whether a `<hh:underline type="...">` value represents a real
/// rendered underline.
///
/// OWPML defines `type` as position of the line relative to the baseline:
///   - `NONE`   — no underline
///   - `BOTTOM` — under the text (standard underline)
///   - `TOP`    — above the text (used in vertical layouts)
///
/// Like `strikeout`, Hancom can emit placeholder/unknown values on default
/// charPr entries. Treating "anything but NONE" as underline (blacklist) is
/// a forward-compatibility hazard identical to the strike shape bug fixed
/// in the previous commit — a future placeholder (e.g. `"3D"`) on
/// `<hh:underline type>` would silently underline entire bodies.
///
/// Whitelist only the two positions Hancom actually renders; unknown values
/// are fail-closed to no-underline.
fn is_real_underline_type(underline_type: &str) -> bool {
    matches!(underline_type, "BOTTOM" | "TOP")
}

/// Determine whether a `<hh:charPr symMark="...">` value represents a real
/// emphasis dot (강조점 / 방점).
///
/// OWPML symMark values:
///   - `NONE`        — no mark
///   - `DOT`         — ● filled circle
///   - `CIRCLE`      — ○ open circle
///   - `TICK`        — ˇ caron
///   - `TILDE`       — ˜ tilde
///   - `MIDDLE_DOT`  — ･ halfwidth middle dot
///   - `COLON`       — : colon (rare)
///
/// Korean government documents and legal texts heavily use emphasis dots
/// for highlighting important terms (see 공문서 작성 규정 별표 2). Without
/// preserving this signal, the extracted markdown loses author intent that
/// matters to downstream LLM tasks.
///
/// Whitelist approach mirrors `is_real_strikeout_shape` / `is_real_underline_type`:
/// unknown future placeholders are fail-closed to no-emphasis.
fn is_real_emphasis_mark(sym_mark: &str) -> bool {
    matches!(
        sym_mark,
        "DOT" | "CIRCLE" | "TICK" | "TILDE" | "MIDDLE_DOT" | "COLON"
    )
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

                    // Check for underline. Whitelist OWPML position values.
                    // Symmetric to the strikeout whitelist: unknown / future
                    // placeholder values on default charPr must NOT be rendered
                    // as underline, or entire body text will be underlined.
                    if let Some(underline_pos) = char_pr_xml.find("<hh:underline ") {
                        let underline_xml = &char_pr_xml[underline_pos..];
                        if let Some(type_val) = extract_attr(underline_xml, "type") {
                            style.underline = is_real_underline_type(&type_val);
                        }
                    }

                    // Check for strikeout. Whitelist known OWPML LineSym2 values.
                    // Hancom Office sometimes emits non-spec values like `shape="3D"`
                    // as a placeholder default that is NOT rendered as strikethrough.
                    // Treating any non-NONE value as strike causes body text to be
                    // wrapped in ~~...~~ (regression: 251113 venture press release).
                    if let Some(strike_pos) = char_pr_xml.find("<hh:strikeout ") {
                        let strike_xml = &char_pr_xml[strike_pos..];
                        if let Some(shape_val) = extract_attr(strike_xml, "shape") {
                            style.strikeout = is_real_strikeout_shape(&shape_val);
                        }
                    }

                    // Check for emphasis dot (강조점 / 방점). The symMark attribute
                    // lives on <hh:charPr> itself, not a sub-element.
                    if let Some(sym_mark) = extract_attr(char_pr_xml, "symMark") {
                        style.emphasis_dot = is_real_emphasis_mark(&sym_mark);
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

/// Parse `<hh:style>` elements from header.xml to identify heading (outline) styles.
///
/// Looks for styles whose `name` starts with "개요" or `engName` starts with "Outline".
/// The trailing number gives the heading level (1-7). Returns a map of `styleId -> level`.
fn parse_heading_styles(header_xml: &str) -> HashMap<u32, u8> {
    let mut map = HashMap::new();
    let mut pos = 0;

    while let Some(start) = header_xml[pos..].find("<hh:style ") {
        let style_start = pos + start;

        // Find end of this style element (self-closing or opening tag end)
        let tag_end = header_xml[style_start..]
            .find("/>")
            .map(|i| style_start + i + 2)
            .or_else(|| header_xml[style_start..].find('>').map(|i| style_start + i + 1));

        let Some(tag_end) = tag_end else { break; };
        let style_tag = &header_xml[style_start..tag_end];

        if let Some(id_str) = extract_attr(style_tag, "id") {
            if let Ok(id) = id_str.parse::<u32>() {
                let level = extract_attr(style_tag, "name")
                    .and_then(|n| extract_outline_level(&n))
                    .or_else(|| {
                        extract_attr(style_tag, "engName")
                            .and_then(|n| extract_outline_level(&n))
                    });
                if let Some(lvl) = level {
                    map.insert(id, lvl);
                }
            }
        }

        pos = tag_end;
    }

    map
}

/// Extract heading level from a style name like "개요 1", "Outline 3", etc.
fn extract_outline_level(name: &str) -> Option<u8> {
    let trimmed = name.trim();
    if let Some(rest) = trimmed.strip_prefix("개요") {
        return rest.trim().parse::<u8>().ok().filter(|&l| (1..=7).contains(&l));
    }
    if let Some(rest) = trimmed.strip_prefix("Outline") {
        return rest.trim().parse::<u8>().ok().filter(|&l| (1..=7).contains(&l));
    }
    None
}

/// Remove `<hp:secPr>...</hp:secPr>` blocks from section XML.
fn strip_sec_pr(xml: &str) -> String {
    let mut out = String::with_capacity(xml.len());
    let mut pos = 0;
    while let Some(start) = xml[pos..].find("<hp:secPr") {
        let abs_start = pos + start;
        out.push_str(&xml[pos..abs_start]);
        if let Some(end) = xml[abs_start..].find("</hp:secPr>") {
            pos = abs_start + end + "</hp:secPr>".len();
        } else if let Some(end) = xml[abs_start..].find("/>") {
            pos = abs_start + end + 2;
        } else {
            pos = abs_start;
            break;
        }
    }
    out.push_str(&xml[pos..]);
    out
}

/// Depth-aware finder for the MATCHING close tag when the same tag can nest.
///
/// Starts scanning at `from`, assumes we've already opened 1 level of `open`.
/// Returns the absolute index of the close tag that balances the opened depth,
/// or `None` if the XML is unbalanced.
///
/// Critical for HWPX: tables (`<hp:tbl>`) and cells (`<hp:tc>`) can nest, and
/// naïve `find("</hp:tbl>")` returns the FIRST close which is the inner one —
/// causing the outer table to truncate and lose all nested-cell content.
/// Reference: pyhwpx/kordoc use recursive tree walkers; we need the same
/// correctness without pulling in a DOM parser.
fn find_matching_close(xml: &str, from: usize, open: &str, close: &str) -> Option<usize> {
    let mut depth: usize = 1;
    let mut scan = from;
    while scan < xml.len() && depth > 0 {
        let o = xml[scan..].find(open).map(|i| scan + i);
        let c = xml[scan..].find(close).map(|i| scan + i);
        match (o, c) {
            (Some(oo), Some(cc)) if oo < cc => {
                depth += 1;
                scan = oo + open.len();
            }
            (_, Some(cc)) => {
                depth -= 1;
                if depth == 0 {
                    return Some(cc);
                }
                scan = cc + close.len();
            }
            _ => return None,
        }
    }
    None
}

/// Parse section XML and extract text with tables
fn parse_section_xml(
    xml: &str,
    char_styles: &HashMap<u32, CharStyle>,
    heading_styles: &HashMap<u32, u8>,
) -> (String, Vec<Table>) {
    // Strip <hp:secPr>...</hp:secPr> section-property blocks before processing.
    let xml = strip_sec_pr(xml);
    let xml = xml.as_str();

    let mut result = String::new();
    let mut tables = Vec::new();
    let mut pos = 0;
    // Section-wide counter keeps `[중첩 테이블 #N]` numbering stable and
    // unique across every table encountered in this section, so a reader
    // can cross-reference markers with the hoisted blocks below.
    let mut nested_counter = NestedTableCounter::default();

    while pos < xml.len() {
        // Look for table start
        if let Some(tbl_start) = xml[pos..].find("<hp:tbl ") {
            let tbl_pos = pos + tbl_start;

            // Extract text before table
            let before_table = &xml[pos..tbl_pos];
            result.push_str(&extract_text_with_formatting(before_table, char_styles, heading_styles));

            // Find matching table close — must be depth-aware because HWPX
            // tables can nest. See find_matching_close() for rationale.
            let scan_from = tbl_pos + "<hp:tbl ".len();
            if let Some(tbl_end) = find_matching_close(xml, scan_from, "<hp:tbl ", "</hp:tbl>") {
                let table_xml = &xml[tbl_pos..tbl_end + 9];

                let mut hoisted: Vec<Table> = Vec::new();
                if let Some(table) = parse_table_ctx(
                    table_xml,
                    char_styles,
                    &mut nested_counter,
                    &mut hoisted,
                    0,
                ) {
                    result.push_str("\n\n");
                    result.push_str(&table.to_markdown());
                    result.push('\n');
                    tables.push(table);
                    // Emit each hoisted "big nested" table right after its
                    // parent, preceded by a header that carries the marker
                    // number so `[중첩 테이블 #N]` in the parent maps clearly.
                    for nested in hoisted {
                        let marker_n = tables.len(); // rough — real N is in marker text
                        let _ = marker_n;
                        result.push_str("\n");
                        result.push_str(&nested.to_markdown());
                        result.push('\n');
                        tables.push(nested);
                    }
                }

                pos = tbl_end + 9;
            } else {
                pos = tbl_pos + 1;
            }
        } else {
            // No more tables, extract remaining text
            result.push_str(&extract_text_with_formatting(&xml[pos..], char_styles, heading_styles));
            break;
        }
    }

    // Clean up result
    let cleaned = clean_text(&result);
    (cleaned, tables)
}

/// Classification threshold copied from kordoc's `handleNestedTable`
/// (chrisryugj/kordoc `src/hwpx/parser.ts:606`). A nested table with at
/// least this many rows AND columns is considered "real data" and gets
/// hoisted to its own markdown block; anything smaller is treated as a
/// layout wrapper and flattened into the parent cell's text. The parent
/// cell receives a `[중첩 테이블 #N]` marker in both cases so readers
/// and LLMs can see the parent→child relationship even after hoisting.
const NESTED_TABLE_MIN_ROWS: usize = 3;
const NESTED_TABLE_MIN_COLS: usize = 2;

/// Maximum table nesting depth the parser will recurse through. kordoc
/// uses a single global `MAX_XML_DEPTH` on its DOM walker; our parser is
/// string-based and most internal walkers are iterative, so the only
/// unbounded recursion path is `parse_table_ctx` → `preprocess_nested_tables`
/// → `parse_table_ctx`. Cap that chain to defend against hand-crafted or
/// malformed HWPX whose nesting would blow the stack.
///
/// Real-world HWPX documents never exceed depth 3-4 even for elaborate
/// forms; 16 leaves comfortable headroom.
const MAX_NESTED_TABLE_DEPTH: usize = 16;

/// Monotonic counter for nested-table marker numbering. Threaded through
/// `parse_table_ctx` → `preprocess_nested_tables` so markers remain stable
/// within a single section/document traversal.
#[derive(Default)]
struct NestedTableCounter {
    n: u32,
}

impl NestedTableCounter {
    fn next_id(&mut self) -> u32 {
        self.n += 1;
        self.n
    }
}

/// Flatten a nested table to a single compact text string suitable for
/// embedding inside a parent markdown cell (where newlines and pipes are
/// forbidden by GFM table syntax). Rows are joined with `; `, cells with
/// ` | `. Empty cells are dropped.
fn flatten_table_to_text(t: &Table) -> String {
    t.cells
        .iter()
        .map(|row| {
            row.iter()
                .map(|c| c.trim())
                .filter(|c| !c.is_empty())
                .collect::<Vec<_>>()
                .join(" | ")
        })
        .filter(|row| !row.trim().is_empty())
        .collect::<Vec<_>>()
        .join("; ")
}

/// Walk `cell_xml`, locate every top-level `<hp:tbl ...>` (depth-aware so
/// we don't re-enter nested-in-nested), classify each against the
/// `NESTED_TABLE_MIN_*` thresholds, and rewrite the XML span.
///
/// Returns the rewritten cell XML with marker text substituted in place of
/// nested tables, and pushes every "big" nested table into `separate_out`
/// so the caller can emit them as sibling blocks after the outer table.
fn preprocess_nested_tables(
    cell_xml: &str,
    char_styles: &HashMap<u32, CharStyle>,
    counter: &mut NestedTableCounter,
    separate_out: &mut Vec<Table>,
    depth: usize,
) -> String {
    // Stop recursing once we'd exceed the depth cap. At this point we still
    // emit the rest of the cell verbatim (so text in deeply nested tables
    // isn't lost outright) but do NOT descend into another parse pass.
    if depth >= MAX_NESTED_TABLE_DEPTH {
        return cell_xml.to_string();
    }

    let mut result = String::new();
    let mut pos = 0;

    while let Some(rel) = cell_xml[pos..].find("<hp:tbl ") {
        let abs = pos + rel;
        result.push_str(&cell_xml[pos..abs]);

        let scan_from = abs + "<hp:tbl ".len();
        let close_idx = match find_matching_close(cell_xml, scan_from, "<hp:tbl ", "</hp:tbl>") {
            Some(i) => i,
            None => {
                // Malformed — keep the remainder as-is.
                result.push_str(&cell_xml[abs..]);
                return result;
            }
        };
        let nested_xml = &cell_xml[abs..close_idx + "</hp:tbl>".len()];
        let id = counter.next_id();
        let marker = format!("[중첩 테이블 #{}]", id);

        match parse_table_ctx(nested_xml, char_styles, counter, separate_out, depth + 1) {
            Some(nested_table) => {
                let is_big = nested_table.rows >= NESTED_TABLE_MIN_ROWS
                    && nested_table.cols >= NESTED_TABLE_MIN_COLS;
                if is_big {
                    separate_out.push(nested_table);
                    // Parent cell keeps only the reference — big nested is
                    // hoisted to its own block for clean GFM rendering.
                    result.push_str(&marker);
                } else {
                    // Small / layout wrapper — inline flatten under the marker
                    // so no data is lost and the cell remains GFM-legal.
                    let flat = flatten_table_to_text(&nested_table);
                    result.push_str(&marker);
                    if !flat.is_empty() {
                        result.push(' ');
                        result.push_str(&flat);
                    }
                }
            }
            None => {
                // Unparseable — fall back to keeping the raw XML so downstream
                // text extraction at least picks up the inner <hp:t> runs.
                result.push_str(nested_xml);
            }
        }

        pos = close_idx + "</hp:tbl>".len();
    }

    result.push_str(&cell_xml[pos..]);
    result
}

/// Parse a single HWPX table from XML.
///
/// Walks `<hp:tc>` elements collecting cell text + position metadata from
/// `<hp:cellAddr colAddr rowAddr>` and `<hp:cellSpan colSpan rowSpan>` child
/// elements (HWPX spec). Cells are placed in a `rows × cols` grid using direct
/// addresses, with span shadow-fill — same algorithm as the HWP 5.x parser's
/// `build_gfm_table` (vendor/markdown-media/core/src/hwp/parser.rs).
///
/// Without addr-based placement, mdm's HWPX parser previously rendered merged
/// header tables with mis-aligned columns and dropped rows where the cell
/// count didn't match `rowCnt × colCnt`.
///
/// Wraps `parse_table_ctx` with a fresh counter — callers who want marker
/// numbering to stay coherent across multiple top-level tables should call
/// `parse_table_ctx` directly and supply a shared counter.
fn parse_table(xml: &str, char_styles: &HashMap<u32, CharStyle>) -> Option<Table> {
    let mut counter = NestedTableCounter::default();
    let mut _separate = Vec::new();
    parse_table_ctx(xml, char_styles, &mut counter, &mut _separate, 0)
}

/// Like `parse_table` but threads a counter + out-list for nested-table
/// hoisting. Big nested tables (≥`NESTED_TABLE_MIN_ROWS` AND ≥
/// `NESTED_TABLE_MIN_COLS`) are appended to `separate_out`; their positions
/// in the parent cell are replaced with `[중첩 테이블 #N]` markers. Small
/// nested tables are flattened into the parent cell under the same marker.
fn parse_table_ctx(
    xml: &str,
    char_styles: &HashMap<u32, CharStyle>,
    counter: &mut NestedTableCounter,
    separate_out: &mut Vec<Table>,
    depth: usize,
) -> Option<Table> {
    let rows: usize = extract_attr(xml, "rowCnt").and_then(|s| s.parse().ok()).unwrap_or(0);
    let cols: usize = extract_attr(xml, "colCnt").and_then(|s| s.parse().ok()).unwrap_or(0);
    if rows == 0 || cols == 0 {
        return None;
    }

    // Bound to avoid runaway allocation on malformed files
    let rows = rows.min(1024);
    let cols = cols.min(256);

    // Collect every <hp:tc> with its address + span + text
    #[derive(Clone)]
    struct CellMeta {
        col_addr: usize,
        row_addr: usize,
        col_span: usize,
        row_span: usize,
        text: String,
        has_addr: bool,
    }

    let mut collected: Vec<CellMeta> = Vec::new();
    let mut has_header = false;
    let mut sequential_idx: usize = 0; // for files without explicit addr
    let mut pos = 0;

    while let Some(tc_start) = xml[pos..].find("<hp:tc ") {
        let tc_pos = pos + tc_start;

        if extract_attr(&xml[tc_pos..], "header").as_deref() == Some("1") {
            has_header = true;
        }

        // Depth-aware close finder — cells can contain nested tables with
        // nested cells. Without depth tracking, the inner cell's `</hp:tc>`
        // would close the outer cell prematurely and we'd mis-count rows.
        let scan_from = tc_pos + "<hp:tc ".len();
        let Some(tc_end) = find_matching_close(xml, scan_from, "<hp:tc ", "</hp:tc>")
        else { break; };
        let cell_xml = &xml[tc_pos..tc_end];

        // <hp:cellAddr colAddr="..." rowAddr="..."/>
        let (col_addr, row_addr, has_addr) = match cell_xml.find("<hp:cellAddr ") {
            Some(addr_start) => {
                let addr_xml = &cell_xml[addr_start..];
                let ca = extract_attr(addr_xml, "colAddr").and_then(|s| s.parse().ok());
                let ra = extract_attr(addr_xml, "rowAddr").and_then(|s| s.parse().ok());
                match (ca, ra) {
                    (Some(c), Some(r)) => (c, r, true),
                    _ => (sequential_idx % cols, sequential_idx / cols, false),
                }
            }
            None => (sequential_idx % cols, sequential_idx / cols, false),
        };

        // <hp:cellSpan colSpan="..." rowSpan="..."/>
        let (col_span, row_span) = match cell_xml.find("<hp:cellSpan ") {
            Some(span_start) => {
                let span_xml = &cell_xml[span_start..];
                let cs = extract_attr(span_xml, "colSpan").and_then(|s| s.parse().ok()).unwrap_or(1usize);
                let rs = extract_attr(span_xml, "rowSpan").and_then(|s| s.parse().ok()).unwrap_or(1usize);
                (cs.max(1).min(cols), rs.max(1).min(rows))
            }
            None => (1, 1),
        };

        // Pre-process: hoist big nested tables to `separate_out` and drop
        // `[중첩 테이블 #N]` markers in place so the GFM-renderable cell text
        // never contains nested pipes or newlines.
        let cell_xml_processed =
            preprocess_nested_tables(cell_xml, char_styles, counter, separate_out, depth);
        let text = extract_cell_text(&cell_xml_processed, char_styles);
        collected.push(CellMeta {
            col_addr,
            row_addr,
            col_span,
            row_span,
            text,
            has_addr,
        });
        sequential_idx += 1;
        // tc_end is now ABSOLUTE (from find_matching_close) — advance past
        // `</hp:tc>` to skip the entire closed cell including any nested tables.
        pos = tc_end + "</hp:tc>".len();
    }

    if collected.is_empty() {
        return None;
    }

    // Build grid: None = unplaced, Some("") = shadow span fill, Some("text") = cell
    let mut grid: Vec<Vec<Option<String>>> = vec![vec![None; cols]; rows];
    // Parallel span grid: (col_span, row_span). (1,1)=normal cell, (0,0)=shadow.
    let mut span_grid: Vec<Vec<(u16, u16)>> = vec![vec![(1, 1); cols]; rows];
    let mut has_merged = false;

    let any_addr = collected.iter().any(|c| c.has_addr);

    if any_addr {
        for cell in &collected {
            if cell.row_addr >= rows || cell.col_addr >= cols {
                continue;
            }
            grid[cell.row_addr][cell.col_addr] = Some(cell.text.clone());
            if cell.col_span > 1 || cell.row_span > 1 {
                has_merged = true;
                span_grid[cell.row_addr][cell.col_addr] =
                    (cell.col_span as u16, cell.row_span as u16);
            }
            for dr in 0..cell.row_span {
                for dc in 0..cell.col_span {
                    if dr == 0 && dc == 0 {
                        continue;
                    }
                    let rr = cell.row_addr + dr;
                    let cc = cell.col_addr + dc;
                    if rr < rows && cc < cols && grid[rr][cc].is_none() {
                        grid[rr][cc] = Some(String::new());
                        span_grid[rr][cc] = (0, 0); // shadow
                    }
                }
            }
        }
    } else {
        // Sequential fill fallback (for files that omit cellAddr)
        let mut idx = 0usize;
        for r in 0..rows {
            for c in 0..cols {
                if grid[r][c].is_some() {
                    continue;
                }
                if idx >= collected.len() {
                    break;
                }
                let cell = &collected[idx];
                idx += 1;
                grid[r][c] = Some(cell.text.clone());
                if cell.col_span > 1 || cell.row_span > 1 {
                    has_merged = true;
                    span_grid[r][c] = (cell.col_span as u16, cell.row_span as u16);
                }
                for dr in 0..cell.row_span {
                    for dc in 0..cell.col_span {
                        if dr == 0 && dc == 0 {
                            continue;
                        }
                        let rr = r + dr;
                        let cc = c + dc;
                        if rr < rows && cc < cols && grid[rr][cc].is_none() {
                            grid[rr][cc] = Some(String::new());
                            span_grid[rr][cc] = (0, 0); // shadow
                        }
                    }
                }
            }
        }
    }

    // Drop fully-empty rows (information-free shadow noise) — but ONLY when
    // no merged cells exist, otherwise span indices would drift.
    let (cells, spans): (Vec<Vec<String>>, Vec<Vec<(u16, u16)>>) = if has_merged {
        let cells: Vec<Vec<String>> = grid
            .into_iter()
            .map(|row| {
                row.into_iter()
                    .map(|cell| cell.unwrap_or_default())
                    .collect()
            })
            .collect();
        (cells, span_grid)
    } else {
        let mut out_cells: Vec<Vec<String>> = Vec::new();
        for row in grid {
            let row_str: Vec<String> = row
                .into_iter()
                .map(|cell| cell.unwrap_or_default())
                .collect();
            if row_str.iter().any(|c| !c.trim().is_empty()) {
                out_cells.push(row_str);
            }
        }
        (out_cells, Vec::new())
    };

    if cells.is_empty() {
        return None;
    }

    Some(Table {
        rows: cells.len(),
        cols,
        cells,
        has_header,
        spans,
    })
}

/// Extract cell text from cell XML, applying bold/italic formatting from `<hp:run>` attributes.
///
/// Walks all `<hp:t>` runs (with or without attrs) AND any nested `<hp:drawText>`
/// (textbox) content. Multiple `<hp:p>` paragraphs inside a cell are joined
/// with `\n` (preserved as `<br>` by the table renderer). Text is decoded for
/// XML entities (`&amp;` → `&`, `&lt;` → `<`, etc).
///
/// For each `<hp:run>`, reads the `charPrIDRef` attribute and checks if the
/// referenced style has bold/italic set. Applies `**` / `*` wrapping inline.
fn extract_cell_text(xml: &str, char_styles: &HashMap<u32, CharStyle>) -> String {
    // Collect text segments per <hp:p> paragraph
    let mut paragraphs: Vec<String> = Vec::new();
    let mut p_pos = 0;

    loop {
        // Find next <hp:p>... opening (allow with or without attrs)
        let opening = xml[p_pos..].find("<hp:p>").map(|i| (p_pos + i, 6))
            .or_else(|| xml[p_pos..].find("<hp:p ").map(|i| (p_pos + i, 6)));

        match opening {
            Some((p_start, _)) => {
                let after_open = match xml[p_start..].find('>') {
                    Some(i) => p_start + i + 1,
                    None => break,
                };
                let p_end = match xml[after_open..].find("</hp:p>") {
                    Some(i) => after_open + i,
                    None => break,
                };
                let p_xml = &xml[after_open..p_end];
                let p_text = extract_runs_text_with_formatting(p_xml, char_styles);
                if !p_text.trim().is_empty() {
                    paragraphs.push(p_text);
                }
                p_pos = p_end + 7;
            }
            None => break,
        }
    }

    // Fallback: if no <hp:p> wrappers, just walk runs at the cell level
    if paragraphs.is_empty() {
        let direct = extract_runs_text_with_formatting(xml, char_styles);
        if !direct.trim().is_empty() {
            paragraphs.push(direct);
        }
    }

    paragraphs.join("\n").trim().to_string()
}

/// Like `extract_runs_text` but reads `<hp:run>` wrapper attributes for formatting.
/// Looks up `charPrIDRef` in `char_styles` to apply bold/italic wrapping.
fn extract_runs_text_with_formatting(xml: &str, char_styles: &HashMap<u32, CharStyle>) -> String {
    let mut out = String::new();
    let mut pos = 0;

    while pos < xml.len() {
        // Find the next paragraph-level element we care about: <hp:run> or
        // <hp:dutmal> (ruby annotation). Whichever comes first wins.
        let run_start = xml[pos..].find("<hp:run").map(|i| pos + i);
        let dutmal_start = xml[pos..].find("<hp:dutmal").map(|i| pos + i);

        let next = match (run_start, dutmal_start) {
            (Some(r), Some(d)) if d < r => ("dutmal", d),
            (Some(r), _) => ("run", r),
            (None, Some(d)) => ("dutmal", d),
            (None, None) => break,
        };

        let abs = next.1;
        let after_open = match xml[abs..].find('>') {
            Some(i) => abs + i + 1,
            None => break,
        };

        if next.0 == "dutmal" {
            // <hp:dutmal>...</hp:dutmal>  — ruby wrapper at paragraph level.
            // subText becomes a parenthetical annotation appended to the
            // preceding run's output (which is already in `out`).
            let close = match xml[after_open..].find("</hp:dutmal>") {
                Some(i) => after_open + i,
                None => break,
            };
            let inner = &xml[after_open..close];
            if let Some(annotation) = extract_dutmal_annotation(inner) {
                out.push_str(&annotation);
            }
            pos = close + 12; // len("</hp:dutmal>")
            continue;
        }

        // Self-closing <hp:run .../> — skip
        if xml[abs..after_open].ends_with('/') {
            pos = after_open;
            continue;
        }

        let open_tag = &xml[abs..after_open];

        // Lookup charPrIDRef → char_styles for bold/italic
        let style = extract_attr(open_tag, "charPrIDRef")
            .and_then(|id_str| id_str.parse::<u32>().ok())
            .and_then(|id| char_styles.get(&id));

        let run_end = match xml[after_open..].find("</hp:run>") {
            Some(i) => after_open + i,
            None => break,
        };
        let run_content = &xml[after_open..run_end];

        // Extract text from <hp:t> inside this run
        let text = extract_runs_text(run_content);
        if !text.is_empty() {
            match style {
                Some(s) if s.bold || s.italic || s.underline || s.strikeout || s.emphasis_dot => {
                    out.push_str(&apply_markdown_formatting(&text, s));
                }
                _ => out.push_str(&text),
            }
        }

        pos = run_end + 9; // skip "</hp:run>"
    }

    // If no <hp:run> found, fall back to plain extraction
    if out.is_empty() {
        return extract_runs_text(xml);
    }

    out
}

/// Extract equation script from `<hp:equation>` and emit as a fenced LaTeX
/// block. Hancom's equation script is a near-superset of LaTeX for ~80% of
/// common formulas (`\frac`, `\sqrt`, Greek letters, integrals, sums, etc.)
/// so round-tripping the raw script into `$$...$$` renders usefully in
/// GitHub, Obsidian, and most markdown pipelines — and is directly readable
/// by LLMs, which was the main reason MDM's old `[수식: …]` placeholder was
/// insufficient.
///
///   <hp:equation version="...">
///     <hp:script>\frac{a^2 + b^2}{c}</hp:script>
///   </hp:equation>
///
/// Returns `Some("$$ script $$")` when a non-empty script is present,
/// else `None`. Uses a single-line `$...$` when the script has no newline
/// so short inline equations don't create stray paragraph breaks.
/// Walk a `<hp:run>` body and extract inline text + nested ctrl / equation
/// markers. This is the replacement for the linear `<hp:t>` scanner that used
/// to live in `extract_paragraph_text_with_hyperlinks`. It handles:
///
///   - `<hp:t>`                → decoded text
///   - `<hp:lineBreak/>`       → newline
///   - `<hp:tab.../>`          → tab (width-attributed form too)
///   - `<hp:fwSpace/>`         → fixed-width space
///   - `<hc:img binaryItemIDRef="…"/>` → `[이미지: id]`
///   - `<hp:ctrl>…</hp:ctrl>`  → dispatch on inner element:
///       - `<hp:footNote>`     → `[각주: body]`
///       - `<hp:endNote>`      → `[미주: body]`
///       - `<hp:header>`       → `[머리말: body]`
///       - `<hp:footer>`       → `[꼬리말: body]`
///       - `<hp:bookmark>`     → (skip — invisible anchor)
///       - other               → (skip — colPr, autoNum number, etc.)
///   - `<hp:equation>…</hp:equation>` → `$…$` / `$$…$$`
///   - nested `<hp:run>…</hp:run>` → SKIP (processed as part of the ctrl
///     that contains it; don't leak its `<hp:t>` content into the outer run)
fn walk_run_body(body: &str) -> String {
    let mut out = String::new();
    let mut pos = 0;
    while pos < body.len() {
        // Find earliest relevant element
        let t1 = body[pos..].find("<hp:t>").map(|i| (pos + i, "t"));
        let t2 = body[pos..].find("<hp:t ").map(|i| (pos + i, "t_attr"));
        let tself = body[pos..].find("<hp:t/>").map(|i| (pos + i, "t_self"));
        let lb1 = body[pos..].find("<hp:lineBreak/>").map(|i| (pos + i, "lb"));
        let lb2 = body[pos..].find("<hp:lineBreak ").map(|i| (pos + i, "lb_attr"));
        let tab = body[pos..].find("<hp:tab/>").map(|i| (pos + i, "tab"));
        let tab2 = body[pos..].find("<hp:tab ").map(|i| (pos + i, "tab_attr"));
        let fws = body[pos..].find("<hp:fwSpace").map(|i| (pos + i, "fws"));
        let img = body[pos..].find("<hc:img ").map(|i| (pos + i, "img"));
        let ctrl = body[pos..].find("<hp:ctrl>").map(|i| (pos + i, "ctrl"));
        let ctrl2 = body[pos..].find("<hp:ctrl ").map(|i| (pos + i, "ctrl_attr"));
        let eq1 = body[pos..].find("<hp:equation>").map(|i| (pos + i, "eq"));
        let eq2 = body[pos..].find("<hp:equation ").map(|i| (pos + i, "eq_attr"));
        let nested_run = body[pos..].find("<hp:run ").map(|i| (pos + i, "nested_run"));
        let nested_run2 = body[pos..].find("<hp:run>").map(|i| (pos + i, "nested_run"));

        let candidates = [
            t1, t2, tself, lb1, lb2, tab, tab2, fws, img, ctrl, ctrl2, eq1, eq2,
            nested_run, nested_run2,
        ];
        let Some((abs, kind)) = candidates.iter().filter_map(|c| *c).min_by_key(|(i, _)| *i) else {
            break;
        };

        match kind {
            "t" | "t_attr" => {
                // skip to '>'
                let after_open = match body[abs..].find('>') {
                    Some(i) => abs + i + 1,
                    None => break,
                };
                if let Some(end_rel) = body[after_open..].find("</hp:t>") {
                    let text = &body[after_open..after_open + end_rel];
                    let text = text.replace("<hp:lineBreak/>", "\n");
                    let text = text.replace("<hp:lineBreak />", "\n");
                    let text = text.replace("<hp:tab/>", "\t");
                    let text = replace_attributed_tab(&text);
                    // Decode XML entities (&lt; &gt; &amp; etc) — law.go.kr
                    // embeds literal entity refs in <hp:t> for amendment
                    // markers like "<개정 2014. 3. 24.>".
                    out.push_str(&decode_xml_entities(&text));
                    pos = after_open + end_rel + 7;
                } else {
                    break;
                }
            }
            "t_self" => {
                pos = abs + 7; // len("<hp:t/>")
            }
            "lb" => {
                out.push('\n');
                pos = abs + 15; // len("<hp:lineBreak/>")
            }
            "lb_attr" => {
                out.push('\n');
                pos = match body[abs..].find("/>") {
                    Some(i) => abs + i + 2,
                    None => break,
                };
            }
            "tab" => {
                out.push('\t');
                pos = abs + 9;
            }
            "tab_attr" => {
                out.push('\t');
                pos = match body[abs..].find("/>") {
                    Some(i) => abs + i + 2,
                    None => break,
                };
            }
            "fws" => {
                out.push(' ');
                pos = match body[abs..].find('>') {
                    Some(i) => abs + i + 1,
                    None => break,
                };
            }
            "img" => {
                let after = match body[abs..].find('>') {
                    Some(i) => abs + i + 1,
                    None => break,
                };
                let tag = &body[abs..after];
                if let Some(id) = extract_attr(tag, "binaryItemIDRef") {
                    if !out.is_empty() && !out.ends_with(char::is_whitespace) {
                        out.push(' ');
                    }
                    out.push_str(&format!("[이미지: {}]", id));
                }
                pos = after;
            }
            "ctrl" | "ctrl_attr" => {
                // Find matching </hp:ctrl> — find_matching_close scans from
                // `from`; callers must pass the position AFTER the opening tag.
                let scan_from = match body[abs..].find('>') {
                    Some(i) => abs + i + 1,
                    None => break,
                };
                let close = match find_matching_close(body, scan_from, "<hp:ctrl>", "</hp:ctrl>") {
                    Some(c) => c,
                    None => break,
                };
                let body_start = match body[abs..].find('>') {
                    Some(i) => abs + i + 1,
                    None => break,
                };
                let ctrl_inner = &body[body_start..close];
                if let Some(marker) = extract_ctrl_marker(ctrl_inner) {
                    if !out.is_empty() && !out.ends_with(char::is_whitespace)
                        && !marker.starts_with(' ') && !marker.starts_with('\n')
                    {
                        out.push(' ');
                    }
                    out.push_str(&marker);
                }
                pos = close + 10; // len("</hp:ctrl>")
            }
            "eq" | "eq_attr" => {
                let close = match body[abs..].find("</hp:equation>") {
                    Some(i) => abs + i,
                    None => break,
                };
                let body_start = match body[abs..].find('>') {
                    Some(i) => abs + i + 1,
                    None => break,
                };
                let eq_inner = &body[body_start..close];
                if let Some(block) = extract_equation_markdown(eq_inner) {
                    out.push_str(&block);
                }
                pos = close + 14; // len("</hp:equation>")
            }
            "nested_run" => {
                // Skip entire nested run — its content belongs to the
                // containing ctrl (footnote/endnote subList) and has
                // already been emitted via the ctrl marker.
                let scan_from = match body[abs..].find('>') {
                    Some(i) => abs + i + 1,
                    None => break,
                };
                let close = match find_matching_close(body, scan_from, "<hp:run ", "</hp:run>") {
                    Some(c) => c,
                    None => break,
                };
                pos = close + 9;
            }
            _ => break,
        }
    }
    out
}

/// Dispatch on the element inside `<hp:ctrl>` and return its markdown marker.
/// Returns None for ctrls that don't produce visible output (bookmark, colPr,
/// autoNum metadata, etc.).
fn extract_ctrl_marker(ctrl_inner: &str) -> Option<String> {
    // Order matters: check the most distinctive elements first
    if let Some(fn_pos) = ctrl_inner.find("<hp:footNote") {
        if let Some(close_rel) = ctrl_inner[fn_pos..].find("</hp:footNote>") {
            let body_start = fn_pos + ctrl_inner[fn_pos..].find('>')? + 1;
            let inner = &ctrl_inner[body_start..fn_pos + close_rel];
            return extract_note_content(inner, "각주");
        }
    }
    if let Some(en_pos) = ctrl_inner.find("<hp:endNote") {
        if let Some(close_rel) = ctrl_inner[en_pos..].find("</hp:endNote>") {
            let body_start = en_pos + ctrl_inner[en_pos..].find('>')? + 1;
            let inner = &ctrl_inner[body_start..en_pos + close_rel];
            return extract_note_content(inner, "미주");
        }
    }
    if let Some(h_pos) = ctrl_inner.find("<hp:header") {
        if let Some(close_rel) = ctrl_inner[h_pos..].find("</hp:header>") {
            let body_start = h_pos + ctrl_inner[h_pos..].find('>')? + 1;
            let inner = &ctrl_inner[body_start..h_pos + close_rel];
            return extract_note_content(inner, "머리말");
        }
    }
    if let Some(f_pos) = ctrl_inner.find("<hp:footer") {
        if let Some(close_rel) = ctrl_inner[f_pos..].find("</hp:footer>") {
            let body_start = f_pos + ctrl_inner[f_pos..].find('>')? + 1;
            let inner = &ctrl_inner[body_start..f_pos + close_rel];
            return extract_note_content(inner, "꼬리말");
        }
    }
    // Bookmark, colPr, autoNum, fieldBegin/End, etc. — no visible marker
    None
}

fn extract_equation_markdown(inner: &str) -> Option<String> {
    let script_start = inner.find("<hp:script")?;
    let after_open = script_start + inner[script_start..].find('>')? + 1;
    let script_close = after_open + inner[after_open..].find("</hp:script>")?;
    let raw = &inner[after_open..script_close];
    let decoded = decode_xml_entities(raw);
    let trimmed = decoded.trim();
    if trimmed.is_empty() {
        return None;
    }
    if trimmed.contains('\n') {
        Some(format!("\n\n$$\n{}\n$$\n\n", trimmed))
    } else {
        Some(format!(" ${}$ ", trimmed))
    }
}

/// Extract inline footnote / endnote content as `[각주: ...]` /
/// `[미주: ...]` markers appended at the position of the reference.
///
/// HWPX represents footnotes/endnotes as controls attached to paragraphs:
///
///   <hp:ctrl>
///     <hp:footNote number="1" instId="..." suffixChar="">
///       <hp:subList>
///         <hp:p><hp:run><hp:t>각주 본문</hp:t></hp:run></hp:p>
///       </hp:subList>
///     </hp:footNote>
///   </hp:ctrl>
///
/// For LLM consumption and search, inline expansion is more useful than
/// a pandoc-style `[^N]`/`[^N]: ...` pair — the annotation stays local to
/// the paragraph that owns it. This matches the existing `[이미지: ...]`
/// placeholder convention already used for embedded images.
///
/// Returns `Some("[각주: body]")` when a subList paragraph is found,
/// else `None`. The label argument is "각주" (footnote) or "미주" (endnote).
fn extract_note_content(note_inner: &str, label: &str) -> Option<String> {
    // subList contains one or more <hp:p> paragraphs; concatenate their text.
    let sublist_start = note_inner.find("<hp:subList")?;
    let after_open = sublist_start + note_inner[sublist_start..].find('>')? + 1;
    let sublist_close = after_open + note_inner[after_open..].find("</hp:subList>")?;
    let sublist_xml = &note_inner[after_open..sublist_close];

    let mut body = String::new();
    let mut pos = 0;
    while let Some(p_rel) = sublist_xml[pos..].find("<hp:p") {
        let p_start = pos + p_rel;
        let p_open_end = match sublist_xml[p_start..].find('>') {
            Some(i) => p_start + i + 1,
            None => break,
        };
        let p_close = match sublist_xml[p_open_end..].find("</hp:p>") {
            Some(i) => p_open_end + i,
            None => break,
        };
        let p_inner = &sublist_xml[p_open_end..p_close];
        let p_text = extract_runs_text(p_inner);
        if !p_text.trim().is_empty() {
            if !body.is_empty() {
                body.push(' ');
            }
            body.push_str(p_text.trim());
        }
        pos = p_close + 7;
    }

    let trimmed = body.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(format!("[{}: {}]", label, trimmed))
    }
}

/// Extract the `<hp:subText>...</hp:subText>` content of an `<hp:dutmal>`
/// element and emit it as a parenthetical ruby annotation.
///
/// HWPX represents 덧말(ruby) via `<hp:dutmal>` at paragraph level. The
/// element contains two children:
///   - `<hp:mainText>` — the base text, already duplicated in the
///     surrounding `<hp:run>` flow (rhwp confirms this). We skip it to
///     avoid double-emission.
///   - `<hp:subText>`  — the ruby annotation (reading / hanja / gloss).
///
/// Korean markdown convention for ruby is parenthetical: `한자(hanja)`.
/// This preserves the annotation for LLM consumption without fragile
/// HTML ruby tags, and matches how human authors transcribe these
/// documents. Returns `Some(" (sub)")` when subText is present, else `None`.
fn extract_dutmal_annotation(dutmal_inner: &str) -> Option<String> {
    let sub_start = dutmal_inner.find("<hp:subText")?;
    let after_open = sub_start + dutmal_inner[sub_start..].find('>')? + 1;
    let sub_close = after_open + dutmal_inner[after_open..].find("</hp:subText>")?;
    let sub_xml = &dutmal_inner[after_open..sub_close];
    // Reuse extract_runs_text to pull <hp:t> contents out of subText.
    let sub_text = extract_runs_text(sub_xml);
    let trimmed = sub_text.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(format!("({})", trimmed))
    }
}

/// Walk every `<hp:t>` run inside a fragment, decode XML entities, also pick up
/// `<hp:drawText>...</hp:drawText>` textbox bodies recursively, plus
/// `<hp:pic>` / `<hp:img>` references which become `[이미지: imageN]` markers.
fn extract_runs_text(xml: &str) -> String {
    let mut out = String::new();
    let mut pos = 0;

    while pos < xml.len() {
        // Pick the earliest opening tag — order matters for ties.
        let t1 = xml[pos..].find("<hp:t>");
        let t2 = xml[pos..].find("<hp:t ");
        let dt1 = xml[pos..].find("<hp:drawText>");
        let dt2 = xml[pos..].find("<hp:drawText ");
        let tab = xml[pos..].find("<hp:tab/>");
        // Hancom 실제 출력 형식: `<hp:tab width="..." leader="0" type="1"/>`
        let tab2 = xml[pos..].find("<hp:tab ");
        let lb = xml[pos..].find("<hp:lineBreak/>");
        // Ruby / 덧말 — treat as a single unit; we skip mainText (already
        // in the surrounding run flow) and emit subText as parenthetical.
        let dm1 = xml[pos..].find("<hp:dutmal>");
        let dm2 = xml[pos..].find("<hp:dutmal ");
        // Image references: hc:img carries binaryItemIDRef. It usually lives
        // inside an <hp:pic> wrapper but kordoc just emits one marker per
        // hc:img occurrence regardless of wrapper.
        let img1 = xml[pos..].find("<hc:img ");
        let img2 = xml[pos..].find("<hc:img>");

        let candidates = [
            t1.map(|i| (i, "t")),
            t2.map(|i| (i, "t")),
            dt1.map(|i| (i, "dt")),
            dt2.map(|i| (i, "dt")),
            dm1.map(|i| (i, "dm")),
            dm2.map(|i| (i, "dm")),
            tab.map(|i| (i, "tab")),
            tab2.map(|i| (i, "tab2")),
            lb.map(|i| (i, "lb")),
            img1.map(|i| (i, "img")),
            img2.map(|i| (i, "img")),
        ];
        let next = candidates
            .iter()
            .filter_map(|c| *c)
            .min_by_key(|(i, _)| *i);

        let Some((rel, kind)) = next else { break; };
        let abs = pos + rel;

        match kind {
            "t" => {
                let after_open = match xml[abs..].find('>') {
                    Some(i) => abs + i + 1,
                    None => break,
                };
                let close = match xml[after_open..].find("</hp:t>") {
                    Some(i) => after_open + i,
                    None => break,
                };
                let raw = &xml[after_open..close];
                // <hp:lineBreak/> / <hp:tab/> can appear inside <hp:t> content
                let raw = raw.replace("<hp:lineBreak/>", "\n");
                let raw = raw.replace("<hp:lineBreak />", "\n");
                let raw = raw.replace("<hp:tab/>", "\t");
                // 한컴 실제 형식: <hp:tab width="..." leader="0" type="1"/>
                let raw = replace_attributed_tab(&raw);
                out.push_str(&decode_xml_entities(&raw));
                pos = close + 7;
            }
            "dt" => {
                let after_open = match xml[abs..].find('>') {
                    Some(i) => abs + i + 1,
                    None => break,
                };
                let close = match xml[after_open..].find("</hp:drawText>") {
                    Some(i) => after_open + i,
                    None => break,
                };
                let inner = &xml[after_open..close];
                let inner_text = extract_runs_text(inner);
                if !inner_text.trim().is_empty() {
                    if !out.is_empty() && !out.ends_with('\n') {
                        out.push('\n');
                    }
                    out.push_str(inner_text.trim());
                }
                pos = close + 14;
            }
            "dm" => {
                // <hp:dutmal> ... </hp:dutmal>  — ruby annotation wrapper
                let after_open = match xml[abs..].find('>') {
                    Some(i) => abs + i + 1,
                    None => break,
                };
                let close = match xml[after_open..].find("</hp:dutmal>") {
                    Some(i) => after_open + i,
                    None => break,
                };
                let inner = &xml[after_open..close];
                if let Some(annotation) = extract_dutmal_annotation(inner) {
                    out.push_str(&annotation);
                }
                pos = close + 12; // len("</hp:dutmal>")
            }
            "tab" => {
                out.push('\t');
                pos = abs + 9;
            }
            "tab2" => {
                // attributed: `<hp:tab width="..." leader="0" type="1"/>`
                out.push('\t');
                pos = match xml[abs..].find("/>") {
                    Some(i) => abs + i + 2,
                    None => break,
                };
            }
            "lb" => {
                out.push('\n');
                pos = abs + 15;
            }
            "img" => {
                // <hc:img binaryItemIDRef="image1" .../>  →  [이미지: image1]
                let after_open = match xml[abs..].find('>') {
                    Some(i) => abs + i + 1,
                    None => break,
                };
                let tag_xml = &xml[abs..after_open];
                if let Some(id) = extract_attr(tag_xml, "binaryItemIDRef") {
                    if !out.is_empty() && !out.ends_with(char::is_whitespace) {
                        out.push(' ');
                    }
                    out.push_str(&format!("[이미지: {}]", id));
                }
                pos = after_open;
            }
            _ => break,
        }
    }

    out
}

/// `<hp:tab width="..." leader="0" type="1"/>` 같은 속성 포함 탭을 `\t`로 치환.
fn replace_attributed_tab(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut pos = 0;
    while let Some(rel) = s[pos..].find("<hp:tab ") {
        let abs = pos + rel;
        out.push_str(&s[pos..abs]);
        match s[abs..].find("/>") {
            Some(end) => {
                out.push('\t');
                pos = abs + end + 2;
            }
            None => {
                out.push_str(&s[abs..]);
                return out;
            }
        }
    }
    out.push_str(&s[pos..]);
    out
}

fn decode_xml_entities(s: &str) -> String {
    s.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
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

/// Extract text with formatting from section XML.
///
/// When `heading_styles` contains a mapping for the paragraph's `styleIDRef`,
/// the paragraph text is prefixed with the appropriate number of `#` markers.
/// Depth-aware locator for the `</hp:p>` that closes the currently open
/// paragraph starting just past `from`. Needed because paragraphs can
/// nest — `<hp:footNote>` / `<hp:endNote>` / `<hp:tc>` each carry their
/// own `<hp:subList>` with inner `<hp:p>` elements. Naive substring search
/// for `</hp:p>` would grab the first inner close, truncating the outer
/// paragraph.
///
/// `find_matching_close` exists but uses substring matching for the
/// opening token, which would count `<hp:pic>` / `<hp:pageHide>` as
/// paragraph openings. This helper checks the character following
/// `<hp:p` to distinguish real paragraph opens.
fn find_matching_close_para(xml: &str, from: usize) -> Option<usize> {
    let mut depth: usize = 1;
    let mut scan = from;
    while scan < xml.len() && depth > 0 {
        // Next real `<hp:p>` / `<hp:p ` opening (skip `<hp:pic>` etc).
        let next_open: Option<usize> = {
            let mut search_from = scan;
            loop {
                match xml[search_from..].find("<hp:p") {
                    Some(rel) => {
                        let abs = search_from + rel;
                        let next_char = xml[abs + 5..].chars().next();
                        if matches!(next_char, Some('>') | Some(' ')) {
                            break Some(abs);
                        }
                        // False positive (pic/pageHide/...) — advance past it.
                        search_from = abs + 5;
                    }
                    None => break None,
                }
            }
        };
        let next_close = xml[scan..].find("</hp:p>").map(|i| scan + i);
        match (next_open, next_close) {
            (Some(o), Some(c)) if o < c => {
                depth += 1;
                scan = o + 5; // past "<hp:p"
            }
            (_, Some(c)) => {
                depth -= 1;
                if depth == 0 {
                    return Some(c);
                }
                scan = c + 7; // past "</hp:p>"
            }
            _ => return None,
        }
    }
    None
}

fn extract_text_with_formatting(
    xml: &str,
    char_styles: &HashMap<u32, CharStyle>,
    heading_styles: &HashMap<u32, u8>,
) -> String {
    let mut result = String::new();
    let mut pos = 0;

    // Process paragraph by paragraph. We scan for `<hp:p` opening and use
    // depth-aware matching to find the paired `</hp:p>` — naive substring
    // matching would be fooled by nested `<hp:p>` inside `<hp:footNote>` /
    // `<hp:endNote>` subLists.
    while let Some(p_start) = xml[pos..].find("<hp:p") {
        let p_pos = pos + p_start;

        // Confirm this is a real `<hp:p` opening (not `<hp:pic>` etc).
        let after_tag_start = p_pos + 5;
        let ch = xml[after_tag_start..].chars().next();
        if !matches!(ch, Some('>') | Some(' ')) {
            pos = after_tag_start;
            continue;
        }

        let p_close = match find_matching_close_para(xml, after_tag_start) {
            Some(idx) => idx,
            None => break,
        };

        let para_xml = &xml[p_pos..p_close + 7];

        // Check for heading via styleIDRef on the <hp:p> tag
        let heading_level = extract_attr(para_xml, "styleIDRef")
            .and_then(|id_str| id_str.parse::<u32>().ok())
            .and_then(|id| heading_styles.get(&id).copied())
            .unwrap_or(0);

        // Extract runs from this paragraph
        let para_text = extract_runs_with_formatting(para_xml, char_styles);
        if !para_text.is_empty() {
            if heading_level > 0 && heading_level <= 7 {
                // Ensure blank line before heading for proper Markdown rendering
                if !result.is_empty() && !result.ends_with('\n') {
                    result.push('\n');
                }
                for _ in 0..heading_level {
                    result.push('#');
                }
                result.push(' ');
                // Strip bold markers from heading text (headings are inherently prominent)
                let clean_heading = para_text.trim().replace("**", "");
                result.push_str(&clean_heading);
            } else {
                result.push_str(&para_text);
            }
            result.push('\n');
        }

        pos = p_close + 7;
    }

    result
}

/// Extract runs with formatting applied
fn extract_runs_with_formatting(para_xml: &str, char_styles: &HashMap<u32, CharStyle>) -> String {
    let mut result = String::new();
    let mut pos = 0;

    // Hyperlink field span state. When `<hp:fieldBegin type="HYPERLINK"
    // name="URL">` is seen we remember the URL and the `result.len()` at
    // that point; every run-emitted char between it and the matching
    // `<hp:fieldEnd>` becomes the link text. The accumulated range then
    // gets rewritten from `text` to `[text](url)` at fieldEnd.
    //
    // Non-hyperlink fields (DATE, BOOKMARK, CROSSREF, …) are left alone —
    // their text is already inline in the following runs and carries
    // itself.
    let mut link_url: Option<String> = None;
    let mut link_start: usize = 0;

    loop {
        // Find the next paragraph-level element. `<hp:run >`, `<hp:dutmal>`,
        // `<hp:footNote>`, and `<hp:endNote>` are siblings at this level; the
        // earliest wins. Notes and ruby both annotate surrounding text so we
        // emit their content inline right after whatever precedes them.
        let next_run = para_xml[pos..].find("<hp:run ").map(|i| pos + i);
        let next_dutmal = para_xml[pos..].find("<hp:dutmal").map(|i| pos + i);
        let next_footnote = para_xml[pos..].find("<hp:footNote").map(|i| pos + i);
        let next_endnote = para_xml[pos..].find("<hp:endNote").map(|i| pos + i);
        let next_equation = para_xml[pos..].find("<hp:equation").map(|i| pos + i);
        let next_header = para_xml[pos..].find("<hp:header").map(|i| pos + i);
        let next_footer = para_xml[pos..].find("<hp:footer").map(|i| pos + i);
        let next_field_begin = para_xml[pos..].find("<hp:fieldBegin").map(|i| pos + i);
        let next_field_end = para_xml[pos..].find("<hp:fieldEnd").map(|i| pos + i);

        let candidates = [
            next_run.map(|i| (i, "run")),
            next_dutmal.map(|i| (i, "dutmal")),
            next_footnote.map(|i| (i, "footnote")),
            next_endnote.map(|i| (i, "endnote")),
            next_equation.map(|i| (i, "equation")),
            next_header.map(|i| (i, "header")),
            next_footer.map(|i| (i, "footer")),
            next_field_begin.map(|i| (i, "fieldBegin")),
            next_field_end.map(|i| (i, "fieldEnd")),
        ];
        let Some((abs, kind)) = candidates.iter().filter_map(|c| *c).min_by_key(|(i, _)| *i) else {
            break;
        };

        if kind == "dutmal" {
            let after_open = match para_xml[abs..].find('>') {
                Some(i) => abs + i + 1,
                None => break,
            };
            let close = match para_xml[after_open..].find("</hp:dutmal>") {
                Some(i) => after_open + i,
                None => break,
            };
            let inner = &para_xml[after_open..close];
            if let Some(annotation) = extract_dutmal_annotation(inner) {
                result.push_str(&annotation);
            }
            pos = close + 12; // len("</hp:dutmal>")
            continue;
        }

        if kind == "footnote" || kind == "endnote" {
            let (close_tag, label) = if kind == "footnote" {
                ("</hp:footNote>", "각주")
            } else {
                ("</hp:endNote>", "미주")
            };
            let after_open = match para_xml[abs..].find('>') {
                Some(i) => abs + i + 1,
                None => break,
            };
            let close = match para_xml[after_open..].find(close_tag) {
                Some(i) => after_open + i,
                None => break,
            };
            let inner = &para_xml[after_open..close];
            if let Some(marker) = extract_note_content(inner, label) {
                if !result.is_empty() && !result.ends_with(char::is_whitespace) {
                    result.push(' ');
                }
                result.push_str(&marker);
            }
            pos = close + close_tag.len();
            continue;
        }

        if kind == "equation" {
            let after_open = match para_xml[abs..].find('>') {
                Some(i) => abs + i + 1,
                None => break,
            };
            let close = match para_xml[after_open..].find("</hp:equation>") {
                Some(i) => after_open + i,
                None => break,
            };
            let inner = &para_xml[after_open..close];
            if let Some(block) = extract_equation_markdown(inner) {
                result.push_str(&block);
            }
            pos = close + 14; // len("</hp:equation>")
            continue;
        }

        if kind == "header" || kind == "footer" {
            let (close_tag, label) = if kind == "header" {
                ("</hp:header>", "머리말")
            } else {
                ("</hp:footer>", "꼬리말")
            };
            let after_open = match para_xml[abs..].find('>') {
                Some(i) => abs + i + 1,
                None => break,
            };
            let close = match para_xml[after_open..].find(close_tag) {
                Some(i) => after_open + i,
                None => break,
            };
            let inner = &para_xml[after_open..close];
            // Headers/footers share the subList structure with footnotes,
            // so reuse the same extractor.
            if let Some(marker) = extract_note_content(inner, label) {
                if !result.is_empty() && !result.ends_with(char::is_whitespace) {
                    result.push(' ');
                }
                result.push_str(&marker);
            }
            pos = close + close_tag.len();
            continue;
        }

        if kind == "fieldBegin" {
            // `<hp:fieldBegin type="HYPERLINK" name="https://…"/>` is a
            // self-closing element that begins a hyperlink span. Other
            // field types (DATE, CROSSREF, …) are ignored — their text is
            // already in the following run flow.
            let after_open = match para_xml[abs..].find('>') {
                Some(i) => abs + i + 1,
                None => break,
            };
            let tag_xml = &para_xml[abs..after_open];
            let field_type = extract_attr(tag_xml, "type").unwrap_or_default();
            if field_type == "HYPERLINK" {
                if let Some(url) = extract_attr(tag_xml, "name") {
                    link_url = Some(url);
                    link_start = result.len();
                }
            }
            pos = after_open;
            continue;
        }

        if kind == "fieldEnd" {
            let after_open = match para_xml[abs..].find('>') {
                Some(i) => abs + i + 1,
                None => break,
            };
            if let Some(url) = link_url.take() {
                // Rewrite the text we accumulated since fieldBegin into a
                // `[text](url)` markdown link. Trim leading/trailing space
                // off the text portion so we don't swallow paragraph gaps.
                let link_text_raw = result[link_start..].to_string();
                let link_text = link_text_raw.trim().to_string();
                if !link_text.is_empty() {
                    result.truncate(link_start);
                    // Preserve leading whitespace that was before the trim.
                    let leading = link_text_raw
                        .chars()
                        .take_while(|c| c.is_whitespace())
                        .collect::<String>();
                    let trailing = link_text_raw
                        .chars()
                        .rev()
                        .take_while(|c| c.is_whitespace())
                        .collect::<String>();
                    result.push_str(&leading);
                    result.push_str(&format!("[{}]({})", link_text, url));
                    // Append trailing whitespace in original order.
                    let trailing: String = trailing.chars().rev().collect();
                    result.push_str(&trailing);
                }
            }
            pos = after_open;
            continue;
        }

        let run_pos = abs;

        // Get charPrIDRef attribute
        let run_tag_end = para_xml[run_pos..].find('>').unwrap_or(0);
        let run_tag = &para_xml[run_pos..run_pos + run_tag_end];
        let char_pr_id = extract_attr(run_tag, "charPrIDRef")
            .and_then(|s| s.parse::<u32>().ok());

        // Find run content end (nesting-aware: footnotes/endnotes nest runs inside)
        let body_start = run_pos + run_tag_end + 1;
        if let Some(run_end) = find_matching_close(para_xml, body_start, "<hp:run ", "</hp:run>") {
            // run_content is the bytes between the opening '>' of this run and the matching '</hp:run>'
            let run_body = &para_xml[body_start..run_end];

            // Walk the run body linearly, handling nested ctrl / equation / run
            // elements BEFORE their inner <hp:t> contents would leak out.
            let text_content = walk_run_body(run_body);

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

            pos = run_end + 9; // advance past "</hp:run>"
        } else {
            // Self-closing run or malformed
            pos = run_pos + 1;
        }
    }

    result
}

/// Apply Markdown formatting based on CharStyle
fn apply_markdown_formatting(text: &str, style: &CharStyle) -> String {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return text.to_string();
    }

    let prefix_space = if text.starts_with(' ') { " " } else { "" };
    let suffix_space = if text.ends_with(' ') { " " } else { "" };

    let mut result = trimmed.to_string();

    // Apply formatting in order: strikeout, bold/italic, underline, emphasis_dot
    // Emphasis dot (강조점) uses <mark> to preserve the "emphasized term" semantic
    // that Korean government and legal documents rely on heavily. <mark> renders
    // visually in most markdown previewers and carries a clear signal to LLMs.
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
        result = format!("<u>{}</u>", result);
    }
    if style.emphasis_dot {
        result = format!("<mark>{}</mark>", result);
    }

    format!("{}{}{}", prefix_space, result, suffix_space)
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
            spans: Vec::new(),
        };

        let md = table.to_markdown();
        assert!(md.contains("| 헤더1 |"));
        assert!(md.contains("| --- | --- | --- |"));
        assert!(md.contains("| 데이터1 |"));
    }

    #[test]
    fn test_extract_cell_text() {
        let xml = r#"<hp:tc><hp:subList><hp:p><hp:run><hp:t>테스트</hp:t></hp:run></hp:p></hp:subList></hp:tc>"#;
        let char_styles = HashMap::new();
        let text = extract_cell_text(xml, &char_styles);
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
            ..Default::default()
        };
        assert_eq!(apply_markdown_formatting("테스트", &style), "**테스트**");

        // Test italic
        let style = CharStyle {
            italic: true,
            ..Default::default()
        };
        assert_eq!(apply_markdown_formatting("테스트", &style), "*테스트*");

        // Test bold+italic
        let style = CharStyle {
            bold: true,
            italic: true,
            ..Default::default()
        };
        assert_eq!(apply_markdown_formatting("테스트", &style), "***테스트***");

        // Test underline
        let style = CharStyle {
            underline: true,
            ..Default::default()
        };
        assert_eq!(apply_markdown_formatting("테스트", &style), "<u>테스트</u>");

        // Test strikeout
        let style = CharStyle {
            strikeout: true,
            ..Default::default()
        };
        assert_eq!(apply_markdown_formatting("테스트", &style), "~~테스트~~");

        // Test combined: bold + underline + strikeout
        let style = CharStyle {
            bold: true,
            underline: true,
            strikeout: true,
            ..Default::default()
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
                <hh:strikeout shape="CONT"/>
            </hh:charPr>
            <hh:charPr id="42" height="1400">
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

        // id 99: real strikeout (CONT shape is in the whitelist)
        let style99 = styles.get(&99).unwrap();
        assert!(!style99.bold);
        assert!(!style99.underline);
        assert!(style99.strikeout);

        // id 42: shape="3D" is a Hancom export default, NOT real strikethrough.
        // Regression: 251113 venture press release wrapped body text in ~~...~~.
        let style42 = styles.get(&42).unwrap();
        assert!(!style42.strikeout, "shape=\"3D\" must not be treated as strike");
    }

    #[test]
    fn test_is_real_strikeout_shape() {
        // Real strikethrough shapes
        assert!(is_real_strikeout_shape("CONT"));
        assert!(is_real_strikeout_shape("SOLID"));
        assert!(is_real_strikeout_shape("DASH"));
        assert!(is_real_strikeout_shape("DOT"));

        // Non-strike values: explicit none, Hancom export quirk, unknown
        assert!(!is_real_strikeout_shape("NONE"));
        assert!(!is_real_strikeout_shape("3D"));
        assert!(!is_real_strikeout_shape(""));
        assert!(!is_real_strikeout_shape("WHATEVER"));
    }

    #[test]
    fn test_is_real_underline_type() {
        // Real underline positions
        assert!(is_real_underline_type("BOTTOM"));
        assert!(is_real_underline_type("TOP"));

        // Non-underline values, including forward-compat fail-closed cases
        assert!(!is_real_underline_type("NONE"));
        assert!(!is_real_underline_type("3D")); // hypothetical future placeholder
        assert!(!is_real_underline_type(""));
        assert!(!is_real_underline_type("bottom")); // case-sensitive
    }

    #[test]
    fn test_flatten_table_to_text() {
        let t = Table {
            rows: 2,
            cols: 2,
            cells: vec![
                vec!["A".to_string(), "B".to_string()],
                vec!["C".to_string(), "D".to_string()],
            ],
            has_header: false,
            spans: Vec::new(),
        };
        assert_eq!(flatten_table_to_text(&t), "A | B; C | D");
    }

    #[test]
    fn test_flatten_table_skips_empty_cells() {
        let t = Table {
            rows: 2,
            cols: 2,
            cells: vec![
                vec!["A".to_string(), "".to_string()],
                vec!["".to_string(), "D".to_string()],
            ],
            has_header: false,
            spans: Vec::new(),
        };
        assert_eq!(flatten_table_to_text(&t), "A; D");
    }

    /// Ported from kordoc `tests/table-builder.test.ts` (2026-04-09, f68e825).
    /// colSpan merge → HTML `<table>` with `colspan="N"` attr.
    #[test]
    fn test_merged_colspan_emits_html() {
        // 2×2 grid: row 0 = one cell spanning 2 cols; row 1 = two plain cells.
        let t = Table {
            rows: 2,
            cols: 2,
            cells: vec![
                vec!["병합셀".to_string(), "".to_string()],
                vec!["값1".to_string(), "값2".to_string()],
            ],
            has_header: false,
            spans: vec![vec![(2, 1), (0, 0)], vec![(1, 1), (1, 1)]],
        };
        assert!(t.has_merged_cells());
        let out = t.to_markdown();
        assert!(out.contains("<table>"), "merged table must emit HTML");
        assert!(out.contains("colspan=\"2\""), "colspan attr preserved");
        assert!(out.contains("병합셀"));
        assert!(out.contains("값1"));
        assert!(out.contains("값2"));
        // Shadow cell must not produce an empty <th></th>
        assert!(!out.contains("<th></th>"));
    }

    /// rowSpan merge → HTML `<table>` with `rowspan="N"` attr.
    #[test]
    fn test_merged_rowspan_emits_html() {
        // 2×2 grid: col 0 = one cell spanning 2 rows; col 1 = two plain cells.
        let t = Table {
            rows: 2,
            cols: 2,
            cells: vec![
                vec!["행병합".to_string(), "값1".to_string()],
                vec!["".to_string(), "값2".to_string()],
            ],
            has_header: false,
            spans: vec![vec![(1, 2), (1, 1)], vec![(0, 0), (1, 1)]],
        };
        assert!(t.has_merged_cells());
        let out = t.to_markdown();
        assert!(out.contains("<table>"));
        assert!(out.contains("rowspan=\"2\""));
        assert!(out.contains("행병합"));
        assert!(out.contains("값2"));
    }

    /// No merged cells → classic GFM pipe table (no HTML).
    #[test]
    fn test_no_merge_stays_markdown() {
        let t = Table {
            rows: 2,
            cols: 2,
            cells: vec![
                vec!["A".to_string(), "B".to_string()],
                vec!["C".to_string(), "D".to_string()],
            ],
            has_header: true,
            spans: vec![vec![(1, 1), (1, 1)], vec![(1, 1), (1, 1)]],
        };
        assert!(!t.has_merged_cells());
        let out = t.to_markdown();
        assert!(!out.contains("<table>"), "non-merged table stays as GFM");
        assert!(out.contains("| A |"));
        assert!(out.contains("| --- |"));
    }

    /// HTML injection in cell text is escaped.
    #[test]
    fn test_merged_cell_html_escapes_text() {
        let t = Table {
            rows: 1,
            cols: 2,
            cells: vec![vec!["<script>".to_string(), "".to_string()]],
            has_header: false,
            spans: vec![vec![(2, 1), (0, 0)]],
        };
        let out = t.to_markdown();
        assert!(out.contains("&lt;script&gt;"));
        assert!(!out.contains("<script>"));
    }

    #[test]
    fn test_preprocess_nested_tables_no_nested() {
        // Plain cell content with no <hp:tbl>: preprocessing is a no-op.
        let styles = HashMap::new();
        let mut counter = NestedTableCounter::default();
        let mut separate = Vec::new();
        let input = "<hp:subList><hp:p><hp:run><hp:t>hello</hp:t></hp:run></hp:p></hp:subList>";
        let out = preprocess_nested_tables(input, &styles, &mut counter, &mut separate, 0);
        assert_eq!(out, input);
        assert_eq!(counter.n, 0);
        assert!(separate.is_empty());
    }

    #[test]
    fn test_preprocess_nested_tables_small_flattens() {
        // 1×1 nested table should be flattened into the parent cell text
        // under the `[중첩 테이블 #1]` marker — no separate block emitted.
        let styles = HashMap::new();
        let mut counter = NestedTableCounter::default();
        let mut separate = Vec::new();
        let cell_xml = concat!(
            "<hp:subList>",
            "<hp:p><hp:run><hp:t>before</hp:t></hp:run></hp:p>",
            "<hp:tbl rowCnt=\"1\" colCnt=\"1\">",
            "<hp:tr><hp:tc name=\"\"><hp:cellAddr colAddr=\"0\" rowAddr=\"0\"/>",
            "<hp:cellSpan colSpan=\"1\" rowSpan=\"1\"/>",
            "<hp:subList><hp:p><hp:run><hp:t>inner</hp:t></hp:run></hp:p></hp:subList>",
            "</hp:tc></hp:tr>",
            "</hp:tbl>",
            "<hp:p><hp:run><hp:t>after</hp:t></hp:run></hp:p>",
            "</hp:subList>",
        );
        let out = preprocess_nested_tables(cell_xml, &styles, &mut counter, &mut separate, 0);
        assert!(
            out.contains("[중첩 테이블 #1]"),
            "marker should be inserted: {}",
            out
        );
        assert!(
            out.contains("inner"),
            "small nested body should be flattened inline: {}",
            out
        );
        assert!(separate.is_empty(), "small nested must NOT be hoisted");
        assert_eq!(counter.n, 1);
    }

    #[test]
    fn test_preprocess_nested_tables_big_hoists() {
        // 3×2 nested table exceeds the threshold: should be hoisted into
        // `separate_out` and the parent cell should only see the marker.
        let styles = HashMap::new();
        let mut counter = NestedTableCounter::default();
        let mut separate = Vec::new();
        let cell_xml = concat!(
            "<hp:subList>",
            "<hp:tbl rowCnt=\"3\" colCnt=\"2\">",
            // row 0
            "<hp:tr>",
            "<hp:tc name=\"\"><hp:cellAddr colAddr=\"0\" rowAddr=\"0\"/><hp:cellSpan colSpan=\"1\" rowSpan=\"1\"/>",
            "<hp:subList><hp:p><hp:run><hp:t>r0c0</hp:t></hp:run></hp:p></hp:subList></hp:tc>",
            "<hp:tc name=\"\"><hp:cellAddr colAddr=\"1\" rowAddr=\"0\"/><hp:cellSpan colSpan=\"1\" rowSpan=\"1\"/>",
            "<hp:subList><hp:p><hp:run><hp:t>r0c1</hp:t></hp:run></hp:p></hp:subList></hp:tc>",
            "</hp:tr>",
            // row 1
            "<hp:tr>",
            "<hp:tc name=\"\"><hp:cellAddr colAddr=\"0\" rowAddr=\"1\"/><hp:cellSpan colSpan=\"1\" rowSpan=\"1\"/>",
            "<hp:subList><hp:p><hp:run><hp:t>r1c0</hp:t></hp:run></hp:p></hp:subList></hp:tc>",
            "<hp:tc name=\"\"><hp:cellAddr colAddr=\"1\" rowAddr=\"1\"/><hp:cellSpan colSpan=\"1\" rowSpan=\"1\"/>",
            "<hp:subList><hp:p><hp:run><hp:t>r1c1</hp:t></hp:run></hp:p></hp:subList></hp:tc>",
            "</hp:tr>",
            // row 2
            "<hp:tr>",
            "<hp:tc name=\"\"><hp:cellAddr colAddr=\"0\" rowAddr=\"2\"/><hp:cellSpan colSpan=\"1\" rowSpan=\"1\"/>",
            "<hp:subList><hp:p><hp:run><hp:t>r2c0</hp:t></hp:run></hp:p></hp:subList></hp:tc>",
            "<hp:tc name=\"\"><hp:cellAddr colAddr=\"1\" rowAddr=\"2\"/><hp:cellSpan colSpan=\"1\" rowSpan=\"1\"/>",
            "<hp:subList><hp:p><hp:run><hp:t>r2c1</hp:t></hp:run></hp:p></hp:subList></hp:tc>",
            "</hp:tr>",
            "</hp:tbl>",
            "</hp:subList>",
        );
        let out = preprocess_nested_tables(cell_xml, &styles, &mut counter, &mut separate, 0);
        assert!(out.contains("[중첩 테이블 #1]"), "marker: {}", out);
        assert!(
            !out.contains("r0c0"),
            "big nested body should NOT appear in parent cell text: {}",
            out
        );
        assert_eq!(separate.len(), 1, "big nested should be hoisted");
        let hoisted = &separate[0];
        assert_eq!(hoisted.rows, 3);
        assert_eq!(hoisted.cols, 2);
    }

    #[test]
    fn test_preprocess_nested_tables_depth_guard() {
        // At MAX depth the function must return input unchanged and NOT
        // descend further. We pass the cap as the starting depth to force
        // the guard on the very first call.
        let styles = HashMap::new();
        let mut counter = NestedTableCounter::default();
        let mut separate = Vec::new();
        let input = concat!(
            "<hp:tbl rowCnt=\"1\" colCnt=\"1\">",
            "<hp:tr><hp:tc name=\"\"><hp:cellAddr colAddr=\"0\" rowAddr=\"0\"/>",
            "<hp:cellSpan colSpan=\"1\" rowSpan=\"1\"/>",
            "<hp:subList><hp:p><hp:run><hp:t>inner</hp:t></hp:run></hp:p></hp:subList>",
            "</hp:tc></hp:tr></hp:tbl>",
        );
        let out = preprocess_nested_tables(
            input,
            &styles,
            &mut counter,
            &mut separate,
            MAX_NESTED_TABLE_DEPTH,
        );
        assert_eq!(out, input, "at the depth cap we should short-circuit");
        assert_eq!(counter.n, 0, "counter must not advance past the cap");
        assert!(separate.is_empty(), "nothing should be hoisted past the cap");
    }

    #[test]
    fn test_is_real_emphasis_mark() {
        // All six OWPML symMark values
        assert!(is_real_emphasis_mark("DOT"));
        assert!(is_real_emphasis_mark("CIRCLE"));
        assert!(is_real_emphasis_mark("TICK"));
        assert!(is_real_emphasis_mark("TILDE"));
        assert!(is_real_emphasis_mark("MIDDLE_DOT"));
        assert!(is_real_emphasis_mark("COLON"));

        // Non-emphasis / fail-closed
        assert!(!is_real_emphasis_mark("NONE"));
        assert!(!is_real_emphasis_mark(""));
        assert!(!is_real_emphasis_mark("FILLED_CIRCLE")); // not an OWPML value
        assert!(!is_real_emphasis_mark("dot")); // case-sensitive
    }

    #[test]
    fn test_parse_char_properties_emphasis_dot() {
        let header_xml = r#"
            <hh:charPr id="100" height="1000" symMark="NONE">
                <hh:underline type="NONE"/>
                <hh:strikeout shape="NONE"/>
            </hh:charPr>
            <hh:charPr id="101" height="1000" symMark="DOT">
                <hh:underline type="NONE"/>
                <hh:strikeout shape="NONE"/>
            </hh:charPr>
            <hh:charPr id="102" height="1000" symMark="CIRCLE">
                <hh:underline type="NONE"/>
                <hh:strikeout shape="NONE"/>
            </hh:charPr>
        "#;
        let styles = parse_char_properties(header_xml);

        // symMark="NONE" → no emphasis
        assert!(!styles.get(&100).unwrap().emphasis_dot);

        // symMark="DOT" (●) → emphasis present
        assert!(styles.get(&101).unwrap().emphasis_dot);

        // symMark="CIRCLE" (○) → emphasis present
        assert!(styles.get(&102).unwrap().emphasis_dot);
    }

    #[test]
    fn test_emphasis_dot_markdown_formatting() {
        let style = CharStyle {
            bold: false,
            italic: false,
            underline: false,
            strikeout: false,
            emphasis_dot: true,
        };
        assert_eq!(apply_markdown_formatting("중요", &style), "<mark>중요</mark>");

        // Combined: bold + emphasis dot → both preserved
        let style_bold_emph = CharStyle {
            bold: true,
            italic: false,
            underline: false,
            strikeout: false,
            emphasis_dot: true,
        };
        assert_eq!(
            apply_markdown_formatting("핵심", &style_bold_emph),
            "<mark>**핵심**</mark>"
        );
    }

    #[test]
    fn test_attributed_tab_converts_to_tab_char() {
        // 한컴 실제 출력 형식
        let xml = r#"<hp:t>앞<hp:tab width="3028" leader="0" type="1"/>뒤</hp:t>"#;
        let result = extract_runs_text(xml);
        assert_eq!(result, "앞\t뒤");
    }

    #[test]
    fn test_self_closing_tab_converts_to_tab_char() {
        let xml = r#"<hp:t>앞<hp:tab/>뒤</hp:t>"#;
        let result = extract_runs_text(xml);
        assert_eq!(result, "앞\t뒤");
    }

    #[test]
    fn test_replace_attributed_tab_helper() {
        let s = r#"A<hp:tab width="100" leader="0" type="1"/>B<hp:tab width="200" type="1"/>C"#;
        assert_eq!(replace_attributed_tab(s), "A\tB\tC");
    }

    #[test]
    fn test_remove_xml_tags() {
        let text = "앞<hp:tab width=\"100\" type=\"1\"/>뒤";
        let result = remove_xml_tags(text, "hp:tab");
        assert!(result.contains("앞"));
        assert!(result.contains("뒤"));
        assert!(!result.contains("<hp:tab"));
    }

    #[test]
    fn test_bold_spacing_fix() {
        let style = CharStyle {
            bold: true,
            ..Default::default()
        };
        assert_eq!(apply_markdown_formatting(" text", &style), " **text**");
        assert_eq!(apply_markdown_formatting("text ", &style), "**text** ");
        assert_eq!(apply_markdown_formatting(" text ", &style), " **text** ");
        assert_eq!(apply_markdown_formatting("text", &style), "**text**");
        assert_eq!(apply_markdown_formatting("   ", &style), "   ");
    }

    #[test]
    fn test_parse_heading_styles() {
        let header_xml = r#"
            <hh:style id="0" type="PARA" name="바탕글" engName="Body Text"/>
            <hh:style id="2" type="PARA" name="개요 1" engName="Outline 1"/>
            <hh:style id="3" type="PARA" name="개요 2" engName="Outline 2"/>
            <hh:style id="8" type="PARA" name="개요 7" engName="Outline 7"/>
            <hh:style id="10" type="CHAR" name="본문" engName="Normal"/>
        "#;
        let map = parse_heading_styles(header_xml);
        assert_eq!(map.get(&2), Some(&1));
        assert_eq!(map.get(&3), Some(&2));
        assert_eq!(map.get(&8), Some(&7));
        assert!(map.get(&0).is_none());
        assert!(map.get(&10).is_none());
    }

    #[test]
    fn test_heading_in_section_xml() {
        let heading_styles: HashMap<u32, u8> = [(2, 1)].into_iter().collect();
        let char_styles = HashMap::new();
        let xml = r#"<hp:sec><hp:p styleIDRef="2"><hp:run charPrIDRef="0"><hp:t>제목입니다</hp:t></hp:run></hp:p><hp:p styleIDRef="0"><hp:run charPrIDRef="0"><hp:t>본문입니다</hp:t></hp:run></hp:p></hp:sec>"#;
        let (result, _) = parse_section_xml(xml, &char_styles, &heading_styles);
        assert!(result.contains("# 제목입니다"), "heading marker missing: {}", result);
        assert!(result.contains("본문입니다"));
    }

    #[test]
    fn test_linebreak_in_runs() {
        let char_styles = HashMap::new();
        let heading_styles = HashMap::new();
        // extract_runs_with_formatting requires <hp:run with attrs (space after "run")
        let xml = r#"<hp:p styleIDRef="0"><hp:run charPrIDRef="0"><hp:t>줄1</hp:t><hp:lineBreak/><hp:t>줄2</hp:t></hp:run></hp:p>"#;
        let result = extract_text_with_formatting(xml, &char_styles, &heading_styles);
        assert!(result.contains("줄1\n줄2"), "linebreak not handled: {:?}", result);
    }

    #[test]
    fn test_footnote_in_run_ctrl() {
        // Hancom HWPX nests footnote inside <hp:run><hp:ctrl>, not as a
        // paragraph-level sibling. Previously the parser leaked the
        // footnote body as raw text without the [각주: ...] marker.
        let char_styles = HashMap::new();
        let heading_styles = HashMap::new();
        let xml = r#"<hp:sec><hp:p><hp:run charPrIDRef="0"><hp:ctrl><hp:footNote number="1" suffixChar="41"><hp:subList><hp:p><hp:run charPrIDRef="3"><hp:t>12345</hp:t></hp:run></hp:p></hp:subList></hp:footNote></hp:ctrl><hp:t/></hp:run></hp:p></hp:sec>"#;
        let (result, _) = parse_section_xml(xml, &char_styles, &heading_styles);
        assert!(
            result.contains("[각주: 12345]"),
            "footnote marker missing or malformed: {:?}",
            result
        );
    }

    #[test]
    fn test_endnote_in_run_ctrl() {
        let char_styles = HashMap::new();
        let heading_styles = HashMap::new();
        let xml = r#"<hp:sec><hp:p><hp:run charPrIDRef="0"><hp:ctrl><hp:endNote number="1" suffixChar="41"><hp:subList><hp:p><hp:run charPrIDRef="3"><hp:t>7890</hp:t></hp:run></hp:p></hp:subList></hp:endNote></hp:ctrl><hp:t/></hp:run></hp:p></hp:sec>"#;
        let (result, _) = parse_section_xml(xml, &char_styles, &heading_styles);
        assert!(
            result.contains("[미주: 7890]"),
            "endnote marker missing or malformed: {:?}",
            result
        );
    }

    #[test]
    fn test_equation_in_run() {
        // Hancom emits <hp:equation> as a direct child of <hp:run>, not at
        // paragraph level. The inner <hp:script> content should be wrapped
        // in $...$ (inline) or $$...$$ (block for multi-line scripts).
        let char_styles = HashMap::new();
        let heading_styles = HashMap::new();
        let xml = r#"<hp:sec><hp:p><hp:run charPrIDRef="0"><hp:equation version="Equation Version 60"><hp:script>y = x^2 + 2x + 1</hp:script></hp:equation><hp:t/></hp:run></hp:p></hp:sec>"#;
        let (result, _) = parse_section_xml(xml, &char_styles, &heading_styles);
        assert!(
            result.contains("$y = x^2 + 2x + 1$"),
            "equation script not extracted: {:?}",
            result
        );
    }

    #[test]
    fn test_footnote_does_not_leak_raw_text() {
        // Verify the fix: footnote body text must NOT appear OUTSIDE the
        // [각주: ...] marker. Before the fix, the nested subList's <hp:t>
        // would leak into the outer run's text.
        let char_styles = HashMap::new();
        let heading_styles = HashMap::new();
        let xml = r#"<hp:sec><hp:p><hp:run charPrIDRef="0"><hp:t>본문</hp:t></hp:run><hp:run charPrIDRef="0"><hp:ctrl><hp:footNote number="1"><hp:subList><hp:p><hp:run charPrIDRef="3"><hp:t>주석내용</hp:t></hp:run></hp:p></hp:subList></hp:footNote></hp:ctrl><hp:t/></hp:run></hp:p></hp:sec>"#;
        let (result, _) = parse_section_xml(xml, &char_styles, &heading_styles);
        // Body "본문" appears exactly once; footnote body "주석내용" appears
        // only inside the marker, not as standalone text.
        let body_count = result.matches("본문").count();
        let note_count = result.matches("주석내용").count();
        let marker_count = result.matches("[각주: 주석내용]").count();
        assert_eq!(body_count, 1, "body text mismatch: {:?}", result);
        assert_eq!(note_count, 1, "footnote body text should appear exactly once (inside marker): {:?}", result);
        assert_eq!(marker_count, 1, "footnote marker missing: {:?}", result);
    }

    #[test]
    fn test_nested_run_end_matching() {
        // Outer run contains a footnote whose subList has its own <hp:run>.
        // find_matching_close must not stop at the INNER </hp:run>.
        let char_styles = HashMap::new();
        let heading_styles = HashMap::new();
        let xml = r#"<hp:sec><hp:p><hp:run charPrIDRef="0"><hp:ctrl><hp:footNote><hp:subList><hp:p><hp:run charPrIDRef="3"><hp:t>inner</hp:t></hp:run></hp:p></hp:subList></hp:footNote></hp:ctrl><hp:t>outer_after_note</hp:t></hp:run></hp:p></hp:sec>"#;
        let (result, _) = parse_section_xml(xml, &char_styles, &heading_styles);
        // After the fix, the outer run completes properly and
        // "outer_after_note" is emitted too.
        assert!(
            result.contains("outer_after_note"),
            "outer run truncated at nested </hp:run>: {:?}",
            result
        );
        assert!(
            result.contains("[각주: inner]"),
            "footnote marker missing: {:?}",
            result
        );
    }

    #[test]
    fn test_strip_sec_pr() {
        let xml = r#"<hp:sec><hp:p><hp:run><hp:t>본문</hp:t></hp:run></hp:p><hp:secPr><hp:p><hp:run><hp:t>숨김</hp:t></hp:run></hp:p></hp:secPr></hp:sec>"#;
        let stripped = strip_sec_pr(xml);
        assert!(stripped.contains("본문"));
        assert!(!stripped.contains("숨김"));
        assert!(!stripped.contains("secPr"));
    }
}
