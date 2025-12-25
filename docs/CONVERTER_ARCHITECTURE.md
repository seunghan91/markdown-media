# MDM Converter Architecture

> 다양한 문서 포맷을 Markdown + MDM 형식으로 변환하는 통합 아키텍처 설계

## 1. 아키텍처 개요

### 1.1 변환 파이프라인

```
┌─────────────────────────────────────────────────────────────────────────┐
│                         INPUT LAYER                                      │
├─────────────────────────────────────────────────────────────────────────┤
│  HWP 5.0  │  DOCX  │  PDF  │  TXT  │  RTF  │  HTML  │  ODT  │  HWPX   │
└─────┬─────┴────┬───┴───┬───┴───┬───┴───┬───┴────┬───┴───┬───┴────┬────┘
      │          │       │       │       │        │       │        │
      ▼          ▼       ▼       ▼       ▼        ▼       ▼        ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                       PARSER LAYER (Rust)                                │
├─────────────────────────────────────────────────────────────────────────┤
│  HwpParser │ DocxParser │ PdfParser │ TxtParser │ HtmlParser │ ...     │
│  (hwpers)  │ (zip+xml)  │ (pdf-ext) │ (native)  │ (scraper)  │         │
└─────────────────────────────┬───────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                    INTERMEDIATE REPRESENTATION (IR)                      │
├─────────────────────────────────────────────────────────────────────────┤
│  Document {                                                              │
│    metadata: Metadata,                                                   │
│    content: Vec<Block>,      // 문단, 표, 이미지, 리스트 등              │
│    resources: Vec<Resource>, // 이미지, 임베디드 파일                    │
│    styles: StyleMap,         // 스타일 정보                              │
│  }                                                                       │
└─────────────────────────────┬───────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                      TRANSFORM LAYER                                     │
├─────────────────────────────────────────────────────────────────────────┤
│  MarkdownEmitter  │  MdmGenerator  │  ImageOptimizer  │  SvgRenderer   │
└─────────────────────────────┬───────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                        OUTPUT LAYER                                      │
├─────────────────────────────────────────────────────────────────────────┤
│  output/                                                                 │
│  ├── document.md        # Markdown 본문                                  │
│  ├── document.mdm       # MDM 메타데이터 (JSON)                          │
│  └── assets/            # 추출된 미디어                                  │
│      ├── image_001.png                                                   │
│      ├── table_001.svg                                                   │
│      └── ...                                                             │
└─────────────────────────────────────────────────────────────────────────┘
```

### 1.2 핵심 설계 원칙

1. **단일 책임**: 각 파서는 하나의 포맷만 담당
2. **공통 IR**: 모든 포맷은 동일한 중간 표현으로 변환
3. **플러그인 구조**: 새 포맷 추가가 용이
4. **스트리밍 지원**: 대용량 파일 처리 가능
5. **오류 복구**: 부분 실패 시에도 가능한 결과 출력

---

## 2. Intermediate Representation (IR)

### 2.1 핵심 데이터 구조

```rust
/// 문서 전체를 표현하는 최상위 구조
#[derive(Debug, Clone)]
pub struct Document {
    /// 문서 메타데이터
    pub metadata: Metadata,

    /// 본문 블록들
    pub blocks: Vec<Block>,

    /// 리소스 (이미지, 파일 등)
    pub resources: Vec<Resource>,

    /// 스타일 맵
    pub styles: HashMap<String, Style>,

    /// 원본 포맷 정보
    pub source_format: DocumentFormat,
}

/// 문서 메타데이터
#[derive(Debug, Clone, Default)]
pub struct Metadata {
    pub title: Option<String>,
    pub author: Option<String>,
    pub subject: Option<String>,
    pub keywords: Vec<String>,
    pub created_at: Option<DateTime<Utc>>,
    pub modified_at: Option<DateTime<Utc>>,
    pub generator: Option<String>,
    pub page_count: Option<u32>,
    pub word_count: Option<u32>,
}

/// 블록 레벨 요소
#[derive(Debug, Clone)]
pub enum Block {
    /// 문단
    Paragraph(Paragraph),

    /// 제목 (레벨 1-6)
    Heading {
        level: u8,
        content: Vec<Inline>,
        id: Option<String>,
    },

    /// 코드 블록
    CodeBlock {
        language: Option<String>,
        content: String,
    },

    /// 인용
    BlockQuote(Vec<Block>),

    /// 순서 없는 목록
    UnorderedList(Vec<ListItem>),

    /// 순서 있는 목록
    OrderedList {
        start: u32,
        items: Vec<ListItem>,
    },

    /// 표
    Table(Table),

    /// 이미지 (블록 레벨)
    Image(ImageRef),

    /// 수평선
    HorizontalRule,

    /// 페이지 나누기
    PageBreak,

    /// 각주/미주
    Footnote {
        id: String,
        content: Vec<Block>,
    },

    /// 원시 HTML (변환 불가 요소)
    RawHtml(String),
}

/// 인라인 요소
#[derive(Debug, Clone)]
pub enum Inline {
    /// 일반 텍스트
    Text(String),

    /// 강조
    Emphasis(Vec<Inline>),

    /// 굵게
    Strong(Vec<Inline>),

    /// 취소선
    Strikethrough(Vec<Inline>),

    /// 밑줄
    Underline(Vec<Inline>),

    /// 인라인 코드
    Code(String),

    /// 링크
    Link {
        url: String,
        title: Option<String>,
        content: Vec<Inline>,
    },

    /// 인라인 이미지
    Image(ImageRef),

    /// 줄 바꿈
    LineBreak,

    /// 각주 참조
    FootnoteRef(String),

    /// 위/아래 첨자
    Superscript(Vec<Inline>),
    Subscript(Vec<Inline>),
}

/// 문단
#[derive(Debug, Clone)]
pub struct Paragraph {
    pub content: Vec<Inline>,
    pub alignment: Option<Alignment>,
    pub indent: Option<u32>,
}

/// 목록 아이템
#[derive(Debug, Clone)]
pub struct ListItem {
    pub content: Vec<Block>,
    pub checked: Option<bool>, // 체크박스 (Task list)
}

/// 표
#[derive(Debug, Clone)]
pub struct Table {
    pub headers: Vec<TableCell>,
    pub rows: Vec<Vec<TableCell>>,
    pub column_alignments: Vec<Alignment>,
    pub has_complex_structure: bool, // 병합셀 등
}

#[derive(Debug, Clone)]
pub struct TableCell {
    pub content: Vec<Block>,
    pub colspan: u32,
    pub rowspan: u32,
}

/// 이미지 참조
#[derive(Debug, Clone)]
pub struct ImageRef {
    pub resource_id: String,
    pub alt_text: Option<String>,
    pub title: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
}

/// 리소스 (미디어 파일)
#[derive(Debug, Clone)]
pub struct Resource {
    pub id: String,
    pub original_name: Option<String>,
    pub content_type: String,
    pub data: ResourceData,
}

#[derive(Debug, Clone)]
pub enum ResourceData {
    /// 메모리 내 데이터
    Inline(Vec<u8>),
    /// 파일 경로 참조
    File(PathBuf),
    /// 외부 URL
    Url(String),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Alignment {
    Left,
    Center,
    Right,
    Justify,
}
```

---

## 3. Parser Layer

### 3.1 Parser Trait

```rust
use std::path::Path;

/// 모든 문서 파서가 구현해야 하는 trait
pub trait DocumentParser: Send + Sync {
    /// 지원하는 포맷
    fn supported_formats(&self) -> &[DocumentFormat];

    /// 파일에서 파싱
    fn parse_file(&self, path: &Path) -> Result<Document, ParseError>;

    /// 바이트에서 파싱
    fn parse_bytes(&self, data: &[u8]) -> Result<Document, ParseError>;

    /// 스트리밍 파싱 (대용량 파일용)
    fn parse_streaming<R: Read>(&self, reader: R) -> Result<Document, ParseError> {
        // 기본 구현: 전체 읽기 후 파싱
        let mut data = Vec::new();
        reader.read_to_end(&mut data)?;
        self.parse_bytes(&data)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Invalid format: {0}")]
    InvalidFormat(String),

    #[error("Unsupported feature: {0}")]
    UnsupportedFeature(String),

    #[error("Corrupted file: {0}")]
    Corrupted(String),

    #[error("Password protected")]
    PasswordProtected,
}
```

### 3.2 포맷별 파서 구현

#### HWP Parser

```rust
pub struct HwpParser;

impl DocumentParser for HwpParser {
    fn supported_formats(&self) -> &[DocumentFormat] {
        &[DocumentFormat::Hwp]
    }

    fn parse_file(&self, path: &Path) -> Result<Document, ParseError> {
        let hwp_doc = hwpers::HwpDocument::from_path(path)
            .map_err(|e| ParseError::InvalidFormat(e.to_string()))?;

        let mut blocks = Vec::new();
        let mut resources = Vec::new();

        // 문단 변환
        for para in hwp_doc.paragraphs() {
            blocks.push(self.convert_paragraph(&para)?);
        }

        // 테이블 변환
        for table in hwp_doc.tables() {
            blocks.push(self.convert_table(&table)?);
        }

        // 이미지 추출
        for (idx, bin) in hwp_doc.bin_data().iter().enumerate() {
            resources.push(Resource {
                id: format!("image_{:03}", idx + 1),
                original_name: bin.name.clone(),
                content_type: detect_mime(&bin.data),
                data: ResourceData::Inline(bin.data.clone()),
            });
        }

        Ok(Document {
            metadata: self.extract_metadata(&hwp_doc),
            blocks,
            resources,
            styles: HashMap::new(),
            source_format: DocumentFormat::Hwp,
        })
    }

    fn parse_bytes(&self, data: &[u8]) -> Result<Document, ParseError> {
        // hwpers는 파일 경로 기반이므로 임시 파일 사용
        let temp = tempfile::NamedTempFile::new()?;
        std::fs::write(temp.path(), data)?;
        self.parse_file(temp.path())
    }
}

impl HwpParser {
    fn convert_paragraph(&self, para: &hwpers::Paragraph) -> Result<Block, ParseError> {
        let mut inlines = Vec::new();

        // 텍스트와 서식 변환
        for run in para.runs() {
            let text = run.text();
            let inline = if run.is_bold() && run.is_italic() {
                Inline::Strong(vec![Inline::Emphasis(vec![Inline::Text(text)])])
            } else if run.is_bold() {
                Inline::Strong(vec![Inline::Text(text)])
            } else if run.is_italic() {
                Inline::Emphasis(vec![Inline::Text(text)])
            } else {
                Inline::Text(text)
            };
            inlines.push(inline);
        }

        Ok(Block::Paragraph(Paragraph {
            content: inlines,
            alignment: None,
            indent: None,
        }))
    }

    fn convert_table(&self, table: &hwpers::Table) -> Result<Block, ParseError> {
        let rows: Vec<Vec<TableCell>> = table.rows()
            .map(|row| {
                row.cells()
                    .map(|cell| TableCell {
                        content: vec![Block::Paragraph(Paragraph {
                            content: vec![Inline::Text(cell.text())],
                            alignment: None,
                            indent: None,
                        })],
                        colspan: 1,
                        rowspan: 1,
                    })
                    .collect()
            })
            .collect();

        let has_header = !rows.is_empty();
        let (headers, body_rows) = if has_header {
            (rows[0].clone(), rows[1..].to_vec())
        } else {
            (Vec::new(), rows)
        };

        Ok(Block::Table(Table {
            headers,
            rows: body_rows,
            column_alignments: Vec::new(),
            has_complex_structure: false,
        }))
    }

    fn extract_metadata(&self, _doc: &hwpers::HwpDocument) -> Metadata {
        Metadata::default() // TODO: 실제 메타데이터 추출
    }
}
```

#### DOCX Parser

```rust
pub struct DocxParser;

impl DocumentParser for DocxParser {
    fn supported_formats(&self) -> &[DocumentFormat] {
        &[DocumentFormat::Docx]
    }

    fn parse_file(&self, path: &Path) -> Result<Document, ParseError> {
        let file = std::fs::File::open(path)?;
        let mut archive = ZipArchive::new(file)
            .map_err(|e| ParseError::InvalidFormat(e.to_string()))?;

        // document.xml 파싱
        let mut doc_xml = String::new();
        archive.by_name("word/document.xml")?
            .read_to_string(&mut doc_xml)?;

        let blocks = self.parse_document_xml(&doc_xml)?;

        // 이미지 추출
        let resources = self.extract_media(&mut archive)?;

        // 메타데이터
        let metadata = self.parse_core_properties(&mut archive)?;

        Ok(Document {
            metadata,
            blocks,
            resources,
            styles: HashMap::new(),
            source_format: DocumentFormat::Docx,
        })
    }

    fn parse_bytes(&self, data: &[u8]) -> Result<Document, ParseError> {
        let cursor = std::io::Cursor::new(data);
        let mut archive = ZipArchive::new(cursor)
            .map_err(|e| ParseError::InvalidFormat(e.to_string()))?;

        // ... (parse_file과 동일한 로직)
        todo!()
    }
}
```

#### TXT Parser

```rust
pub struct TxtParser {
    /// 빈 줄을 문단 구분으로 처리할지 여부
    pub paragraph_on_blank_line: bool,
}

impl Default for TxtParser {
    fn default() -> Self {
        Self {
            paragraph_on_blank_line: true,
        }
    }
}

impl DocumentParser for TxtParser {
    fn supported_formats(&self) -> &[DocumentFormat] {
        &[DocumentFormat::Txt]
    }

    fn parse_file(&self, path: &Path) -> Result<Document, ParseError> {
        let content = std::fs::read_to_string(path)?;
        self.parse_text(&content, path.file_name().and_then(|n| n.to_str()))
    }

    fn parse_bytes(&self, data: &[u8]) -> Result<Document, ParseError> {
        // 인코딩 자동 감지
        let (content, _, had_errors) = encoding_rs::UTF_8.decode(data);
        if had_errors {
            // UTF-8 실패 시 EUC-KR 시도 (한국어 문서 대응)
            let (content, _, _) = encoding_rs::EUC_KR.decode(data);
            self.parse_text(&content, None)
        } else {
            self.parse_text(&content, None)
        }
    }
}

impl TxtParser {
    fn parse_text(&self, content: &str, filename: Option<&str>) -> Result<Document, ParseError> {
        let mut blocks = Vec::new();

        if self.paragraph_on_blank_line {
            // 빈 줄로 문단 분리
            for para_text in content.split("\n\n") {
                let trimmed = para_text.trim();
                if !trimmed.is_empty() {
                    blocks.push(Block::Paragraph(Paragraph {
                        content: vec![Inline::Text(trimmed.to_string())],
                        alignment: None,
                        indent: None,
                    }));
                }
            }
        } else {
            // 각 줄을 문단으로
            for line in content.lines() {
                let trimmed = line.trim();
                if !trimmed.is_empty() {
                    blocks.push(Block::Paragraph(Paragraph {
                        content: vec![Inline::Text(trimmed.to_string())],
                        alignment: None,
                        indent: None,
                    }));
                }
            }
        }

        Ok(Document {
            metadata: Metadata {
                title: filename.map(|s| s.to_string()),
                ..Default::default()
            },
            blocks,
            resources: Vec::new(),
            styles: HashMap::new(),
            source_format: DocumentFormat::Txt,
        })
    }
}
```

#### PDF Parser

```rust
pub struct PdfParser;

impl DocumentParser for PdfParser {
    fn supported_formats(&self) -> &[DocumentFormat] {
        &[DocumentFormat::Pdf]
    }

    fn parse_file(&self, path: &Path) -> Result<Document, ParseError> {
        let text = pdf_extract::extract_text(path)
            .map_err(|e| ParseError::InvalidFormat(e.to_string()))?;

        let blocks = self.text_to_blocks(&text);

        Ok(Document {
            metadata: Metadata::default(), // TODO: PDF 메타데이터 추출
            blocks,
            resources: Vec::new(), // TODO: PDF 이미지 추출
            styles: HashMap::new(),
            source_format: DocumentFormat::Pdf,
        })
    }

    fn parse_bytes(&self, data: &[u8]) -> Result<Document, ParseError> {
        let temp = tempfile::NamedTempFile::new()?;
        std::fs::write(temp.path(), data)?;
        self.parse_file(temp.path())
    }
}

impl PdfParser {
    fn text_to_blocks(&self, text: &str) -> Vec<Block> {
        let mut blocks = Vec::new();

        // 페이지 구분 처리 (PDF 추출 시 일반적으로 \f 사용)
        for page in text.split('\x0C') {
            for para in page.split("\n\n") {
                let trimmed = para.trim();
                if !trimmed.is_empty() {
                    blocks.push(Block::Paragraph(Paragraph {
                        content: vec![Inline::Text(trimmed.to_string())],
                        alignment: None,
                        indent: None,
                    }));
                }
            }
            // 페이지 나누기 표시
            blocks.push(Block::PageBreak);
        }

        blocks
    }
}
```

---

## 4. Transform Layer

### 4.1 Markdown Emitter

```rust
pub struct MarkdownEmitter {
    /// 표를 SVG로 변환할지 여부
    pub render_complex_tables_as_svg: bool,
    /// 이미지 경로 prefix
    pub asset_path_prefix: String,
}

impl Default for MarkdownEmitter {
    fn default() -> Self {
        Self {
            render_complex_tables_as_svg: true,
            asset_path_prefix: "assets/".to_string(),
        }
    }
}

impl MarkdownEmitter {
    pub fn emit(&self, doc: &Document) -> String {
        let mut output = String::new();

        for block in &doc.blocks {
            output.push_str(&self.emit_block(block));
            output.push_str("\n\n");
        }

        output
    }

    fn emit_block(&self, block: &Block) -> String {
        match block {
            Block::Paragraph(para) => self.emit_paragraph(para),

            Block::Heading { level, content, id } => {
                let hashes = "#".repeat(*level as usize);
                let text = self.emit_inlines(content);
                if let Some(id) = id {
                    format!("{} {} {{#{}}}", hashes, text, id)
                } else {
                    format!("{} {}", hashes, text)
                }
            }

            Block::CodeBlock { language, content } => {
                let lang = language.as_deref().unwrap_or("");
                format!("```{}\n{}\n```", lang, content)
            }

            Block::BlockQuote(blocks) => {
                blocks
                    .iter()
                    .map(|b| self.emit_block(b))
                    .map(|s| format!("> {}", s.replace('\n', "\n> ")))
                    .collect::<Vec<_>>()
                    .join("\n>\n")
            }

            Block::UnorderedList(items) => {
                items
                    .iter()
                    .map(|item| self.emit_list_item(item, "-"))
                    .collect::<Vec<_>>()
                    .join("\n")
            }

            Block::OrderedList { start, items } => {
                items
                    .iter()
                    .enumerate()
                    .map(|(i, item)| {
                        self.emit_list_item(item, &format!("{}.", start + i as u32))
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            }

            Block::Table(table) => self.emit_table(table),

            Block::Image(img) => self.emit_image(img),

            Block::HorizontalRule => "---".to_string(),

            Block::PageBreak => "<!-- page break -->".to_string(),

            Block::Footnote { id, content } => {
                let text = content
                    .iter()
                    .map(|b| self.emit_block(b))
                    .collect::<Vec<_>>()
                    .join("\n");
                format!("[^{}]: {}", id, text)
            }

            Block::RawHtml(html) => html.clone(),
        }
    }

    fn emit_paragraph(&self, para: &Paragraph) -> String {
        self.emit_inlines(&para.content)
    }

    fn emit_inlines(&self, inlines: &[Inline]) -> String {
        inlines
            .iter()
            .map(|i| self.emit_inline(i))
            .collect::<Vec<_>>()
            .join("")
    }

    fn emit_inline(&self, inline: &Inline) -> String {
        match inline {
            Inline::Text(s) => s.clone(),
            Inline::Emphasis(inner) => format!("*{}*", self.emit_inlines(inner)),
            Inline::Strong(inner) => format!("**{}**", self.emit_inlines(inner)),
            Inline::Strikethrough(inner) => format!("~~{}~~", self.emit_inlines(inner)),
            Inline::Underline(inner) => format!("<u>{}</u>", self.emit_inlines(inner)),
            Inline::Code(s) => format!("`{}`", s),
            Inline::Link { url, title, content } => {
                let text = self.emit_inlines(content);
                if let Some(title) = title {
                    format!("[{}]({} \"{}\")", text, url, title)
                } else {
                    format!("[{}]({})", text, url)
                }
            }
            Inline::Image(img) => self.emit_image(img),
            Inline::LineBreak => "  \n".to_string(),
            Inline::FootnoteRef(id) => format!("[^{}]", id),
            Inline::Superscript(inner) => format!("<sup>{}</sup>", self.emit_inlines(inner)),
            Inline::Subscript(inner) => format!("<sub>{}</sub>", self.emit_inlines(inner)),
        }
    }

    fn emit_list_item(&self, item: &ListItem, marker: &str) -> String {
        let content = item
            .content
            .iter()
            .map(|b| self.emit_block(b))
            .collect::<Vec<_>>()
            .join("\n  ");

        if let Some(checked) = item.checked {
            let checkbox = if checked { "[x]" } else { "[ ]" };
            format!("{} {} {}", marker, checkbox, content)
        } else {
            format!("{} {}", marker, content)
        }
    }

    fn emit_table(&self, table: &Table) -> String {
        // 복잡한 표 (병합 셀 등)는 SVG 참조로 변환
        if table.has_complex_structure && self.render_complex_tables_as_svg {
            return format!("![[table.svg | preset:table]]");
        }

        let mut md = String::new();

        // 헤더
        if !table.headers.is_empty() {
            md.push('|');
            for cell in &table.headers {
                let text = cell.content
                    .iter()
                    .map(|b| self.emit_block(b))
                    .collect::<Vec<_>>()
                    .join(" ");
                md.push_str(&format!(" {} |", text.trim()));
            }
            md.push('\n');

            // 구분선
            md.push('|');
            for (i, _) in table.headers.iter().enumerate() {
                let align = table.column_alignments.get(i);
                match align {
                    Some(Alignment::Left) => md.push_str(":---|"),
                    Some(Alignment::Center) => md.push_str(":---:|"),
                    Some(Alignment::Right) => md.push_str("---:|"),
                    _ => md.push_str("---|"),
                }
            }
            md.push('\n');
        }

        // 본문
        for row in &table.rows {
            md.push('|');
            for cell in row {
                let text = cell.content
                    .iter()
                    .map(|b| self.emit_block(b))
                    .collect::<Vec<_>>()
                    .join(" ");
                md.push_str(&format!(" {} |", text.trim()));
            }
            md.push('\n');
        }

        md
    }

    fn emit_image(&self, img: &ImageRef) -> String {
        let mut attrs = Vec::new();

        if let Some(w) = img.width {
            attrs.push(format!("width={}", w));
        }
        if let Some(h) = img.height {
            attrs.push(format!("height={}", h));
        }
        if let Some(alt) = &img.alt_text {
            attrs.push(format!("alt=\"{}\"", alt));
        }

        let path = format!("{}{}", self.asset_path_prefix, img.resource_id);

        if attrs.is_empty() {
            format!("![[{}]]", path)
        } else {
            format!("![[{} | {}]]", path, attrs.join(" "))
        }
    }
}
```

### 4.2 MDM Generator

```rust
use serde::{Serialize, Deserialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct MdmFile {
    /// 버전
    pub version: String,

    /// 메타데이터
    pub metadata: MdmMetadata,

    /// 리소스 정의
    pub resources: HashMap<String, MdmResource>,

    /// 프리셋 정의
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub presets: HashMap<String, MdmPreset>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MdmMetadata {
    pub source_file: Option<String>,
    pub source_format: String,
    pub converted_at: String,
    pub converter_version: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MdmResource {
    pub file: String,
    #[serde(rename = "type")]
    pub resource_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub height: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alt: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MdmPreset {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub height: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quality: Option<u8>,
}

pub struct MdmGenerator;

impl MdmGenerator {
    pub fn generate(doc: &Document, source_path: Option<&str>) -> MdmFile {
        let mut resources = HashMap::new();

        for res in &doc.resources {
            resources.insert(res.id.clone(), MdmResource {
                file: format!("assets/{}", res.id),
                resource_type: res.content_type.clone(),
                width: None,
                height: None,
                alt: res.original_name.clone(),
                title: None,
            });
        }

        MdmFile {
            version: "1.0".to_string(),
            metadata: MdmMetadata {
                source_file: source_path.map(|s| s.to_string()),
                source_format: format!("{:?}", doc.source_format),
                converted_at: chrono::Utc::now().to_rfc3339(),
                converter_version: env!("CARGO_PKG_VERSION").to_string(),
            },
            resources,
            presets: HashMap::new(),
        }
    }

    pub fn to_json(&self, mdm: &MdmFile) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(mdm)
    }
}
```

---

## 5. Output Layer

### 5.1 Bundle Writer

```rust
use std::path::Path;
use std::fs;

pub struct BundleWriter {
    pub output_dir: PathBuf,
    pub asset_dir: String,
    pub optimize_images: bool,
    pub image_quality: u8,
}

impl BundleWriter {
    pub fn new(output_dir: impl Into<PathBuf>) -> Self {
        Self {
            output_dir: output_dir.into(),
            asset_dir: "assets".to_string(),
            optimize_images: true,
            image_quality: 85,
        }
    }

    pub fn write(&self, doc: &Document, base_name: &str) -> Result<BundleOutput, std::io::Error> {
        // 디렉토리 생성
        fs::create_dir_all(&self.output_dir)?;
        let assets_path = self.output_dir.join(&self.asset_dir);
        fs::create_dir_all(&assets_path)?;

        // Markdown 생성
        let emitter = MarkdownEmitter {
            asset_path_prefix: format!("{}/", self.asset_dir),
            ..Default::default()
        };
        let markdown = emitter.emit(doc);
        let md_path = self.output_dir.join(format!("{}.md", base_name));
        fs::write(&md_path, &markdown)?;

        // MDM 생성
        let mdm = MdmGenerator::generate(doc, None);
        let mdm_json = serde_json::to_string_pretty(&mdm)?;
        let mdm_path = self.output_dir.join(format!("{}.mdm", base_name));
        fs::write(&mdm_path, &mdm_json)?;

        // 리소스 저장
        let mut asset_files = Vec::new();
        for resource in &doc.resources {
            let asset_path = assets_path.join(&resource.id);
            match &resource.data {
                ResourceData::Inline(data) => {
                    let data = if self.optimize_images {
                        self.optimize_image(data, &resource.content_type)?
                    } else {
                        data.clone()
                    };
                    fs::write(&asset_path, &data)?;
                }
                ResourceData::File(source) => {
                    fs::copy(source, &asset_path)?;
                }
                ResourceData::Url(_) => {
                    // URL은 그대로 참조 (다운로드 안 함)
                }
            }
            asset_files.push(asset_path);
        }

        Ok(BundleOutput {
            markdown_file: md_path,
            mdm_file: mdm_path,
            asset_files,
        })
    }

    fn optimize_image(&self, data: &[u8], _content_type: &str) -> Result<Vec<u8>, std::io::Error> {
        let optimizer = ImageOptimizer {
            quality: self.image_quality,
            ..Default::default()
        };

        optimizer.optimize(data, image::ImageFormat::Png)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))
    }
}

#[derive(Debug)]
pub struct BundleOutput {
    pub markdown_file: PathBuf,
    pub mdm_file: PathBuf,
    pub asset_files: Vec<PathBuf>,
}
```

---

## 6. 통합 API

### 6.1 Converter Facade

```rust
use std::path::Path;

pub struct Converter {
    parsers: HashMap<DocumentFormat, Box<dyn DocumentParser>>,
    writer: BundleWriter,
}

impl Converter {
    pub fn new(output_dir: impl Into<PathBuf>) -> Self {
        let mut parsers: HashMap<DocumentFormat, Box<dyn DocumentParser>> = HashMap::new();

        // 파서 등록
        parsers.insert(DocumentFormat::Hwp, Box::new(HwpParser));
        parsers.insert(DocumentFormat::Docx, Box::new(DocxParser));
        parsers.insert(DocumentFormat::Pdf, Box::new(PdfParser));
        parsers.insert(DocumentFormat::Txt, Box::new(TxtParser::default()));

        Self {
            parsers,
            writer: BundleWriter::new(output_dir),
        }
    }

    pub fn convert(&self, input: &Path) -> Result<BundleOutput, ConvertError> {
        let format = DocumentFormat::from_path(input);

        let parser = self.parsers.get(&format)
            .ok_or_else(|| ConvertError::UnsupportedFormat(format!("{:?}", format)))?;

        let doc = parser.parse_file(input)?;

        let base_name = input
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("document");

        let output = self.writer.write(&doc, base_name)?;

        Ok(output)
    }

    pub fn convert_batch(&self, inputs: &[PathBuf]) -> Vec<Result<BundleOutput, ConvertError>> {
        inputs
            .iter()
            .map(|path| self.convert(path))
            .collect()
    }
}
```

### 6.2 CLI 사용 예시

```bash
# 단일 파일 변환
mdm convert -i document.hwp -o output/

# 여러 파일 변환
mdm convert -i *.docx -o output/

# 텍스트만 추출
mdm extract-text -i document.pdf

# 이미지만 추출
mdm extract-images -i document.docx -o images/
```

---

## 7. 확장 가이드

### 7.1 새 포맷 추가

```rust
// 1. 파서 구현
pub struct RtfParser;

impl DocumentParser for RtfParser {
    fn supported_formats(&self) -> &[DocumentFormat] {
        &[DocumentFormat::Rtf]
    }

    fn parse_file(&self, path: &Path) -> Result<Document, ParseError> {
        // RTF 파싱 로직
        todo!()
    }

    fn parse_bytes(&self, data: &[u8]) -> Result<Document, ParseError> {
        todo!()
    }
}

// 2. Converter에 등록
converter.register_parser(DocumentFormat::Rtf, Box::new(RtfParser));
```

### 7.2 새 출력 포맷 추가

```rust
// HTML Emitter 예시
pub struct HtmlEmitter;

impl HtmlEmitter {
    pub fn emit(&self, doc: &Document) -> String {
        let mut html = String::from("<!DOCTYPE html><html><body>");

        for block in &doc.blocks {
            html.push_str(&self.emit_block(block));
        }

        html.push_str("</body></html>");
        html
    }

    fn emit_block(&self, block: &Block) -> String {
        match block {
            Block::Paragraph(para) => format!("<p>{}</p>", self.emit_inlines(&para.content)),
            Block::Heading { level, content, .. } => {
                format!("<h{}>{}</h{}>", level, self.emit_inlines(content), level)
            }
            // ...
            _ => String::new(),
        }
    }
}
```

---

## 8. 참고 자료

- [HWP_FORMAT_SPEC.md](./HWP_FORMAT_SPEC.md) - HWP 5.0 상세 명세
- [DOCX_FORMAT_SPEC.md](./DOCX_FORMAT_SPEC.md) - DOCX/OOXML 상세 명세
- [RUST_DEV_GUIDE.md](./RUST_DEV_GUIDE.md) - Rust 개발 가이드

---

## 변경 이력

| 날짜 | 버전 | 변경 내용 |
|------|------|----------|
| 2025-12-25 | 1.0 | 초기 문서 작성 |
