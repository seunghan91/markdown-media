//! Annex/Form parser for Korean legal documents
//!
//! 법률 문서의 별표(Annex), 별지(Form), 첨부(Attachment) 감지 및 파싱

use serde::{Deserialize, Serialize};

use crate::hwp::parser::TableData;
use super::patterns::{RE_ANNEX, RE_ANNEX_FORM, RE_ATTACHMENT};

/// 별표/별지 유형
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnnexType {
    Annex,
    Form,
    Attachment,
}

/// 별표/별지 정보
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnnexInfo {
    pub annex_type: AnnexType,
    pub number: u32,
    pub sub_number: Option<u32>,
    pub title: String,
    pub tables: Vec<TableData>,
    pub raw_content: String,
    pub markdown: String,
}

/// 텍스트 내 별표 영역 (시작/끝 라인)
#[derive(Debug, Clone)]
pub struct AnnexRegion {
    pub annex_type: AnnexType,
    pub number: u32,
    pub sub_number: Option<u32>,
    pub title: String,
    pub start_line: usize,
    pub end_line: usize,
}

pub struct AnnexParser;

impl AnnexParser {
    /// 텍스트에서 별표/별지 영역 감지
    pub fn detect_regions(text: &str) -> Vec<AnnexRegion> {
        let lines: Vec<&str> = text.lines().collect();
        let mut regions: Vec<AnnexRegion> = Vec::new();

        for (i, line) in lines.iter().enumerate() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            if let Some((annex_type, number, sub_number, title)) = Self::detect_line(trimmed) {
                // Close previous region
                if let Some(last) = regions.last_mut() {
                    last.end_line = i;
                }

                regions.push(AnnexRegion {
                    annex_type,
                    number,
                    sub_number,
                    title,
                    start_line: i,
                    end_line: lines.len(),
                });
            }
        }

        regions
    }

    /// 단일 라인에서 별표/별지 감지
    fn detect_line(line: &str) -> Option<(AnnexType, u32, Option<u32>, String)> {
        if let Some(caps) = RE_ANNEX.captures(line) {
            let number: u32 = caps[1].parse().ok()?;
            let sub_number = caps.get(2).and_then(|m| m.as_str().parse().ok());
            let title = caps.get(3).map(|m| m.as_str().trim().to_string()).unwrap_or_default();
            return Some((AnnexType::Annex, number, sub_number, title));
        }

        if let Some(caps) = RE_ANNEX_FORM.captures(line) {
            let number: u32 = caps[1].parse().ok()?;
            let sub_number = caps.get(2).and_then(|m| m.as_str().parse().ok());
            let title = caps.get(3).map(|m| m.as_str().trim().to_string()).unwrap_or_default();
            return Some((AnnexType::Form, number, sub_number, title));
        }

        if let Some(caps) = RE_ATTACHMENT.captures(line) {
            let number: u32 = caps[1].parse().ok()?;
            let title = caps.get(2).map(|m| m.as_str().trim().to_string()).unwrap_or_default();
            return Some((AnnexType::Attachment, number, None, title));
        }

        None
    }

    /// 감지된 영역에서 내용 추출 -> AnnexInfo 생성
    pub fn extract_from_text(text: &str) -> Vec<AnnexInfo> {
        let lines: Vec<&str> = text.lines().collect();
        let regions = Self::detect_regions(text);

        regions
            .iter()
            .map(|region| {
                let content_lines = &lines[region.start_line..region.end_line];
                let raw_content = content_lines.join("\n");

                AnnexInfo {
                    annex_type: region.annex_type.clone(),
                    number: region.number,
                    sub_number: region.sub_number,
                    title: region.title.clone(),
                    tables: Vec::new(),
                    raw_content: raw_content.clone(),
                    markdown: raw_content,
                }
            })
            .collect()
    }

    /// HWP 파일 경로에서 별표 추출
    pub fn from_hwp_file<P: AsRef<std::path::Path>>(path: P) -> Result<Vec<AnnexInfo>, String> {
        use crate::hwp::HwpParser;

        let mut parser = HwpParser::open(path).map_err(|e| e.to_string())?;
        let text = parser.extract_text().map_err(|e| e.to_string())?;
        let tables = parser.extract_tables().map_err(|e| e.to_string())?;

        let regions = Self::detect_regions(&text);
        if regions.is_empty() && !tables.is_empty() {
            // No annex markers -> treat entire file as single annex (standalone annex file)
            return Ok(vec![AnnexInfo {
                annex_type: AnnexType::Annex,
                number: 1,
                sub_number: None,
                title: String::new(),
                markdown: tables.iter().map(|t| t.to_markdown()).collect::<Vec<_>>().join("\n\n"),
                raw_content: text,
                tables,
            }]);
        }

        let lines: Vec<&str> = text.lines().collect();
        Ok(regions.iter().map(|region| {
            let content = lines[region.start_line..region.end_line].join("\n");
            AnnexInfo {
                annex_type: region.annex_type.clone(),
                number: region.number,
                sub_number: region.sub_number,
                title: region.title.clone(),
                tables: Vec::new(),
                raw_content: content.clone(),
                markdown: content,
            }
        }).collect())
    }

    /// HWPX 파일 경로에서 별표 추출
    pub fn from_hwpx_file<P: AsRef<std::path::Path>>(path: P) -> Result<Vec<AnnexInfo>, String> {
        use crate::hwpx::HwpxParser;

        let mut parser = HwpxParser::open(path).map_err(|e| e.to_string())?;
        let doc = parser.parse().map_err(|e| e.to_string())?;

        let full_text = doc.sections.join("\n");
        let regions = Self::detect_regions(&full_text);

        if regions.is_empty() && !doc.tables.is_empty() {
            return Ok(vec![AnnexInfo {
                annex_type: AnnexType::Annex,
                number: 1,
                sub_number: None,
                title: String::new(),
                markdown: doc.tables.iter().map(|t| {
                    let td = TableData {
                        rows: t.rows,
                        cols: t.cols,
                        cells: t.cells.clone(),
                        cell_spans: Vec::new(),
                    };
                    td.to_markdown()
                }).collect::<Vec<_>>().join("\n\n"),
                raw_content: full_text,
                tables: Vec::new(),
            }]);
        }

        let lines: Vec<&str> = full_text.lines().collect();
        Ok(regions.iter().map(|region| {
            let content = lines[region.start_line..region.end_line].join("\n");
            AnnexInfo {
                annex_type: region.annex_type.clone(),
                number: region.number,
                sub_number: region.sub_number,
                title: region.title.clone(),
                tables: Vec::new(),
                raw_content: content.clone(),
                markdown: content,
            }
        }).collect())
    }
}
