mod types;

use napi_derive::napi;
use types::*;

// ============ Annex Parser ============

/// Parse annex/form markers from Korean legal text
#[napi]
pub fn parse_annex_text(text: String) -> Vec<NapiAnnexInfo> {
    mdm_core::legal::AnnexParser::extract_from_text(&text)
        .into_iter()
        .map(NapiAnnexInfo::from)
        .collect()
}

/// Parse annexes from an HWP file path
#[napi]
pub fn parse_annex_hwp(path: String) -> napi::Result<Vec<NapiAnnexInfo>> {
    mdm_core::legal::AnnexParser::from_hwp_file(&path)
        .map(|v| v.into_iter().map(NapiAnnexInfo::from).collect())
        .map_err(|e| napi::Error::from_reason(e))
}

/// Parse annexes from an HWPX file path
#[napi]
pub fn parse_annex_hwpx(path: String) -> napi::Result<Vec<NapiAnnexInfo>> {
    mdm_core::legal::AnnexParser::from_hwpx_file(&path)
        .map(|v| v.into_iter().map(NapiAnnexInfo::from).collect())
        .map_err(|e| napi::Error::from_reason(e))
}

// ============ Date Parser ============

/// Parse Korean date expression with today as reference
#[napi]
pub fn parse_date(text: String) -> Option<NapiDateResult> {
    mdm_core::utils::date_parser::KoreanDateParser::today()
        .parse(&text)
        .map(NapiDateResult::from)
}

/// Parse Korean date expression with custom reference date (YYYYMMDD)
#[napi]
pub fn parse_date_with_reference(
    text: String,
    reference_date: String,
) -> napi::Result<Option<NapiDateResult>> {
    let ref_date = chrono::NaiveDate::parse_from_str(&reference_date, "%Y%m%d").map_err(|e| {
        napi::Error::from_reason(format!(
            "Invalid reference date '{}': {}",
            reference_date, e
        ))
    })?;
    Ok(mdm_core::utils::date_parser::KoreanDateParser::new(ref_date)
        .parse(&text)
        .map(NapiDateResult::from))
}

// ============ Chain Planner ============

/// Create a chain execution plan
#[napi]
pub fn create_chain_plan(chain_type: String, query: String) -> napi::Result<NapiChainPlan> {
    let ct =
        mdm_core::legal::ChainType::from_str(&chain_type).map_err(napi::Error::from_reason)?;
    let plan = mdm_core::legal::ChainPlan::from_query(ct, &query);
    Ok(NapiChainPlan::from(plan))
}

/// Aggregate chain step results into markdown
#[napi]
pub fn aggregate_chain_results(chain_type: String, results: Vec<String>) -> napi::Result<String> {
    let ct =
        mdm_core::legal::ChainType::from_str(&chain_type).map_err(napi::Error::from_reason)?;
    Ok(mdm_core::legal::ChainPlan::aggregate_results(&ct, &results))
}

// ============ HWP/HWPX Parser ============

/// Extract text and tables from HWP file
#[napi]
pub fn parse_hwp_file(path: String) -> napi::Result<NapiHwpResult> {
    let mut parser = mdm_core::hwp::HwpParser::open(&path)
        .map_err(|e| napi::Error::from_reason(format!("Failed to open HWP: {}", e)))?;
    let text = parser
        .extract_text()
        .map_err(|e| napi::Error::from_reason(format!("Failed to extract text: {}", e)))?;
    let tables = parser
        .extract_tables()
        .map_err(|e| napi::Error::from_reason(format!("Failed to extract tables: {}", e)))?;
    Ok(NapiHwpResult {
        text,
        tables: tables.into_iter().map(NapiTableData::from).collect(),
    })
}

/// Extract text from HWPX file
#[napi]
pub fn parse_hwpx_file(path: String) -> napi::Result<String> {
    let mut parser = mdm_core::hwpx::HwpxParser::open(&path)
        .map_err(|e| napi::Error::from_reason(format!("Failed to open HWPX: {}", e)))?;
    let doc = parser
        .parse()
        .map_err(|e| napi::Error::from_reason(format!("Failed to parse HWPX: {}", e)))?;
    Ok(doc.sections.join("\n"))
}
