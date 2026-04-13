use chrono::Utc;
use quick_xml::escape::escape;
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::Path;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

fn paragraph_xml(style: Option<&str>, text: &str) -> String {
    let escaped = escape(text).into_owned();
    let style_xml = style
        .map(|style| format!(r#"<w:pPr><w:pStyle w:val="{}"/></w:pPr>"#, style))
        .unwrap_or_default();

    format!(
        r#"<w:p>{style_xml}<w:r><w:t xml:space="preserve">{escaped}</w:t></w:r></w:p>"#
    )
}

fn markdown_to_document_xml(markdown: &str) -> String {
    let mut body = String::new();

    for line in markdown.lines() {
        let trimmed = line.trim();

        if trimmed.is_empty() {
            body.push_str("<w:p/>");
            continue;
        }

        let (style, text): (Option<&str>, String) = if let Some(text) = trimmed.strip_prefix("# ") {
            (Some("Heading1"), text.to_string())
        } else if let Some(text) = trimmed.strip_prefix("## ") {
            (Some("Heading2"), text.to_string())
        } else if let Some(text) = trimmed.strip_prefix("### ") {
            (Some("Heading3"), text.to_string())
        } else if let Some(text) = trimmed.strip_prefix("- ") {
            (None, format!("- {}", text))
        } else {
            (None, trimmed.to_string())
        };

        body.push_str(&paragraph_xml(style, &text));
    }

    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:wpc="http://schemas.microsoft.com/office/word/2010/wordprocessingCanvas"
 xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006"
 xmlns:o="urn:schemas-microsoft-com:office:office"
 xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"
 xmlns:m="http://schemas.openxmlformats.org/officeDocument/2006/math"
 xmlns:v="urn:schemas-microsoft-com:vml"
 xmlns:wp14="http://schemas.microsoft.com/office/word/2010/wordprocessingDrawing"
 xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing"
 xmlns:w10="urn:schemas-microsoft-com:office:word"
 xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"
 xmlns:w14="http://schemas.microsoft.com/office/word/2010/wordml"
 xmlns:wpg="http://schemas.microsoft.com/office/word/2010/wordprocessingGroup"
 xmlns:wpi="http://schemas.microsoft.com/office/word/2010/wordprocessingInk"
 xmlns:wne="http://schemas.microsoft.com/office/word/2006/wordml"
 xmlns:wps="http://schemas.microsoft.com/office/word/2010/wordprocessingShape"
 mc:Ignorable="w14 wp14">
  <w:body>
    {body}
    <w:sectPr>
      <w:pgSz w:w="11906" w:h="16838"/>
      <w:pgMar w:top="1440" w:right="1440" w:bottom="1440" w:left="1440" w:header="720" w:footer="720" w:gutter="0"/>
    </w:sectPr>
  </w:body>
</w:document>"#
    )
}

fn styles_xml() -> &'static str {
    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:style w:type="paragraph" w:default="1" w:styleId="Normal">
    <w:name w:val="Normal"/>
  </w:style>
  <w:style w:type="paragraph" w:styleId="Heading1">
    <w:name w:val="heading 1"/>
    <w:basedOn w:val="Normal"/>
    <w:uiPriority w:val="9"/>
    <w:qFormat/>
    <w:rPr><w:b/><w:sz w:val="32"/></w:rPr>
  </w:style>
  <w:style w:type="paragraph" w:styleId="Heading2">
    <w:name w:val="heading 2"/>
    <w:basedOn w:val="Normal"/>
    <w:uiPriority w:val="9"/>
    <w:qFormat/>
    <w:rPr><w:b/><w:sz w:val="28"/></w:rPr>
  </w:style>
  <w:style w:type="paragraph" w:styleId="Heading3">
    <w:name w:val="heading 3"/>
    <w:basedOn w:val="Normal"/>
    <w:uiPriority w:val="9"/>
    <w:qFormat/>
    <w:rPr><w:b/><w:sz w:val="24"/></w:rPr>
  </w:style>
</w:styles>"#
}

pub fn export(markdown: &str, template: &str, output: &Path) -> io::Result<()> {
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }

    let file = File::create(output)?;
    let mut zip = ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
    let title = output
        .file_stem()
        .map(|value| value.to_string_lossy().to_string())
        .unwrap_or_else(|| "MDM Export".into());

    zip.start_file("[Content_Types].xml", options)?;
    zip.write_all(
        br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>
  <Override PartName="/word/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml"/>
  <Override PartName="/docProps/core.xml" ContentType="application/vnd.openxmlformats-package.core-properties+xml"/>
  <Override PartName="/docProps/app.xml" ContentType="application/vnd.openxmlformats-officedocument.extended-properties+xml"/>
</Types>"#,
    )?;

    zip.start_file("_rels/.rels", options)?;
    zip.write_all(
        br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/>
  <Relationship Id="rId2" Type="http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties" Target="docProps/core.xml"/>
  <Relationship Id="rId3" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/extended-properties" Target="docProps/app.xml"/>
</Relationships>"#,
    )?;

    zip.start_file("docProps/core.xml", options)?;
    let created = Utc::now().format("%Y-%m-%dT%H:%M:%SZ");
    zip.write_all(
        format!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties"
 xmlns:dc="http://purl.org/dc/elements/1.1/"
 xmlns:dcterms="http://purl.org/dc/terms/"
 xmlns:dcmitype="http://purl.org/dc/dcmitype/"
 xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance">
  <dc:title>{}</dc:title>
  <dc:subject>{}</dc:subject>
  <dc:creator>MDM Desktop</dc:creator>
  <cp:lastModifiedBy>MDM Desktop</cp:lastModifiedBy>
  <dcterms:created xsi:type="dcterms:W3CDTF">{}</dcterms:created>
  <dcterms:modified xsi:type="dcterms:W3CDTF">{}</dcterms:modified>
</cp:coreProperties>"#,
            escape(&title),
            escape(template),
            created,
            created
        )
        .as_bytes(),
    )?;

    zip.start_file("docProps/app.xml", options)?;
    zip.write_all(
        br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Properties xmlns="http://schemas.openxmlformats.org/officeDocument/2006/extended-properties"
 xmlns:vt="http://schemas.openxmlformats.org/officeDocument/2006/docPropsVTypes">
  <Application>MDM Desktop</Application>
</Properties>"#,
    )?;

    zip.start_file("word/document.xml", options)?;
    zip.write_all(markdown_to_document_xml(markdown).as_bytes())?;

    zip.start_file("word/styles.xml", options)?;
    zip.write_all(styles_xml().as_bytes())?;

    zip.start_file("word/_rels/document.xml.rels", options)?;
    zip.write_all(
        br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"/>"#,
    )?;

    zip.finish()?;
    Ok(())
}
