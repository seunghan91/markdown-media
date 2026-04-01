//! Chain tool definitions for multi-step legal research
//!
//! Generates execution plans combining multiple MCP tools.
//! Actual API calls are executed by the Node.js korea-law MCP layer.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChainType {
    FullResearch,
    ActionBasis,
    CompareOldNew,
    SearchWithInterpretation,
    ExtractAnnexes,
    CompareDelegation,
    FindSimilarPrecedents,
    ResearchSpecialized,
}

impl ChainType {
    pub fn from_str(s: &str) -> Result<Self, String> {
        match s {
            "full_research" | "FullResearch" => Ok(Self::FullResearch),
            "action_basis" | "ActionBasis" => Ok(Self::ActionBasis),
            "compare_old_new" | "CompareOldNew" => Ok(Self::CompareOldNew),
            "search_with_interpretation" | "SearchWithInterpretation" => {
                Ok(Self::SearchWithInterpretation)
            }
            "extract_annexes" | "ExtractAnnexes" => Ok(Self::ExtractAnnexes),
            "compare_delegation" | "CompareDelegation" => Ok(Self::CompareDelegation),
            "find_similar_precedents" | "FindSimilarPrecedents" => {
                Ok(Self::FindSimilarPrecedents)
            }
            "research_specialized" | "ResearchSpecialized" => Ok(Self::ResearchSpecialized),
            _ => Err(format!("Unknown chain type: {s}")),
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            Self::FullResearch => "포괄적 법률 조사 (법령 + 조문 + 판례 + 해석례 일괄)",
            Self::ActionBasis => "행정 처분의 법적 근거 추적",
            Self::CompareOldNew => "법령 개정 전후 비교",
            Self::SearchWithInterpretation => "특정 조문과 관련 해석례 함께 검색",
            Self::ExtractAnnexes => "법령 별표/별지를 Markdown 테이블로 변환",
            Self::CompareDelegation => "법률-시행령-시행규칙 3단 위임 구조 비교",
            Self::FindSimilarPrecedents => "특정 사건과 유사한 판례 검색",
            Self::ResearchSpecialized => "전문기관 결정례 일괄 조사",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainStep {
    pub tool_name: String,
    pub params: serde_json::Value,
    pub depends_on: Vec<usize>,
    pub parallel_group: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainPlan {
    pub chain_type: ChainType,
    pub description: String,
    pub steps: Vec<ChainStep>,
}

impl ChainPlan {
    pub fn from_query(chain_type: ChainType, query: &str) -> Self {
        let q = serde_json::json!({ "query": query });
        let from = |step: usize| serde_json::json!({ "from_step": step });

        let steps = match &chain_type {
            ChainType::FullResearch => vec![
                ChainStep {
                    tool_name: "search_law_names".into(),
                    params: q.clone(),
                    depends_on: vec![],
                    parallel_group: Some(0),
                },
                ChainStep {
                    tool_name: "get_law_text".into(),
                    params: from(0),
                    depends_on: vec![0],
                    parallel_group: Some(1),
                },
                ChainStep {
                    tool_name: "search_precedents".into(),
                    params: q.clone(),
                    depends_on: vec![0],
                    parallel_group: Some(1),
                },
                ChainStep {
                    tool_name: "search_legal_interpretations".into(),
                    params: q.clone(),
                    depends_on: vec![0],
                    parallel_group: Some(1),
                },
            ],
            ChainType::ActionBasis => vec![
                ChainStep {
                    tool_name: "search_law_names".into(),
                    params: q.clone(),
                    depends_on: vec![],
                    parallel_group: Some(0),
                },
                ChainStep {
                    tool_name: "get_law_text".into(),
                    params: from(0),
                    depends_on: vec![0],
                    parallel_group: Some(1),
                },
                ChainStep {
                    tool_name: "search_legal_interpretations".into(),
                    params: q.clone(),
                    depends_on: vec![0],
                    parallel_group: Some(1),
                },
                ChainStep {
                    tool_name: "search_admin_appeals".into(),
                    params: q.clone(),
                    depends_on: vec![0],
                    parallel_group: Some(1),
                },
            ],
            ChainType::CompareOldNew => vec![
                ChainStep {
                    tool_name: "search_law_names".into(),
                    params: q.clone(),
                    depends_on: vec![],
                    parallel_group: Some(0),
                },
                ChainStep {
                    tool_name: "get_law_text".into(),
                    params: serde_json::json!({"from_step": 0, "version": "current"}),
                    depends_on: vec![0],
                    parallel_group: Some(1),
                },
                ChainStep {
                    tool_name: "get_law_text".into(),
                    params: serde_json::json!({"from_step": 0, "version": "previous"}),
                    depends_on: vec![0],
                    parallel_group: Some(1),
                },
            ],
            ChainType::SearchWithInterpretation => vec![
                ChainStep {
                    tool_name: "search_law_names".into(),
                    params: q.clone(),
                    depends_on: vec![],
                    parallel_group: Some(0),
                },
                ChainStep {
                    tool_name: "get_law_text".into(),
                    params: from(0),
                    depends_on: vec![0],
                    parallel_group: Some(1),
                },
                ChainStep {
                    tool_name: "search_legal_interpretations".into(),
                    params: q.clone(),
                    depends_on: vec![0],
                    parallel_group: Some(1),
                },
            ],
            ChainType::ExtractAnnexes => vec![
                ChainStep {
                    tool_name: "search_law_names".into(),
                    params: q.clone(),
                    depends_on: vec![],
                    parallel_group: Some(0),
                },
                ChainStep {
                    tool_name: "get_annex_urls".into(),
                    params: from(0),
                    depends_on: vec![0],
                    parallel_group: None,
                },
                ChainStep {
                    tool_name: "download_hwpx".into(),
                    params: from(1),
                    depends_on: vec![1],
                    parallel_group: None,
                },
                ChainStep {
                    tool_name: "parse_annex".into(),
                    params: from(2),
                    depends_on: vec![2],
                    parallel_group: None,
                },
            ],
            ChainType::CompareDelegation => vec![
                ChainStep {
                    tool_name: "search_law_names".into(),
                    params: q.clone(),
                    depends_on: vec![],
                    parallel_group: Some(0),
                },
                ChainStep {
                    tool_name: "get_law_text".into(),
                    params: serde_json::json!({"from_step": 0, "level": "법률"}),
                    depends_on: vec![0],
                    parallel_group: Some(1),
                },
                ChainStep {
                    tool_name: "get_law_text".into(),
                    params: serde_json::json!({"from_step": 0, "level": "시행령"}),
                    depends_on: vec![0],
                    parallel_group: Some(1),
                },
                ChainStep {
                    tool_name: "get_law_text".into(),
                    params: serde_json::json!({"from_step": 0, "level": "시행규칙"}),
                    depends_on: vec![0],
                    parallel_group: Some(1),
                },
            ],
            ChainType::FindSimilarPrecedents => vec![
                ChainStep {
                    tool_name: "search_precedents".into(),
                    params: q.clone(),
                    depends_on: vec![],
                    parallel_group: Some(0),
                },
                ChainStep {
                    tool_name: "search_precedents_by_title".into(),
                    params: q.clone(),
                    depends_on: vec![],
                    parallel_group: Some(0),
                },
            ],
            ChainType::ResearchSpecialized => vec![
                ChainStep {
                    tool_name: "search_tax_tribunal".into(),
                    params: q.clone(),
                    depends_on: vec![],
                    parallel_group: Some(0),
                },
                ChainStep {
                    tool_name: "search_constitutional_decisions".into(),
                    params: q.clone(),
                    depends_on: vec![],
                    parallel_group: Some(0),
                },
                ChainStep {
                    tool_name: "search_ftc_decisions".into(),
                    params: q.clone(),
                    depends_on: vec![],
                    parallel_group: Some(0),
                },
                ChainStep {
                    tool_name: "search_admin_appeals".into(),
                    params: q.clone(),
                    depends_on: vec![],
                    parallel_group: Some(0),
                },
            ],
        };

        ChainPlan {
            description: chain_type.description().to_string(),
            chain_type,
            steps,
        }
    }

    pub fn executable_groups(&self) -> Vec<Vec<usize>> {
        let mut groups: std::collections::BTreeMap<u32, Vec<usize>> =
            std::collections::BTreeMap::new();
        for (i, step) in self.steps.iter().enumerate() {
            let group = step.parallel_group.unwrap_or(i as u32 + 100);
            groups.entry(group).or_default().push(i);
        }
        groups.into_values().collect()
    }

    pub fn aggregate_results(chain_type: &ChainType, results: &[String]) -> String {
        let mut md = format!("# {}\n\n", chain_type.description());
        for (i, result) in results.iter().enumerate() {
            if !result.is_empty() {
                md.push_str(&format!(
                    "## 단계 {} 결과\n\n{}\n\n---\n\n",
                    i + 1,
                    result
                ));
            }
        }
        md
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_full_research_plan() {
        let plan = ChainPlan::from_query(ChainType::FullResearch, "음주운전 처벌");
        assert_eq!(plan.steps.len(), 4);
        assert_eq!(plan.steps[0].tool_name, "search_law_names");
        assert!(plan.steps[0].depends_on.is_empty());
        assert_eq!(plan.steps[1].depends_on, vec![0]);
        assert_eq!(plan.steps[2].depends_on, vec![0]);
        assert_eq!(plan.steps[1].parallel_group, Some(1));
        assert_eq!(plan.steps[2].parallel_group, Some(1));
    }

    #[test]
    fn test_compare_delegation_plan() {
        let plan = ChainPlan::from_query(ChainType::CompareDelegation, "산업안전보건법");
        assert_eq!(plan.steps.len(), 4);
        assert_eq!(plan.steps[1].parallel_group, Some(1));
        assert_eq!(plan.steps[2].parallel_group, Some(1));
        assert_eq!(plan.steps[3].parallel_group, Some(1));
    }

    #[test]
    fn test_research_specialized_all_parallel() {
        let plan = ChainPlan::from_query(ChainType::ResearchSpecialized, "부동산 거래");
        assert_eq!(plan.steps.len(), 4);
        for step in &plan.steps {
            assert_eq!(step.parallel_group, Some(0));
            assert!(step.depends_on.is_empty());
        }
    }

    #[test]
    fn test_extract_annexes_sequential() {
        let plan = ChainPlan::from_query(ChainType::ExtractAnnexes, "화학물질관리법 별표");
        assert_eq!(plan.steps.len(), 4);
        assert_eq!(plan.steps[1].depends_on, vec![0]);
        assert_eq!(plan.steps[2].depends_on, vec![1]);
        assert_eq!(plan.steps[3].depends_on, vec![2]);
    }

    #[test]
    fn test_chain_type_from_str() {
        assert_eq!(
            ChainType::from_str("full_research").unwrap(),
            ChainType::FullResearch
        );
        assert_eq!(
            ChainType::from_str("FullResearch").unwrap(),
            ChainType::FullResearch
        );
        assert!(ChainType::from_str("invalid").is_err());
    }

    #[test]
    fn test_executable_groups() {
        let plan = ChainPlan::from_query(ChainType::FullResearch, "test");
        let groups = plan.executable_groups();
        assert_eq!(groups.len(), 2);
    }

    #[test]
    fn test_aggregate_results() {
        let results = vec![
            "법령 검색 결과".into(),
            "조문 내용".into(),
            "판례 목록".into(),
        ];
        let md = ChainPlan::aggregate_results(&ChainType::FullResearch, &results);
        assert!(md.contains("# 포괄적 법률 조사"));
        assert!(md.contains("법령 검색 결과"));
        assert!(md.contains("판례 목록"));
    }
}
