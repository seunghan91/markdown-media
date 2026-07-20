//! EPUB (Electronic Publication) parser.
//!
//! Converts `.epub` e-books to Markdown. EPUB is a ZIP container of XHTML
//! content files with an OPF manifest and NCX/NAV spine ordering.
//!
//! Uses existing `zip` + `quick-xml` dependencies — no additional crates needed.
//!
//! Feature-gated behind `epub` (see `core/Cargo.toml`).

use std::collections::HashMap;
use std::io::{self, Cursor, Read};
use std::path::Path;

use quick_xml::events::Event;
use quick_xml::Reader;

#[derive(Debug, Clone)]
pub struct EpubMetadata {
    pub title: Option<String>,
    pub author: Option<String>,
    pub language: Option<String>,
    pub chapters: Vec<EpubChapter>,
}

#[derive(Debug, Clone)]
pub struct EpubChapter {
    pub id: String,
    pub title: String,
    pub path: String,
    pub play_order: u32,
}

#[derive(Debug, Clone)]
pub struct EpubDocument {
    pub metadata: EpubMetadata,
    pub text: String,
}

pub struct EpubParser {
    data: Vec<u8>,
}

impl EpubParser {
    pub fn open<P: AsRef<Path>>(path: P) -> io::Result<Self> {
        let mut data = Vec::new();
        std::fs::File::open(path.as_ref())?.read_to_end(&mut data)?;
        Ok(Self { data })
    }

    pub fn from_bytes(data: Vec<u8>) -> io::Result<Self> {
        Ok(Self { data })
    }

    pub fn parse(&self) -> io::Result<EpubDocument> {
        parse_epub(&self.data)
    }
}

pub fn parse_epub(data: &[u8]) -> io::Result<EpubDocument> {
    let cursor = Cursor::new(data);
    let mut archive = zip::ZipArchive::new(cursor)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;

    let container_xml = read_zip_entry(&mut archive, "META-INF/container.xml")?;
    let rootfile_path = parse_container(&container_xml)?;

    let opf_xml = read_zip_entry(&mut archive, &rootfile_path)?;
    let (metadata, spine_ids, manifest) = parse_opf(&opf_xml)?;

    let ncx_xml = find_ncx(&mut archive, &manifest)?;
    let mut chapters = if let Some(ncx) = ncx_xml {
        parse_ncx(&ncx)?
    } else {
        Vec::new()
    };

    if chapters.is_empty() {
        chapters = spine_ids
            .iter()
            .enumerate()
            .map(|(i, id)| {
                let (path, _) = manifest.get(id).cloned().unwrap_or_default();
                EpubChapter {
                    id: id.clone(),
                    title: format!("Chapter {}", i + 1),
                    path,
                    play_order: i as u32,
                }
            })
            .collect();
    }

    let base = Path::new(&rootfile_path)
        .parent()
        .unwrap_or(Path::new(""));
    let mut text = String::new();

    for ch in &chapters {
        if !ch.path.is_empty() {
            let full_path = if ch.path.starts_with('/') {
                ch.path.clone()
            } else {
                base.join(&ch.path).to_string_lossy().to_string()
            };
            let full_path = full_path.replace('\\', "/");

            if let Ok(html) = read_zip_entry(&mut archive, &full_path) {
                text.push_str(&format!("# {}\n\n", ch.title));
                text.push_str(&html_to_markdown(&html));
                text.push_str("\n\n");
            }
        }
    }

    Ok(EpubDocument {
        metadata: EpubMetadata {
            title: metadata.get("title").cloned(),
            author: metadata.get("creator").cloned(),
            language: metadata.get("language").cloned(),
            chapters,
        },
        text: text.trim().to_string(),
    })
}

fn read_zip_entry<R: std::io::Read + std::io::Seek>(
    archive: &mut zip::ZipArchive<R>,
    name: &str,
) -> io::Result<String> {
    let mut file = archive
        .by_name(name)
        .map_err(|e| io::Error::new(io::ErrorKind::NotFound, e.to_string()))?;
    let mut buf = Vec::new();
    file.read_to_end(&mut buf)?;
    Ok(String::from_utf8_lossy(&buf).into_owned())
}

fn parse_container(xml: &str) -> io::Result<String> {
    let mut reader = Reader::from_reader(Cursor::new(xml.as_bytes()));
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Empty(ref e)) if e.name().as_ref() == b"rootfile" => {
                for attr in e.attributes().flatten() {
                    if attr.key.as_ref() == b"full-path" {
                        return Ok(String::from_utf8_lossy(&attr.value).to_string());
                    }
                }
                break;
            }
            Ok(Event::End(_)) => break,
            Err(_) => break,
            _ => {}
        }
        buf.clear();
    }
    Err(io::Error::new(io::ErrorKind::InvalidData, "no rootfile in container.xml"))
}

fn parse_opf(xml: &str) -> io::Result<(HashMap<String, String>, Vec<String>, HashMap<String, (String, String)>)> {
    let mut reader = Reader::from_reader(Cursor::new(xml.as_bytes()));
    let mut buf = Vec::new();
    let mut metadata: HashMap<String, String> = HashMap::new();
    let mut spine_ids: Vec<String> = Vec::new();
    let mut manifest: HashMap<String, (String, String)> = HashMap::new();
    let mut current_tag = String::new();
    let mut in_metadata = false;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                match name.as_str() {
                    "metadata" | "opf:metadata" => in_metadata = true,
                    "spine" | "opf:spine" => {}
                    _ if in_metadata => current_tag = name.clone(),
                    _ => {}
                }
                if name == "itemref" || name == "opf:itemref" {
                    if let Some(id) = e.attributes().flatten().find(|a| a.key.as_ref() == b"idref") {
                        spine_ids.push(String::from_utf8_lossy(&id.value).to_string());
                    }
                }
                if name == "item" || name == "opf:item" {
                    let mut id = String::new();
                    let mut href = String::new();
                    let mut media_type = String::new();
                    for attr in e.attributes().flatten() {
                        match attr.key.as_ref() {
                            b"id" => id = String::from_utf8_lossy(&attr.value).to_string(),
                            b"href" => href = String::from_utf8_lossy(&attr.value).to_string(),
                            b"media-type" => media_type = String::from_utf8_lossy(&attr.value).to_string(),
                            _ => {}
                        }
                    }
                    if !id.is_empty() {
                        manifest.insert(id, (href, media_type));
                    }
                }
            }
            Ok(Event::Text(ref e)) => {
                if in_metadata && !current_tag.is_empty() {
                    let val = e.unescape().unwrap_or_default().to_string();
                    metadata
                        .entry(current_tag.clone())
                        .and_modify(|v| {
                            v.push_str(&val);
                        })
                        .or_insert(val);
                    current_tag.clear();
                }
            }
            Ok(Event::End(ref e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                if name == "metadata" || name == "opf:metadata" {
                    in_metadata = false;
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
        buf.clear();
    }

    Ok((metadata, spine_ids, manifest))
}

fn find_ncx<R: std::io::Read + std::io::Seek>(
    archive: &mut zip::ZipArchive<R>,
    manifest: &HashMap<String, (String, String)>,
) -> io::Result<Option<String>> {
    for (_id, (_href, media_type)) in manifest {
        if media_type == "application/x-dtbncx+xml" {
            return read_zip_entry(archive, _href).map(Some);
        }
    }
    for (_id, (href, _)) in manifest {
        if href.ends_with(".ncx") {
            return read_zip_entry(archive, href).map(Some);
        }
    }
    Ok(None)
}

fn parse_ncx(xml: &str) -> io::Result<Vec<EpubChapter>> {
    let mut reader = Reader::from_reader(Cursor::new(xml.as_bytes()));
    let mut buf = Vec::new();
    let mut chapters = Vec::new();
    let mut in_navpoint = false;
    let mut current_id = String::new();
    let mut current_order: u32 = 0;
    let mut in_label = false;
    let mut label_text = String::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                if name == "navPoint" {
                    in_navpoint = true;
                    current_id.clear();
                    label_text.clear();
                    if let Some(attr) = e.attributes().flatten().find(|a| a.key.as_ref() == b"id") {
                        current_id = String::from_utf8_lossy(&attr.value).to_string();
                    }
                    if let Some(attr) = e.attributes().flatten().find(|a| a.key.as_ref() == b"playOrder") {
                        current_order = String::from_utf8_lossy(&attr.value).parse().unwrap_or(0);
                    }
                }
                if in_navpoint && (name == "navLabel" || name == "text") {
                    in_label = true;
                }
            }
            Ok(Event::Text(ref e)) if in_label => {
                label_text = e.unescape().unwrap_or_default().to_string();
                in_label = false;
            }
            Ok(Event::Start(ref e)) if in_navpoint => {
                if String::from_utf8_lossy(e.name().as_ref()) == "content" {
                    if let Some(attr) = e.attributes().flatten().find(|a| a.key.as_ref() == b"src") {
                        let src = String::from_utf8_lossy(&attr.value).to_string();
                        let path = src.split('#').next().unwrap_or(&src).to_string();
                        chapters.push(EpubChapter {
                            id: current_id.clone(),
                            title: if label_text.is_empty() { current_id.clone() } else { label_text.clone() },
                            path,
                            play_order: current_order,
                        });
                    }
                }
            }
            Ok(Event::End(ref e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                if name == "navPoint" {
                    in_navpoint = false;
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
        buf.clear();
    }
    Ok(chapters)
}

fn html_to_markdown(html: &str) -> String {
    let mut out = String::new();
    let mut skip = false;
    let mut skip_tag = String::new();

    for ch in html.chars() {
        if ch == '<' {
            skip = true;
            skip_tag.clear();
            continue;
        }
        if skip {
            if ch == '>' {
                skip = false;
                let tag = skip_tag.to_lowercase();
                if tag == "br" || tag == "br/" || tag.starts_with("br ") {
                    out.push('\n');
                }
                if tag == "p" || tag == "/p" || tag == "div" || tag == "/div"
                    || tag.starts_with("h1") || tag == "/h1"
                    || tag.starts_with("h2") || tag == "/h2"
                    || tag.starts_with("h3") || tag == "/h3"
                    || tag.starts_with("li") || tag == "/li"
                    || tag == "tr" || tag == "/tr"
                {
                    out.push('\n');
                }
                if tag.starts_with("td") || tag == "/td" || tag.starts_with("th") || tag == "/th" {
                    out.push_str(" | ");
                }
            } else {
                skip_tag.push(ch);
            }
            continue;
        }
        match ch {
            '\u{00A0}' => out.push(' '),
            _ => out.push(ch),
        }
    }

    let s = out;
    let mut result = String::new();
    let mut prev_blank = false;
    for line in s.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            if !prev_blank {
                result.push('\n');
                prev_blank = true;
            }
        } else {
            result.push_str(trimmed);
            result.push('\n');
            prev_blank = false;
        }
    }
    result.trim().to_string()
}

impl EpubDocument {
    pub fn to_markdown(&self) -> String {
        self.text.clone()
    }

    pub fn to_mdx(&self, source_name: &str) -> String {
        let chapters = self.metadata.chapters.len();
        let title = self.metadata.title.as_deref().unwrap_or("");
        format!(
            "---\nformat: epub\nsource: \"{}\"\ntitle: \"{}\"\nchapters: {}\n---\n\n{}",
            source_name.replace('"', "\\\""),
            title.replace('"', "\\\""),
            chapters,
            self.to_markdown(),
        )
    }
}

pub fn looks_like_epub(data: &[u8]) -> bool {
    if data.len() < 58 {
        return false;
    }
    if data[..4] != *b"PK\x03\x04" {
        return false;
    }
    let s = String::from_utf8_lossy(data);
    s.contains("mimetypeapplication/epub+zip")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_looks_like_epub_true() {
        let mut buf = vec![0u8; 100];
        buf[..4].copy_from_slice(b"PK\x03\x04");
        let mimetype = "mimetypeapplication/epub+zip";
        buf[30..30 + mimetype.len()].copy_from_slice(mimetype.as_bytes());
        assert!(looks_like_epub(&buf));
    }

    #[test]
    fn test_looks_like_epub_false() {
        assert!(!looks_like_epub(b"PK\x03\x04not epub"));
        assert!(!looks_like_epub(b""));
    }

    #[test]
    fn test_parse_container() {
        let xml = r#"<?xml version='1.0' encoding='utf-8'?>
        <container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
          <rootfiles>
            <rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/>
          </rootfiles>
        </container>"#;
        let path = parse_container(xml).expect("parse");
        assert_eq!(path, "OEBPS/content.opf");
    }
}
