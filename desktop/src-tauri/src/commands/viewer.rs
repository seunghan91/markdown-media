use crate::commands::convert::{convert_path, read_markdown_path};
use crate::markdown::render_markdown_to_html;
use crate::models::{DocumentMetadata, ViewerData};
use std::path::{Path, PathBuf};

fn is_markdown(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|value| value.to_str())
            .map(|value| value.to_ascii_lowercase()),
        Some(extension) if matches!(extension.as_str(), "md" | "mdm" | "mdx" | "markdown")
    )
}

#[tauri::command]
pub async fn open_file(path: String) -> Result<ViewerData, String> {
    let path = PathBuf::from(path);

    if is_markdown(&path) {
        let markdown = read_markdown_path(&path)?;
        let html = render_markdown_to_html(&markdown);

        return Ok(ViewerData {
            html,
            markdown,
            metadata: DocumentMetadata {
                format: "markdown".into(),
                title: path.file_name().map(|value| value.to_string_lossy().to_string()),
                ..DocumentMetadata::default()
            },
        });
    }

    let result = convert_path(&path)?;
    Ok(ViewerData {
        html: render_markdown_to_html(&result.markdown),
        markdown: result.markdown,
        metadata: result.metadata,
    })
}

#[tauri::command]
pub async fn get_markdown_source(path: String) -> Result<String, String> {
    let path = PathBuf::from(path);
    if is_markdown(&path) {
        return read_markdown_path(&path);
    }

    Ok(convert_path(&path)?.markdown)
}
