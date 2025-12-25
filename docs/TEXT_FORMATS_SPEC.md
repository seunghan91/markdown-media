# Text Format Specifications

> TXT, RTF, CSV 등 텍스트 기반 포맷의 변환 명세

## 1. Plain Text (TXT)

### 1.1 개요

가장 단순한 문서 포맷. 서식 정보 없이 순수 텍스트만 포함.

| 속성 | 값 |
|------|-----|
| 확장자 | `.txt`, `.text` |
| MIME | `text/plain` |
| 인코딩 | UTF-8 (기본), EUC-KR, CP949 (레거시) |

### 1.2 인코딩 감지

```rust
use encoding_rs::{UTF_8, EUC_KR, WINDOWS_949};

pub fn detect_and_decode(data: &[u8]) -> (String, &'static str) {
    // 1. BOM 확인
    if data.starts_with(&[0xEF, 0xBB, 0xBF]) {
        let (text, _, _) = UTF_8.decode(&data[3..]);
        return (text.into_owned(), "UTF-8 (BOM)");
    }
    if data.starts_with(&[0xFF, 0xFE]) {
        let (text, _, _) = encoding_rs::UTF_16LE.decode(&data[2..]);
        return (text.into_owned(), "UTF-16LE");
    }
    if data.starts_with(&[0xFE, 0xFF]) {
        let (text, _, _) = encoding_rs::UTF_16BE.decode(&data[2..]);
        return (text.into_owned(), "UTF-16BE");
    }

    // 2. UTF-8 시도
    let (text, _, had_errors) = UTF_8.decode(data);
    if !had_errors {
        return (text.into_owned(), "UTF-8");
    }

    // 3. EUC-KR / CP949 (한국어)
    let (text, _, _) = EUC_KR.decode(data);
    (text.into_owned(), "EUC-KR")
}
```

### 1.3 문단 분리 전략

```rust
pub enum ParagraphStrategy {
    /// 빈 줄로 문단 구분 (기본)
    BlankLine,
    /// 각 줄이 문단
    EveryLine,
    /// 들여쓰기로 문단 구분
    Indentation,
    /// 문장 끝(. ! ?)으로 문단 구분
    Sentence,
}

pub fn split_paragraphs(text: &str, strategy: ParagraphStrategy) -> Vec<String> {
    match strategy {
        ParagraphStrategy::BlankLine => {
            text.split("\n\n")
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect()
        }
        ParagraphStrategy::EveryLine => {
            text.lines()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect()
        }
        ParagraphStrategy::Indentation => {
            let mut paragraphs = Vec::new();
            let mut current = String::new();

            for line in text.lines() {
                if line.starts_with(' ') || line.starts_with('\t') {
                    // 들여쓰기 = 새 문단
                    if !current.is_empty() {
                        paragraphs.push(current.trim().to_string());
                        current.clear();
                    }
                }
                current.push_str(line.trim());
                current.push(' ');
            }

            if !current.is_empty() {
                paragraphs.push(current.trim().to_string());
            }

            paragraphs
        }
        ParagraphStrategy::Sentence => {
            let mut paragraphs = Vec::new();
            let mut current = String::new();

            for char in text.chars() {
                current.push(char);
                if char == '.' || char == '!' || char == '?' {
                    paragraphs.push(current.trim().to_string());
                    current.clear();
                }
            }

            if !current.is_empty() {
                paragraphs.push(current.trim().to_string());
            }

            paragraphs
        }
    }
}
```

### 1.4 Markdown 변환

TXT → Markdown은 거의 1:1 변환이지만, 다음을 처리:

```rust
pub fn txt_to_markdown(text: &str) -> String {
    let paragraphs = split_paragraphs(text, ParagraphStrategy::BlankLine);

    paragraphs
        .iter()
        .map(|p| {
            // URL 자동 링크화
            let p = auto_link_urls(p);
            // Markdown 특수문자 이스케이프
            let p = escape_markdown_chars(&p);
            p
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn auto_link_urls(text: &str) -> String {
    let url_regex = regex::Regex::new(
        r"https?://[^\s<>\[\](){}]+"
    ).unwrap();

    url_regex.replace_all(text, |caps: &regex::Captures| {
        format!("<{}>", &caps[0])
    }).to_string()
}

fn escape_markdown_chars(text: &str) -> String {
    text.replace('\\', "\\\\")
        .replace('*', "\\*")
        .replace('_', "\\_")
        .replace('`', "\\`")
        .replace('#', "\\#")
        .replace('[', "\\[")
        .replace(']', "\\]")
}
```

---

## 2. Rich Text Format (RTF)

### 2.1 개요

Microsoft에서 개발한 크로스 플랫폼 서식 문서 포맷.

| 속성 | 값 |
|------|-----|
| 확장자 | `.rtf` |
| MIME | `application/rtf`, `text/rtf` |
| Magic | `{\rtf` |

### 2.2 기본 구조

```
{\rtf1\ansi\deff0
{\fonttbl{\f0 Times New Roman;}}
{\colortbl;\red0\green0\blue0;}
\pard
Hello \b World\b0 !
\par
}
```

### 2.3 주요 제어 문자

| 코드 | 의미 | Markdown |
|------|------|----------|
| `\b` | Bold 시작 | `**` |
| `\b0` | Bold 끝 | `**` |
| `\i` | Italic 시작 | `*` |
| `\i0` | Italic 끝 | `*` |
| `\ul` | Underline 시작 | `<u>` |
| `\ulnone` | Underline 끝 | `</u>` |
| `\strike` | Strikethrough | `~~` |
| `\par` | 문단 끝 | `\n\n` |
| `\line` | 줄 바꿈 | `  \n` |
| `\tab` | 탭 | `\t` |

### 2.4 Rust 파싱

```rust
pub struct RtfParser;

impl RtfParser {
    pub fn parse(&self, data: &[u8]) -> Result<Document, ParseError> {
        let text = String::from_utf8_lossy(data);

        // RTF 시작 확인
        if !text.starts_with("{\\rtf") {
            return Err(ParseError::InvalidFormat("Not an RTF file".into()));
        }

        let mut blocks = Vec::new();
        let mut current_text = String::new();
        let mut in_bold = false;
        let mut in_italic = false;

        let mut chars = text.chars().peekable();
        while let Some(c) = chars.next() {
            match c {
                '\\' => {
                    // 제어 문자 파싱
                    let mut control = String::new();
                    while let Some(&nc) = chars.peek() {
                        if nc.is_alphabetic() {
                            control.push(chars.next().unwrap());
                        } else {
                            break;
                        }
                    }

                    match control.as_str() {
                        "par" => {
                            if !current_text.is_empty() {
                                blocks.push(Block::Paragraph(Paragraph {
                                    content: vec![Inline::Text(current_text.clone())],
                                    alignment: None,
                                    indent: None,
                                }));
                                current_text.clear();
                            }
                        }
                        "b" => in_bold = true,
                        "i" => in_italic = true,
                        "line" => current_text.push('\n'),
                        "tab" => current_text.push('\t'),
                        _ => {}
                    }

                    // 숫자 파라미터 스킵
                    while chars.peek().map(|c| c.is_numeric()).unwrap_or(false) {
                        chars.next();
                    }
                    // 공백 스킵
                    if chars.peek() == Some(&' ') {
                        chars.next();
                    }
                }
                '{' | '}' => {
                    // 그룹 시작/끝 - 현재는 무시
                }
                '\n' | '\r' => {
                    // RTF에서 줄바꿈은 의미 없음
                }
                _ => {
                    current_text.push(c);
                }
            }
        }

        // 남은 텍스트 처리
        if !current_text.is_empty() {
            blocks.push(Block::Paragraph(Paragraph {
                content: vec![Inline::Text(current_text)],
                alignment: None,
                indent: None,
            }));
        }

        Ok(Document {
            metadata: Metadata::default(),
            blocks,
            resources: Vec::new(),
            styles: HashMap::new(),
            source_format: DocumentFormat::Rtf,
        })
    }
}
```

### 2.5 권장 크레이트

```toml
[dependencies]
# RTF 파싱 (제한적)
rtf-parser = "0.3"  # 기본 파싱

# 또는 직접 구현 (더 완전한 지원)
```

---

## 3. CSV (Comma-Separated Values)

### 3.1 개요

테이블 데이터의 텍스트 표현.

| 속성 | 값 |
|------|-----|
| 확장자 | `.csv`, `.tsv` |
| MIME | `text/csv` |
| 구분자 | `,` (CSV), `\t` (TSV) |

### 3.2 Markdown 표 변환

```rust
use csv::Reader;

pub fn csv_to_markdown(data: &[u8], has_header: bool) -> Result<String, ParseError> {
    let mut reader = Reader::from_reader(data);

    let mut md = String::new();

    // 헤더 처리
    if has_header {
        if let Ok(headers) = reader.headers() {
            md.push('|');
            for header in headers {
                md.push_str(&format!(" {} |", header));
            }
            md.push('\n');

            // 구분선
            md.push('|');
            for _ in headers.iter() {
                md.push_str("---|");
            }
            md.push('\n');
        }
    }

    // 데이터 행
    for result in reader.records() {
        let record = result.map_err(|e| ParseError::Parse(e.to_string()))?;
        md.push('|');
        for field in record.iter() {
            // Markdown 표 내 파이프 이스케이프
            let escaped = field.replace('|', "\\|");
            md.push_str(&format!(" {} |", escaped));
        }
        md.push('\n');
    }

    Ok(md)
}
```

### 3.3 대용량 CSV → MDM 변환

대용량 CSV는 SVG 테이블로 변환:

```rust
pub fn csv_to_mdm_bundle(
    csv_path: &Path,
    output_dir: &Path,
    rows_per_table: usize,
) -> Result<BundleOutput, ConvertError> {
    let mut reader = csv::Reader::from_path(csv_path)?;
    let headers = reader.headers()?.clone();

    let mut markdown = String::new();
    let mut resources = Vec::new();
    let mut table_count = 0;

    let mut current_rows: Vec<csv::StringRecord> = Vec::new();

    for result in reader.records() {
        let record = result?;
        current_rows.push(record);

        if current_rows.len() >= rows_per_table {
            // SVG 테이블 생성
            table_count += 1;
            let svg = render_table_svg(&headers, &current_rows);
            let svg_name = format!("table_{:03}.svg", table_count);

            // 리소스 저장
            let svg_path = output_dir.join("assets").join(&svg_name);
            std::fs::write(&svg_path, &svg)?;

            resources.push(Resource {
                id: svg_name.clone(),
                original_name: None,
                content_type: "image/svg+xml".to_string(),
                data: ResourceData::File(svg_path),
            });

            // Markdown 참조
            markdown.push_str(&format!("![[assets/{} | preset:table]]\n\n", svg_name));

            current_rows.clear();
        }
    }

    // 남은 행 처리
    if !current_rows.is_empty() {
        if current_rows.len() <= 20 {
            // 작은 테이블은 Markdown으로
            markdown.push_str(&small_table_to_md(&headers, &current_rows));
        } else {
            // SVG로
            table_count += 1;
            let svg = render_table_svg(&headers, &current_rows);
            let svg_name = format!("table_{:03}.svg", table_count);
            // ... (위와 동일)
        }
    }

    // 번들 생성
    todo!()
}
```

---

## 4. Log Files

### 4.1 로그 파일 변환

로그 파일을 Markdown 코드 블록으로 변환:

```rust
pub fn log_to_markdown(log_content: &str) -> String {
    let mut md = String::new();

    // 로그 타입 감지
    let log_type = detect_log_type(log_content);

    md.push_str(&format!("# Log File\n\n"));
    md.push_str(&format!("```{}\n", log_type));
    md.push_str(log_content);
    md.push_str("\n```\n");

    md
}

fn detect_log_type(content: &str) -> &str {
    let first_line = content.lines().next().unwrap_or("");

    if first_line.contains("ERROR") || first_line.contains("WARN") || first_line.contains("INFO") {
        "log"
    } else if first_line.starts_with('[') && first_line.contains(']') {
        "accesslog"
    } else if first_line.contains("nginx") {
        "nginx"
    } else if first_line.contains("apache") {
        "apache"
    } else {
        "text"
    }
}
```

---

## 5. Configuration Files

### 5.1 설정 파일 변환

INI, TOML, YAML 등을 코드 블록으로:

```rust
pub fn config_to_markdown(content: &str, file_ext: &str) -> String {
    let lang = match file_ext {
        "ini" | "cfg" => "ini",
        "toml" => "toml",
        "yaml" | "yml" => "yaml",
        "json" => "json",
        "xml" => "xml",
        "properties" => "properties",
        _ => "text",
    };

    format!("```{}\n{}\n```", lang, content)
}
```

---

## 6. Encoding Reference

### 6.1 한국어 인코딩

| 인코딩 | 용도 | 특징 |
|--------|------|------|
| UTF-8 | 현대 표준 | 가변 길이, BOM 선택적 |
| EUC-KR | 레거시 | 2바이트 한글 |
| CP949 | Windows | EUC-KR 확장 |
| ISO-2022-KR | 이메일 | 7비트 안전 |

### 6.2 인코딩 감지 라이브러리

```toml
[dependencies]
encoding_rs = "0.8"       # Mozilla의 인코딩 라이브러리
chardetng = "0.1"         # 인코딩 자동 감지
```

```rust
use chardetng::EncodingDetector;

pub fn detect_encoding(data: &[u8]) -> &'static encoding_rs::Encoding {
    let mut detector = EncodingDetector::new();
    detector.feed(data, true);
    detector.guess(None, true)
}
```

---

## 7. MDM 출력 형식

### 7.1 TXT → MDM Bundle

```
input.txt
    ↓
output/
├── input.md          # 변환된 Markdown
├── input.mdm         # 메타데이터
└── (assets 없음)     # 텍스트에는 미디어 없음
```

### 7.2 MDM 메타데이터 예시

```json
{
  "version": "1.0",
  "metadata": {
    "source_file": "document.txt",
    "source_format": "Txt",
    "source_encoding": "UTF-8",
    "converted_at": "2025-12-25T10:30:00Z",
    "converter_version": "0.1.0",
    "paragraph_count": 15,
    "word_count": 1250,
    "character_count": 7500
  },
  "resources": {},
  "presets": {}
}
```

---

## 8. 참고 자료

### 인코딩
- [encoding_rs](https://crates.io/crates/encoding_rs) - Rust 인코딩 라이브러리
- [chardetng](https://crates.io/crates/chardetng) - 인코딩 감지

### CSV
- [csv crate](https://crates.io/crates/csv) - Rust CSV 파서
- [RFC 4180](https://tools.ietf.org/html/rfc4180) - CSV 표준

### RTF
- [RTF Specification](https://www.microsoft.com/en-us/download/details.aspx?id=10725)

---

## 변경 이력

| 날짜 | 버전 | 변경 내용 |
|------|------|----------|
| 2025-12-25 | 1.0 | 초기 문서 작성 |
