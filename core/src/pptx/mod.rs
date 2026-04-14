//! PPTX (PowerPoint) parser.
//!
//! Extracts slide text, titles, and speaker notes from `.pptx` files using
//! `zip` + `quick-xml` (same infrastructure as the DOCX parser).

use std::io::{self, Cursor, Read};
use std::path::Path;

use quick_xml::events::Event;
use quick_xml::Reader;

/// A single slide.
#[derive(Debug, Clone)]
pub struct Slide {
    pub number: usize,
    pub title: Option<String>,
    pub content: String,
    pub notes: Option<String>,
}

/// Workbook-level metadata.
#[derive(Debug, Clone)]
pub struct PptxMetadata {
    pub slide_count: usize,
}

/// Fully parsed presentation.
#[derive(Debug, Clone)]
pub struct PptxDocument {
    pub slides: Vec<Slide>,
    pub metadata: PptxMetadata,
}

/// PPTX parser backed by raw bytes.
pub struct PptxParser {
    data: Vec<u8>,
}

impl PptxParser {
    /// Open a PPTX file from disk.
    pub fn open<P: AsRef<Path>>(path: P) -> io::Result<Self> {
        let mut data = Vec::new();
        std::fs::File::open(path.as_ref())?.read_to_end(&mut data)?;
        Ok(Self { data })
    }

    /// Create a parser from raw bytes.
    pub fn from_bytes(data: Vec<u8>) -> io::Result<Self> {
        Ok(Self { data })
    }

    /// Parse the PPTX into a `PptxDocument`.
    pub fn parse(&self) -> io::Result<PptxDocument> {
        let cursor = Cursor::new(&self.data);
        let mut archive = zip::ZipArchive::new(cursor)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;

        // Discover slide entries sorted by number.
        let mut slide_entries: Vec<(usize, String)> = Vec::new();
        for i in 0..archive.len() {
            if let Ok(f) = archive.by_index(i) {
                let name = f.name().to_string();
                if let Some(num) = parse_slide_number(&name) {
                    slide_entries.push((num, name));
                }
            }
        }
        slide_entries.sort_by_key(|(n, _)| *n);

        let mut slides = Vec::with_capacity(slide_entries.len());

        for (num, entry_name) in &slide_entries {
            // Read slide XML.
            let slide_xml = read_zip_entry(&mut archive, entry_name)?;
            let (title, body_parts) = parse_slide_xml(&slide_xml);

            // Resolve the notes slide via the slide's relationships file, not by
            // positional guess — notesSlides are numbered independently of
            // slides (only slides with notes get an entry).
            let rels_path = format!("ppt/slides/_rels/slide{}.xml.rels", num);
            let notes = read_zip_entry(&mut archive, &rels_path)
                .ok()
                .and_then(|rels_xml| find_notes_target(&rels_xml))
                .and_then(|target| {
                    let notes_path = resolve_rel_target("ppt/slides/", &target);
                    read_zip_entry(&mut archive, &notes_path).ok()
                })
                .and_then(|xml| {
                    let text = extract_notes_text(&xml);
                    if text.trim().is_empty() { None } else { Some(text) }
                });

            let content = body_parts.join("\n\n");

            slides.push(Slide {
                number: *num,
                title,
                content,
                notes,
            });
        }

        let slide_count = slides.len();
        Ok(PptxDocument {
            slides,
            metadata: PptxMetadata { slide_count },
        })
    }
}

impl PptxDocument {
    /// Render the presentation as Markdown.
    pub fn to_markdown(&self) -> String {
        let mut out = String::new();

        for (idx, slide) in self.slides.iter().enumerate() {
            if idx > 0 {
                out.push_str("\n\n---\n\n");
            }

            // Heading
            if let Some(ref title) = slide.title {
                out.push_str(&format!("## Slide {}: {}\n\n", slide.number, title));
            } else {
                out.push_str(&format!("## Slide {}\n\n", slide.number));
            }

            // Body
            if !slide.content.is_empty() {
                out.push_str(&slide.content);
                out.push('\n');
            }

            // Notes
            if let Some(ref notes) = slide.notes {
                out.push_str(&format!("\n> **Notes:** {}\n", notes));
            }
        }

        out
    }

    /// Convenience: render to MDX with front-matter.
    pub fn to_mdx(&self, source_name: &str) -> String {
        format!(
            "---\nformat: pptx\nsource: \"{}\"\nslides: {}\n---\n\n{}",
            source_name.replace('"', "\\\""),
            self.metadata.slide_count,
            self.to_markdown(),
        )
    }
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Scan a slide's `.rels` file for the notesSlide relationship and return its Target.
fn find_notes_target(rels_xml: &str) -> Option<String> {
    let mut reader = Reader::from_str(rels_xml);
    reader.trim_text(true);
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Empty(ref e)) | Ok(Event::Start(ref e)) => {
                if local_name(e.name().as_ref()) == b"Relationship" {
                    let mut rel_type = String::new();
                    let mut rel_target = String::new();
                    for attr in e.attributes().flatten() {
                        match attr.key.as_ref() {
                            b"Type"   => rel_type = String::from_utf8_lossy(&attr.value).to_string(),
                            b"Target" => rel_target = String::from_utf8_lossy(&attr.value).to_string(),
                            _ => {}
                        }
                    }
                    if rel_type.ends_with("/notesSlide") || rel_type.ends_with("/notesSlides") {
                        return Some(rel_target);
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
        buf.clear();
    }
    None
}

/// Resolve a relationship target relative to the owning part's directory.
/// Example: base="ppt/slides/", target="../notesSlides/notesSlide1.xml"
///   -> "ppt/notesSlides/notesSlide1.xml".
fn resolve_rel_target(base_dir: &str, target: &str) -> String {
    let base = base_dir.trim_end_matches('/');
    let mut parts: Vec<&str> = base.split('/').collect();
    for seg in target.split('/') {
        match seg {
            "" => {}
            ".." => { parts.pop(); }
            "." => {}
            other => parts.push(other),
        }
    }
    parts.join("/")
}

/// Extract slide number from paths like `ppt/slides/slide3.xml`.
fn parse_slide_number(name: &str) -> Option<usize> {
    let lower = name.to_ascii_lowercase();
    if !lower.starts_with("ppt/slides/slide") || !lower.ends_with(".xml") {
        return None;
    }
    // Strip directory prefix and `.xml` suffix.
    let base = &name["ppt/slides/slide".len()..name.len() - 4];
    base.parse::<usize>().ok()
}

/// Read a single entry from the ZIP archive.
fn read_zip_entry(archive: &mut zip::ZipArchive<Cursor<&Vec<u8>>>, name: &str) -> io::Result<String> {
    let mut file = archive.by_name(name)
        .map_err(|e| io::Error::new(io::ErrorKind::NotFound, e.to_string()))?;
    let mut buf = String::new();
    file.read_to_string(&mut buf)?;
    Ok(buf)
}

/// Parse a slide XML and return (title, body_text_paragraphs).
///
/// Title detection: `<p:ph type="title"/>` or `<p:ph type="ctrTitle"/>`.
/// Also extracts embedded tables (`a:tbl` inside `p:graphicFrame`) as GFM
/// pipe tables, and pictures (`p:pic`) as Markdown image links with alt text.
fn parse_slide_xml(xml: &str) -> (Option<String>, Vec<String>) {
    let mut reader = Reader::from_str(xml);
    reader.trim_text(true);
    let mut buf = Vec::new();

    let mut title: Option<String> = None;
    let mut body_parts: Vec<String> = Vec::new();

    // Shape (p:sp) state
    let mut in_shape = false;
    let mut is_title_shape = false;
    let mut in_text_body = false;
    let mut in_paragraph = false;
    let mut current_para = String::new();
    let mut shape_paragraphs: Vec<String> = Vec::new();

    // Table (a:tbl inside p:graphicFrame) state
    let mut in_table = false;
    let mut table_rows: Vec<Vec<String>> = Vec::new();
    let mut current_row: Vec<String> = Vec::new();
    let mut in_tc = false;
    let mut cell_buffer = String::new();

    // Picture (p:pic) state
    let mut in_pic = false;
    let mut pic_alt = String::new();
    let mut pic_embed = String::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) | Ok(Event::Empty(ref e)) => {
                let name_bytes = e.name().as_ref().to_vec();
                let local = local_name(&name_bytes);
                match local {
                    b"sp" => {
                        in_shape = true;
                        is_title_shape = false;
                        shape_paragraphs.clear();
                    }
                    b"ph" if in_shape => {
                        for attr in e.attributes().flatten() {
                            if attr.key.as_ref() == b"type" {
                                let val = String::from_utf8_lossy(&attr.value);
                                if val == "title" || val == "ctrTitle" {
                                    is_title_shape = true;
                                }
                            }
                        }
                    }
                    b"txBody" if in_shape && !in_table => {
                        in_text_body = true;
                    }
                    b"p" if in_text_body && !in_tc => {
                        in_paragraph = true;
                        current_para.clear();
                    }
                    // --- Table ---
                    b"tbl" => {
                        in_table = true;
                        table_rows.clear();
                    }
                    b"tr" if in_table => {
                        current_row.clear();
                    }
                    b"tc" if in_table => {
                        in_tc = true;
                        cell_buffer.clear();
                    }
                    b"p" if in_tc => {
                        // Paragraph break within a table cell — add a space between lines.
                        if !cell_buffer.is_empty() && !cell_buffer.ends_with(' ') {
                            cell_buffer.push(' ');
                        }
                    }
                    // --- Picture ---
                    b"pic" => {
                        in_pic = true;
                        pic_alt.clear();
                        pic_embed.clear();
                    }
                    b"cNvPr" if in_pic => {
                        for attr in e.attributes().flatten() {
                            if attr.key.as_ref() == b"descr" {
                                pic_alt = String::from_utf8_lossy(&attr.value).to_string();
                            } else if pic_alt.is_empty() && attr.key.as_ref() == b"name" {
                                pic_alt = String::from_utf8_lossy(&attr.value).to_string();
                            }
                        }
                    }
                    b"blip" if in_pic => {
                        for attr in e.attributes().flatten() {
                            if local_name(attr.key.as_ref()) == b"embed" {
                                pic_embed = String::from_utf8_lossy(&attr.value).to_string();
                            }
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::End(ref e)) => {
                let name_bytes = e.name().as_ref().to_vec();
                let local = local_name(&name_bytes);
                match local {
                    b"sp" => {
                        // Flush shape paragraphs
                        if is_title_shape && title.is_none() {
                            let combined = shape_paragraphs.join(" ").trim().to_string();
                            if !combined.is_empty() {
                                title = Some(combined);
                            }
                        } else {
                            for p in &shape_paragraphs {
                                if !p.is_empty() {
                                    body_parts.push(p.clone());
                                }
                            }
                        }
                        in_shape = false;
                        is_title_shape = false;
                        shape_paragraphs.clear();
                    }
                    b"txBody" => {
                        in_text_body = false;
                    }
                    b"p" if in_paragraph => {
                        let trimmed = current_para.trim().to_string();
                        shape_paragraphs.push(trimmed);
                        in_paragraph = false;
                        current_para.clear();
                    }
                    // --- Table ---
                    b"tc" if in_tc => {
                        current_row.push(cell_buffer.trim().to_string());
                        in_tc = false;
                    }
                    b"tr" if in_table => {
                        table_rows.push(std::mem::take(&mut current_row));
                    }
                    b"tbl" if in_table => {
                        if !table_rows.is_empty() {
                            body_parts.push(format_gfm_table(&table_rows));
                        }
                        in_table = false;
                        table_rows.clear();
                    }
                    // --- Picture ---
                    b"pic" if in_pic => {
                        let alt = if pic_alt.trim().is_empty() { "image" } else { pic_alt.trim() };
                        // Alt text escaping: strip newlines/brackets per markitdown convention.
                        let alt_clean: String = alt
                            .chars()
                            .map(|c| if matches!(c, '\r' | '\n' | '[' | ']') { ' ' } else { c })
                            .collect();
                        let alt_collapsed = alt_clean.split_whitespace().collect::<Vec<_>>().join(" ");
                        let src = if pic_embed.trim().is_empty() { "image" } else { pic_embed.trim() };
                        body_parts.push(format!("![{}]({})", alt_collapsed, src));
                        in_pic = false;
                    }
                    _ => {}
                }
            }
            Ok(Event::Text(ref e)) => {
                if let Ok(text) = e.unescape() {
                    if in_tc {
                        cell_buffer.push_str(&text);
                    } else if in_paragraph {
                        current_para.push_str(&text);
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
        buf.clear();
    }

    (title, body_parts)
}

/// Format a 2-D string matrix as a GFM pipe table.
/// First row is treated as the header.
fn format_gfm_table(rows: &[Vec<String>]) -> String {
    if rows.is_empty() { return String::new(); }
    let cols = rows.iter().map(|r| r.len()).max().unwrap_or(0);
    if cols == 0 { return String::new(); }

    let esc = |s: &str| s.replace('|', "\\|").replace('\n', " ");
    let mut out = String::new();

    // Header
    out.push_str("| ");
    for c in 0..cols {
        let cell = rows[0].get(c).map(|s| esc(s.trim())).unwrap_or_default();
        out.push_str(&cell);
        out.push_str(" | ");
    }
    out.pop(); // drop trailing space
    out.push('\n');

    // Separator
    out.push_str("| ");
    for _ in 0..cols {
        out.push_str("--- | ");
    }
    out.pop();
    out.push('\n');

    // Body
    for row in rows.iter().skip(1) {
        out.push_str("| ");
        for c in 0..cols {
            let cell = row.get(c).map(|s| esc(s.trim())).unwrap_or_default();
            out.push_str(&cell);
            out.push_str(" | ");
        }
        out.pop();
        out.push('\n');
    }
    out.pop(); // trailing newline
    out
}

/// Extract plain text from a notes slide XML.
fn extract_notes_text(xml: &str) -> String {
    let mut reader = Reader::from_str(xml);
    reader.trim_text(true);
    let mut buf = Vec::new();

    let mut in_text = false;
    let mut parts: Vec<String> = Vec::new();
    let mut current = String::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let name_bytes = e.name().as_ref().to_vec();
                let local = local_name(&name_bytes);
                if local == b"p" {
                    in_text = true;
                    current.clear();
                }
            }
            Ok(Event::End(ref e)) => {
                let name_bytes = e.name().as_ref().to_vec();
                let local = local_name(&name_bytes);
                if local == b"p" && in_text {
                    let t = current.trim().to_string();
                    if !t.is_empty() {
                        parts.push(t);
                    }
                    in_text = false;
                }
            }
            Ok(Event::Text(ref e)) => {
                if in_text {
                    if let Ok(text) = e.unescape() {
                        current.push_str(&text);
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
        buf.clear();
    }

    parts.join(" ")
}

/// Strip namespace prefix from a tag name (e.g., `p:sp` -> `sp`).
fn local_name(full: &[u8]) -> &[u8] {
    match full.iter().position(|&b| b == b':') {
        Some(pos) => &full[pos + 1..],
        None => full,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_slide_number() {
        assert_eq!(parse_slide_number("ppt/slides/slide1.xml"), Some(1));
        assert_eq!(parse_slide_number("ppt/slides/slide12.xml"), Some(12));
        assert_eq!(parse_slide_number("ppt/slides/slideLayouts/slideLayout1.xml"), None);
        assert_eq!(parse_slide_number("ppt/notesSlides/notesSlide1.xml"), None);
    }

    #[test]
    fn test_local_name() {
        assert_eq!(local_name(b"p:sp"), b"sp");
        assert_eq!(local_name(b"a:t"), b"t");
        assert_eq!(local_name(b"sp"), b"sp");
    }

    #[test]
    fn test_parse_slide_xml_basic() {
        let xml = r#"<?xml version="1.0"?>
<p:sld xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"
       xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main">
  <p:cSld>
    <p:spTree>
      <p:sp>
        <p:nvSpPr><p:nvPr><p:ph type="title"/></p:nvPr></p:nvSpPr>
        <p:txBody>
          <a:p><a:r><a:t>My Title</a:t></a:r></a:p>
        </p:txBody>
      </p:sp>
      <p:sp>
        <p:nvSpPr><p:nvPr><p:ph type="body"/></p:nvPr></p:nvSpPr>
        <p:txBody>
          <a:p><a:r><a:t>Body text here</a:t></a:r></a:p>
        </p:txBody>
      </p:sp>
    </p:spTree>
  </p:cSld>
</p:sld>"#;
        let (title, body) = parse_slide_xml(xml);
        assert_eq!(title, Some("My Title".to_string()));
        assert!(body.iter().any(|p| p.contains("Body text")));
    }

    #[test]
    fn test_parse_slide_xml_table() {
        let xml = r#"<?xml version="1.0"?>
<p:sld xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"
       xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main">
  <p:cSld><p:spTree>
    <p:graphicFrame><a:graphic><a:graphicData>
      <a:tbl>
        <a:tr>
          <a:tc><a:txBody><a:p><a:r><a:t>Name</a:t></a:r></a:p></a:txBody></a:tc>
          <a:tc><a:txBody><a:p><a:r><a:t>Role</a:t></a:r></a:p></a:txBody></a:tc>
        </a:tr>
        <a:tr>
          <a:tc><a:txBody><a:p><a:r><a:t>Alice</a:t></a:r></a:p></a:txBody></a:tc>
          <a:tc><a:txBody><a:p><a:r><a:t>Eng</a:t></a:r></a:p></a:txBody></a:tc>
        </a:tr>
      </a:tbl>
    </a:graphicData></a:graphic></p:graphicFrame>
  </p:spTree></p:cSld>
</p:sld>"#;
        let (_title, body) = parse_slide_xml(xml);
        let table_md = body.iter().find(|p| p.contains("| Name |")).expect("table emitted");
        assert!(table_md.contains("| Alice | Eng |"));
        assert!(table_md.contains("| --- | --- |"));
    }

    #[test]
    fn test_parse_slide_xml_picture() {
        let xml = r#"<?xml version="1.0"?>
<p:sld xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"
       xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"
       xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <p:cSld><p:spTree>
    <p:pic>
      <p:nvPicPr>
        <p:cNvPr id="1" name="Picture 1" descr="Cat photo"/>
      </p:nvPicPr>
      <p:blipFill>
        <a:blip r:embed="rId42"/>
      </p:blipFill>
    </p:pic>
  </p:spTree></p:cSld>
</p:sld>"#;
        let (_title, body) = parse_slide_xml(xml);
        assert!(body.iter().any(|p| p == "![Cat photo](rId42)"), "got: {:?}", body);
    }

    #[test]
    fn test_resolve_rel_target() {
        assert_eq!(
            resolve_rel_target("ppt/slides/", "../notesSlides/notesSlide1.xml"),
            "ppt/notesSlides/notesSlide1.xml"
        );
        assert_eq!(
            resolve_rel_target("ppt/slides/", "../media/image2.png"),
            "ppt/media/image2.png"
        );
        assert_eq!(
            resolve_rel_target("ppt/slides", "notesSlide.xml"),
            "ppt/slides/notesSlide.xml"
        );
    }

    #[test]
    fn test_find_notes_target() {
        let rels = r#"<?xml version="1.0"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout" Target="../slideLayouts/slideLayout2.xml"/>
  <Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/notesSlide" Target="../notesSlides/notesSlide1.xml"/>
</Relationships>"#;
        assert_eq!(
            find_notes_target(rels),
            Some("../notesSlides/notesSlide1.xml".to_string())
        );

        let no_notes = r#"<?xml version="1.0"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout" Target="../slideLayouts/slideLayout1.xml"/>
</Relationships>"#;
        assert_eq!(find_notes_target(no_notes), None);
    }

    #[test]
    fn test_format_gfm_table_ragged() {
        // Short row gets padded with empty cells.
        let rows = vec![
            vec!["A".into(), "B".into(), "C".into()],
            vec!["1".into(), "2".into()],
        ];
        let md = format_gfm_table(&rows);
        assert!(md.contains("| A | B | C |"));
        assert!(md.contains("| 1 | 2 |  |"));
    }
}
