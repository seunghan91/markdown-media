use napi_derive::napi;

#[napi(object)]
pub struct NapiAnnexInfo {
    pub annex_type: String,
    pub number: u32,
    pub sub_number: Option<u32>,
    pub title: String,
    pub raw_content: String,
    pub markdown: String,
}

#[napi(object)]
pub struct NapiDateResult {
    pub date: String,
    pub end_date: Option<String>,
    pub format: String,
    pub confidence: f64,
}

#[napi(object)]
pub struct NapiChainStep {
    pub tool_name: String,
    pub params: String,
    pub depends_on: Vec<u32>,
    pub parallel_group: Option<u32>,
}

#[napi(object)]
pub struct NapiChainPlan {
    pub chain_type: String,
    pub description: String,
    pub steps: Vec<NapiChainStep>,
    pub executable_groups: Vec<Vec<u32>>,
}

#[napi(object)]
pub struct NapiTableData {
    pub rows: u32,
    pub cols: u32,
    pub cells: Vec<Vec<String>>,
    pub markdown: String,
}

#[napi(object)]
pub struct NapiHwpResult {
    pub text: String,
    pub tables: Vec<NapiTableData>,
}

// ============ From impls ============

impl From<mdm_core::legal::AnnexInfo> for NapiAnnexInfo {
    fn from(a: mdm_core::legal::AnnexInfo) -> Self {
        Self {
            annex_type: match a.annex_type {
                mdm_core::legal::AnnexType::Annex => "annex".into(),
                mdm_core::legal::AnnexType::Form => "form".into(),
                mdm_core::legal::AnnexType::Attachment => "attachment".into(),
            },
            number: a.number,
            sub_number: a.sub_number,
            title: a.title,
            raw_content: a.raw_content,
            markdown: a.markdown,
        }
    }
}

impl From<mdm_core::utils::date_parser::DateResult> for NapiDateResult {
    fn from(d: mdm_core::utils::date_parser::DateResult) -> Self {
        Self {
            date: d.date,
            end_date: d.end_date,
            format: match d.format {
                mdm_core::utils::date_parser::DateFormat::Absolute => "absolute".into(),
                mdm_core::utils::date_parser::DateFormat::Relative => "relative".into(),
                mdm_core::utils::date_parser::DateFormat::Duration => "duration".into(),
                mdm_core::utils::date_parser::DateFormat::Legal => "legal".into(),
                mdm_core::utils::date_parser::DateFormat::Weekday => "weekday".into(),
            },
            confidence: d.confidence,
        }
    }
}

impl From<mdm_core::legal::ChainPlan> for NapiChainPlan {
    fn from(p: mdm_core::legal::ChainPlan) -> Self {
        let groups = p
            .executable_groups()
            .into_iter()
            .map(|g| g.into_iter().map(|i| i as u32).collect())
            .collect();
        Self {
            chain_type: serde_json::to_string(&p.chain_type)
                .unwrap_or_default()
                .trim_matches('"')
                .to_string(),
            description: p.description,
            steps: p.steps.into_iter().map(NapiChainStep::from).collect(),
            executable_groups: groups,
        }
    }
}

impl From<mdm_core::legal::ChainStep> for NapiChainStep {
    fn from(s: mdm_core::legal::ChainStep) -> Self {
        Self {
            tool_name: s.tool_name,
            params: s.params.to_string(),
            depends_on: s.depends_on.into_iter().map(|i| i as u32).collect(),
            parallel_group: s.parallel_group,
        }
    }
}

impl From<mdm_core::hwp::parser::TableData> for NapiTableData {
    fn from(t: mdm_core::hwp::parser::TableData) -> Self {
        let markdown = t.to_markdown();
        Self {
            rows: t.rows as u32,
            cols: t.cols as u32,
            cells: t.cells,
            markdown,
        }
    }
}
