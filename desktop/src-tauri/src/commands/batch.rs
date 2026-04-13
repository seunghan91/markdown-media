use crate::commands::convert::{convert_path, read_markdown_path};
use crate::history::HistoryStore;
use crate::models::{BatchItemResult, BatchResult};
use std::fs;
use std::path::{Path, PathBuf};
use tauri::State;
use walkdir::WalkDir;

fn collect_files(paths: &[String]) -> Vec<PathBuf> {
    let mut files = Vec::new();

    for path in paths {
        let path = PathBuf::from(path);
        if path.is_dir() {
            for entry in WalkDir::new(path).into_iter().flatten() {
                if entry.path().is_file() {
                    files.push(entry.path().to_path_buf());
                }
            }
        } else if path.is_file() {
            files.push(path);
        }
    }

    files
}

fn markdown_like(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|value| value.to_str())
            .map(|value| value.to_ascii_lowercase()),
        Some(extension) if matches!(extension.as_str(), "md" | "mdm" | "mdx" | "markdown")
    )
}

#[tauri::command]
pub async fn batch_convert(
    paths: Vec<String>,
    format: String,
    output_dir: String,
    history: State<'_, HistoryStore>,
) -> Result<BatchResult, String> {
    let files = collect_files(&paths);
    let output_dir = PathBuf::from(output_dir);
    fs::create_dir_all(&output_dir).map_err(|error| error.to_string())?;

    let mut result = BatchResult::default();
    result.total = files.len();

    for input_path in files {
        let output_path = output_dir.join(format!(
            "{}.{}",
            input_path
                .file_stem()
                .map(|value| value.to_string_lossy().to_string())
                .unwrap_or_else(|| "output".into()),
            format
        ));

        let item_result = match format.as_str() {
            "md" => {
                let markdown = if markdown_like(&input_path) {
                    read_markdown_path(&input_path)
                } else {
                    convert_path(&input_path).map(|value| value.markdown)
                };

                match markdown.and_then(|content| {
                    fs::write(&output_path, content).map_err(|error| error.to_string())
                }) {
                    Ok(()) => {
                        let _ = history.record(&input_path, "to_md", "md", "success");
                        result.success += 1;
                        BatchItemResult {
                            input_path: input_path.to_string_lossy().to_string(),
                            output_path: Some(output_path.to_string_lossy().to_string()),
                            status: "success".into(),
                            message: None,
                        }
                    }
                    Err(message) => {
                        let _ = history.record(&input_path, "to_md", "md", "failed");
                        result.failed += 1;
                        BatchItemResult {
                            input_path: input_path.to_string_lossy().to_string(),
                            output_path: None,
                            status: "failed".into(),
                            message: Some(message),
                        }
                    }
                }
            }
            "docx" | "hwpx" | "pdf" => {
                let markdown = if markdown_like(&input_path) {
                    read_markdown_path(&input_path)
                } else {
                    convert_path(&input_path).map(|value| value.markdown)
                };

                match markdown {
                    Ok(markdown) => {
                        let export_result = match format.as_str() {
                            "docx" => crate::export::md_to_docx::export(&markdown, "기본", &output_path)
                                .map_err(|error| error.to_string()),
                            "hwpx" => crate::export::md_to_hwpx::export(&markdown, "기본", &output_path)
                                .map_err(|error| error.to_string()),
                            _ => crate::export::md_to_pdf::export(&markdown, &output_path)
                                .map_err(|error| error.to_string()),
                        };

                        match export_result {
                            Ok(()) => {
                                let _ = history.record(&input_path, "from_md", &format, "success");
                                result.success += 1;
                                BatchItemResult {
                                    input_path: input_path.to_string_lossy().to_string(),
                                    output_path: Some(output_path.to_string_lossy().to_string()),
                                    status: "success".into(),
                                    message: None,
                                }
                            }
                            Err(message) => {
                                let _ = history.record(&input_path, "from_md", &format, "failed");
                                result.failed += 1;
                                BatchItemResult {
                                    input_path: input_path.to_string_lossy().to_string(),
                                    output_path: None,
                                    status: "failed".into(),
                                    message: Some(message),
                                }
                            }
                        }
                    }
                    Err(message) => {
                        let _ = history.record(&input_path, "from_md", &format, "failed");
                        result.failed += 1;
                        BatchItemResult {
                            input_path: input_path.to_string_lossy().to_string(),
                            output_path: None,
                            status: "failed".into(),
                            message: Some(message),
                        }
                    }
                }
            }
            _ => {
                return Err(format!("지원되지 않는 배치 출력 형식입니다: {}", format));
            }
        };

        result.results.push(item_result);
    }

    Ok(result)
}
