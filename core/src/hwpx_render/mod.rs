// Ported from kkdoc (MIT): src/render/svg-render.ts (entry) + rasterize.ts
//! HWPX 레이아웃 보존 SVG 렌더러 — 한컴 조판 캐시(linesegarray)를 SVG 절대배치로
//! 그린다. 캐시 없는 문단은 `reflow` 옵션으로 합성 조판(reflow.rs) 후 렌더.
//!
//! 공개 API:
//! - [`render_hwpx_svg`] — 페이지별 자립 SVG 문자열(Vec<String>)
//! - [`render_hwpx_svg_detailed`] — 경고·통계 포함
//! - [`render_hwpx_png`] (feature `hwpx-render-png`) — 페이지별 PNG(resvg)

mod dom;
mod layout;
mod metrics;
mod reflow;
mod styles;
mod svg;

pub use metrics::WrapMode;
pub use styles::RenderStyles;
pub use svg::RenderStats;

use crate::utils::bounded_io::{read_limited, read_limited_to_string, MAX_HWPX_BINDATA, MAX_HWPX_XML};
use base64::Engine;
use std::collections::HashMap;
use std::io::Cursor;

#[derive(Debug, thiserror::Error)]
pub enum RenderError {
    #[error("HWPX(ZIP) 형식이 아닙니다 — 렌더는 HWPX만 지원")]
    NotZip,
    #[error("Contents/section0.xml 없음 — HWPX가 아니거나 손상됨")]
    NoSection,
    #[error("조판 캐시(linesegarray) 없음 — 한컴에서 저장한 HWPX만 렌더 가능 (reflow 옵션으로 합성 렌더 가능)")]
    NoCache,
    #[error("렌더할 구역이 없습니다 — HWPX가 손상되었을 수 있습니다")]
    NoRenderableSection,
    #[error("입출력 오류: {0}")]
    Io(#[from] std::io::Error),
    #[error("ZIP 오류: {0}")]
    Zip(#[from] zip::result::ZipError),
    #[error("{0}")]
    Other(String),
}

/// SVG 렌더 옵션
pub struct RenderOptions {
    /// 이미지 1장당 허용 최대 바이트 (기본 40MB)
    pub max_image_bytes: usize,
    /// 검색어 형광펜 (대소문자 무시)
    pub highlights: Vec<String>,
    /// Tier-2 reflow — 조판 캐시 없는 파일도 순수 조판으로 렌더 (기본 false)
    pub reflow: bool,
    /// reflow 줄바꿈 폴백 모드 (기본 Keep=어절)
    pub reflow_mode: WrapMode,
}

impl Default for RenderOptions {
    fn default() -> Self {
        RenderOptions { max_image_bytes: 40 * 1024 * 1024, highlights: Vec::new(), reflow: false, reflow_mode: WrapMode::Keep }
    }
}

/// 렌더 결과(상세)
pub struct RenderOutput {
    /// 페이지별 자립 SVG 문자열
    pub pages: Vec<String>,
    /// 최대 페이지 폭 (pt)
    pub width: f64,
    pub warnings: Vec<String>,
    pub stats: RenderStats,
}

const SVG_FONT_FAMILY: &str =
    "'HCR Batang','함초롬바탕','Hancom Batang',AppleMyungjo,'Noto Serif CJK KR',serif";

/// HWPX → 페이지별 레이아웃 보존 SVG 문자열.
pub fn render_hwpx_svg(input: &[u8], opts: &RenderOptions) -> Result<Vec<String>, RenderError> {
    render_hwpx_svg_detailed(input, opts).map(|o| o.pages)
}

/// HWPX → 페이지별 SVG + 경고·통계.
pub fn render_hwpx_svg_detailed(input: &[u8], opts: &RenderOptions) -> Result<RenderOutput, RenderError> {
    let mut archive = zip::ZipArchive::new(Cursor::new(input)).map_err(|_| RenderError::NotZip)?;
    let names: Vec<String> = archive.file_names().map(|s| s.to_string()).collect();

    let sec_re = regex::Regex::new(r"(?i)^Contents/section\d+\.xml$").unwrap();
    let mut sec_files: Vec<String> = names.iter().filter(|n| sec_re.is_match(n)).cloned().collect();
    sec_files.sort();
    if sec_files.is_empty() {
        return Err(RenderError::NoSection);
    }

    let mut warnings: Vec<String> = Vec::new();

    // header.xml (구버전 head.xml)
    let head_name = names
        .iter()
        .find(|n| n.as_str() == "Contents/header.xml")
        .or_else(|| names.iter().find(|n| n.as_str() == "Contents/head.xml"))
        .cloned();
    let styles = match &head_name {
        Some(name) => {
            let mut f = archive.by_name(name)?;
            let xml = read_limited_to_string(&mut f, MAX_HWPX_XML)?;
            styles::parse_render_styles(&xml)
        }
        None => {
            warnings.push("header.xml 없음 — 기본 스타일로 렌더".to_string());
            RenderStyles::default()
        }
    };

    // 구역 XML 선로딩
    let mut sec_xmls: Vec<String> = Vec::with_capacity(sec_files.len());
    for name in &sec_files {
        let mut f = archive.by_name(name)?;
        let xml = read_limited_to_string(&mut f, MAX_HWPX_XML)?;
        sec_xmls.push(xml);
    }

    // BinData 매니페스트 (content.hpf)
    let mut binmap: HashMap<String, String> = HashMap::new();
    if let Some(hpf_name) = names.iter().find(|n| n.to_lowercase().ends_with("content.hpf")).cloned() {
        let mut f = archive.by_name(&hpf_name)?;
        let man = read_limited_to_string(&mut f, MAX_HWPX_XML)?;
        let re1 = regex::Regex::new(r#"<[^>]*\bid="([^"]+)"[^>]*\bhref="(BinData/[^"]+)"[^>]*>"#).unwrap();
        for c in re1.captures_iter(&man) {
            binmap.insert(c[1].to_string(), c[2].to_string());
        }
        let re2 = regex::Regex::new(r#"<[^>]*\bhref="(BinData/[^"]+)"[^>]*\bid="([^"]+)"[^>]*>"#).unwrap();
        for c in re2.captures_iter(&man) {
            binmap.insert(c[2].to_string(), c[1].to_string());
        }
    }

    // 참조 이미지 로딩
    const MAX_IMAGE_REFS: usize = 256;
    const MAX_TOTAL_IMAGE_BYTES: usize = 128 * 1024 * 1024;
    let ref_re = regex::Regex::new(r#"binaryItemIDRef="([^"]+)""#).unwrap();
    let mut refs: Vec<String> = Vec::new();
    {
        let mut seen = std::collections::HashSet::new();
        for xml in &sec_xmls {
            for c in ref_re.captures_iter(xml) {
                let r = c[1].to_string();
                if seen.insert(r.clone()) {
                    refs.push(r);
                }
            }
        }
    }
    let mut images: HashMap<String, svg::LoadedImage> = HashMap::new();
    let mut total_img_bytes = 0usize;
    let refs_count = refs.len();
    for r in refs {
        if images.len() >= MAX_IMAGE_REFS {
            warnings.push(format!("이미지 {}종 중 {}종만 로딩 — 개수 한도 초과분 생략", refs_count, MAX_IMAGE_REFS));
            break;
        }
        let href = binmap.get(&r).cloned().or_else(|| {
            // 파일명 휴리스틱 폴백
            names.iter().find(|n| n.contains("BinData/") && n.contains(&r)).cloned()
        });
        let href = match href {
            Some(h) => h,
            None => continue,
        };
        let entry_name = if names.iter().any(|n| n == &href) {
            href.clone()
        } else {
            format!("Contents/{}", href)
        };
        let bytes = {
            let mut f = match archive.by_name(&entry_name) {
                Ok(f) => f,
                Err(_) => continue,
            };
            match read_limited(&mut f, MAX_HWPX_BINDATA) {
                Ok(b) => b,
                Err(_) => continue,
            }
        };
        if bytes.len() > opts.max_image_bytes {
            warnings.push(format!("이미지 {} {:.1}MB — 한도 초과로 생략", href, bytes.len() as f64 / 1048576.0));
            continue;
        }
        if total_img_bytes + bytes.len() > MAX_TOTAL_IMAGE_BYTES {
            warnings.push(format!("이미지 누적 {}MB 한도 초과 — 이후 생략", MAX_TOTAL_IMAGE_BYTES / 1048576));
            break;
        }
        total_img_bytes += bytes.len();
        let mime = sniff_mime(&href, &bytes);
        let data_uri = format!("data:{};base64,{}", mime, base64::engine::general_purpose::STANDARD.encode(&bytes));
        images.insert(r, svg::LoadedImage { data_uri, sym_id: None });
    }

    let highlights: Vec<String> =
        opts.highlights.iter().map(|s| s.trim().to_lowercase()).filter(|s| !s.is_empty()).collect();

    let mut ctx = svg::new_ctx(&styles, images, highlights);
    ctx.warnings = warnings;

    let cache_re = regex::Regex::new(r"<(?:[A-Za-z][\w.\-]*:)?linesegarray[\s/>]").unwrap();

    // 구역별 렌더 → (페이지 버퍼, PW, pageH)
    let mut rendered: Vec<(Vec<Vec<String>>, f64, f64)> = Vec::new();
    let mut no_cache_skipped = false;
    for (si, sec_xml) in sec_xmls.iter().enumerate() {
        let has_cache = cache_re.is_match(sec_xml);
        if !has_cache && !opts.reflow {
            no_cache_skipped = true;
            ctx.warnings.push(format!("구역 {}: 조판 캐시 없음 — reflow 옵션 필요, 생략", si));
            continue;
        }
        let doc = match roxmltree::Document::parse(sec_xml) {
            Ok(d) => d,
            Err(_) => {
                ctx.warnings.push(format!("구역 {} XML 파싱 실패 — 생략", si));
                continue;
            }
        };
        let root = doc.root_element();
        let geom = svg::read_section_geom(root);
        let (pages, page_h) = svg::render_section_to_pages(root, geom, &mut ctx, opts.reflow, opts.reflow_mode);
        rendered.push((pages, geom.pw, page_h));
    }

    if rendered.is_empty() {
        if no_cache_skipped {
            return Err(RenderError::NoCache);
        }
        return Err(RenderError::NoRenderableSection);
    }

    // 페이지별 자립 SVG 조립 — 공유 이미지 심볼 defs 를 각 페이지에 포함.
    let defs_joined = ctx.defs.join("");
    let mut pages_out: Vec<String> = Vec::new();
    let mut max_pw = 0.0_f64;
    for (pages, pw, page_h) in &rendered {
        max_pw = max_pw.max(*pw);
        for buf in pages {
            let body = buf.join("\n");
            let s = format!(
                concat!(
                    "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {vw} {vh}\" ",
                    "width=\"{vw}pt\" height=\"{vh}pt\" font-family=\"{ff}\" xml:space=\"preserve\">\n",
                    "<defs><clipPath id=\"pgclip\"><rect x=\"0\" y=\"0\" width=\"{vw}\" height=\"{vh}\"/></clipPath>{defs}</defs>\n",
                    "<rect width=\"{vw}\" height=\"{vh}\" fill=\"white\" stroke=\"#c9c7c4\" stroke-width=\"0.75\"/>\n",
                    "<g clip-path=\"url(#pgclip)\">\n{body}\n</g>\n</svg>"
                ),
                vw = svg::pt(*pw),
                vh = svg::pt(*page_h),
                ff = SVG_FONT_FAMILY,
                defs = defs_joined,
                body = body
            );
            pages_out.push(s);
        }
    }

    Ok(RenderOutput { pages: pages_out, width: (max_pw / 100.0).round(), warnings: ctx.warnings, stats: ctx.stats })
}

/// 순수 로직 테스트용 얇은 래퍼 (내부 서브모듈은 pub 아님).
#[doc(hidden)]
pub mod testonly {
    pub fn pt(u: f64) -> String {
        super::svg::pt(u)
    }
    pub fn to_int32(s: Option<&str>, fallback: f64) -> f64 {
        super::layout::to_int32(s, fallback)
    }
    pub fn measure_hangul(text: &str, height: f64, ratio: f64) -> f64 {
        super::metrics::measure_text_width(text, height, ratio, &super::metrics::MeasureOptions::default())
    }
    pub fn wrap_lines(text: &str, first_w: f64, cont_w: f64, height: f64, ratio: f64) -> usize {
        super::metrics::simulate_wrap(
            text,
            first_w,
            cont_w,
            height,
            ratio,
            super::metrics::WrapMode::Keep,
            &super::metrics::MeasureOptions::default(),
        )
        .lines
    }
}

fn sniff_mime(name: &str, bytes: &[u8]) -> &'static str {
    let lower = name.to_lowercase();
    if lower.ends_with(".png") || (bytes.len() > 4 && bytes[0] == 0x89 && bytes[1] == 0x50) {
        return "image/png";
    }
    if lower.ends_with(".bmp") || (bytes.len() > 2 && bytes[0] == 0x42 && bytes[1] == 0x4d) {
        return "image/bmp";
    }
    if lower.ends_with(".gif") || (bytes.len() > 3 && bytes[0] == 0x47 && bytes[1] == 0x49 && bytes[2] == 0x46) {
        return "image/gif";
    }
    if lower.ends_with(".svg") {
        return "image/svg+xml";
    }
    if bytes.len() > 3 && bytes[0] == 0xff && bytes[1] == 0xd8 && bytes[2] == 0xff {
        return "image/jpeg";
    }
    "image/jpeg"
}

/// HWPX → 페이지별 PNG (resvg 래스터). 시스템 폰트 DB 로 텍스트 렌더.
#[cfg(feature = "hwpx-render-png")]
pub fn render_hwpx_png(input: &[u8], opts: &RenderOptions, scale: f32) -> Result<Vec<Vec<u8>>, RenderError> {
    use resvg::tiny_skia;
    use resvg::usvg;

    let pages = render_hwpx_svg(input, opts)?;
    let mut fontdb = usvg::fontdb::Database::new();
    fontdb.load_system_fonts();
    let uopts = usvg::Options { fontdb: std::sync::Arc::new(fontdb), ..Default::default() };

    let mut out = Vec::with_capacity(pages.len());
    for svg_str in &pages {
        let tree = usvg::Tree::from_str(svg_str, &uopts).map_err(|e| RenderError::Other(format!("SVG 파싱 실패: {}", e)))?;
        let size = tree.size();
        let w = (size.width() * scale).ceil().max(1.0) as u32;
        let h = (size.height() * scale).ceil().max(1.0) as u32;
        let mut pixmap = tiny_skia::Pixmap::new(w, h).ok_or_else(|| RenderError::Other("Pixmap 생성 실패".to_string()))?;
        let transform = tiny_skia::Transform::from_scale(scale, scale);
        resvg::render(&tree, transform, &mut pixmap.as_mut());
        let png = pixmap.encode_png().map_err(|e| RenderError::Other(format!("PNG 인코딩 실패: {}", e)))?;
        out.push(png);
    }
    Ok(out)
}
