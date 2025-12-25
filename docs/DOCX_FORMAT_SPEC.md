# DOCX (Office Open XML) File Format Specification

> Rust 개발을 위한 DOCX/OOXML 파일 포맷 기술 명세 문서

## 1. 개요

### 1.1 DOCX란?

**DOCX (Office Open XML WordprocessingML)**: Microsoft Office 2007+의 기본 문서 포맷

- **표준**: ISO/IEC 29500, ECMA-376
- **구조**: ZIP 압축된 XML 파일 모음
- **확장자**: `.docx` (문서), `.dotx` (템플릿), `.docm` (매크로 포함)

### 1.2 포맷 변형

| 변형 | 설명 | 호환성 |
|------|------|--------|
| **Transitional** | MS Office 기본 포맷, 레거시 호환 | Office 2007+ |
| **Strict** | ISO 29500 완전 준수 | Office 2013+ |

> 대부분의 DOCX는 Transitional 포맷

### 1.3 공식 명세서

- [MS-DOCX Structure Overview](https://learn.microsoft.com/en-us/openspecs/office_standards/ms-docx/728a7abc-7f55-40dc-90a7-1276ff53c8b2)
- [Office Open XML Anatomy](http://officeopenxml.com/anatomyofOOXML.php)
- [ECMA-376 Specification](https://ecma-international.org/publications-and-standards/standards/ecma-376/)
- [Library of Congress Format Description](https://www.loc.gov/preservation/digital/formats/fdd/fdd000397.shtml)

---

## 2. 파일 구조

### 2.1 ZIP 아카이브 구조

```
document.docx (ZIP archive)
├── [Content_Types].xml          # 콘텐츠 타입 정의 (필수)
├── _rels/
│   └── .rels                    # 패키지 관계 정의
├── word/
│   ├── document.xml             # 메인 문서 본문 (필수)
│   ├── styles.xml               # 스타일 정의
│   ├── settings.xml             # 문서 설정
│   ├── fontTable.xml            # 폰트 테이블
│   ├── numbering.xml            # 번호 매기기 정의
│   ├── footnotes.xml            # 각주
│   ├── endnotes.xml             # 미주
│   ├── header1.xml              # 머리글
│   ├── footer1.xml              # 바닥글
│   ├── comments.xml             # 주석
│   ├── _rels/
│   │   └── document.xml.rels    # 문서 관계
│   └── media/                   # 미디어 파일
│       ├── image1.png
│       └── image2.jpeg
├── docProps/
│   ├── core.xml                 # 핵심 메타데이터 (제목, 작성자 등)
│   └── app.xml                  # 애플리케이션 메타데이터
└── customXml/                   # 사용자 정의 XML (선택)
```

### 2.2 Content Types ([Content_Types].xml)

```xml
<?xml version="1.0" encoding="UTF-8"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels"
           ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml"
           ContentType="application/xml"/>
  <Default Extension="png"
           ContentType="image/png"/>
  <Default Extension="jpeg"
           ContentType="image/jpeg"/>
  <Override PartName="/word/document.xml"
            ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>
  <Override PartName="/word/styles.xml"
            ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml"/>
</Types>
```

### 2.3 Relationships (_rels/.rels)

```xml
<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1"
    Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument"
    Target="word/document.xml"/>
  <Relationship Id="rId2"
    Type="http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties"
    Target="docProps/core.xml"/>
</Relationships>
```

---

## 3. WordprocessingML 구조

### 3.1 네임스페이스

```xml
<!-- 주요 네임스페이스 -->
xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"
xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"
xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing"
xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"
xmlns:pic="http://schemas.openxmlformats.org/drawingml/2006/picture"
```

### 3.2 문서 본문 (document.xml)

```xml
<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    <!-- 문단 -->
    <w:p>
      <w:pPr>
        <w:pStyle w:val="Heading1"/>
      </w:pPr>
      <w:r>
        <w:t>제목 텍스트</w:t>
      </w:r>
    </w:p>

    <!-- 일반 텍스트 -->
    <w:p>
      <w:r>
        <w:rPr>
          <w:b/>  <!-- 굵게 -->
        </w:rPr>
        <w:t>굵은 텍스트</w:t>
      </w:r>
    </w:p>

    <!-- 섹션 속성 (마지막) -->
    <w:sectPr>
      <w:pgSz w:w="12240" w:h="15840"/>  <!-- 페이지 크기 -->
      <w:pgMar w:top="1440" w:right="1440" w:bottom="1440" w:left="1440"/>
    </w:sectPr>
  </w:body>
</w:document>
```

### 3.3 주요 요소

| 요소 | 설명 | Markdown 변환 |
|------|------|---------------|
| `<w:p>` | 문단 (Paragraph) | 줄바꿈 |
| `<w:r>` | 런 (Run) - 동일 서식 텍스트 그룹 | - |
| `<w:t>` | 텍스트 내용 | 텍스트 |
| `<w:br/>` | 줄 바꿈 | `\n` |
| `<w:tab/>` | 탭 | `\t` |
| `<w:tbl>` | 표 | `\|...\|` |
| `<w:hyperlink>` | 하이퍼링크 | `[text](url)` |
| `<w:drawing>` | 이미지/도형 | `![[]]` |

### 3.4 텍스트 서식 (Run Properties)

```xml
<w:rPr>
  <w:b/>           <!-- Bold: **text** -->
  <w:i/>           <!-- Italic: *text* -->
  <w:u/>           <!-- Underline: <u>text</u> -->
  <w:strike/>      <!-- Strikethrough: ~~text~~ -->
  <w:vertAlign w:val="superscript"/>  <!-- 위 첨자 -->
  <w:vertAlign w:val="subscript"/>    <!-- 아래 첨자 -->
  <w:highlight w:val="yellow"/>       <!-- 형광펜 -->
  <w:sz w:val="24"/>                  <!-- 폰트 크기 (half-points) -->
  <w:rFonts w:ascii="Arial"/>         <!-- 폰트 -->
</w:rPr>
```

### 3.5 문단 서식 (Paragraph Properties)

```xml
<w:pPr>
  <w:pStyle w:val="Heading1"/>        <!-- 스타일: # -->
  <w:jc w:val="center"/>              <!-- 정렬 -->
  <w:numPr>                           <!-- 번호/목록 -->
    <w:ilvl w:val="0"/>
    <w:numId w:val="1"/>
  </w:numPr>
  <w:ind w:left="720"/>               <!-- 들여쓰기 -->
</w:pPr>
```

---

## 4. 표 구조

```xml
<w:tbl>
  <w:tblPr>
    <w:tblW w:w="5000" w:type="pct"/>  <!-- 테이블 너비 -->
  </w:tblPr>
  <w:tblGrid>
    <w:gridCol w:w="2500"/>
    <w:gridCol w:w="2500"/>
  </w:tblGrid>
  <w:tr>  <!-- 행 -->
    <w:tc>  <!-- 셀 -->
      <w:tcPr>
        <w:gridSpan w:val="2"/>  <!-- 셀 병합 -->
      </w:tcPr>
      <w:p>
        <w:r><w:t>셀 내용</w:t></w:r>
      </w:p>
    </w:tc>
  </w:tr>
</w:tbl>
```

---

## 5. 이미지/미디어

### 5.1 이미지 참조 구조

```xml
<w:drawing>
  <wp:inline>
    <wp:extent cx="914400" cy="914400"/>  <!-- 크기 (EMU) -->
    <wp:docPr id="1" name="Picture 1"/>
    <a:graphic>
      <a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/picture">
        <pic:pic>
          <pic:blipFill>
            <a:blip r:embed="rId5"/>  <!-- 관계 ID로 미디어 참조 -->
          </pic:blipFill>
        </pic:pic>
      </a:graphicData>
    </a:graphic>
  </wp:inline>
</w:drawing>
```

### 5.2 미디어 관계 (word/_rels/document.xml.rels)

```xml
<Relationship Id="rId5"
  Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image"
  Target="media/image1.png"/>
```

### 5.3 지원 미디어 포맷

| 포맷 | Content Type | 비고 |
|------|--------------|------|
| PNG | image/png | 권장 |
| JPEG | image/jpeg | 권장 |
| GIF | image/gif | 지원 |
| BMP | image/bmp | 레거시 |
| TIFF | image/tiff | 지원 |
| WMF | image/x-wmf | Windows Metafile |
| EMF | image/x-emf | Enhanced Metafile |

---

## 6. Rust 구현

### 6.1 권장 크레이트

```toml
[dependencies]
# ZIP 처리
zip = "2.2"

# XML 파싱
quick-xml = "0.37"
# 또는
roxmltree = "0.20"   # 읽기 전용, 더 간단

# OOXML 전용 (있다면)
ooxml = "0.1"        # 제한적 지원

# 유틸리티
serde = { version = "1.0", features = ["derive"] }
```

### 6.2 기본 파싱 구조

```rust
use std::io::{Read, Seek};
use zip::ZipArchive;
use quick_xml::Reader;
use quick_xml::events::Event;

pub struct DocxDocument {
    pub paragraphs: Vec<Paragraph>,
    pub images: Vec<ImageRef>,
    pub tables: Vec<Table>,
}

pub struct Paragraph {
    pub text: String,
    pub style: Option<String>,
    pub runs: Vec<Run>,
}

pub struct Run {
    pub text: String,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strikethrough: bool,
}

impl DocxDocument {
    pub fn from_path(path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let file = std::fs::File::open(path)?;
        let mut archive = ZipArchive::new(file)?;

        // document.xml 읽기
        let mut doc_xml = String::new();
        archive.by_name("word/document.xml")?.read_to_string(&mut doc_xml)?;

        // XML 파싱
        Self::parse_document_xml(&doc_xml)
    }

    fn parse_document_xml(xml: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let mut reader = Reader::from_str(xml);
        reader.config_mut().trim_text(true);

        let mut paragraphs = Vec::new();
        let mut current_para: Option<Paragraph> = None;
        let mut current_run: Option<Run> = None;
        let mut in_text = false;

        loop {
            match reader.read_event()? {
                Event::Start(e) => {
                    match e.name().as_ref() {
                        b"w:p" => {
                            current_para = Some(Paragraph::default());
                        }
                        b"w:r" => {
                            current_run = Some(Run::default());
                        }
                        b"w:t" => {
                            in_text = true;
                        }
                        b"w:b" => {
                            if let Some(ref mut run) = current_run {
                                run.bold = true;
                            }
                        }
                        b"w:i" => {
                            if let Some(ref mut run) = current_run {
                                run.italic = true;
                            }
                        }
                        _ => {}
                    }
                }
                Event::Text(e) if in_text => {
                    if let Some(ref mut run) = current_run {
                        run.text.push_str(&e.unescape()?);
                    }
                }
                Event::End(e) => {
                    match e.name().as_ref() {
                        b"w:t" => in_text = false,
                        b"w:r" => {
                            if let (Some(ref mut para), Some(run)) =
                                (&mut current_para, current_run.take())
                            {
                                para.runs.push(run);
                            }
                        }
                        b"w:p" => {
                            if let Some(para) = current_para.take() {
                                paragraphs.push(para);
                            }
                        }
                        _ => {}
                    }
                }
                Event::Eof => break,
                _ => {}
            }
        }

        Ok(Self {
            paragraphs,
            images: Vec::new(),
            tables: Vec::new(),
        })
    }
}
```

### 6.3 이미지 추출

```rust
impl DocxDocument {
    pub fn extract_images<P: AsRef<std::path::Path>>(
        docx_path: P,
        output_dir: P,
    ) -> Result<Vec<String>, Box<dyn std::error::Error>> {
        let file = std::fs::File::open(docx_path)?;
        let mut archive = ZipArchive::new(file)?;
        let mut extracted = Vec::new();

        for i in 0..archive.len() {
            let mut file = archive.by_index(i)?;
            let name = file.name().to_string();

            if name.starts_with("word/media/") {
                let file_name = std::path::Path::new(&name)
                    .file_name()
                    .unwrap()
                    .to_str()
                    .unwrap();

                let output_path = output_dir.as_ref().join(file_name);
                let mut output = std::fs::File::create(&output_path)?;
                std::io::copy(&mut file, &mut output)?;

                extracted.push(file_name.to_string());
            }
        }

        Ok(extracted)
    }
}
```

---

## 7. MDM 변환 전략

### 7.1 스타일 매핑

```rust
fn style_to_markdown(style: &str) -> &str {
    match style {
        "Heading1" | "Title" => "# ",
        "Heading2" => "## ",
        "Heading3" => "### ",
        "Heading4" => "#### ",
        "Heading5" => "##### ",
        "Heading6" => "###### ",
        "Quote" | "IntenseQuote" => "> ",
        "ListParagraph" => "- ",
        _ => "",
    }
}
```

### 7.2 Run을 Markdown으로 변환

```rust
fn run_to_markdown(run: &Run) -> String {
    let mut text = run.text.clone();

    if run.bold && run.italic {
        text = format!("***{}***", text);
    } else if run.bold {
        text = format!("**{}**", text);
    } else if run.italic {
        text = format!("*{}*", text);
    }

    if run.strikethrough {
        text = format!("~~{}~~", text);
    }

    text
}
```

### 7.3 전체 변환 예시

```rust
fn docx_to_markdown(doc: &DocxDocument) -> String {
    let mut md = String::new();

    for para in &doc.paragraphs {
        // 스타일 적용
        if let Some(ref style) = para.style {
            md.push_str(style_to_markdown(style));
        }

        // 모든 run 변환
        for run in &para.runs {
            md.push_str(&run_to_markdown(run));
        }

        md.push_str("\n\n");
    }

    md
}
```

---

## 8. 참고 자료

### 공식 문서
- [MS-DOCX Specification](https://learn.microsoft.com/en-us/openspecs/office_standards/ms-docx/)
- [ECMA-376 Standard](https://ecma-international.org/publications-and-standards/standards/ecma-376/)
- [Office Open XML Explained](http://officeopenxml.com/)

### Rust 라이브러리
- [quick-xml](https://github.com/tafia/quick-xml) - 고성능 XML 파서
- [zip-rs](https://github.com/zip-rs/zip2) - ZIP 아카이브 처리
- [ooxml crate](https://lib.rs/crates/ooxml) - OOXML 파싱

### 유용한 도구
- [OOXML Viewer](http://officeopenxml.com/WPcontentOverview.php) - 구조 시각화
- [Open XML SDK](https://github.com/OfficeDev/Open-XML-SDK) - .NET 참조 구현

---

## 변경 이력

| 날짜 | 버전 | 변경 내용 |
|------|------|----------|
| 2025-12-25 | 1.0 | 초기 문서 작성 |
