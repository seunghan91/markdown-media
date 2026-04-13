use crate::history::HistoryStore;
use crate::markdown::{escape_html, render_markdown_to_html};
use crate::models::{ConvertResult, DocumentMetadata, ExtractedImage};
use mdm_core::{DocxParser, HwpParser, HwpxParser};
use mdm_core::pdf::PdfParser;
use std::fs;
use std::path::{Path, PathBuf};
use tauri::State;

fn is_markdown_path(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|value| value.to_str())
            .map(|value| value.to_ascii_lowercase()),
        Some(extension) if matches!(extension.as_str(), "md" | "mdm" | "mdx" | "markdown")
    )
}

pub(crate) fn read_markdown_path(path: &Path) -> Result<String, String> {
    fs::read_to_string(path).map_err(|error| error.to_string())
}

fn hwp_result(path: &Path) -> Result<ConvertResult, String> {
    let mut parser = HwpParser::open(path).map_err(|error| error.to_string())?;
    let markdown = parser.extract_text().map_err(|error| error.to_string())?;
    let images = parser
        .extract_images()
        .unwrap_or_default()
        .into_iter()
        .map(|image| ExtractedImage {
            id: image.name,
            filename: image.original_name,
            media_type: image.format,
            width: None,
            height: None,
        })
        .collect();

    let metadata = match parser.extract_metadata() {
        Ok(metadata) => DocumentMetadata {
            format: "hwp".into(),
            title: metadata.title,
            author: metadata.author,
            subject: metadata.subject,
            description: metadata.description,
            keywords: metadata.keywords,
            version: Some(metadata.version),
            page_count: Some(metadata.section_count),
            word_count: None,
        },
        Err(_) => DocumentMetadata {
            format: "hwp".into(),
            ..DocumentMetadata::default()
        },
    };

    Ok(ConvertResult {
        markdown,
        images,
        metadata,
    })
}

fn hwpx_result(path: &Path) -> Result<ConvertResult, String> {
    let mut parser = HwpxParser::open(path).map_err(|error| error.to_string())?;
    let document = parser.parse().map_err(|error| error.to_string())?;
    let images = document
        .image_info
        .into_iter()
        .map(|image| ExtractedImage {
            id: image.id,
            filename: image.path,
            media_type: image.media_type,
            width: None,
            height: None,
        })
        .collect();

    Ok(ConvertResult {
        markdown: document.sections.join("\n\n"),
        images,
        metadata: DocumentMetadata {
            format: "hwpx".into(),
            title: None,
            author: None,
            subject: None,
            description: Some(document.preview_text),
            keywords: None,
            version: Some(document.version),
            page_count: Some(document.sections.len()),
            word_count: None,
        },
    })
}

fn pdf_result(path: &Path) -> Result<ConvertResult, String> {
    let parser = PdfParser::open(path).map_err(|error| error.to_string())?;
    let document = parser.parse_from_memory().map_err(|error| error.to_string())?;
    let images = document
        .images
        .iter()
        .map(|image| ExtractedImage {
            id: image.id.clone(),
            filename: format!("{}.bin", image.id),
            media_type: format!("{:?}", image.format).to_lowercase(),
            width: Some(image.width),
            height: Some(image.height),
        })
        .collect();

    Ok(ConvertResult {
        markdown: document.to_markdown_with_layout(),
        images,
        metadata: DocumentMetadata {
            format: "pdf".into(),
            title: if document.metadata.title.is_empty() {
                None
            } else {
                Some(document.metadata.title)
            },
            author: if document.metadata.author.is_empty() {
                None
            } else {
                Some(document.metadata.author)
            },
            subject: if document.metadata.subject.is_empty() {
                None
            } else {
                Some(document.metadata.subject)
            },
            description: None,
            keywords: None,
            version: Some(document.version),
            page_count: Some(document.page_count),
            word_count: None,
        },
    })
}

fn docx_result(path: &Path) -> Result<ConvertResult, String> {
    let mut parser = DocxParser::open(path).map_err(|error| error.to_string())?;
    let document = parser.parse().map_err(|error| error.to_string())?;
    let images = document
        .images
        .iter()
        .map(|image| ExtractedImage {
            id: image.id.clone(),
            filename: image.filename.clone(),
            media_type: "image".into(),
            width: image.width,
            height: image.height,
        })
        .collect();

    Ok(ConvertResult {
        markdown: document.to_markdown(),
        images,
        metadata: DocumentMetadata {
            format: "docx".into(),
            title: document.metadata.title,
            author: document.metadata.author,
            subject: document.metadata.subject,
            description: None,
            keywords: None,
            version: None,
            page_count: document.metadata.page_count.map(|value| value as usize),
            word_count: document.metadata.word_count.map(|value| value as usize),
        },
    })
}

pub(crate) fn convert_path(path: &Path) -> Result<ConvertResult, String> {
    if is_markdown_path(path) {
        return Ok(ConvertResult {
            markdown: read_markdown_path(path)?,
            images: Vec::new(),
            metadata: DocumentMetadata {
                format: path
                    .extension()
                    .and_then(|value| value.to_str())
                    .unwrap_or("markdown")
                    .to_string(),
                title: path.file_name().map(|value| value.to_string_lossy().to_string()),
                ..DocumentMetadata::default()
            },
        });
    }

    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase())
        .ok_or_else(|| "지원되지 않는 파일 형식입니다.".to_string())?;

    match extension.as_str() {
        "hwp" => hwp_result(path),
        "hwpx" => hwpx_result(path),
        "pdf" => pdf_result(path),
        "docx" => docx_result(path),
        _ => Err(format!("지원되지 않는 입력 형식입니다: {}", extension)),
    }
}

#[tauri::command]
pub async fn convert_file(
    path: String,
    _format: String,
    history: State<'_, HistoryStore>,
) -> Result<ConvertResult, String> {
    let path_buf = PathBuf::from(&path);
    let result = convert_path(&path_buf);
    let status = if result.is_ok() { "success" } else { "failed" };
    let _ = history.record(&path_buf, "to_md", "md", status);
    result
}

#[tauri::command]
pub async fn convert_text(content: String, from_format: String) -> Result<String, String> {
    if matches!(from_format.as_str(), "markdown" | "md" | "mdm" | "mdx") {
        return Ok(render_markdown_to_html(&content));
    }

    Ok(format!("<pre>{}</pre>", escape_html(&content)))
}
