use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ExtractedImage {
    pub id: String,
    pub filename: String,
    pub media_type: String,
    pub width: Option<u32>,
    pub height: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DocumentMetadata {
    pub format: String,
    pub title: Option<String>,
    pub author: Option<String>,
    pub subject: Option<String>,
    pub description: Option<String>,
    pub keywords: Option<String>,
    pub version: Option<String>,
    pub page_count: Option<usize>,
    pub word_count: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ConvertResult {
    pub markdown: String,
    pub images: Vec<ExtractedImage>,
    pub metadata: DocumentMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ViewerData {
    pub html: String,
    pub markdown: String,
    pub metadata: DocumentMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchItemResult {
    pub input_path: String,
    pub output_path: Option<String>,
    pub status: String,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct BatchResult {
    pub total: usize,
    pub success: usize,
    pub failed: usize,
    pub results: Vec<BatchItemResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryEntry {
    pub id: i64,
    pub file_name: String,
    pub file_path: String,
    pub direction: String,
    pub output_format: String,
    pub created_at: String,
    pub status: String,
}
