// Ported from kkdoc (MIT): src/hwpx/gen-image.ts
//! Image embedding — markdown `![alt](url)` / HTML `<img>` refs become embedded
//! BinData parts plus an inline `<hp:pic>`.
//!
//! Data URIs embed the real decoded bytes; bare filenames embed a 1×1
//! placeholder (preserving reference + position, as the reference does).
//! Unacceptable refs return None so the caller falls back to alt text.

use std::collections::{HashMap, HashSet};

use base64::Engine;
use lazy_static::lazy_static;
use regex::Regex;

const PLACEHOLDER_BMP: &[u8] = &[
    0x42, 0x4d, 0x3a, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x36, 0x00, 0x00, 0x00, 0x28, 0x00,
    0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x00, 0x18, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x04, 0x00, 0x00, 0x00, 0x13, 0x0b, 0x00, 0x00, 0x13, 0x0b, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xff, 0xff, 0xff, 0x00,
];
const PLACEHOLDER_PNG: &[u8] = &[
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1f, 0x15, 0xc4,
    0x89, 0x00, 0x00, 0x00, 0x0b, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9c, 0x63, 0x00, 0x01, 0x00, 0x00,
    0x05, 0x00, 0x01, 0x0d, 0x0a, 0x2d, 0xb4, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4e, 0x44, 0xae,
    0x42, 0x60, 0x82,
];

fn ext_mime(ext: &str) -> Option<&'static str> {
    match ext.to_ascii_lowercase().as_str() {
        "png" => Some("image/png"),
        "jpg" | "jpeg" => Some("image/jpeg"),
        "gif" => Some("image/gif"),
        "bmp" => Some("image/bmp"),
        "tif" | "tiff" => Some("image/tiff"),
        _ => None,
    }
}

fn mime_ext(mime: &str) -> Option<&'static str> {
    match mime.to_ascii_lowercase().as_str() {
        "image/png" => Some("png"),
        "image/jpeg" | "image/jpg" => Some("jpg"),
        "image/gif" => Some("gif"),
        "image/bmp" => Some("bmp"),
        "image/tiff" => Some("tif"),
        _ => None,
    }
}

/// An embedded image part for the ZIP + manifest.
#[derive(Debug, Clone)]
pub struct ImagePart {
    /// ZIP path (BinData/xxx.ext).
    pub name: String,
    /// manifest item id == binaryItemIDRef.
    pub item_id: String,
    pub mime: String,
    pub data: Vec<u8>,
}

lazy_static! {
    static ref RE_FILENAME: Regex = Regex::new(r"^([A-Za-z0-9._-]+)\.([A-Za-z0-9]+)$").unwrap();
    static ref RE_DATA_URI: Regex =
        Regex::new(r"^data:([A-Za-z0-9/+.-]+);base64,(.+)$").unwrap();
    static ref RE_MD_IMAGE: Regex = Regex::new(r"!\[[^\]]*\]\(([^)\s]+)\)").unwrap();
}

/// Per-document image registry: url-deduped, safe relative filenames only.
#[derive(Default)]
pub struct ImageRegistry {
    by_url: HashMap<String, Option<ImagePart>>,
    ids: HashSet<String>,
    pub parts: Vec<ImagePart>,
    pic_seq: u32,
    data_seq: u32,
}

impl ImageRegistry {
    pub fn new() -> Self {
        ImageRegistry::default()
    }

    /// Register (or fetch cached) a url. None if not acceptable.
    pub fn take(&mut self, url: &str) -> Option<ImagePart> {
        if let Some(cached) = self.by_url.get(url) {
            return cached.clone();
        }
        let part = self.build_part(url);
        self.by_url.insert(url.to_string(), part.clone());
        part
    }

    fn build_part(&mut self, url: &str) -> Option<ImagePart> {
        if let Some(caps) = RE_DATA_URI.captures(url) {
            let mime = caps.get(1).unwrap().as_str().to_string();
            let ext = mime_ext(&mime)?;
            let data = base64::engine::general_purpose::STANDARD
                .decode(caps.get(2).unwrap().as_str().trim())
                .ok()?;
            self.data_seq += 1;
            let base = format!("image{}", self.data_seq);
            let item_id = self.unique_id(&base);
            let part = ImagePart {
                name: format!("BinData/{item_id}.{ext}"),
                item_id,
                mime,
                data,
            };
            self.parts.push(part.clone());
            return Some(part);
        }
        if let Some(caps) = RE_FILENAME.captures(url) {
            if url.contains("..") {
                return None;
            }
            let stem = caps.get(1).unwrap().as_str();
            let ext = caps.get(2).unwrap().as_str();
            let mime = ext_mime(ext)?.to_string();
            let sanitized: String = stem
                .chars()
                .map(|c| if c.is_ascii_alphanumeric() || c == '_' { c } else { '_' })
                .collect();
            let item_id = self.unique_id(&sanitized);
            let data = if ext.eq_ignore_ascii_case("bmp") {
                PLACEHOLDER_BMP.to_vec()
            } else {
                PLACEHOLDER_PNG.to_vec()
            };
            let part = ImagePart {
                name: format!("BinData/{url}"),
                item_id,
                mime,
                data,
            };
            self.parts.push(part.clone());
            return Some(part);
        }
        None
    }

    fn unique_id(&mut self, base: &str) -> String {
        let mut id = base.to_string();
        let mut n = 1;
        while self.ids.contains(&id) {
            id = format!("{base}_{n}");
            n += 1;
        }
        self.ids.insert(id.clone());
        id
    }

    /// manifest `<opf:item>` fragments.
    pub fn manifest_items(&self) -> Vec<String> {
        self.parts
            .iter()
            .map(|p| {
                format!(
                    "<opf:item id=\"{id}\" href=\"{name}\" media-type=\"{mime}\" isEmbeded=\"1\"/>",
                    id = p.item_id,
                    name = p.name,
                    mime = p.mime
                )
            })
            .collect()
    }

    /// Inline `<hp:pic>` XML mirroring the reference saved form (treatAsChar=1).
    pub fn inline_pic_xml(&mut self, part: &ImagePart) -> String {
        self.pic_seq += 1;
        let id = 9_400_000 + self.pic_seq;
        let s = 1130i32; // ≈4mm
        let half = s / 2;
        format!(
            "<hp:pic id=\"{id}\" zOrder=\"0\" numberingType=\"PICTURE\" textWrap=\"TOP_AND_BOTTOM\" textFlow=\"BOTH_SIDES\" lock=\"0\" dropcapstyle=\"None\" href=\"\" groupLevel=\"0\" instid=\"{id}\" reverse=\"0\" xmlns:hc=\"http://www.hancom.co.kr/hwpml/2011/core\">\
            <hp:offset x=\"0\" y=\"0\"/><hp:orgSz width=\"{s}\" height=\"{s}\"/><hp:curSz width=\"{s}\" height=\"{s}\"/>\
            <hp:flip horizontal=\"0\" vertical=\"0\"/><hp:rotationInfo angle=\"0\" centerX=\"{half}\" centerY=\"{half}\" rotateimage=\"1\"/>\
            <hp:renderingInfo><hc:transMatrix e1=\"1\" e2=\"0\" e3=\"0\" e4=\"0\" e5=\"1\" e6=\"0\"/><hc:scaMatrix e1=\"1\" e2=\"0\" e3=\"0\" e4=\"0\" e5=\"1\" e6=\"0\"/><hc:rotMatrix e1=\"1\" e2=\"0\" e3=\"0\" e4=\"0\" e5=\"1\" e6=\"0\"/></hp:renderingInfo>\
            <hp:imgRect><hc:pt0 x=\"0\" y=\"0\"/><hc:pt1 x=\"{s}\" y=\"0\"/><hc:pt2 x=\"{s}\" y=\"{s}\"/><hc:pt3 x=\"0\" y=\"{s}\"/></hp:imgRect>\
            <hp:imgClip left=\"0\" right=\"{s}\" top=\"0\" bottom=\"{s}\"/><hp:inMargin left=\"0\" right=\"0\" top=\"0\" bottom=\"0\"/>\
            <hp:imgDim dimwidth=\"{s}\" dimheight=\"{s}\"/>\
            <hc:img binaryItemIDRef=\"{item}\" bright=\"0\" contrast=\"0\" effect=\"REAL_PIC\" alpha=\"0\"/><hp:effects/>\
            <hp:sz width=\"{s}\" widthRelTo=\"ABSOLUTE\" height=\"{s}\" heightRelTo=\"ABSOLUTE\" protect=\"0\"/>\
            <hp:pos treatAsChar=\"1\" affectLSpacing=\"0\" flowWithText=\"1\" allowOverlap=\"0\" holdAnchorAndSO=\"0\" vertRelTo=\"PARA\" horzRelTo=\"PARA\" vertAlign=\"TOP\" horzAlign=\"LEFT\" vertOffset=\"0\" horzOffset=\"0\"/>\
            <hp:outMargin left=\"0\" right=\"0\" top=\"0\" bottom=\"0\"/>\
            </hp:pic>",
            id = id, s = s, half = half, item = part.item_id
        )
    }
}

/// Strip image refs from text, returning (remaining_text, urls).
pub fn split_image_refs(text: &str) -> (String, Vec<String>) {
    let mut urls = Vec::new();
    let out = RE_MD_IMAGE.replace_all(text, |c: &regex::Captures| {
        urls.push(c.get(1).unwrap().as_str().to_string());
        ""
    });
    (out.into_owned(), urls)
}
