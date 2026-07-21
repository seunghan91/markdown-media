//! HWPML 2.x 문서 본체 파싱 — 소형 XML 트리 빌더 + IRBlock 변환.
//!
//! quick-xml 은 스트리밍 리더라 kkdoc(TypeScript, DOMParser 기반)의 재귀
//! 워커를 그대로 옮기려면 먼저 이벤트 스트림을 작은 트리로 조립해야 한다.
//! [`Elem`]/[`Node`] 가 그 트리이고, 이후 로직(`build_para_shape_map`,
//! `walk_content`, `parse_table` 등)은 kkdoc `src/hwpml/parser.ts`의 동명
//! 함수를 1:1로 옮긴 것이다.
//!
//! Ported from kkdoc (MIT): src/hwpml/parser.ts

use crate::ir::{blocks_to_markdown, IRBlock, IRCell, IRTable};
use quick_xml::events::{BytesStart, Event};
use quick_xml::Reader;
use std::collections::HashMap;
use std::io;

/// XML 재귀 깊이 상한 — 손으로 만든/손상된 파일이 스택을 고갈시키는 것을 방지.
/// kkdoc `MAX_XML_DEPTH` 와 동일.
const MAX_XML_DEPTH: usize = 200;
/// 표 크기 상한 — 초과 시 스킵 + 경고(kkdoc `MAX_TABLE_ROWS`/`MAX_TABLE_COLS`).
const MAX_TABLE_ROWS: usize = 5000;
const MAX_TABLE_COLS: usize = 500;
/// 파일 크기 상한 (kkdoc `MAX_HWPML_BYTES`).
const MAX_HWPML_BYTES: usize = 50 * 1024 * 1024;

/// 문서에서 추출한 메타데이터 (DOCSUMMARY).
#[derive(Debug, Clone, Default)]
pub struct HwpmlMetadata {
    /// `<HWPML Version="...">` 속성.
    pub version: Option<String>,
    pub title: Option<String>,
    pub author: Option<String>,
    pub date: Option<String>,
}

/// HWPML 파싱 결과 — hwp3/hwpx 파서와 동일한 형태(`markdown` + `blocks` +
/// `metadata` + `warnings`).
#[derive(Debug, Clone)]
pub struct HwpmlDocument {
    pub markdown: String,
    pub blocks: Vec<IRBlock>,
    pub metadata: HwpmlMetadata,
    /// 부분 파싱/제한 초과 등에 대한 경고 메시지 ("CODE: message" 형식).
    pub warnings: Vec<String>,
}

// ─────────────────────────────────────────────────────────────────────────
// 인코딩 처리
// ─────────────────────────────────────────────────────────────────────────

/// 원시 바이트를 UTF-8 문자열로 디코드한다.
///
/// HWPML 은 UTF-8 이 일반적이지만 UTF-16(BOM 동반) 내보내기도 존재한다.
/// 전략은 `txt_parser.rs::decode_text` 와 동일:
/// 1. BOM 으로 UTF-16 LE/BE 판별
/// 2. UTF-8 BOM 제거
/// 3. UTF-8 시도, 실패 시 EUC-KR 폴백
pub(crate) fn decode_hwpml_bytes(data: &[u8]) -> String {
    if data.len() >= 2 && data[0] == 0xFF && data[1] == 0xFE {
        let (decoded, _, _) = encoding_rs::UTF_16LE.decode(&data[2..]);
        return decoded.to_string();
    }
    if data.len() >= 2 && data[0] == 0xFE && data[1] == 0xFF {
        let (decoded, _, _) = encoding_rs::UTF_16BE.decode(&data[2..]);
        return decoded.to_string();
    }
    let data = if data.len() >= 3 && data[0] == 0xEF && data[1] == 0xBB && data[2] == 0xBF {
        &data[3..]
    } else {
        data
    };
    if let Ok(s) = std::str::from_utf8(data) {
        return s.to_string();
    }
    let (decoded, _, _) = encoding_rs::EUC_KR.decode(data);
    decoded.to_string()
}

/// XML 선언/DOCTYPE/선행 주석을 건너뛰고 루트 요소가 시작되는 지점부터
/// 반환한다. `is_hwpml` 탐지 전용 — 실제 파싱(`build_tree`)은 quick-xml 이
/// 이 이벤트들을 논-밸리데이팅으로 그냥 스킵하므로 별도 처리가 필요 없다.
pub(crate) fn strip_prolog(input: &str) -> &str {
    let mut s = input;
    loop {
        let trimmed = s.trim_start();
        if let Some(rest) = trimmed.strip_prefix("<?xml") {
            match rest.find("?>") {
                Some(end) => {
                    s = &rest[end + 2..];
                    continue;
                }
                None => return "",
            }
        }
        if let Some(rest) = trimmed.strip_prefix("<!--") {
            match rest.find("-->") {
                Some(end) => {
                    s = &rest[end + 3..];
                    continue;
                }
                None => return "",
            }
        }
        if let Some(rest) = trimmed.strip_prefix("<!DOCTYPE") {
            match find_doctype_end(rest) {
                Some(end) => {
                    s = &rest[end + 1..];
                    continue;
                }
                None => return "",
            }
        }
        return trimmed;
    }
}

/// `<!DOCTYPE ...>` 의 닫는 `>` 위치를 찾는다. 내부 서브셋(`[ ... ]`)에 포함된
/// `>` 는 무시해야 하므로 대괄호 깊이를 추적한다.
fn find_doctype_end(s: &str) -> Option<usize> {
    let mut depth: i32 = 0;
    for (i, b) in s.bytes().enumerate() {
        match b {
            b'[' => depth += 1,
            b']' => depth -= 1,
            b'>' if depth <= 0 => return Some(i),
            _ => {}
        }
    }
    None
}

// ─────────────────────────────────────────────────────────────────────────
// 소형 XML 트리
// ─────────────────────────────────────────────────────────────────────────

enum Node {
    Element(Elem),
    Text(String),
}

struct Elem {
    tag: String,
    attrs: HashMap<String, String>,
    children: Vec<Node>,
}

impl Elem {
    fn attr(&self, name: &str) -> Option<&str> {
        self.attrs.get(name).map(|s| s.as_str())
    }

    fn find_child(&self, tag: &str) -> Option<&Elem> {
        self.children.iter().find_map(|n| match n {
            Node::Element(e) if e.tag == tag => Some(e),
            _ => None,
        })
    }

    fn child_elements(&self) -> impl Iterator<Item = &Elem> {
        self.children.iter().filter_map(|n| match n {
            Node::Element(e) => Some(e),
            _ => None,
        })
    }

    /// 하위의 모든 텍스트 노드를 재귀적으로 이어붙인다 (자식 요소 안의
    /// 텍스트도 포함 — `<TITLE>` 처럼 단순 텍스트 컨테이너용).
    fn text_content(&self) -> String {
        let mut out = String::new();
        for n in &self.children {
            match n {
                Node::Text(t) => out.push_str(t),
                Node::Element(e) => out.push_str(&e.text_content()),
            }
        }
        out
    }
}

fn local_name_string(e: &BytesStart) -> String {
    String::from_utf8_lossy(e.local_name().as_ref()).into_owned()
}

fn collect_attrs(e: &BytesStart) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for attr in e.attributes().flatten() {
        let key = String::from_utf8_lossy(attr.key.local_name().as_ref()).into_owned();
        let val = String::from_utf8_lossy(&attr.value).into_owned();
        map.insert(key, val);
    }
    map
}

fn attach(stack: &mut Vec<Elem>, root: &mut Option<Elem>, elem: Elem) {
    if let Some(parent) = stack.last_mut() {
        parent.children.push(Node::Element(elem));
    } else {
        *root = Some(elem);
    }
}

/// quick-xml 이벤트 스트림 → [`Elem`] 트리. 잘 만들어진 문서는 마지막 `</HWPML>`
/// 의 `End` 이벤트에서 스택이 비며 `root` 가 채워진다. 깊이 초과나 파싱
/// 오류가 나면 `warnings` 에 기록하고 그때까지 조립된 부분 트리를 최선으로
/// 반환한다(전체 포기보다 부분 결과가 유용 — hwp3 파서와 동일한 태도).
fn build_tree(xml: &str, warnings: &mut Vec<String>) -> Option<Elem> {
    let mut reader = Reader::from_str(xml);
    reader.trim_text(true);
    let mut buf = Vec::new();
    let mut stack: Vec<Elem> = Vec::new();
    let mut root: Option<Elem> = None;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                if stack.len() >= MAX_XML_DEPTH {
                    warnings.push("MALFORMED_XML: XML 중첩 깊이 초과 — 파싱 중단".to_string());
                    break;
                }
                stack.push(Elem {
                    tag: local_name_string(&e),
                    attrs: collect_attrs(&e),
                    children: Vec::new(),
                });
            }
            Ok(Event::Empty(e)) => {
                let elem = Elem {
                    tag: local_name_string(&e),
                    attrs: collect_attrs(&e),
                    children: Vec::new(),
                };
                attach(&mut stack, &mut root, elem);
            }
            Ok(Event::End(_)) => {
                if let Some(finished) = stack.pop() {
                    attach(&mut stack, &mut root, finished);
                }
            }
            Ok(Event::Text(e)) => {
                if let Ok(text) = e.unescape() {
                    let text = text.into_owned();
                    if !text.is_empty() {
                        if let Some(top) = stack.last_mut() {
                            top.children.push(Node::Text(text));
                        }
                    }
                }
            }
            Ok(Event::CData(e)) => {
                let text = String::from_utf8_lossy(e.as_ref()).into_owned();
                if !text.is_empty() {
                    if let Some(top) = stack.last_mut() {
                        top.children.push(Node::Text(text));
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                warnings.push(format!("MALFORMED_XML: HWPML XML 파싱 오류: {}", e));
                break;
            }
            _ => {}
        }
        buf.clear();
    }

    // 스트림이 중간에 끊겨(깊이 초과/파싱 오류) root 가 아직 안 채워졌으면,
    // 스택 바닥(가장 바깥 미종료 요소)을 최선의 부분 결과로 반환한다.
    if root.is_none() && !stack.is_empty() {
        root = Some(stack.remove(0));
    }
    root
}

// ─────────────────────────────────────────────────────────────────────────
// ParaShape → HeadingType 맵
// ─────────────────────────────────────────────────────────────────────────

struct ParaShapeInfo {
    /// `None` = 일반 단락, `Some(1..=6)` = 헤딩 레벨.
    heading_level: Option<u8>,
}

/// `HEAD > MAPPINGTABLE > PARASHAPELIST > PARASHAPE` 를 순회해 `Id → 헤딩레벨`
/// 맵을 만든다. `HeadingType="Outline"` 인 것만 헤딩으로 취급하고, `Level`
/// (0-based)을 1..=6 범위로 클램프해 마크다운 헤딩 레벨(H1..H6)로 변환한다.
fn build_para_shape_map(root: &Elem) -> HashMap<String, ParaShapeInfo> {
    let mut map = HashMap::new();
    let Some(head) = root.find_child("HEAD") else {
        return map;
    };
    let Some(mapping_table) = head.find_child("MAPPINGTABLE") else {
        return map;
    };
    let Some(para_shape_list) = mapping_table.find_child("PARASHAPELIST") else {
        return map;
    };

    for el in para_shape_list.child_elements() {
        if el.tag != "PARASHAPE" {
            continue;
        }
        let id = el.attr("Id").unwrap_or("").to_string();
        let heading_type = el.attr("HeadingType").unwrap_or("None");
        let heading_level = if heading_type == "Outline" {
            let level: i32 = el.attr("Level").and_then(|s| s.parse().ok()).unwrap_or(0);
            let safe_level = level.max(0);
            Some((safe_level + 1).min(6) as u8)
        } else {
            None
        };
        map.insert(id, ParaShapeInfo { heading_level });
    }

    map
}

// ─────────────────────────────────────────────────────────────────────────
// BODY 순회 — 문단 / 표
// ─────────────────────────────────────────────────────────────────────────

/// 콘텐츠 노드를 재귀적으로 순회하며 IRBlock 을 만든다.
/// `in_header_footer=true` 이면 문단/표 블록 출력을 억제한다(HEADER/FOOTER
/// 하위에 들어갔을 때).
fn walk_content(
    node: &Elem,
    blocks: &mut Vec<IRBlock>,
    para_shape_map: &HashMap<String, ParaShapeInfo>,
    warnings: &mut Vec<String>,
    in_header_footer: bool,
    depth: usize,
) {
    if depth > MAX_XML_DEPTH {
        return;
    }
    for el in node.child_elements() {
        match el.tag.as_str() {
            "HEADER" | "FOOTER" => continue,
            "P" => {
                if !in_header_footer {
                    parse_paragraph(el, blocks, para_shape_map);
                    // HWPML 의 표는 <P><TEXT>… 안에 앵커로 들어있다 — 문단
                    // 텍스트만 뽑고 지나치면 표 전체가 소실된다.
                    walk_tables_in_p(el, blocks, warnings, 0);
                }
            }
            "TABLE" => {
                if !in_header_footer {
                    parse_table(el, blocks, warnings);
                }
            }
            "PARALIST" | "SECTION" | "COLDEF" => {
                walk_content(el, blocks, para_shape_map, warnings, in_header_footer, depth + 1);
            }
            _ => {
                walk_content(el, blocks, para_shape_map, warnings, in_header_footer, depth + 1);
            }
        }
    }
}

/// `<P>` 내부의 `<TABLE>` 앵커만 수집한다(문단 텍스트는 `parse_paragraph` 소관).
/// FOOTNOTE/ENDNOTE/HEADER/FOOTER 하위는 제외 — 그 텍스트는 이미 문단
/// 텍스트로 수집되므로 내부 표까지 올리면 중복된다.
fn walk_tables_in_p(node: &Elem, blocks: &mut Vec<IRBlock>, warnings: &mut Vec<String>, depth: usize) {
    if depth > MAX_XML_DEPTH {
        return;
    }
    for el in node.child_elements() {
        match el.tag.as_str() {
            "TABLE" => parse_table(el, blocks, warnings),
            "FOOTNOTE" | "ENDNOTE" | "HEADER" | "FOOTER" => continue,
            _ => walk_tables_in_p(el, blocks, warnings, depth + 1),
        }
    }
}

fn parse_paragraph(el: &Elem, blocks: &mut Vec<IRBlock>, para_shape_map: &HashMap<String, ParaShapeInfo>) {
    let para_shape_id = el.attr("ParaShape").unwrap_or("");
    let heading_level = para_shape_map.get(para_shape_id).and_then(|s| s.heading_level);

    let text = extract_paragraph_text(el);
    if text.is_empty() {
        return;
    }

    if let Some(level) = heading_level {
        blocks.push(IRBlock::heading(level, text));
    } else {
        blocks.push(IRBlock::paragraph(text));
    }
}

/// `<P>` 에서 텍스트 추출 — `<TEXT><CHAR>` 순회.
fn extract_paragraph_text(p: &Elem) -> String {
    let mut parts = Vec::new();
    collect_char_text(p, &mut parts, 0);
    parts.join("").trim().to_string()
}

fn collect_char_text(node: &Elem, parts: &mut Vec<String>, depth: usize) {
    if depth > MAX_XML_DEPTH {
        return;
    }
    for el in node.child_elements() {
        match el.tag.as_str() {
            "CHAR" => {
                let t = el.text_content();
                if !t.is_empty() {
                    parts.push(t);
                }
            }
            // 단락 내 표/그림은 별도 블록으로 처리되므로 여기선 스킵.
            "TABLE" | "PICTURE" | "SHAPEOBJECT" => {}
            // 자동 번호(페이지 번호 등) 스킵.
            "AUTONUM" => {}
            _ => collect_char_text(el, parts, depth + 1),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────
// 표
// ─────────────────────────────────────────────────────────────────────────

fn parse_table(el: &Elem, blocks: &mut Vec<IRBlock>, warnings: &mut Vec<String>) {
    let row_count: usize = el.attr("RowCount").and_then(|s| s.parse().ok()).unwrap_or(0);
    let col_count: usize = el.attr("ColCount").and_then(|s| s.parse().ok()).unwrap_or(0);
    if row_count == 0 || col_count == 0 {
        return;
    }
    if row_count > MAX_TABLE_ROWS || col_count > MAX_TABLE_COLS {
        warnings.push(format!(
            "TRUNCATED_TABLE: 테이블 크기 초과 ({}x{}) — 스킵",
            row_count, col_count
        ));
        return;
    }

    // 셀은 colAddr/rowAddr 절대좌표를 가지므로 (row, col) 그리드에 직접
    // 배치한다. IRTable/render_table 은 span 을 가진 origin 셀만 있으면
    // shadow 위치를 렌더 시점에 계산하므로(ir.rs::render_table_html), 별도
    // shadow-fill 은 필요 없다.
    let mut grid: Vec<Vec<IRCell>> = vec![vec![IRCell::new(""); col_count]; row_count];
    let mut any_cell = false;

    for row_el in el.child_elements().filter(|c| c.tag == "ROW") {
        for cell_el in row_el.child_elements().filter(|c| c.tag == "CELL") {
            let col_addr: usize = cell_el.attr("ColAddr").and_then(|s| s.parse().ok()).unwrap_or(0);
            let row_addr: usize = cell_el.attr("RowAddr").and_then(|s| s.parse().ok()).unwrap_or(0);
            let col_span: u16 = cell_el
                .attr("ColSpan")
                .and_then(|s| s.parse::<u32>().ok())
                .unwrap_or(1)
                .clamp(1, MAX_TABLE_COLS as u32) as u16;
            let row_span: u16 = cell_el
                .attr("RowSpan")
                .and_then(|s| s.parse::<u32>().ok())
                .unwrap_or(1)
                .clamp(1, MAX_TABLE_ROWS as u32) as u16;

            let text = extract_cell_text(cell_el);

            if row_addr < row_count && col_addr < col_count {
                grid[row_addr][col_addr] = IRCell {
                    text,
                    col_span,
                    row_span,
                };
                any_cell = true;
            }
        }
    }

    if !any_cell {
        return;
    }

    // 표의 SHAPEOBJECT > CAPTION 텍스트 — Side=Top/Left 면 표 앞, 그 외는 뒤.
    let caption = extract_shape_caption(el);
    if let Some((text, true)) = &caption {
        blocks.push(IRBlock::paragraph(text.clone()));
    }
    blocks.push(IRBlock::Table(IRTable::new(grid)));
    if let Some((text, false)) = &caption {
        blocks.push(IRBlock::paragraph(text.clone()));
    }
}

fn extract_shape_caption(table_el: &Elem) -> Option<(String, bool)> {
    let shape = table_el.find_child("SHAPEOBJECT")?;
    let caption = shape.find_child("CAPTION")?;
    let mut parts = Vec::new();
    collect_cell_text(caption, &mut parts, 0);
    let text = parts
        .into_iter()
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>()
        .join("\n");
    let text = text.trim().to_string();
    if text.is_empty() {
        return None;
    }
    let side = caption.attr("Side").unwrap_or("");
    Some((text, side == "Top" || side == "Left"))
}

/// 셀 내부 텍스트 추출 — `PARALIST > P` 재귀, 중첩 표는 평탄화.
fn extract_cell_text(cell_el: &Elem) -> String {
    let mut parts = Vec::new();
    collect_cell_text(cell_el, &mut parts, 0);
    parts
        .into_iter()
        .filter(|p: &String| !p.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

fn collect_cell_text(node: &Elem, parts: &mut Vec<String>, depth: usize) {
    if depth > 20 {
        return;
    }
    for el in node.child_elements() {
        match el.tag.as_str() {
            "P" => {
                let t = extract_paragraph_text(el);
                if !t.is_empty() {
                    parts.push(t);
                }
                // P 안에 앵커된 중첩 표 — 텍스트만 평탄화(안내 박스가 1×1
                // 표로 흔함). 각주류는 extract_paragraph_text 가 이미 수집.
                collect_nested_table_text(el, parts, depth + 1);
            }
            "TABLE" => {
                // 중첩 표 — 셀 문단 텍스트로 평탄화.
                collect_cell_text(el, parts, depth + 1);
            }
            _ => collect_cell_text(el, parts, depth + 1),
        }
    }
}

fn collect_nested_table_text(node: &Elem, parts: &mut Vec<String>, depth: usize) {
    if depth > 20 {
        return;
    }
    for el in node.child_elements() {
        match el.tag.as_str() {
            "TABLE" => collect_cell_text(el, parts, depth + 1),
            "FOOTNOTE" | "ENDNOTE" | "HEADER" | "FOOTER" => continue,
            _ => collect_nested_table_text(el, parts, depth + 1),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────
// 엔트리 포인트
// ─────────────────────────────────────────────────────────────────────────

fn non_empty(s: String) -> Option<String> {
    let t = s.trim();
    if t.is_empty() {
        None
    } else {
        Some(t.to_string())
    }
}

/// HWPML 버퍼 → [`HwpmlDocument`].
///
/// 50MB 초과 파일은 `InvalidData` 로 거부한다(kkdoc `MAX_HWPML_BYTES` 와
/// 동일 상한). XML 파싱 중 오류/깊이 초과는 치명적으로 취급하지 않고
/// `warnings` 에 기록한 뒤 그때까지 조립된 부분 결과를 반환한다.
pub fn parse_hwpml_document(buffer: &[u8]) -> io::Result<HwpmlDocument> {
    if buffer.len() > MAX_HWPML_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "HWPML 파일 크기 초과 ({:.1}MB > 50MB)",
                buffer.len() as f64 / 1024.0 / 1024.0
            ),
        ));
    }

    let text = decode_hwpml_bytes(buffer);
    let text = text.trim_start_matches('\u{feff}');
    // &nbsp; 엔티티는 표준 XML 엔티티가 아니라 quick-xml 이 거부한다 —
    // 파싱 전에 표준 엔티티(&#160;)로 치환한다.
    let normalized = text.replace("&nbsp;", "&#160;");

    let mut warnings: Vec<String> = Vec::new();
    let root = build_tree(&normalized, &mut warnings);

    let Some(root) = root else {
        return Ok(HwpmlDocument {
            markdown: String::new(),
            blocks: Vec::new(),
            metadata: HwpmlMetadata::default(),
            warnings,
        });
    };

    let mut metadata = HwpmlMetadata {
        version: root.attr("Version").map(|s| s.to_string()),
        ..Default::default()
    };
    // DOCSUMMARY 위치는 실제 문서마다 다르다 — DTD상 HWPML 의 직계 자식이지만,
    // 한글에서 내보낸 UTF-16 문서는 HEAD 하위에 중첩된 경우가 흔하다(opt-cli
    // 리포트로 확인). 둘 다 지원.
    let doc_summary = root
        .find_child("DOCSUMMARY")
        .or_else(|| root.find_child("HEAD").and_then(|head| head.find_child("DOCSUMMARY")));
    if let Some(doc_summary) = doc_summary {
        if let Some(title) = doc_summary.find_child("TITLE") {
            metadata.title = non_empty(title.text_content());
        }
        if let Some(author) = doc_summary.find_child("AUTHOR") {
            metadata.author = non_empty(author.text_content());
        }
        if let Some(date) = doc_summary.find_child("DATE") {
            metadata.date = non_empty(date.text_content());
        }
    }

    let para_shape_map = build_para_shape_map(&root);

    let mut blocks: Vec<IRBlock> = Vec::new();
    if let Some(body) = root.find_child("BODY") {
        for section_el in body.child_elements().filter(|c| c.tag == "SECTION") {
            walk_content(section_el, &mut blocks, &para_shape_map, &mut warnings, false, 0);
        }
    }

    let markdown = blocks_to_markdown(&blocks);
    Ok(HwpmlDocument {
        markdown,
        blocks,
        metadata,
        warnings,
    })
}

// ─────────────────────────────────────────────────────────────────────────
// 테스트
// ─────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<HWPML Version="2.8">
  <HEAD>
    <MAPPINGTABLE>
      <PARASHAPELIST Count="2">
        <PARASHAPE Id="0" HeadingType="None" Level="0"/>
        <PARASHAPE Id="1" HeadingType="Outline" Level="0"/>
      </PARASHAPELIST>
    </MAPPINGTABLE>
  </HEAD>
  <BODY>
    <SECTION Id="0">
      <P ParaShape="1"><TEXT><CHAR>제목입니다</CHAR></TEXT></P>
      <P ParaShape="0"><TEXT><CHAR>본문 문단입니다.</CHAR></TEXT></P>
      <TABLE RowCount="2" ColCount="2">
        <ROW>
          <CELL ColAddr="0" RowAddr="0" ColSpan="1" RowSpan="1"><PARALIST><P ParaShape="0"><TEXT><CHAR>헤더1</CHAR></TEXT></P></PARALIST></CELL>
          <CELL ColAddr="1" RowAddr="0" ColSpan="1" RowSpan="1"><PARALIST><P ParaShape="0"><TEXT><CHAR>헤더2</CHAR></TEXT></P></PARALIST></CELL>
        </ROW>
        <ROW>
          <CELL ColAddr="0" RowAddr="1" ColSpan="1" RowSpan="1"><PARALIST><P ParaShape="0"><TEXT><CHAR>값1</CHAR></TEXT></P></PARALIST></CELL>
          <CELL ColAddr="1" RowAddr="1" ColSpan="1" RowSpan="1"><PARALIST><P ParaShape="0"><TEXT><CHAR>값2</CHAR></TEXT></P></PARALIST></CELL>
        </ROW>
      </TABLE>
    </SECTION>
  </BODY>
</HWPML>"#;

    #[test]
    fn heading_detected_via_parashape_outline() {
        let doc = parse_hwpml_document(FIXTURE.as_bytes()).unwrap();
        let heading = doc.blocks.iter().find_map(|b| match b {
            IRBlock::Heading { level, text } => Some((*level, text.clone())),
            _ => None,
        });
        assert_eq!(heading, Some((1, "제목입니다".to_string())));
    }

    #[test]
    fn plain_paragraph_extracted() {
        let doc = parse_hwpml_document(FIXTURE.as_bytes()).unwrap();
        let has_para = doc.blocks.iter().any(|b| match b {
            IRBlock::Paragraph { text, .. } => text == "본문 문단입니다.",
            _ => false,
        });
        assert!(has_para, "blocks: {:?}", doc.blocks);
    }

    #[test]
    fn table_cells_placed_by_addr() {
        let doc = parse_hwpml_document(FIXTURE.as_bytes()).unwrap();
        let table = doc.blocks.iter().find_map(|b| match b {
            IRBlock::Table(t) => Some(t),
            _ => None,
        });
        let table = table.expect("table block present");
        assert_eq!(table.rows, 2);
        assert_eq!(table.cols, 2);
        assert_eq!(table.cells[0][0].text, "헤더1");
        assert_eq!(table.cells[0][1].text, "헤더2");
        assert_eq!(table.cells[1][0].text, "값1");
        assert_eq!(table.cells[1][1].text, "값2");
    }

    #[test]
    fn heading_level_clamped_to_h6() {
        let xml = r#"<?xml version="1.0"?>
<HWPML Version="2.8">
  <HEAD><MAPPINGTABLE><PARASHAPELIST>
    <PARASHAPE Id="9" HeadingType="Outline" Level="99"/>
  </PARASHAPELIST></MAPPINGTABLE></HEAD>
  <BODY><SECTION>
    <P ParaShape="9"><TEXT><CHAR>깊은 헤딩</CHAR></TEXT></P>
  </SECTION></BODY>
</HWPML>"#;
        let doc = parse_hwpml_document(xml.as_bytes()).unwrap();
        let level = doc.blocks.iter().find_map(|b| match b {
            IRBlock::Heading { level, .. } => Some(*level),
            _ => None,
        });
        assert_eq!(level, Some(6));
    }

    #[test]
    fn merged_cell_colspan_preserved() {
        let xml = r#"<?xml version="1.0"?>
<HWPML Version="2.8">
  <BODY><SECTION>
    <TABLE RowCount="2" ColCount="2">
      <ROW>
        <CELL ColAddr="0" RowAddr="0" ColSpan="2" RowSpan="1"><PARALIST><P><TEXT><CHAR>병합헤더</CHAR></TEXT></P></PARALIST></CELL>
      </ROW>
      <ROW>
        <CELL ColAddr="0" RowAddr="1"><PARALIST><P><TEXT><CHAR>a</CHAR></TEXT></P></PARALIST></CELL>
        <CELL ColAddr="1" RowAddr="1"><PARALIST><P><TEXT><CHAR>b</CHAR></TEXT></P></PARALIST></CELL>
      </ROW>
    </TABLE>
  </SECTION></BODY>
</HWPML>"#;
        let doc = parse_hwpml_document(xml.as_bytes()).unwrap();
        let table = doc
            .blocks
            .iter()
            .find_map(|b| match b {
                IRBlock::Table(t) => Some(t),
                _ => None,
            })
            .expect("table present");
        assert_eq!(table.cells[0][0].col_span, 2);
        assert_eq!(table.cells[0][0].text, "병합헤더");
        // Rendered markdown falls back to HTML <table> because of the merge.
        assert!(doc.markdown.contains("<table>"));
        assert!(doc.markdown.contains("colspan=\"2\""));
    }

    #[test]
    fn oversized_table_skipped_with_warning() {
        let xml = r#"<?xml version="1.0"?>
<HWPML Version="2.8">
  <BODY><SECTION>
    <TABLE RowCount="99999" ColCount="2">
      <ROW><CELL ColAddr="0" RowAddr="0"><PARALIST><P><TEXT><CHAR>x</CHAR></TEXT></P></PARALIST></CELL></ROW>
    </TABLE>
  </SECTION></BODY>
</HWPML>"#;
        let doc = parse_hwpml_document(xml.as_bytes()).unwrap();
        assert!(!doc.blocks.iter().any(|b| matches!(b, IRBlock::Table(_))));
        assert!(doc.warnings.iter().any(|w| w.starts_with("TRUNCATED_TABLE")));
    }

    #[test]
    fn metadata_extracted_from_docsummary() {
        let xml = r#"<?xml version="1.0"?>
<HWPML Version="2.8">
  <DOCSUMMARY>
    <TITLE>테스트 문서</TITLE>
    <AUTHOR>홍길동</AUTHOR>
    <DATE>2026-01-01</DATE>
  </DOCSUMMARY>
  <BODY><SECTION><P><TEXT><CHAR>본문</CHAR></TEXT></P></SECTION></BODY>
</HWPML>"#;
        let doc = parse_hwpml_document(xml.as_bytes()).unwrap();
        assert_eq!(doc.metadata.version.as_deref(), Some("2.8"));
        assert_eq!(doc.metadata.title.as_deref(), Some("테스트 문서"));
        assert_eq!(doc.metadata.author.as_deref(), Some("홍길동"));
        assert_eq!(doc.metadata.date.as_deref(), Some("2026-01-01"));
    }

    #[test]
    fn utf16le_bom_encoded_document_parses() {
        // encoding_rs only *decodes* UTF-16 (browsers never need to encode
        // into it) — build the UTF-16LE bytes by hand via `encode_utf16`.
        let mut bytes = vec![0xFF, 0xFE];
        for unit in FIXTURE.encode_utf16() {
            bytes.extend_from_slice(&unit.to_le_bytes());
        }
        let doc = parse_hwpml_document(&bytes).unwrap();
        let has_heading = doc
            .blocks
            .iter()
            .any(|b| matches!(b, IRBlock::Heading { text, .. } if text == "제목입니다"));
        assert!(has_heading, "blocks: {:?}", doc.blocks);
    }

    #[test]
    fn utf16_title_under_head_extracted() {
        // 한글에서 내보낸 실제 UTF-16 HWPML 문서는 DOCSUMMARY 가 HWPML 의
        // 직계 자식이 아니라 HEAD 하위에 중첩되는 경우가 흔하다(opt-cli
        // 리포트로 재현). 본문/버전은 DOCSUMMARY 위치와 무관해 정상 추출되던
        // 반면 title/author/date 만 누락됐었다.
        let xml = r#"<?xml version="1.0" encoding="utf-16"?>
<!DOCTYPE HWPML SYSTEM "hwpml.dtd">
<HWPML Version="2.8" SubVersion="8.0.0.0">
<HEAD SecCnt="1">
<DOCSUMMARY>
<TITLE>제목입니다</TITLE>
<AUTHOR>홍길동</AUTHOR>
<DATE>2026-01-01</DATE>
</DOCSUMMARY>
<MAPPINGTABLE>
</MAPPINGTABLE>
</HEAD>
<BODY><SECTION><P><TEXT><CHAR>본문</CHAR></TEXT></P></SECTION></BODY>
</HWPML>"#;
        let mut bytes = vec![0xFF, 0xFE];
        for unit in xml.encode_utf16() {
            bytes.extend_from_slice(&unit.to_le_bytes());
        }
        let doc = parse_hwpml_document(&bytes).unwrap();
        assert_eq!(doc.metadata.version.as_deref(), Some("2.8"));
        assert_eq!(doc.metadata.title.as_deref(), Some("제목입니다"));
        assert_eq!(doc.metadata.author.as_deref(), Some("홍길동"));
        assert_eq!(doc.metadata.date.as_deref(), Some("2026-01-01"));
    }

    #[test]
    fn oversized_file_rejected() {
        let big = vec![b'a'; MAX_HWPML_BYTES + 1];
        let err = parse_hwpml_document(&big).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
    }

    #[test]
    fn nested_table_flattened_into_cell_text() {
        let xml = r#"<?xml version="1.0"?>
<HWPML Version="2.8">
  <BODY><SECTION>
    <TABLE RowCount="1" ColCount="1">
      <ROW>
        <CELL ColAddr="0" RowAddr="0">
          <PARALIST>
            <P><TEXT><CHAR>안내</CHAR></TEXT>
              <TABLE RowCount="1" ColCount="1">
                <ROW><CELL ColAddr="0" RowAddr="0"><PARALIST><P><TEXT><CHAR>내부표</CHAR></TEXT></P></PARALIST></CELL></ROW>
              </TABLE>
            </P>
          </PARALIST>
        </CELL>
      </ROW>
    </TABLE>
  </SECTION></BODY>
</HWPML>"#;
        let doc = parse_hwpml_document(xml.as_bytes()).unwrap();
        let table = doc
            .blocks
            .iter()
            .find_map(|b| match b {
                IRBlock::Table(t) => Some(t),
                _ => None,
            })
            .expect("outer table present");
        assert!(table.cells[0][0].text.contains("안내"));
        assert!(table.cells[0][0].text.contains("내부표"));
    }
}
