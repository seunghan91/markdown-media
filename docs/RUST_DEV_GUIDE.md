# Rust Development Guide for MDM

> MDM 문서 변환기 개발을 위한 Rust 크레이트 및 개발 가이드

## 1. 개요

### 1.1 기술 스택

```
MDM Core Engine (Rust)
├── 문서 파싱 레이어
│   ├── HWP 5.0 (hwpers/cfb)
│   ├── DOCX (zip/quick-xml)
│   └── PDF (pdf-extract)
├── 변환 레이어
│   ├── Markdown 생성
│   └── MDM 메타데이터
├── 미디어 처리 레이어
│   ├── 이미지 최적화 (image)
│   └── SVG 렌더링 (resvg)
└── 출력 레이어
    ├── CLI 도구
    ├── FFI (Python/Node)
    └── WASM
```

### 1.2 Cargo.toml 권장 설정

```toml
[package]
name = "mdm-core"
version = "0.1.0"
edition = "2024"
rust-version = "1.75"

[dependencies]
# === 파일 포맷 ===

# HWP 파싱 (선택 1: 고수준)
hwpers = "0.3"

# HWP 파싱 (선택 2: 저수준)
# hwp = "0.2"

# OLE2/CFB 직접 처리
cfb = "0.11"

# DOCX/XLSX/PPTX (ZIP + XML)
zip = "2.2"
quick-xml = { version = "0.37", features = ["serialize"] }

# PDF
pdf-extract = "0.7"
# 또는
lopdf = "0.34"

# === 인코딩/압축 ===
encoding_rs = "0.8"
flate2 = "1.0"

# === 이미지 처리 ===
image = { version = "0.25", default-features = false, features = ["png", "jpeg", "webp"] }
resvg = "0.45"
usvg = "0.45"

# === 유틸리티 ===
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
thiserror = "2.0"
anyhow = "1.0"
byteorder = "1.5"
chrono = { version = "0.4", features = ["serde"] }
regex = "1.11"
walkdir = "2.5"

# === CLI ===
clap = { version = "4.5", features = ["derive"] }
indicatif = "0.17"  # 진행률 표시

# === 로깅 ===
tracing = "0.1"
tracing-subscriber = "0.3"

[dev-dependencies]
tempfile = "3.14"
pretty_assertions = "1.4"

[features]
default = ["hwp", "docx", "pdf"]
hwp = ["hwpers"]
docx = ["zip", "quick-xml"]
pdf = ["pdf-extract"]
wasm = []
ffi = []
```

---

## 2. 핵심 크레이트 상세

### 2.1 HWP 파싱

#### hwpers (권장)

```rust
use hwpers::{HwpDocument, HwpReader};

pub fn parse_hwp(path: &str) -> Result<String, Box<dyn std::error::Error>> {
    let doc = HwpDocument::from_path(path)?;

    let mut markdown = String::new();

    // 문단 추출
    for para in doc.paragraphs() {
        markdown.push_str(&para.text());
        markdown.push_str("\n\n");
    }

    // 테이블 추출
    for table in doc.tables() {
        markdown.push_str(&table_to_markdown(&table));
    }

    Ok(markdown)
}

fn table_to_markdown(table: &hwpers::Table) -> String {
    let mut md = String::new();

    for (i, row) in table.rows().enumerate() {
        md.push('|');
        for cell in row.cells() {
            md.push_str(&format!(" {} |", cell.text().trim()));
        }
        md.push('\n');

        // 헤더 구분선
        if i == 0 {
            md.push('|');
            for _ in row.cells() {
                md.push_str("---|");
            }
            md.push('\n');
        }
    }

    md
}
```

#### cfb 직접 사용 (저수준)

```rust
use cfb::CompoundFile;
use std::io::{Read, Cursor};

pub fn read_hwp_structure(path: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut file = std::fs::File::open(path)?;
    let mut data = Vec::new();
    file.read_to_end(&mut data)?;

    let cursor = Cursor::new(data);
    let mut cfb = CompoundFile::open(cursor)?;

    // 스트림 목록 출력
    for entry in cfb.walk() {
        println!("{}: {:?}", entry.path().display(), entry.len());
    }

    // FileHeader 읽기
    let mut header_data = Vec::new();
    cfb.open_stream("/FileHeader")?.read_to_end(&mut header_data)?;
    println!("FileHeader: {} bytes", header_data.len());

    // BodyText 섹션 읽기
    if let Ok(mut stream) = cfb.open_stream("/BodyText/Section0") {
        let mut section_data = Vec::new();
        stream.read_to_end(&mut section_data)?;
        println!("Section0: {} bytes", section_data.len());
    }

    Ok(())
}
```

### 2.2 DOCX 파싱

```rust
use std::io::Read;
use zip::ZipArchive;
use quick_xml::Reader;
use quick_xml::events::Event;

pub struct DocxParser {
    archive: ZipArchive<std::fs::File>,
}

impl DocxParser {
    pub fn new(path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let file = std::fs::File::open(path)?;
        let archive = ZipArchive::new(file)?;
        Ok(Self { archive })
    }

    pub fn extract_text(&mut self) -> Result<String, Box<dyn std::error::Error>> {
        let mut xml_content = String::new();
        self.archive
            .by_name("word/document.xml")?
            .read_to_string(&mut xml_content)?;

        self.parse_document_xml(&xml_content)
    }

    fn parse_document_xml(&self, xml: &str) -> Result<String, Box<dyn std::error::Error>> {
        let mut reader = Reader::from_str(xml);
        reader.config_mut().trim_text(true);

        let mut result = String::new();
        let mut in_text = false;
        let mut current_para = String::new();

        loop {
            match reader.read_event()? {
                Event::Start(e) | Event::Empty(e) => {
                    let name = e.name();
                    match name.as_ref() {
                        b"w:t" => in_text = true,
                        b"w:br" | b"w:cr" => current_para.push('\n'),
                        b"w:tab" => current_para.push('\t'),
                        _ => {}
                    }
                }
                Event::Text(e) if in_text => {
                    current_para.push_str(&e.unescape()?);
                }
                Event::End(e) => {
                    match e.name().as_ref() {
                        b"w:t" => in_text = false,
                        b"w:p" => {
                            if !current_para.is_empty() {
                                result.push_str(&current_para);
                                result.push_str("\n\n");
                                current_para.clear();
                            }
                        }
                        _ => {}
                    }
                }
                Event::Eof => break,
                _ => {}
            }
        }

        Ok(result)
    }

    pub fn extract_images(&mut self, output_dir: &str) -> Result<Vec<String>, Box<dyn std::error::Error>> {
        std::fs::create_dir_all(output_dir)?;

        let mut images = Vec::new();
        let names: Vec<String> = (0..self.archive.len())
            .filter_map(|i| self.archive.by_index(i).ok())
            .map(|f| f.name().to_string())
            .filter(|n| n.starts_with("word/media/"))
            .collect();

        for name in names {
            let file_name = std::path::Path::new(&name)
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .to_string();

            let mut file = self.archive.by_name(&name)?;
            let output_path = format!("{}/{}", output_dir, file_name);
            let mut output = std::fs::File::create(&output_path)?;
            std::io::copy(&mut file, &mut output)?;

            images.push(file_name);
        }

        Ok(images)
    }
}
```

### 2.3 PDF 파싱

```rust
use pdf_extract::extract_text;

pub fn parse_pdf(path: &str) -> Result<String, Box<dyn std::error::Error>> {
    let text = extract_text(path)?;
    Ok(text)
}

// 페이지별 추출 (lopdf 사용)
use lopdf::Document;

pub fn parse_pdf_pages(path: &str) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let doc = Document::load(path)?;
    let mut pages = Vec::new();

    for page_num in 1..=doc.get_pages().len() {
        let page_id = doc.get_pages().get(&(page_num as u32)).cloned();
        if let Some(_id) = page_id {
            // 페이지별 텍스트 추출 로직
            // (lopdf는 저수준이므로 pdf-extract 권장)
            pages.push(format!("Page {}", page_num));
        }
    }

    Ok(pages)
}
```

### 2.4 이미지 처리

```rust
use image::{DynamicImage, ImageFormat, imageops::FilterType};

pub struct ImageOptimizer {
    pub quality: u8,
    pub max_width: u32,
    pub max_height: u32,
}

impl Default for ImageOptimizer {
    fn default() -> Self {
        Self {
            quality: 85,
            max_width: 1920,
            max_height: 1080,
        }
    }
}

impl ImageOptimizer {
    pub fn optimize(&self, input: &[u8], output_format: ImageFormat) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        let img = image::load_from_memory(input)?;

        // 리사이즈 (필요시)
        let img = self.resize_if_needed(img);

        // 출력
        let mut output = Vec::new();
        let mut cursor = std::io::Cursor::new(&mut output);

        match output_format {
            ImageFormat::Jpeg => {
                img.write_to(&mut cursor, ImageFormat::Jpeg)?;
            }
            ImageFormat::Png => {
                img.write_to(&mut cursor, ImageFormat::Png)?;
            }
            ImageFormat::WebP => {
                img.write_to(&mut cursor, ImageFormat::WebP)?;
            }
            _ => {
                img.write_to(&mut cursor, ImageFormat::Png)?;
            }
        }

        Ok(output)
    }

    fn resize_if_needed(&self, img: DynamicImage) -> DynamicImage {
        let (w, h) = (img.width(), img.height());

        if w <= self.max_width && h <= self.max_height {
            return img;
        }

        let ratio_w = self.max_width as f64 / w as f64;
        let ratio_h = self.max_height as f64 / h as f64;
        let ratio = ratio_w.min(ratio_h);

        let new_w = (w as f64 * ratio) as u32;
        let new_h = (h as f64 * ratio) as u32;

        img.resize(new_w, new_h, FilterType::Lanczos3)
    }
}
```

### 2.5 SVG 렌더링

```rust
use resvg::tiny_skia::Pixmap;
use resvg::usvg::{Options, Tree};

pub fn render_svg_to_png(svg_data: &str, scale: f32) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let opt = Options::default();
    let tree = Tree::from_str(svg_data, &opt)?;

    let size = tree.size();
    let width = (size.width() * scale) as u32;
    let height = (size.height() * scale) as u32;

    let mut pixmap = Pixmap::new(width, height)
        .ok_or("Failed to create pixmap")?;

    let transform = resvg::tiny_skia::Transform::from_scale(scale, scale);
    resvg::render(&tree, transform, &mut pixmap.as_mut());

    Ok(pixmap.encode_png()?)
}

pub fn create_placeholder_svg(width: u32, height: u32, text: &str) -> String {
    format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{}" height="{}" viewBox="0 0 {} {}">
  <rect width="100%" height="100%" fill="#f0f0f0"/>
  <text x="50%" y="50%" dominant-baseline="middle" text-anchor="middle"
        font-family="sans-serif" font-size="14" fill="#666">{}</text>
</svg>"#,
        width, height, width, height, text
    )
}
```

---

## 3. 통합 API 설계

### 3.1 Document Trait

```rust
use std::path::Path;

/// 문서 포맷 공통 인터페이스
pub trait Document {
    /// 텍스트 추출
    fn extract_text(&self) -> Result<String, ConvertError>;

    /// Markdown 변환
    fn to_markdown(&self) -> Result<String, ConvertError>;

    /// 이미지 추출
    fn extract_images(&self, output_dir: &Path) -> Result<Vec<ImageRef>, ConvertError>;

    /// 테이블 추출
    fn extract_tables(&self) -> Result<Vec<Table>, ConvertError>;

    /// 메타데이터 추출
    fn metadata(&self) -> Result<Metadata, ConvertError>;
}

#[derive(Debug)]
pub struct ImageRef {
    pub name: String,
    pub path: std::path::PathBuf,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub alt_text: Option<String>,
}

#[derive(Debug)]
pub struct Table {
    pub rows: Vec<Vec<String>>,
    pub has_header: bool,
}

#[derive(Debug, Default)]
pub struct Metadata {
    pub title: Option<String>,
    pub author: Option<String>,
    pub created: Option<chrono::DateTime<chrono::Utc>>,
    pub modified: Option<chrono::DateTime<chrono::Utc>>,
    pub page_count: Option<u32>,
}

#[derive(Debug, thiserror::Error)]
pub enum ConvertError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Parse error: {0}")]
    Parse(String),

    #[error("Unsupported format: {0}")]
    UnsupportedFormat(String),

    #[error("Image processing error: {0}")]
    Image(String),
}
```

### 3.2 Format Detection

```rust
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DocumentFormat {
    Hwp,
    Hwpx,
    Docx,
    Doc,
    Pdf,
    Txt,
    Rtf,
    Odt,
    Html,
    Markdown,
    Unknown,
}

impl DocumentFormat {
    pub fn from_path(path: &Path) -> Self {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase());

        match ext.as_deref() {
            Some("hwp") => Self::Hwp,
            Some("hwpx") => Self::Hwpx,
            Some("docx") => Self::Docx,
            Some("doc") => Self::Doc,
            Some("pdf") => Self::Pdf,
            Some("txt") => Self::Txt,
            Some("rtf") => Self::Rtf,
            Some("odt") => Self::Odt,
            Some("html") | Some("htm") => Self::Html,
            Some("md") | Some("markdown") => Self::Markdown,
            _ => Self::Unknown,
        }
    }

    pub fn from_magic(bytes: &[u8]) -> Self {
        if bytes.len() < 8 {
            return Self::Unknown;
        }

        // HWP 5.0 (OLE2/CFB)
        if bytes.starts_with(&[0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1]) {
            return Self::Hwp; // 또는 DOC - 추가 검사 필요
        }

        // ZIP (DOCX, HWPX, ODT 등)
        if bytes.starts_with(&[0x50, 0x4B, 0x03, 0x04]) {
            // ZIP 내부 확인 필요
            return Self::Docx; // 기본값, 추가 검사 필요
        }

        // PDF
        if bytes.starts_with(b"%PDF") {
            return Self::Pdf;
        }

        // RTF
        if bytes.starts_with(b"{\\rtf") {
            return Self::Rtf;
        }

        Self::Unknown
    }
}
```

### 3.3 Converter Factory

```rust
use std::path::Path;

pub struct Converter;

impl Converter {
    pub fn open(path: &Path) -> Result<Box<dyn Document>, ConvertError> {
        let format = DocumentFormat::from_path(path);

        match format {
            DocumentFormat::Hwp => {
                #[cfg(feature = "hwp")]
                {
                    Ok(Box::new(HwpDocument::open(path)?))
                }
                #[cfg(not(feature = "hwp"))]
                {
                    Err(ConvertError::UnsupportedFormat("HWP support not enabled".into()))
                }
            }
            DocumentFormat::Docx => {
                #[cfg(feature = "docx")]
                {
                    Ok(Box::new(DocxDocument::open(path)?))
                }
                #[cfg(not(feature = "docx"))]
                {
                    Err(ConvertError::UnsupportedFormat("DOCX support not enabled".into()))
                }
            }
            DocumentFormat::Pdf => {
                #[cfg(feature = "pdf")]
                {
                    Ok(Box::new(PdfDocument::open(path)?))
                }
                #[cfg(not(feature = "pdf"))]
                {
                    Err(ConvertError::UnsupportedFormat("PDF support not enabled".into()))
                }
            }
            DocumentFormat::Txt => {
                Ok(Box::new(TxtDocument::open(path)?))
            }
            _ => Err(ConvertError::UnsupportedFormat(format!("{:?}", format))),
        }
    }
}
```

---

## 4. CLI 구현

### 4.1 명령줄 인터페이스

```rust
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "mdm")]
#[command(about = "MDM - Markdown+Media Converter", long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Convert document to MDM format
    Convert {
        /// Input file path
        #[arg(short, long)]
        input: PathBuf,

        /// Output directory
        #[arg(short, long)]
        output: PathBuf,

        /// Force overwrite existing files
        #[arg(short, long, default_value = "false")]
        force: bool,

        /// Image quality (1-100)
        #[arg(short, long, default_value = "85")]
        quality: u8,
    },

    /// Extract only text content
    Text {
        /// Input file path
        input: PathBuf,
    },

    /// Extract images from document
    Images {
        /// Input file path
        input: PathBuf,

        /// Output directory
        #[arg(short, long)]
        output: PathBuf,
    },

    /// Show document information
    Info {
        /// Input file path
        input: PathBuf,
    },
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Convert { input, output, force, quality } => {
            println!("Converting {:?} to {:?}", input, output);
            // 변환 로직
        }
        Commands::Text { input } => {
            let doc = Converter::open(&input)?;
            println!("{}", doc.extract_text()?);
        }
        Commands::Images { input, output } => {
            let doc = Converter::open(&input)?;
            let images = doc.extract_images(&output)?;
            println!("Extracted {} images", images.len());
        }
        Commands::Info { input } => {
            let doc = Converter::open(&input)?;
            let meta = doc.metadata()?;
            println!("Title: {:?}", meta.title);
            println!("Author: {:?}", meta.author);
        }
    }

    Ok(())
}
```

---

## 5. 테스트 전략

### 5.1 유닛 테스트

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_detection() {
        assert_eq!(
            DocumentFormat::from_path(Path::new("test.hwp")),
            DocumentFormat::Hwp
        );
        assert_eq!(
            DocumentFormat::from_path(Path::new("test.docx")),
            DocumentFormat::Docx
        );
    }

    #[test]
    fn test_magic_bytes() {
        let pdf_magic = b"%PDF-1.4";
        assert_eq!(DocumentFormat::from_magic(pdf_magic), DocumentFormat::Pdf);
    }
}
```

### 5.2 통합 테스트

```rust
// tests/integration_test.rs
use mdm_core::Converter;
use tempfile::tempdir;

#[test]
fn test_hwp_conversion() {
    let sample = std::path::Path::new("samples/sample.hwp");
    if !sample.exists() {
        return; // 샘플 파일 없으면 스킵
    }

    let doc = Converter::open(sample).unwrap();
    let text = doc.extract_text().unwrap();
    assert!(!text.is_empty());
}

#[test]
fn test_docx_image_extraction() {
    let sample = std::path::Path::new("samples/sample.docx");
    if !sample.exists() {
        return;
    }

    let dir = tempdir().unwrap();
    let doc = Converter::open(sample).unwrap();
    let images = doc.extract_images(dir.path()).unwrap();

    // 이미지가 실제로 추출되었는지 확인
    for img in &images {
        assert!(dir.path().join(&img.name).exists());
    }
}
```

---

## 6. 성능 최적화

### 6.1 병렬 처리

```rust
use rayon::prelude::*;

pub fn batch_convert(inputs: &[PathBuf], output_dir: &Path) -> Vec<Result<(), ConvertError>> {
    inputs
        .par_iter()
        .map(|input| {
            let doc = Converter::open(input)?;
            let md = doc.to_markdown()?;

            let output_file = output_dir.join(
                input.file_stem().unwrap()
            ).with_extension("md");

            std::fs::write(output_file, md)?;
            Ok(())
        })
        .collect()
}
```

### 6.2 스트리밍 처리

```rust
use std::io::{BufReader, BufWriter};

pub fn convert_streaming<R: Read, W: Write>(
    input: BufReader<R>,
    output: BufWriter<W>,
) -> Result<(), ConvertError> {
    // 대용량 파일 스트리밍 처리
    // 메모리 사용량 제한
    todo!()
}
```

---

## 7. 참고 자료

### Rust 크레이트
- [hwpers](https://crates.io/crates/hwpers) - HWP 5.0 파서
- [hwp-rs](https://github.com/hahnlee/hwp-rs) - 저수준 HWP 파서
- [cfb](https://crates.io/crates/cfb) - OLE2/CFB 파일 읽기
- [zip](https://crates.io/crates/zip) - ZIP 아카이브
- [quick-xml](https://crates.io/crates/quick-xml) - 고성능 XML 파서
- [pdf-extract](https://crates.io/crates/pdf-extract) - PDF 텍스트 추출
- [image](https://crates.io/crates/image) - 이미지 처리
- [resvg](https://crates.io/crates/resvg) - SVG 렌더링

### 개발 도구
- [cargo-watch](https://crates.io/crates/cargo-watch) - 자동 재빌드
- [cargo-expand](https://crates.io/crates/cargo-expand) - 매크로 확장 확인
- [criterion](https://crates.io/crates/criterion) - 벤치마크

---

## 변경 이력

| 날짜 | 버전 | 변경 내용 |
|------|------|----------|
| 2025-12-25 | 1.0 | 초기 문서 작성 |
