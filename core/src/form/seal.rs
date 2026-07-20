// Ported from kkdoc (MIT): src/form/seal.ts
//! 도장/서명 이미지의 부유(글 앞) 배치 — 앵커 문구 또는 절대 좌표 기준으로
//! HWPX section XML에 `<hp:pic>` 부유 개체를 삽입하고, 이미지 바이너리를
//! `BinData/imageN.png` 엔트리로 ZIP에 추가한다. 대상 section XML 1개(+선택적
//! manifest)만 스플라이스하고, 그 외 모든 ZIP 엔트리는 바이트 단위로 보존된다
//! (교체 없이 신규 엔트리 추가만 수행).
//!
//! kkdoc 원본(seal.ts) 대비 단순화한 부분 (판단 필요 지점 — 보고 참고):
//! - 정렬(가운데/오른쪽 문단)·셀 사용가능폭 기반 "auto" 모드 미구현 — 앵커
//!   모드는 항상 "문구 오른쪽 2mm" 배치만 지원, dx_mm/dy_mm으로 수동 보정.
//! - 셀 좌측 오프셋·중첩표 바깥 셀 체인 미구현 — 표 셀 안의 앵커는 위치가
//!   근사값일 수 있다(dx_mm 보정 필요).
//! - 이미지 포맷은 PNG만 지원 (원본은 png/jpg/bmp/gif).
//! - 네임스페이스 프리픽스는 폼 모듈 전체 관례를 따라 "hp:" 고정.

use std::collections::{HashMap, HashSet};
use std::io::{Cursor, Read};

use lazy_static::lazy_static;
use regex::Regex;
use zip::ZipArchive;

use super::scan::{scan_section, xml_unescape, Para, Scan, TextRun};
use super::zip_patch::patch_zip_entries_with_additions;

const HU_PER_MM: f64 = 7200.0 / 25.4;

fn mm2hu(mm: f64) -> i64 {
    (mm * HU_PER_MM).round() as i64
}

/// 도장 배치 위치 지정 방식.
#[derive(Debug, Clone)]
pub enum SealAnchor {
    /// 앵커 문구(예: "(인)", "서명 또는 인") 기준 — 문구 오른쪽에 배치.
    Text { text: String, occurrence: usize },
    /// 섹션 내 절대 좌표(mm, 용지 좌상단 기준 — `horzRelTo`/`vertRelTo`="PAPER").
    Absolute { section_index: usize, x_mm: f64, y_mm: f64 },
}

/// [`place_seal_hwpx`] 옵션.
#[derive(Debug, Clone)]
pub struct SealOptions {
    pub anchor: SealAnchor,
    /// 도장 한 변 크기(mm).
    pub size_mm: f64,
    /// 불투명도 — 0=완전 불투명 .. 100=완전 투명. `hc:img`의 `alpha` 속성에 매핑.
    pub opacity: u8,
    /// 미세조정(mm).
    pub dx_mm: f64,
    pub dy_mm: f64,
}

impl Default for SealOptions {
    fn default() -> Self {
        Self {
            anchor: SealAnchor::Absolute { section_index: 0, x_mm: 0.0, y_mm: 0.0 },
            size_mm: 15.0,
            opacity: 0,
            dx_mm: 0.0,
            dy_mm: 0.0,
        }
    }
}

fn is_png(buf: &[u8]) -> bool {
    buf.len() >= 8 && buf[0..8] == [0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]
}

/// 코드 포인트 하나의 시각 폭(em) — CJK/한글=1.0, ASCII·반각=0.5, 제어문자=0.
fn glyph_em(ch: char) -> f64 {
    let code = ch as u32;
    if code < 0x20 {
        0.0
    } else if code <= 0x7e || (0xff61..=0xffdc).contains(&code) {
        0.5 // ASCII·반각 가나/자모
    } else {
        1.0
    }
}

fn measure_mm(text: &str, em_mm: f64) -> f64 {
    text.chars().map(|c| glyph_em(c) * em_mm).sum()
}

lazy_static! {
    static ref SECTION_RE: Regex = Regex::new(r"(?i)section\d+\.xml$").unwrap();
    static ref SECTION_NUM_RE: Regex = Regex::new(r"(\d+)\.xml$").unwrap();
    static ref HPF_RE: Regex = Regex::new(r"(?i)\.hpf$").unwrap();
    static ref HEADER_RE: Regex = Regex::new(r"(?i)(^|/)header\.xml$").unwrap();
    static ref BINDATA_NUM_RE: Regex = Regex::new(r"(?i)^BinData/(?:image|img)(\d+)\.").unwrap();
    static ref MANIFEST_ID_RE: Regex = Regex::new(r#"<opf:item\b[^>]*\bid="([^"]+)""#).unwrap();
    static ref ID_SCAN_RE: Regex = Regex::new(r#"\b(?:id|instid)="(\d+)""#).unwrap();
    static ref CHARPR_TAG_RE: Regex = Regex::new(r"<[A-Za-z0-9]+:charPr\b[^>]*>").unwrap();
    static ref ID_ATTR_RE: Regex = Regex::new(r#"\bid="([^"]*)""#).unwrap();
    static ref HEIGHT_ATTR_RE: Regex = Regex::new(r#"\bheight="([^"]*)""#).unwrap();
    static ref CHARPR_IDREF_RE: Regex = Regex::new(r#"\bcharPrIDRef="([^"]*)""#).unwrap();
}

/// header.xml에서 charPrIDRef → height(1/100pt) 매핑 파싱 (폰트 크기 메트릭용).
fn parse_char_pr_heights(header_xml: &str) -> HashMap<String, u32> {
    let mut map = HashMap::new();
    for m in CHARPR_TAG_RE.find_iter(header_xml) {
        let tag = m.as_str();
        if let (Some(id), Some(h)) = (ID_ATTR_RE.captures(tag), HEIGHT_ATTR_RE.captures(tag)) {
            if let Ok(height) = h[1].parse::<u32>() {
                map.insert(id[1].to_string(), height);
            }
        }
    }
    map
}

/// 압축해제 시 ZIP 엔트리 하나당 허용하는 최대 바이트 수 — 악의적 HWPX(zip bomb:
/// 작은 압축 크기로 문자열을 무제한 팽창시켜 메모리를 고갈)로부터 header/manifest/
/// section XML 읽기를 보호한다. 실무 정부 서식 HWPX의 개별 XML 파트는 통상
/// 수 MB를 넘지 않으므로 32MiB는 충분히 넉넉한 상한이다.
const MAX_ENTRY_SIZE: u64 = 32 * 1024 * 1024;

fn read_zip_text(zip: &mut ZipArchive<Cursor<Vec<u8>>>, name: &str) -> Result<String, String> {
    let file = zip.by_name(name).map_err(|e| e.to_string())?;
    // 1차 방어: central directory가 선언한 압축해제 크기 — 정직한 zip bomb(선언된
    // 크기 자체가 거대한 경우)은 실제로 압축을 풀기 전에 여기서 걸러낸다.
    if file.size() > MAX_ENTRY_SIZE {
        return Err(format!(
            "place_seal_hwpx: ZIP 엔트리가 너무 큽니다: {name} (선언된 크기 {}바이트 > 허용 {}바이트) — zip bomb 의심",
            file.size(),
            MAX_ENTRY_SIZE
        ));
    }
    // 2차 방어: 선언된 크기를 신뢰하지 않고 실제 디코딩 바이트 수를 상한 너머까지
    // 읽어 초과 여부를 확인한다 (central directory가 실제보다 작은 크기를 거짓
    // 신고하는 경우까지 커버).
    let mut buf = Vec::new();
    file.take(MAX_ENTRY_SIZE + 1).read_to_end(&mut buf).map_err(|e| e.to_string())?;
    if buf.len() as u64 > MAX_ENTRY_SIZE {
        return Err(format!(
            "place_seal_hwpx: ZIP 엔트리 압축해제 크기가 허용치({MAX_ENTRY_SIZE}바이트)를 초과했습니다: {name} — zip bomb 의심"
        ));
    }
    String::from_utf8(buf).map_err(|e| e.to_string())
}

/// 앵커 탐색용 문단 하나(본문 또는 표 셀). 문서 순서 근사(첫 run 시작 offset)로 정렬.
struct Site<'a> {
    para: &'a Para,
}

fn collect_sites(scan: &Scan) -> Vec<Site<'_>> {
    fn order_of(p: &Para) -> usize {
        p.runs.first().map(|r| r.start).or(p.first_run_open_end).unwrap_or(usize::MAX)
    }
    let mut ordered: Vec<(usize, &Para)> = Vec::new();
    for p in &scan.body_paras {
        ordered.push((order_of(p), p));
    }
    for table in &scan.tables {
        for cell in &table.cells {
            for p in &cell.paras {
                ordered.push((order_of(p), p));
            }
        }
    }
    ordered.sort_by_key(|(o, _)| *o);
    ordered.into_iter().map(|(_, para)| Site { para }).collect()
}

/// idx_in_text(문단 텍스트 내 바이트 오프셋)를 포함하는 run을 찾는다.
/// 정확히 포함하는 run이 없으면(엔티티·run 경계에 걸친 앵커) 마지막 run으로 폴백.
fn locate_containing_run<'a>(xml: &str, para: &'a Para, idx_in_text: usize) -> Option<&'a TextRun> {
    let mut cum = 0usize;
    for run in &para.runs {
        let decoded = xml_unescape(&xml[run.start..run.end]);
        let len = decoded.len();
        if idx_in_text < cum + len {
            return Some(run);
        }
        cum += len;
    }
    para.runs.last()
}

/// run이 속한 `<hp:run ...>` 여는 태그의 charPrIDRef와, 그 run을 감싼
/// `</hp:run>` 닫는 위치(신규 형제 run 삽입 지점) 를 찾는다.
fn anchor_run_info(xml: &str, run: &TextRun) -> Option<(String, usize)> {
    let before = &xml[..run.start];
    let open_start = before.rfind("<hp:run")?;
    let gt_rel = xml[open_start..].find('>')?;
    let open_end = open_start + gt_rel + 1;
    let open_tag = &xml[open_start..open_end];
    let char_pr = CHARPR_IDREF_RE.captures(open_tag).map(|c| c[1].to_string()).unwrap_or_else(|| "0".to_string());
    let close_rel = xml[run.end..].find("</hp:run>")?;
    let close_at = run.end + close_rel;
    let insert_at = close_at + "</hp:run>".len();
    Some((char_pr, insert_at))
}

/// 부유(글 앞) `<hp:pic>` XML — claw-hwp/kkdoc buildPic 템플릿의 float 변형.
#[allow(clippy::too_many_arguments)]
fn build_float_pic_xml(
    item_id: &str,
    size_hu: i64,
    pos_x_hu: i64,
    pos_y_hu: i64,
    id: u64,
    instid: u64,
    horz_rel_to: &str,
    vert_rel_to: &str,
    horz_align: &str,
    vert_align: &str,
    alpha: u8,
) -> String {
    let w = size_hu;
    let h = size_hu;
    format!(
        "<hp:pic xmlns:hc=\"http://www.hancom.co.kr/hwpml/2011/core\" id=\"{id}\" zOrder=\"0\" numberingType=\"PICTURE\" textWrap=\"IN_FRONT_OF_TEXT\" textFlow=\"BOTH_SIDES\" lock=\"0\" dropcapstyle=\"None\" href=\"\" groupLevel=\"0\" instid=\"{instid}\" reverse=\"0\">\
<hp:offset x=\"0\" y=\"0\"/><hp:orgSz width=\"{w}\" height=\"{h}\"/><hp:curSz width=\"{w}\" height=\"{h}\"/>\
<hp:flip horizontal=\"0\" vertical=\"0\"/><hp:rotationInfo angle=\"0\" centerX=\"{cx}\" centerY=\"{cy}\" rotateimage=\"1\"/>\
<hp:renderingInfo><hc:transMatrix e1=\"1\" e2=\"0\" e3=\"0\" e4=\"0\" e5=\"1\" e6=\"0\"/><hc:scaMatrix e1=\"1\" e2=\"0\" e3=\"0\" e4=\"0\" e5=\"1\" e6=\"0\"/><hc:rotMatrix e1=\"1\" e2=\"0\" e3=\"0\" e4=\"0\" e5=\"1\" e6=\"0\"/></hp:renderingInfo>\
<hp:imgRect><hc:pt0 x=\"0\" y=\"0\"/><hc:pt1 x=\"{w}\" y=\"0\"/><hc:pt2 x=\"{w}\" y=\"{h}\"/><hc:pt3 x=\"0\" y=\"{h}\"/></hp:imgRect>\
<hp:imgClip left=\"0\" right=\"{w}\" top=\"0\" bottom=\"{h}\"/><hp:inMargin left=\"0\" right=\"0\" top=\"0\" bottom=\"0\"/>\
<hp:imgDim dimwidth=\"{w}\" dimheight=\"{h}\"/>\
<hc:img binaryItemIDRef=\"{item_id}\" bright=\"0\" contrast=\"0\" effect=\"REAL_PIC\" alpha=\"{alpha}\"/><hp:effects/>\
<hp:sz width=\"{w}\" widthRelTo=\"ABSOLUTE\" height=\"{h}\" heightRelTo=\"ABSOLUTE\" protect=\"0\"/>\
<hp:pos treatAsChar=\"0\" affectLSpacing=\"0\" flowWithText=\"0\" allowOverlap=\"1\" holdAnchorAndSO=\"0\" vertRelTo=\"{vert_rel_to}\" horzRelTo=\"{horz_rel_to}\" vertAlign=\"{vert_align}\" horzAlign=\"{horz_align}\" vertOffset=\"{pos_y_hu}\" horzOffset=\"{pos_x_hu}\"/>\
<hp:outMargin left=\"0\" right=\"0\" top=\"0\" bottom=\"0\"/><hp:shapeComment>mdm seal</hp:shapeComment>\
</hp:pic>",
        id = id,
        instid = instid,
        w = w,
        h = h,
        cx = w / 2,
        cy = h / 2,
        item_id = item_id,
        alpha = alpha,
        vert_rel_to = vert_rel_to,
        horz_rel_to = horz_rel_to,
        vert_align = vert_align,
        horz_align = horz_align,
        pos_y_hu = pos_y_hu,
        pos_x_hu = pos_x_hu,
    )
}

/// HWPX에 도장/서명 PNG를 부유 개체로 배치한다.
///
/// 앵커 문구 기준(`SealAnchor::Text`) 또는 섹션 내 절대 좌표(`SealAnchor::Absolute`)
/// 로 위치를 지정한다. 이미지는 `BinData/imageN.png` 엔트리로 추가되고, 대상
/// section XML에 `<hp:pic>` 부유 개체를 담은 신규 `<hp:run>`이 삽입된다 —
/// 그 외 모든 ZIP 엔트리(기존 바이너리·서식·스타일)는 1바이트도 바뀌지 않는다.
pub fn place_seal_hwpx(hwpx: &[u8], seal_png: &[u8], opts: &SealOptions) -> Result<Vec<u8>, String> {
    if seal_png.is_empty() {
        return Err("place_seal_hwpx: 도장 이미지가 비어 있습니다".into());
    }
    if !is_png(seal_png) {
        return Err("place_seal_hwpx: 이미지가 PNG 형식이 아닙니다 (매직바이트 불일치)".into());
    }
    if !opts.size_mm.is_finite() || opts.size_mm <= 0.0 {
        return Err("place_seal_hwpx: size_mm은 양수여야 합니다".into());
    }

    let mut zip = ZipArchive::new(Cursor::new(hwpx.to_vec())).map_err(|e| e.to_string())?;
    let names: Vec<String> = (0..zip.len())
        .filter_map(|i| zip.by_index(i).ok().map(|f| f.name().to_string()))
        .collect();

    let mut section_paths: Vec<String> = names.iter().filter(|n| SECTION_RE.is_match(n)).cloned().collect();
    section_paths.sort_by_key(|n| {
        SECTION_NUM_RE.captures(n).and_then(|c| c[1].parse::<u64>().ok()).unwrap_or(0)
    });
    if section_paths.is_empty() {
        return Err("place_seal_hwpx: HWPX에서 섹션 파일을 찾을 수 없습니다".into());
    }

    let manifest_path = names.iter().find(|n| HPF_RE.is_match(n)).cloned();
    let header_path = names.iter().find(|n| HEADER_RE.is_match(n)).cloned();

    let char_pr_heights = match &header_path {
        Some(hp) => parse_char_pr_heights(&read_zip_text(&mut zip, hp)?),
        None => HashMap::new(),
    };

    let mut section_xmls: Vec<String> = Vec::with_capacity(section_paths.len());
    for p in &section_paths {
        section_xmls.push(read_zip_text(&mut zip, p)?);
    }

    // 기존 BinData 번호·manifest id와 충돌하지 않는 다음 이미지 번호
    let mut used_ids: HashSet<String> = HashSet::new();
    let mut manifest_xml = String::new();
    if let Some(mp) = &manifest_path {
        manifest_xml = read_zip_text(&mut zip, mp)?;
        for m in MANIFEST_ID_RE.captures_iter(&manifest_xml) {
            used_ids.insert(m[1].to_string());
        }
    }
    let mut used_image_nums: HashSet<u64> = HashSet::new();
    for n in &names {
        if let Some(c) = BINDATA_NUM_RE.captures(n) {
            if let Ok(v) = c[1].parse::<u64>() {
                used_image_nums.insert(v);
            }
        }
    }
    let mut next_image_num = 1u64;
    while used_image_nums.contains(&next_image_num) || used_ids.contains(&format!("image{next_image_num}")) {
        next_image_num += 1;
    }

    // 개체 id — 문서 내 기존 숫자 id 최댓값 다음부터 (충돌 방지)
    let mut max_id: u64 = 1_000_000;
    for xml in &section_xmls {
        for c in ID_SCAN_RE.captures_iter(xml) {
            if let Ok(v) = c[1].parse::<u64>() {
                if v > max_id {
                    max_id = v;
                }
            }
        }
    }

    let (target_si, char_pr, insert_at, pos_x_mm, pos_y_mm): (usize, String, usize, f64, f64) =
        match &opts.anchor {
            SealAnchor::Text { text, occurrence } => {
                if text.is_empty() {
                    return Err("place_seal_hwpx: anchor 문구가 필요합니다".into());
                }
                let scans: Vec<Scan> = section_xmls.iter().map(|x| scan_section(x)).collect();
                let sites_by_section: Vec<Vec<Site>> = scans.iter().map(collect_sites).collect();

                let mut found: Option<(usize, &Para, usize)> = None;
                let mut total = 0usize;
                'outer: for (si, sites) in sites_by_section.iter().enumerate() {
                    for site in sites {
                        let mut from = 0usize;
                        while let Some(rel) = site.para.text[from..].find(text.as_str()) {
                            let abs = from + rel;
                            if total == *occurrence {
                                found = Some((si, site.para, abs));
                                break 'outer;
                            }
                            total += 1;
                            from = abs + text.len();
                        }
                    }
                }

                let (si, para, idx_in_text) = match found {
                    Some(v) => v,
                    None => {
                        let mut total2 = 0usize;
                        for sites in &sites_by_section {
                            for site in sites {
                                let mut from = 0usize;
                                while let Some(rel) = site.para.text[from..].find(text.as_str()) {
                                    total2 += 1;
                                    from += rel + text.len();
                                }
                            }
                        }
                        return Err(format!(
                            "place_seal_hwpx: 앵커 \"{text}\" {occurrence}번째 등장을 찾지 못했습니다 (본문 내 {total2}회 등장 — occurrence 0..{})",
                            total2.saturating_sub(1)
                        ));
                    }
                };

                let xml = &section_xmls[si];
                let run = locate_containing_run(xml, para, idx_in_text)
                    .ok_or_else(|| format!("place_seal_hwpx: 앵커 \"{text}\" 문단에서 run을 찾지 못했습니다"))?;
                let (char_pr, insert_at) = anchor_run_info(xml, run)
                    .ok_or_else(|| format!("place_seal_hwpx: 앵커 \"{text}\" 문단에서 run 경계를 찾지 못했습니다"))?;

                let height = char_pr_heights.get(&char_pr).copied().unwrap_or(1000);
                let font_pt = height as f64 / 100.0;
                let em_mm = font_pt * 25.4 / 72.0;
                let line_h_mm = em_mm;
                let start_x_mm = measure_mm(&para.text[..idx_in_text], em_mm);
                let anchor_w_mm = measure_mm(text, em_mm);
                let pos_x_mm = start_x_mm + anchor_w_mm + 2.0 + opts.dx_mm;
                let pos_y_mm = -(opts.size_mm - line_h_mm) / 2.0 + opts.dy_mm;

                (si, char_pr, insert_at, pos_x_mm, pos_y_mm)
            }
            SealAnchor::Absolute { section_index, x_mm, y_mm } => {
                let si = *section_index;
                if si >= section_xmls.len() {
                    return Err(format!(
                        "place_seal_hwpx: section_index {si} 범위 초과 (섹션 {}개)",
                        section_xmls.len()
                    ));
                }
                let xml = &section_xmls[si];
                let scan = scan_section(xml);
                let sites = collect_sites(&scan);
                let first =
                    sites.first().ok_or_else(|| "place_seal_hwpx: 앵커로 쓸 문단이 없습니다".to_string())?;
                let run = first.para.runs.first().ok_or_else(|| {
                    "place_seal_hwpx: 앵커 문단에 텍스트 run이 없습니다 (빈 문단은 미지원)".to_string()
                })?;
                let (char_pr, insert_at) = anchor_run_info(xml, run)
                    .ok_or_else(|| "place_seal_hwpx: 앵커 문단에서 run 경계를 찾지 못했습니다".to_string())?;
                (si, char_pr, insert_at, *x_mm + opts.dx_mm, *y_mm + opts.dy_mm)
            }
        };

    let entry = format!("BinData/image{next_image_num}.png");
    let item_id = format!("image{next_image_num}");
    let id = max_id + 1;
    let instid = max_id + 2;

    let (horz_rel_to, vert_rel_to) = match &opts.anchor {
        SealAnchor::Text { .. } => ("COLUMN", "PARA"),
        SealAnchor::Absolute { .. } => ("PAPER", "PAPER"),
    };

    let pic_xml = build_float_pic_xml(
        &item_id,
        mm2hu(opts.size_mm),
        mm2hu(pos_x_mm),
        mm2hu(pos_y_mm),
        id,
        instid,
        horz_rel_to,
        vert_rel_to,
        "LEFT",
        "TOP",
        opts.opacity.min(100),
    );
    let run_xml = format!(r#"<hp:run charPrIDRef="{char_pr}">{pic_xml}</hp:run>"#);

    let mut section_xml = section_xmls[target_si].clone();
    section_xml.insert_str(insert_at, &run_xml);

    let mut replacements: HashMap<String, Vec<u8>> = HashMap::new();
    replacements.insert(section_paths[target_si].clone(), section_xml.into_bytes());

    let mut additions: HashMap<String, Vec<u8>> = HashMap::new();
    additions.insert(entry.clone(), seal_png.to_vec());

    if let Some(mp) = &manifest_path {
        if manifest_xml.contains("</opf:manifest>") {
            let item = format!(r#"<opf:item id="{item_id}" href="{entry}" media-type="image/png" isEmbeded="1"/>"#);
            let patched = manifest_xml.replacen("</opf:manifest>", &format!("{item}</opf:manifest>"), 1);
            replacements.insert(mp.clone(), patched.into_bytes());
        }
    }

    patch_zip_entries_with_additions(hwpx, &replacements, &additions)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use zip::write::SimpleFileOptions;

    /// 1x1 투명 PNG (유효한 최소 PNG 바이트).
    fn tiny_png() -> Vec<u8> {
        vec![
            0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52, 0x00,
            0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1f, 0x15, 0xc4, 0x89, 0x00,
            0x00, 0x00, 0x0a, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9c, 0x63, 0x00, 0x01, 0x00, 0x00, 0x05, 0x00, 0x01,
            0x0d, 0x0a, 0x2d, 0xb4, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
        ]
    }

    fn header_xml() -> &'static str {
        r#"<?xml version="1.0"?><hh:head xmlns:hh="http://www.hancom.co.kr/hwpml/2011/head"><hh:refList><hh:charProperties><hh:charPr id="0" height="1000"/></hh:charProperties></hh:refList></hh:head>"#
    }

    fn manifest_xml() -> &'static str {
        r#"<?xml version="1.0"?><opf:package xmlns:opf="http://www.idpf.org/2007/opf"><opf:manifest><opf:item id="header" href="Contents/header.xml" media-type="application/xml"/></opf:manifest></opf:package>"#
    }

    fn section_with_anchor() -> String {
        r#"<?xml version="1.0" encoding="UTF-8"?><hp:sec xmlns:hp="http://www.hancom.co.kr/hwpml/2011/paragraph"><hp:p id="0" paraPrIDRef="0"><hp:run charPrIDRef="0"><hp:t>담당자 확인 (인)</hp:t></hp:run></hp:p></hp:sec>"#
            .to_string()
    }

    fn build_hwpx(section: &str, extra: Option<(&str, &[u8])>) -> Vec<u8> {
        let mut buf = Vec::new();
        {
            let mut zw = zip::ZipWriter::new(Cursor::new(&mut buf));
            let stored = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
            let deflated = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

            zw.start_file("mimetype", stored).unwrap();
            zw.write_all(b"application/hwp+zip").unwrap();

            zw.start_file("version.xml", deflated).unwrap();
            zw.write_all(br#"<?xml version="1.0"?><hv:HCFVersion/>"#).unwrap();

            zw.start_file("Contents/header.xml", deflated).unwrap();
            zw.write_all(header_xml().as_bytes()).unwrap();

            zw.start_file("Contents/content.hpf", deflated).unwrap();
            zw.write_all(manifest_xml().as_bytes()).unwrap();

            if let Some((name, data)) = extra {
                zw.start_file(name, stored).unwrap();
                zw.write_all(data).unwrap();
            }

            zw.start_file("Contents/section0.xml", deflated).unwrap();
            zw.write_all(section.as_bytes()).unwrap();

            zw.finish().unwrap();
        }
        buf
    }

    fn read_entry(bytes: &[u8], name: &str) -> Vec<u8> {
        let mut zip = ZipArchive::new(Cursor::new(bytes.to_vec())).unwrap();
        let mut f = zip.by_name(name).unwrap();
        let mut out = Vec::new();
        f.read_to_end(&mut out).unwrap();
        out
    }

    #[test]
    fn seal_anchor_mode_inserts_pic_and_preserves_other_entries() {
        let bytes = build_hwpx(&section_with_anchor(), None);
        let png = tiny_png();
        let opts = SealOptions {
            anchor: SealAnchor::Text { text: "(인)".to_string(), occurrence: 0 },
            size_mm: 12.0,
            opacity: 0,
            dx_mm: 0.0,
            dy_mm: 0.0,
        };
        let out = place_seal_hwpx(&bytes, &png, &opts).expect("seal placed");

        // re-opens as a valid zip
        let mut zip = ZipArchive::new(Cursor::new(out.clone())).expect("valid zip");
        assert!(zip.by_name("mimetype").is_ok());

        // section now carries the floating pic
        let section = String::from_utf8(read_entry(&out, "Contents/section0.xml")).unwrap();
        assert!(section.contains("<hp:pic"), "pic inserted, got: {section}");
        assert!(section.contains("horzRelTo=\"COLUMN\""));
        assert!(section.contains("담당자 확인 (인)"), "original text untouched");

        // image part added, byte-identical to input PNG
        let img = read_entry(&out, "BinData/image1.png");
        assert_eq!(img, png);

        // manifest gained the new opf:item
        let manifest = String::from_utf8(read_entry(&out, "Contents/content.hpf")).unwrap();
        assert!(manifest.contains(r#"href="BinData/image1.png""#));

        // every pre-existing entry stays byte-identical
        let before = super::super::zip_patch::read_zip_entries(&bytes).unwrap();
        let after = super::super::zip_patch::read_zip_entries(&out).unwrap();
        for (name, (method, data)) in &before {
            if name.ends_with("section0.xml") || name.ends_with("content.hpf") {
                continue; // the two entries we intend to change
            }
            let (m2, d2) = after.get(name).expect("entry survives");
            assert_eq!(method, m2, "method preserved for {name}");
            assert_eq!(data, d2, "bytes preserved for {name}");
        }
        let _ = zip.by_name("Contents/header.xml").unwrap(); // sanity: still readable
    }

    #[test]
    fn seal_absolute_mode_uses_paper_relative_pos() {
        let bytes = build_hwpx(&section_with_anchor(), None);
        let png = tiny_png();
        let opts = SealOptions {
            anchor: SealAnchor::Absolute { section_index: 0, x_mm: 50.0, y_mm: 30.0 },
            size_mm: 15.0,
            opacity: 20,
            dx_mm: 0.0,
            dy_mm: 0.0,
        };
        let out = place_seal_hwpx(&bytes, &png, &opts).expect("seal placed");
        let section = String::from_utf8(read_entry(&out, "Contents/section0.xml")).unwrap();
        assert!(section.contains("horzRelTo=\"PAPER\""));
        assert!(section.contains("vertRelTo=\"PAPER\""));
        assert!(section.contains("alpha=\"20\""));
        // 50mm ≈ 14173 hu, 30mm ≈ 8504 hu
        assert!(section.contains("horzOffset=\"14173\""), "got: {section}");
        assert!(section.contains("vertOffset=\"8504\""), "got: {section}");
    }

    #[test]
    fn seal_anchor_not_found_errors_with_occurrence_count() {
        let bytes = build_hwpx(&section_with_anchor(), None);
        let png = tiny_png();
        let opts = SealOptions {
            anchor: SealAnchor::Text { text: "없는문구".to_string(), occurrence: 0 },
            ..SealOptions::default()
        };
        let err = place_seal_hwpx(&bytes, &png, &opts).unwrap_err();
        assert!(err.contains("찾지 못했습니다"), "got: {err}");
        assert!(err.contains("0회 등장"), "got: {err}");
    }

    #[test]
    fn seal_rejects_non_png_bytes() {
        let bytes = build_hwpx(&section_with_anchor(), None);
        let opts = SealOptions::default();
        let err = place_seal_hwpx(&bytes, b"not a png", &opts).unwrap_err();
        assert!(err.contains("PNG"), "got: {err}");
    }

    #[test]
    fn seal_multiple_placements_increment_image_number() {
        let bytes = build_hwpx(&section_with_anchor(), None);
        let png = tiny_png();
        let opts = SealOptions {
            anchor: SealAnchor::Text { text: "(인)".to_string(), occurrence: 0 },
            ..SealOptions::default()
        };
        let once = place_seal_hwpx(&bytes, &png, &opts).unwrap();
        let twice = place_seal_hwpx(&once, &png, &opts).unwrap();
        assert!(read_entry(&twice, "BinData/image1.png") == png);
        assert!(read_entry(&twice, "BinData/image2.png") == png);
    }

    /// zip bomb 재현 — section0.xml의 선언된(그리고 실제) 압축해제 크기가
    /// MAX_ENTRY_SIZE를 넘으면, 신뢰불가 HWPX를 통째로 String에 담아 메모리를
    /// 고갈시키기 전에 place_seal_hwpx가 명시적으로 거부해야 한다.
    #[test]
    fn seal_rejects_zip_bomb_section_entry() {
        let huge = vec![0u8; (MAX_ENTRY_SIZE + 1024) as usize];
        let mut buf = Vec::new();
        {
            let mut zw = zip::ZipWriter::new(Cursor::new(&mut buf));
            let stored = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
            let deflated = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

            zw.start_file("mimetype", stored).unwrap();
            zw.write_all(b"application/hwp+zip").unwrap();

            zw.start_file("Contents/header.xml", deflated).unwrap();
            zw.write_all(header_xml().as_bytes()).unwrap();

            // 전부 0으로 채운 32MiB+ 데이터 — deflate로는 수 KB로 압축되지만
            // 선언된/실제 압축해제 크기는 상한을 넘는다 (고전적 zip bomb 형태).
            zw.start_file("Contents/section0.xml", deflated).unwrap();
            zw.write_all(&huge).unwrap();

            zw.finish().unwrap();
        }

        let png = tiny_png();
        let opts = SealOptions::default();
        let err = place_seal_hwpx(&buf, &png, &opts).unwrap_err();
        assert!(err.contains("zip bomb") || err.contains("초과") || err.contains("너무 큽니다"), "got: {err}");
    }
}
