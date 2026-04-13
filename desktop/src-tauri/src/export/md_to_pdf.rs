use crate::markdown::wrap_text_lines;
use printpdf::{BuiltinFont, Mm, PdfDocument};
use std::fs::{self, File};
use std::io::{self, BufWriter};
use std::path::Path;

pub fn export(markdown: &str, output: &Path) -> io::Result<()> {
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }

    let title = output
        .file_stem()
        .map(|value| value.to_string_lossy().to_string())
        .unwrap_or_else(|| "MDM Export".into());

    let (document, page, layer) = PdfDocument::new(&title, Mm(210.0), Mm(297.0), "Layer 1");
    let font = document
        .add_builtin_font(BuiltinFont::Helvetica)
        .map_err(|error| io::Error::new(io::ErrorKind::Other, error.to_string()))?;

    let mut current_page = page;
    let mut current_layer = layer;
    let mut y_position = 280.0;

    for line in wrap_text_lines(markdown, 78) {
        if y_position < 18.0 {
            let (new_page, new_layer) = document.add_page(Mm(210.0), Mm(297.0), "Layer");
            current_page = new_page;
            current_layer = new_layer;
            y_position = 280.0;
        }

        let layer_ref = document.get_page(current_page).get_layer(current_layer);
        let font_size = if line.starts_with("# ") {
            18.0
        } else if line.starts_with("## ") {
            16.0
        } else {
            11.0
        };

        let text = line
            .trim_start_matches("# ")
            .trim_start_matches("## ")
            .trim_start_matches("### ")
            .to_string();

        layer_ref.use_text(text, font_size, Mm(16.0), Mm(y_position), &font);
        y_position -= if line.is_empty() { 8.0 } else { 7.0 };
    }

    let file = File::create(output)?;
    document
        .save(&mut BufWriter::new(file))
        .map_err(|error| io::Error::new(io::ErrorKind::Other, error.to_string()))
}
