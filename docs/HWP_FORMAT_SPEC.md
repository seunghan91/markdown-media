# HWP 5.0 File Format Specification

> Rust 개발을 위한 HWP 파일 포맷 기술 명세 문서

## 1. 개요

### 1.1 HWP란?
**HWP (Hangul Word Processor)**: 한글과컴퓨터에서 개발한 한국 표준 워드프로세서 파일 포맷

- **현재 버전**: HWP 5.0 (2002년~현재, HWP 2018까지 호환)
- **이전 버전**: HWP 3.0 (레거시)
- **오픈 포맷**: HWPX (HWP 2010+), OWPML (KS X 6101:2011)

### 1.2 공식 명세서 다운로드

| 버전 | HWP 문서 | PDF 문서 |
|------|----------|----------|
| **HWP 5.0** | [한글문서파일형식_5.0_revision1.3.hwp](https://cdn.hancom.com/link/docs/한글문서파일형식_5.0_revision1.3.hwp) | [PDF](https://cdn.hancom.com/link/docs/한글문서파일형식_5.0_revision1.3.pdf) |
| HWP 3.0/HWPML | [revision1.2.hwp](https://cdn.hancom.com/link/docs/한글문서파일형식3.0_HWPML_revision1.2.hwp) | [PDF](https://cdn.hancom.com/link/docs/한글문서파일형식3.0_HWPML_revision1.2.pdf) |

**추가 명세**:
- 배포용 문서 포맷 (revision 1.2)
- 수식 포맷 (revision 1.3)
- 차트 포맷 (revision 1.2)

> 출처: [Hancom 공식 다운로드](https://store.hancom.com/etc/hwpDownload.do)

---

## 2. 파일 구조

### 2.1 Compound File Binary Format (CFB)

HWP 5.0은 **Microsoft OLE2 Compound Document Format**을 기반으로 함.

```
HWP 파일 구조
├── FileHeader          # 파일 인식 정보
├── DocInfo             # 문서 속성
├── BodyText/           # 본문 섹션들
│   ├── Section0        # 첫 번째 섹션 (압축됨)
│   ├── Section1        # 두 번째 섹션
│   └── ...
├── BinData/            # 임베디드 바이너리 데이터
│   ├── BIN0001.jpg     # 이미지
│   ├── BIN0002.png     # 이미지
│   └── ...
├── PrvText             # 미리보기 텍스트
├── PrvImage            # 미리보기 이미지
├── DocOptions/         # 문서 옵션
├── Scripts/            # 스크립트 (매크로)
└── XMLTemplate/        # XML 템플릿
```

### 2.2 스트림별 상세 구조

#### FileHeader (256 bytes)
```rust
struct FileHeader {
    signature: [u8; 32],      // "HWP Document File" (고정)
    version: u32,             // 파일 버전 (5.0.x.x)
    properties: u32,          // 문서 속성 플래그
    // ... 예약 영역
}

// properties 비트 플래그
const COMPRESSED: u32 = 0x01;      // 압축 여부
const PASSWORD: u32 = 0x02;        // 암호화 여부
const DISTRIBUTE: u32 = 0x04;      // 배포용 문서
const SCRIPT: u32 = 0x08;          // 스크립트 저장
const DRM: u32 = 0x10;             // DRM 보안
const XML_TEMPLATE: u32 = 0x20;    // XML 템플릿 저장
const HISTORY: u32 = 0x40;         // 문서 히스토리
const SIGNATURE: u32 = 0x80;       // 전자 서명
const CERTIFICATE: u32 = 0x100;    // 공인 인증서
const RESERVE_SIGN: u32 = 0x200;   // 전자 서명 예비
const CERTIFICATE_DRM: u32 = 0x400; // 공인 인증서 DRM
const CCL: u32 = 0x800;            // CCL 적용
```

#### BodyText 레코드 구조
```rust
// 레코드 헤더 (4 bytes)
struct RecordHeader {
    tag_id: u10,      // 레코드 태그 ID
    level: u10,       // 레벨
    size: u12,        // 데이터 크기 (최대 4095)
}

// 확장 레코드 (size == 4095일 때)
// 다음 4바이트가 실제 크기
```

### 2.3 주요 레코드 태그 (HWPTAG)

| 태그 ID | 이름 | 설명 |
|---------|------|------|
| 66 | HWPTAG_PARA_HEADER | 문단 헤더 |
| 67 | HWPTAG_PARA_TEXT | 문단 텍스트 |
| 68 | HWPTAG_PARA_CHAR_SHAPE | 글자 모양 |
| 69 | HWPTAG_PARA_LINE_SEG | 줄 세그먼트 |
| 70 | HWPTAG_PARA_RANGE_TAG | 범위 태그 |
| 71 | HWPTAG_CTRL_HEADER | 컨트롤 헤더 |
| 72 | HWPTAG_LIST_HEADER | 리스트 헤더 |
| 75 | HWPTAG_TABLE | 표 |
| 78 | HWPTAG_SHAPE_COMPONENT | 그리기 객체 |

---

## 3. 텍스트 추출 로직

### 3.1 문단 텍스트 구조 (HWPTAG_PARA_TEXT)

```rust
// 텍스트는 UTF-16LE 인코딩
// 특수 문자 처리 필요

const CHAR_INLINE: u16 = 0;      // 인라인 (글자처럼 취급 컨트롤)
const CHAR_EXTENDED: u16 = 1;    // 확장 컨트롤 (표, 그림 등)
const CHAR_LINE_BREAK: u16 = 10; // 강제 줄 바꿈
const CHAR_PARA_BREAK: u16 = 13; // 문단 끝
const CHAR_HYPHEN: u16 = 24;     // 하이픈
const CHAR_TAB: u16 = 9;         // 탭
const CHAR_SECTION_DEF: u16 = 2; // 구역 정의
const CHAR_COLUMN_DEF: u16 = 3;  // 단 정의
const CHAR_FIELD_START: u16 = 4; // 필드 시작
const CHAR_FIELD_END: u16 = 5;   // 필드 끝
const CHAR_BOOKMARK: u16 = 6;    // 책갈피
const CHAR_FOOTNOTE: u16 = 7;    // 각주/미주
const CHAR_AUTO_NUM: u16 = 8;    // 자동 번호

// 0-31 범위는 특수 문자로 처리
fn is_control_char(c: u16) -> bool {
    c < 32
}
```

### 3.2 압축 해제

대부분의 BodyText 섹션은 **zlib (deflate)** 압축됨:

```rust
use flate2::read::DeflateDecoder;
use std::io::Read;

fn decompress_section(data: &[u8]) -> Result<Vec<u8>, Error> {
    let mut decoder = DeflateDecoder::new(data);
    let mut decompressed = Vec::new();
    decoder.read_to_end(&mut decompressed)?;
    Ok(decompressed)
}
```

### 3.3 텍스트 추출 알고리즘

```rust
fn extract_text_from_section(section_data: &[u8]) -> String {
    let mut result = String::new();
    let mut offset = 0;

    while offset < section_data.len() {
        // 레코드 헤더 파싱
        let header = parse_record_header(&section_data[offset..]);
        offset += 4;

        if header.tag_id == HWPTAG_PARA_TEXT {
            let text_data = &section_data[offset..offset + header.size];
            result.push_str(&parse_para_text(text_data));
        }

        offset += header.size;
    }

    result
}

fn parse_para_text(data: &[u8]) -> String {
    let mut text = String::new();
    let chars: Vec<u16> = data
        .chunks_exact(2)
        .map(|b| u16::from_le_bytes([b[0], b[1]]))
        .collect();

    for c in chars {
        match c {
            0..=31 => {
                // 특수 문자 처리
                if c == 10 { text.push('\n'); }  // 줄 바꿈
                if c == 13 { text.push('\n'); }  // 문단 끝
                if c == 9 { text.push('\t'); }   // 탭
                // 나머지는 스킵 또는 마커 삽입
            }
            _ => {
                if let Some(ch) = char::from_u32(c as u32) {
                    text.push(ch);
                }
            }
        }
    }

    text
}
```

---

## 4. 이미지 추출

### 4.1 BinData 스트림

```rust
struct BinDataHeader {
    // HWP OLE2 특수 구조: 앞에 4바이트 길이 정보 추가
    data_length: u32,  // 실제 데이터 길이
    // 이후 실제 이미지 데이터
}

fn extract_bin_data(stream: &[u8]) -> Vec<u8> {
    // HWP는 OLE2에 4바이트 크기를 prepend
    let length = u32::from_le_bytes([stream[0], stream[1], stream[2], stream[3]]);
    stream[4..4 + length as usize].to_vec()
}
```

### 4.2 지원 이미지 포맷

| 확장자 | MIME Type | 비고 |
|--------|-----------|------|
| BMP | image/bmp | Windows Bitmap |
| JPG | image/jpeg | JPEG |
| PNG | image/png | PNG |
| GIF | image/gif | GIF |
| WMF | image/wmf | Windows Metafile |
| EMF | image/emf | Enhanced Metafile |
| OLE | - | OLE Object (별도 처리) |

---

## 5. 표 구조 (HWPTAG_TABLE)

```rust
struct TableRecord {
    // 표 속성
    split_page_mode: u8,  // 페이지 나눔 방식
    repeat_header: bool,  // 머리글 반복

    // 셀 정보
    row_count: u16,
    col_count: u16,
    cell_spacing: u16,

    // 테두리/배경
    border_fill_id: u16,

    // 셀 데이터는 LIST_HEADER로 감싸져 있음
}
```

---

## 6. Rust 구현 참고 라이브러리

### 6.1 hwpers (권장)

```toml
[dependencies]
hwpers = "0.3.1"
```

**주요 기능**:
- HWP 5.0 완전 지원
- 텍스트, 표, 이미지 추출
- SVG 렌더링 지원
- 레이아웃 엔진

**사용 예시**:
```rust
use hwpers::{HwpDocument, HwpReader};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let doc = HwpDocument::from_path("document.hwp")?;

    // 텍스트 추출
    for paragraph in doc.paragraphs() {
        println!("{}", paragraph.text());
    }

    // 테이블 추출
    for table in doc.tables() {
        for row in table.rows() {
            for cell in row.cells() {
                print!("{}\t", cell.text());
            }
            println!();
        }
    }

    Ok(())
}
```

### 6.2 hwp-rs

```toml
[dependencies]
hwp = "0.2"
```

**특징**:
- 저수준 파서
- Python 바인딩 (libhwp)
- WASM 지원 예정

### 6.3 직접 구현 시 필요 크레이트

```toml
[dependencies]
cfb = "0.11"           # OLE2/CFB 파일 읽기
flate2 = "1.0"         # zlib 압축 해제
encoding_rs = "0.8"    # 인코딩 변환
byteorder = "1.5"      # 바이너리 읽기
```

---

## 7. MDM 변환 전략

### 7.1 텍스트 → Markdown

```
HWP 요소           → Markdown
────────────────────────────────
문단               → 일반 텍스트
강조 (굵게)        → **bold**
강조 (기울임)      → *italic*
밑줄               → <u>underline</u>
취소선             → ~~strikethrough~~
제목 스타일        → # Heading
목록               → - list item
번호 목록          → 1. numbered
하이퍼링크         → [text](url)
```

### 7.2 표 → SVG/Markdown

복잡한 표 (병합셀, 서식):
```
![[table_001.svg | preset:table]]
```

단순 표:
```markdown
| 열1 | 열2 | 열3 |
|-----|-----|-----|
| A   | B   | C   |
```

### 7.3 이미지 → MDM 참조

```markdown
// 추출된 이미지
![[image_001.png | alt="원본 캡션" width=800]]
```

---

## 8. 참고 자료

### 공식 문서
- [Hancom HWP 명세 다운로드](https://store.hancom.com/etc/hwpDownload.do)
- [OWPML 국가표준 (KS X 6101:2011)](https://www.kssn.net)

### 오픈소스 구현체
- [hwpers (Rust)](https://github.com/Indosaram/hwpers) - HWP 5.0 완전 지원
- [hwp-rs (Rust)](https://github.com/hahnlee/hwp-rs) - 저수준 파서
- [pyhwp (Python)](https://pypi.org/project/pyhwp/) - Python 구현

### 관련 문서
- [Just Solve: HWP](http://justsolve.archiveteam.org/wiki/HWP)
- [ClamAV HWP 분석](https://blog.clamav.net/2016/03/clamav-0991-hangul-word-processor-hwp.html)

---

## 변경 이력

| 날짜 | 버전 | 변경 내용 |
|------|------|----------|
| 2025-12-25 | 1.0 | 초기 문서 작성 |
