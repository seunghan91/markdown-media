use crate::export::{md_to_docx, md_to_hwpx, md_to_pdf};
use crate::history::HistoryStore;
use std::path::PathBuf;
use tauri::State;

fn record_export(
    history: &HistoryStore,
    output: &PathBuf,
    format: &str,
    result: &Result<(), String>,
) {
    let status = if result.is_ok() { "success" } else { "failed" };
    let _ = history.record(output, "from_md", format, status);
}

#[tauri::command]
pub async fn export_to_docx(
    markdown: String,
    template: String,
    output: String,
    history: State<'_, HistoryStore>,
) -> Result<(), String> {
    let output_path = PathBuf::from(output);
    let result = md_to_docx::export(&markdown, &template, &output_path).map_err(|error| error.to_string());
    record_export(&history, &output_path, "docx", &result);
    result
}

#[tauri::command]
pub async fn export_to_hwpx(
    markdown: String,
    template: String,
    output: String,
    history: State<'_, HistoryStore>,
) -> Result<(), String> {
    let output_path = PathBuf::from(output);
    let result = md_to_hwpx::export(&markdown, &template, &output_path).map_err(|error| error.to_string());
    record_export(&history, &output_path, "hwpx", &result);
    result
}

#[tauri::command]
pub async fn export_to_pdf(
    markdown: String,
    output: String,
    history: State<'_, HistoryStore>,
) -> Result<(), String> {
    let output_path = PathBuf::from(output);
    let result = md_to_pdf::export(&markdown, &output_path).map_err(|error| error.to_string());
    record_export(&history, &output_path, "pdf", &result);
    result
}
