//! hwp2mdm - HWP/HWPX/PDF to MDM converter CLI tool

mod docx;
mod hwp;
mod hwpx;
mod ir;
mod pdf;
mod utils;

use clap::{Parser, Subcommand};
use docx::DocxParser;
use hwp::HwpParser;
use hwpx::HwpxParser;
use pdf::PdfParser;
use quick_xml::events::Event;
use quick_xml::Reader;
use serde_json::json;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Parser)]
#[command(name = "hwp2mdm")]
#[command(author = "MDM Project")]
#[command(version = "0.1.0")]
#[command(about = "Convert HWP files to MDM format", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
    
    /// Input HWP file (for quick conversion)
    #[arg(value_name = "FILE")]
    input: Option<PathBuf>,
    
    /// Output directory
    #[arg(short, long, default_value = "./output")]
    output: PathBuf,
    
    /// Output format (mdx, json)
    #[arg(short, long, default_value = "mdx")]
    format: String,
    
    /// Verbose output
    #[arg(short, long)]
    verbose: bool,
    
    /// Extract images to assets directory
    #[arg(long)]
    extract_images: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Convert a single HWP file
    Convert {
        /// Input HWP file
        input: PathBuf,
        
        /// Output directory
        #[arg(short, long, default_value = "./output")]
        output: PathBuf,
        
        /// Output format (mdx, json)
        #[arg(short, long, default_value = "mdx")]
        format: String,
        
        /// Extract images
        #[arg(long)]
        extract_images: bool,
    },
    
    /// Analyze HWP file structure
    Analyze {
        /// Input HWP file
        input: PathBuf,
    },
    
    /// Extract text from HWP file
    Text {
        /// Input HWP file
        input: PathBuf,
    },
    
    /// Extract images from HWP file
    Images {
        /// Input HWP file
        input: PathBuf,
        
        /// Output directory
        #[arg(short, long, default_value = "./output/assets")]
        output: PathBuf,
    },
    
    /// Batch convert multiple files
    Batch {
        /// Glob pattern (e.g., "*.hwp", "docs/**/*.hwp")
        pattern: String,
        
        /// Output directory
        #[arg(short, long, default_value = "./output")]
        output: PathBuf,
    },
    
    /// Show file information and metadata
    Info {
        /// Input file (HWP, HWPX, PDF)
        input: PathBuf,
        
        /// Output format (text, json)
        #[arg(short, long, default_value = "text")]
        format: String,
    },
}

fn main() {
    let cli = Cli::parse();
    
    match cli.command {
        Some(Commands::Convert { input, output, format, extract_images }) => {
            convert_file(&input, &output, &format, extract_images, true);
        }
        Some(Commands::Analyze { input }) => {
            analyze_file(&input);
        }
        Some(Commands::Text { input }) => {
            extract_text(&input);
        }
        Some(Commands::Images { input, output }) => {
            extract_images(&input, &output);
        }
        Some(Commands::Batch { pattern, output }) => {
            batch_convert(&pattern, &output);
        }
        Some(Commands::Info { input, format }) => {
            show_info(&input, &format);
        }
        None => {
            // Quick conversion mode
            if let Some(input) = cli.input {
                convert_file(&input, &cli.output, &cli.format, cli.extract_images, cli.verbose);
            } else {
                // Show help
                println!("hwp2mdm - HWP to MDM Converter");
                println!();
                println!("USAGE:");
                println!("  hwp2mdm <FILE>              Quick convert HWP to MDX");
                println!("  hwp2mdm convert <FILE>      Convert with options");
                println!("  hwp2mdm analyze <FILE>      Analyze file structure");
                println!("  hwp2mdm text <FILE>         Extract text only");
                println!("  hwp2mdm images <FILE>       Extract images only");
                println!("  hwp2mdm batch <PATTERN>     Batch convert files");
                println!();
                println!("OPTIONS:");
                println!("  -o, --output <DIR>          Output directory [default: ./output]");
                println!("  -f, --format <FMT>          Output format: mdx, json [default: mdx]");
                println!("  -v, --verbose               Verbose output");
                println!("      --extract-images        Extract images to assets/");
                println!();
                println!("EXAMPLES:");
                println!("  hwp2mdm document.hwp");
                println!("  hwp2mdm convert document.hwp -o ./converted --extract-images");
                println!("  hwp2mdm batch \"docs/*.hwp\" -o ./output");
            }
        }
    }
}

fn convert_file(input: &Path, output: &Path, format: &str, extract_images: bool, verbose: bool) {
    println!("📄 Converting: {}", input.display());

    let ext = input.extension().and_then(|e| e.to_str()).unwrap_or("");

    // Detect format by MAGIC BYTES first — many real files have a `.hwp`
    // extension but contain HWPX (ZIP) or PDF data. kordoc verified this is
    // common in Korean gov / financial docs (e.g. bank case-study sample).
    // Without content-based detection mdm fails entirely on these.
    let magic = std::fs::read(input)
        .ok()
        .map(|b| b.into_iter().take(8).collect::<Vec<u8>>())
        .unwrap_or_default();
    let is_zip = magic.len() >= 4
        && magic[0] == 0x50  // P
        && magic[1] == 0x4B  // K
        && (magic[2] == 0x03 || magic[2] == 0x05 || magic[2] == 0x07);
    let is_pdf = magic.len() >= 4
        && magic[0] == b'%'
        && magic[1] == b'P'
        && magic[2] == b'D'
        && magic[3] == b'F';
    let is_cfb = magic.len() >= 8
        && magic == [0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1];

    // DOCX is also ZIP-based — check extension before HWPX fallback
    if ext.eq_ignore_ascii_case("docx") {
        convert_docx(input, output, format, verbose);
        return;
    }

    // ZIP magic → HWPX (regardless of extension)
    if is_zip || ext.eq_ignore_ascii_case("hwpx") {
        convert_hwpx(input, output, format, extract_images, verbose);
        return;
    }

    // PDF magic → PDF
    if is_pdf || ext.eq_ignore_ascii_case("pdf") {
        convert_pdf(input, output, format, verbose);
        return;
    }

    // Neither ZIP nor PDF nor CFB → unknown
    // Detect known unsupported formats with friendly messages BEFORE the
    // confusing 'invalid CFB magic' error fires.
    let _ = is_cfb;

    // Fasoo DRMONE enterprise DRM-encrypted documents start with the literal
    // "0x9B 0x20 D R M O N E   This Document is encrypted ...". These files
    // are AES-encrypted at the OS level and require a license server — no
    // open-source parser (including kordoc) can read them.
    if magic.len() >= 8 && magic[0] == 0x9B && &magic[2..8] == b"DRMONE" {
        eprintln!("❌ This file is DRM-protected (Fasoo DRMONE).");
        eprintln!("   Open it in Hancom Office with a valid license to remove DRM,");
        eprintln!("   then re-export. Open-source parsers cannot read DRM-locked HWPs.");
        return;
    }

    // Plain XML can be raw HWPML exports (often mislabeled as `.hwp`).
    // Handle those directly instead of pretending to be a CFB file.
    if magic.len() >= 5 && magic.starts_with(b"<?xml") {
        convert_hwpml(input, output, format, verbose);
        return;
    }

    match HwpParser::open(input) {
        Ok(mut parser) => {
            // Create output directory
            fs::create_dir_all(output).expect("Failed to create output directory");
            
            // Extract content
            let mdm = match parser.to_mdm() {
                Ok(doc) => doc,
                Err(e) => {
                    eprintln!("❌ Error extracting content: {}", e);
                    return;
                }
            };
            
            let stem = input.file_stem().unwrap_or_default().to_string_lossy();
            
            // Save images if requested
            if extract_images && !mdm.images.is_empty() {
                let assets_dir = output.join("assets");
                fs::create_dir_all(&assets_dir).expect("Failed to create assets directory");
                
                for img in &mdm.images {
                    let img_path = assets_dir.join(&img.name);
                    if let Err(e) = fs::write(&img_path, &img.data) {
                        eprintln!("  ⚠️  Failed to save {}: {}", img.name, e);
                    } else if verbose {
                        println!("  📷 Saved: {}", img.name);
                    }
                }
                println!("  ✓ Extracted {} images", mdm.images.len());
            }
            
            // Save output based on format
            match format {
                "json" => {
                    let json_path = output.join(format!("{}.json", stem));
                    let json_data = json!({
                        "version": "1.0",
                        "metadata": {
                            "hwp_version": mdm.metadata.version,
                            "sections": mdm.metadata.section_count,
                            "compressed": mdm.metadata.compressed,
                            "encrypted": mdm.metadata.encrypted,
                        },
                        "content": mdm.content,
                        "tables": mdm.tables.iter().map(|t| json!({
                            "rows": t.rows,
                            "cols": t.cols,
                            "cells": t.cells,
                            "markdown": t.to_markdown(),
                        })).collect::<Vec<_>>(),
                        "images": mdm.images.iter().map(|i| json!({
                            "name": i.name,
                            "format": i.format,
                            "size": i.data.len(),
                        })).collect::<Vec<_>>(),
                    });
                    
                    fs::write(&json_path, serde_json::to_string_pretty(&json_data).unwrap())
                        .expect("Failed to write JSON");
                    println!("  ✓ Created: {}", json_path.display());
                }
                _ => {
                    // Default: MDX format
                    let mdx_path = output.join(format!("{}.mdx", stem));
                    let mdx_content = mdm.to_mdx();
                    
                    fs::write(&mdx_path, &mdx_content).expect("Failed to write MDX");
                    println!("  ✓ Created: {}", mdx_path.display());
                    
                    // Also create .mdm manifest
                    let mdm_path = output.join(format!("{}.mdm", stem));
                    let mdm_manifest = json!({
                        "version": "1.0",
                        "source": input.file_name().unwrap_or_default().to_string_lossy(),
                        "resources": mdm.images.iter().map(|i| {
                            (i.name.clone(), json!({
                                "type": "image",
                                "format": i.format,
                                "src": format!("assets/{}", i.name),
                            }))
                        }).collect::<serde_json::Map<String, serde_json::Value>>(),
                    });
                    
                    fs::write(&mdm_path, serde_json::to_string_pretty(&mdm_manifest).unwrap())
                        .expect("Failed to write MDM manifest");
                    println!("  ✓ Created: {}", mdm_path.display());
                }
            }
            
            if verbose {
                println!("\n📊 Summary:");
                println!("  - Sections: {}", mdm.metadata.section_count);
                println!("  - Images: {}", mdm.images.len());
                println!("  - Tables: {}", mdm.tables.len());
                println!("  - Text length: {} chars", mdm.content.len());
            }
            
            println!("✅ Conversion complete!");
        }
        Err(e) => {
            eprintln!("❌ Error opening file: {}", e);
        }
    }
}

fn convert_hwpml(input: &Path, output: &Path, format: &str, verbose: bool) {
    let xml = match fs::read_to_string(input) {
        Ok(xml) => xml,
        Err(e) => {
            eprintln!("❌ Error reading XML file: {}", e);
            return;
        }
    };

    let (version, title, content, sections) = match parse_hwpml(&xml) {
        Ok(parsed) => parsed,
        Err(e) => {
            eprintln!("❌ Error parsing HWPML: {}", e);
            return;
        }
    };

    fs::create_dir_all(output).expect("Failed to create output directory");
    let stem = input.file_stem().unwrap_or_default().to_string_lossy();

    match format {
        "json" => {
            let json_path = output.join(format!("{}.json", stem));
            let json_data = json!({
                "version": "1.0",
                "format": "hwpml",
                "metadata": {
                    "hwpml_version": version,
                    "title": title,
                    "sections": sections,
                },
                "content": content,
            });
            fs::write(&json_path, serde_json::to_string_pretty(&json_data).unwrap())
                .expect("Failed to write JSON");
            println!("  ✓ Created: {}", json_path.display());
        }
        _ => {
            let mdx_path = output.join(format!("{}.mdx", stem));
            let mdx_content = format!(
                "---\nformat: hwpml\nversion: \"{}\"\ntitle: \"{}\"\nsections: {}\n---\n\n{}",
                version,
                title.replace('"', "\\\""),
                sections,
                content
            );
            fs::write(&mdx_path, &mdx_content).expect("Failed to write MDX");
            println!("  ✓ Created: {}", mdx_path.display());

            let mdm_path = output.join(format!("{}.mdm", stem));
            let mdm_manifest = json!({
                "version": "1.0",
                "format": "hwpml",
                "source": input.file_name().unwrap_or_default().to_string_lossy(),
                "resources": {},
            });
            fs::write(&mdm_path, serde_json::to_string_pretty(&mdm_manifest).unwrap())
                .expect("Failed to write MDM manifest");
            println!("  ✓ Created: {}", mdm_path.display());
        }
    }

    if verbose {
        println!("\n📊 Summary:");
        println!("  - Format: HWPML (raw XML)");
        println!("  - Sections: {}", sections);
        println!("  - Text length: {} chars", content.len());
    }

    println!("✅ Conversion complete!");
}

fn parse_hwpml(xml: &str) -> Result<(String, String, String, usize), String> {
    let mut reader = Reader::from_str(xml);
    reader.trim_text(true);
    let mut buf = Vec::new();

    let mut version = String::from("unknown");
    let mut title = String::new();
    let mut in_title = false;
    let mut in_body = false;
    let mut in_char = false;
    let mut current_para = String::new();
    let mut paragraphs: Vec<String> = Vec::new();
    let mut sections = 0usize;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => match e.name().as_ref() {
                b"HWPML" => {
                    for attr in e.attributes().flatten() {
                        if attr.key.as_ref() == b"Version" {
                            version = attr
                                .unescape_value()
                                .map(|v| v.into_owned())
                                .unwrap_or_else(|_| "unknown".to_string());
                        }
                    }
                }
                b"TITLE" => in_title = true,
                b"BODY" => in_body = true,
                b"SECTION" if in_body => sections += 1,
                b"P" if in_body => current_para.clear(),
                b"CHAR" if in_body => in_char = true,
                _ => {}
            },
            Ok(Event::End(e)) => match e.name().as_ref() {
                b"TITLE" => in_title = false,
                b"BODY" => in_body = false,
                b"CHAR" => in_char = false,
                b"P" if in_body => {
                    let para = current_para.trim();
                    if !para.is_empty() {
                        paragraphs.push(para.to_string());
                    }
                    current_para.clear();
                }
                _ => {}
            },
            Ok(Event::Text(e)) => {
                let text = e.unescape().map(|t| t.into_owned()).unwrap_or_default();
                if in_title {
                    title.push_str(&text);
                }
                if in_body && in_char {
                    current_para.push_str(&text);
                }
            }
            Ok(Event::CData(e)) => {
                let text = String::from_utf8_lossy(e.as_ref()).into_owned();
                if in_title {
                    title.push_str(&text);
                }
                if in_body && in_char {
                    current_para.push_str(&text);
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(e.to_string()),
            _ => {}
        }
        buf.clear();
    }

    if sections == 0 {
        sections = 1;
    }
    let content = paragraphs.join("\n\n");
    if title.is_empty() {
        title = input_fallback_title(xml);
    }
    Ok((version, title, content, sections))
}

fn input_fallback_title(xml: &str) -> String {
    let start = xml.find("<TITLE>").map(|i| i + 7);
    let end = xml.find("</TITLE>");
    match (start, end) {
        (Some(s), Some(e)) if s <= e => xml[s..e].trim().to_string(),
        _ => "Untitled".to_string(),
    }
}

fn convert_hwpx(input: &Path, output: &Path, format: &str, extract_images: bool, verbose: bool) {
    match HwpxParser::open(input) {
        Ok(mut parser) => {
            fs::create_dir_all(output).expect("Failed to create output directory");

            match parser.parse() {
                Ok(doc) => {
                    let stem = input.file_stem().unwrap_or_default().to_string_lossy();

                    // Extract and save images if requested
                    let mut saved_images = Vec::new();
                    if extract_images && !doc.image_info.is_empty() {
                        let assets_dir = output.join("assets");
                        fs::create_dir_all(&assets_dir).expect("Failed to create assets directory");
                        
                        for img in &doc.image_info {
                            // Get filename from path
                            let filename = img.path.split('/').last().unwrap_or(&img.id);
                            let img_path = assets_dir.join(filename);
                            
                            if let Err(e) = fs::write(&img_path, &img.data) {
                                eprintln!("  ⚠️  Failed to save {}: {}", filename, e);
                            } else {
                                if verbose {
                                    println!("  📷 Saved: {} ({} bytes)", filename, img.data.len());
                                }
                                saved_images.push((img.id.clone(), filename.to_string(), img.media_type.clone(), img.data.len()));
                            }
                        }
                        println!("  ✓ Extracted {} images to assets/", saved_images.len());
                    }

                    // Use sections (with embedded tables) instead of preview text
                    let content = if doc.sections.iter().any(|s| !s.is_empty()) {
                        doc.sections.join("\n\n---\n\n")
                    } else if !doc.preview_text.is_empty() {
                        doc.preview_text.clone()
                    } else {
                        String::new()
                    };

                    match format {
                        "json" => {
                            let json_path = output.join(format!("{}.json", stem));
                            let json_data = json!({
                                "version": "1.0",
                                "format": "hwpx",
                                "metadata": {
                                    "hwpx_version": doc.version,
                                    "sections": doc.sections.len(),
                                },
                                "content": content,
                                "images": doc.image_info.iter().map(|i| json!({
                                    "id": i.id,
                                    "path": i.path,
                                    "mediaType": i.media_type,
                                    "size": i.data.len(),
                                })).collect::<Vec<_>>(),
                            });

                            fs::write(&json_path, serde_json::to_string_pretty(&json_data).unwrap())
                                .expect("Failed to write JSON");
                            println!("  ✓ Created: {}", json_path.display());
                        }
                        _ => {
                            // MDX format with image references
                            let mdx_path = output.join(format!("{}.mdx", stem));
                            
                            // Build image list for frontmatter
                            let image_yaml = if !saved_images.is_empty() {
                                let imgs: Vec<String> = saved_images.iter()
                                    .map(|(id, name, _, _)| format!("  - id: {}\n    src: ./assets/{}", id, name))
                                    .collect();
                                format!("\nimages:\n{}", imgs.join("\n"))
                            } else {
                                String::new()
                            };
                            
                            let mdx_content = format!(
                                "---\nformat: hwpx\nversion: \"{}\"\nsections: {}{}\n---\n\n{}",
                                doc.version,
                                doc.sections.len(),
                                image_yaml,
                                content
                            );

                            fs::write(&mdx_path, &mdx_content).expect("Failed to write MDX");
                            println!("  ✓ Created: {}", mdx_path.display());

                            // MDM manifest with full media information
                            let mdm_path = output.join(format!("{}.mdm", stem));
                            
                            // Build resources map
                            let resources: serde_json::Map<String, serde_json::Value> = saved_images.iter()
                                .map(|(id, name, media_type, size)| {
                                    (id.clone(), json!({
                                        "type": "image",
                                        "format": media_type.split('/').last().unwrap_or("unknown"),
                                        "mediaType": media_type,
                                        "src": format!("assets/{}", name),
                                        "size": size,
                                    }))
                                })
                                .collect();
                            
                            let mdm_manifest = json!({
                                "version": "1.0",
                                "format": "hwpx",
                                "source": input.file_name().unwrap_or_default().to_string_lossy(),
                                "resources": resources,
                            });

                            fs::write(&mdm_path, serde_json::to_string_pretty(&mdm_manifest).unwrap())
                                .expect("Failed to write MDM manifest");
                            println!("  ✓ Created: {}", mdm_path.display());
                        }
                    }

                    if verbose {
                        println!("\n📊 Summary:");
                        println!("  - Format: HWPX (ZIP-based)");
                        println!("  - Sections: {}", doc.sections.len());
                        println!("  - Tables: {}", doc.tables.len());
                        println!("  - Images: {} (extracted: {})", doc.image_info.len(), saved_images.len());
                        println!("  - Text length: {} chars", content.len());
                    }

                    println!("✅ Conversion complete!");
                }
                Err(e) => eprintln!("❌ Error parsing HWPX: {}", e),
            }
        }
        Err(e) => eprintln!("❌ Error opening HWPX file: {}", e),
    }
}

fn convert_pdf(input: &Path, output: &Path, format: &str, verbose: bool) {
    match PdfParser::open(input) {
        Ok(parser) => {
            fs::create_dir_all(output).expect("Failed to create output directory");

            match parser.parse() {
                Ok(doc) => {
                    let stem = input.file_stem().unwrap_or_default().to_string_lossy();

                    match format {
                        "json" => {
                            let json_path = output.join(format!("{}.json", stem));
                            let json_data = json!({
                                "version": "1.0",
                                "format": "pdf",
                                "metadata": {
                                    "pdf_version": doc.version,
                                    "pages": doc.page_count,
                                    "title": doc.metadata.title,
                                    "author": doc.metadata.author,
                                },
                                "content": doc.full_text(),
                                "pages": doc.pages.iter().map(|p| json!({
                                    "page": p.page_number,
                                    "text": p.text,
                                })).collect::<Vec<_>>(),
                            });

                            fs::write(&json_path, serde_json::to_string_pretty(&json_data).unwrap())
                                .expect("Failed to write JSON");
                            println!("  ✓ Created: {}", json_path.display());
                        }
                        _ => {
                            // MDX format
                            let mdx_path = output.join(format!("{}.mdx", stem));
                            fs::write(&mdx_path, doc.to_mdx()).expect("Failed to write MDX");
                            println!("  ✓ Created: {}", mdx_path.display());

                            // MDM manifest
                            let mdm_path = output.join(format!("{}.mdm", stem));
                            let mdm_manifest = json!({
                                "version": "1.0",
                                "format": "pdf",
                                "source": input.file_name().unwrap_or_default().to_string_lossy(),
                                "metadata": {
                                    "pdf_version": doc.version,
                                    "pages": doc.page_count,
                                    "title": doc.metadata.title,
                                    "author": doc.metadata.author,
                                },
                            });

                            fs::write(&mdm_path, serde_json::to_string_pretty(&mdm_manifest).unwrap())
                                .expect("Failed to write MDM manifest");
                            println!("  ✓ Created: {}", mdm_path.display());
                        }
                    }

                    if verbose {
                        println!("\n📊 Summary:");
                        println!("  - Format: PDF");
                        println!("  - Version: {}", doc.version);
                        println!("  - Pages: {}", doc.page_count);
                        if !doc.metadata.title.is_empty() {
                            println!("  - Title: {}", doc.metadata.title);
                        }
                        println!("  - Text length: {} chars", doc.full_text().len());
                    }

                    println!("✅ Conversion complete!");
                }
                Err(e) => eprintln!("❌ Error parsing PDF: {}", e),
            }
        }
        Err(e) => eprintln!("❌ Error opening PDF file: {}", e),
    }
}

fn convert_docx(input: &Path, output: &Path, format: &str, verbose: bool) {
    match DocxParser::open(input) {
        Ok(mut parser) => {
            fs::create_dir_all(output).expect("Failed to create output directory");

            match parser.parse() {
                Ok(doc) => {
                    let stem = input.file_stem().unwrap_or_default().to_string_lossy();
                    let source_name = input.file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| "document.docx".to_string());

                    // Save images
                    let assets_dir = output.join("assets");
                    if !doc.images.is_empty() {
                        fs::create_dir_all(&assets_dir).expect("Failed to create assets directory");
                        let mut saved = 0usize;
                        for image in &doc.images {
                            if let Some(ref data) = image.data {
                                let img_path = assets_dir.join(&image.filename);
                                if let Err(e) = fs::write(&img_path, data) {
                                    eprintln!("  ⚠️  Failed to save {}: {}", image.filename, e);
                                } else {
                                    saved += 1;
                                    if verbose {
                                        println!("  📷 Saved: {} ({} bytes)", image.filename, data.len());
                                    }
                                }
                            }
                        }
                        if saved > 0 {
                            println!("  ✓ Extracted {} images to assets/", saved);
                        }
                    }

                    match format {
                        "json" => {
                            let json_path = output.join(format!("{}.json", stem));
                            let json_data = json!({
                                "version": "1.0",
                                "format": "docx",
                                "metadata": {
                                    "title": doc.metadata.title,
                                    "author": doc.metadata.author,
                                    "subject": doc.metadata.subject,
                                    "pages": doc.metadata.page_count,
                                    "words": doc.metadata.word_count,
                                },
                                "content": doc.to_markdown(),
                                "tables": doc.tables.iter().map(|t| json!({
                                    "markdown": t.to_markdown(),
                                })).collect::<Vec<_>>(),
                                "images": doc.images.iter().map(|i| json!({
                                    "id": i.id,
                                    "filename": i.filename,
                                    "size": i.data.as_ref().map(|d| d.len()).unwrap_or(0),
                                })).collect::<Vec<_>>(),
                            });

                            fs::write(&json_path, serde_json::to_string_pretty(&json_data).unwrap())
                                .expect("Failed to write JSON");
                            println!("  ✓ Created: {}", json_path.display());
                        }
                        _ => {
                            // MDX format
                            let mdx_path = output.join(format!("{}.mdx", stem));
                            let mdx_content = doc.to_mdx(&source_name);
                            fs::write(&mdx_path, &mdx_content).expect("Failed to write MDX");
                            println!("  ✓ Created: {}", mdx_path.display());

                            // MDM manifest
                            let mdm_path = output.join(format!("{}.mdm", stem));
                            let resources: serde_json::Map<String, serde_json::Value> = doc.images.iter()
                                .map(|i| {
                                    (i.id.clone(), json!({
                                        "type": "image",
                                        "filename": i.filename,
                                        "src": format!("assets/{}", i.filename),
                                        "size": i.data.as_ref().map(|d| d.len()).unwrap_or(0),
                                    }))
                                })
                                .collect();

                            let mdm_manifest = json!({
                                "version": "1.0",
                                "format": "docx",
                                "source": source_name,
                                "metadata": {
                                    "title": doc.metadata.title,
                                    "author": doc.metadata.author,
                                },
                                "resources": resources,
                            });

                            fs::write(&mdm_path, serde_json::to_string_pretty(&mdm_manifest).unwrap())
                                .expect("Failed to write MDM manifest");
                            println!("  ✓ Created: {}", mdm_path.display());
                        }
                    }

                    if verbose {
                        println!("\n📊 Summary:");
                        println!("  - Format: DOCX");
                        if let Some(ref title) = doc.metadata.title {
                            println!("  - Title: {}", title);
                        }
                        println!("  - Paragraphs: {}", doc.paragraphs.len());
                        println!("  - Tables: {}", doc.tables.len());
                        println!("  - Images: {}", doc.images.len());
                        println!("  - Text length: {} chars", doc.text().len());
                    }

                    println!("✅ Conversion complete!");
                }
                Err(e) => eprintln!("❌ Error parsing DOCX: {}", e),
            }
        }
        Err(e) => eprintln!("❌ Error opening DOCX file: {}", e),
    }
}

fn analyze_file(input: &Path) {
    println!("🔍 Analyzing: {}", input.display());

    let ext = input.extension().and_then(|e| e.to_str()).unwrap_or("");
    if ext.eq_ignore_ascii_case("hwpx") {
        analyze_hwpx(input);
        return;
    }

    match HwpParser::open(input) {
        Ok(parser) => {
            let structure = parser.analyze();
            
            println!("\n📊 File Structure:");
            println!("  - Total streams: {}", structure.total_streams);
            println!("  - Sections: {}", structure.section_count);
            println!("  - BinData items: {}", structure.bin_data_count);
            println!("  - Compressed: {}", if structure.compressed { "Yes" } else { "No" });
            println!("  - Encrypted: {}", if structure.encrypted { "Yes ⚠️" } else { "No" });
            
            println!("\n📁 Streams:");
            for stream in &structure.streams {
                println!("  {}", stream);
            }
        }
        Err(e) => {
            eprintln!("❌ Error: {}", e);
        }
    }
}

fn analyze_hwpx(input: &Path) {
    match HwpxParser::open(input) {
        Ok(parser) => {
            println!("\n📊 File Structure:");
            println!("  - Format: HWPX (ZIP-based XML)");
            println!("  - Sections: {}", parser.section_count());
            println!("  - Compressed: Yes (ZIP)");
            println!("  - Encrypted: {}", if parser.is_encrypted() { "Yes ⚠️" } else { "No" });
        }
        Err(e) => eprintln!("❌ Error: {}", e),
    }
}

fn extract_text(input: &Path) {
    let ext = input.extension().and_then(|e| e.to_str()).unwrap_or("");
    if ext.eq_ignore_ascii_case("hwpx") {
        match HwpxParser::open(input) {
            Ok(mut parser) => match parser.parse() {
                Ok(doc) => {
                    if !doc.preview_text.is_empty() {
                        println!("{}", doc.preview_text);
                    } else {
                        for section in &doc.sections {
                            println!("{}", section);
                        }
                    }
                }
                Err(e) => eprintln!("❌ Error: {}", e),
            },
            Err(e) => eprintln!("❌ Error: {}", e),
        }
        return;
    }

    match HwpParser::open(input) {
        Ok(mut parser) => {
            match parser.extract_text() {
                Ok(text) => println!("{}", text),
                Err(e) => eprintln!("❌ Error extracting text: {}", e),
            }
        }
        Err(e) => {
            eprintln!("❌ Error: {}", e);
        }
    }
}

fn extract_images(input: &Path, output: &Path) {
    println!("📷 Extracting images from: {}", input.display());
    
    match HwpParser::open(input) {
        Ok(mut parser) => {
            fs::create_dir_all(output).expect("Failed to create output directory");
            
            match parser.extract_images() {
                Ok(images) => {
                    if images.is_empty() {
                        println!("  No images found.");
                        return;
                    }
                    
                    for img in &images {
                        let img_path = output.join(&img.name);
                        match fs::write(&img_path, &img.data) {
                            Ok(_) => println!("  ✓ {}", img.name),
                            Err(e) => println!("  ❌ {} - {}", img.name, e),
                        }
                    }
                    
                    println!("\n✅ Extracted {} images to {}", images.len(), output.display());
                }
                Err(e) => eprintln!("❌ Error: {}", e),
            }
        }
        Err(e) => {
            eprintln!("❌ Error: {}", e);
        }
    }
}

fn batch_convert(pattern: &str, output: &Path) {
    println!("📦 Batch converting: {}", pattern);
    
    // Simple glob matching using walkdir
    let base_dir = Path::new(".");
    let mut count = 0;
    let mut errors = 0;
    
    if let Ok(entries) = fs::read_dir(base_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().map_or(false, |e| e == "hwp") {
                let file_name = path.file_name().unwrap_or_default().to_string_lossy();
                
                // Simple pattern matching
                if pattern == "*.hwp" || file_name.contains(pattern.trim_start_matches('*').trim_end_matches("*.hwp")) {
                    println!("\n  Processing: {}", path.display());
                    
                    if let Err(_) = std::panic::catch_unwind(|| {
                        convert_file(&path, output, "mdx", true, false);
                    }) {
                        errors += 1;
                    } else {
                        count += 1;
                    }
                }
            }
        }
    }
    
    println!("\n📊 Batch complete: {} converted, {} errors", count, errors);
}

fn show_info(input: &Path, format: &str) {
    let ext = input.extension().and_then(|e| e.to_str()).unwrap_or("");
    
    // Get file metadata
    let file_size = fs::metadata(input)
        .map(|m| m.len())
        .unwrap_or(0);
    
    let file_size_str = if file_size >= 1024 * 1024 {
        format!("{:.2} MB", file_size as f64 / (1024.0 * 1024.0))
    } else if file_size >= 1024 {
        format!("{:.2} KB", file_size as f64 / 1024.0)
    } else {
        format!("{} bytes", file_size)
    };
    
    match ext.to_lowercase().as_str() {
        "hwpx" => show_hwpx_info(input, format, &file_size_str),
        "pdf" => show_pdf_info(input, format, &file_size_str),
        "docx" => show_docx_info(input, format, &file_size_str),
        _ => show_hwp_info(input, format, &file_size_str),
    }
}

fn show_hwp_info(input: &Path, format: &str, file_size: &str) {
    match HwpParser::open(input) {
        Ok(parser) => {
            let structure = parser.analyze();
            
            if format == "json" {
                let info = json!({
                    "file": {
                        "name": input.file_name().unwrap_or_default().to_string_lossy(),
                        "path": input.display().to_string(),
                        "size": file_size,
                        "format": "hwp",
                    },
                    "document": {
                        "sections": structure.section_count,
                        "streams": structure.total_streams,
                        "bin_data_count": structure.bin_data_count,
                        "compressed": structure.compressed,
                        "encrypted": structure.encrypted,
                    },
                    "streams": structure.streams,
                });
                println!("{}", serde_json::to_string_pretty(&info).unwrap());
            } else {
                println!("📄 File Information");
                println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
                println!("  Name:       {}", input.file_name().unwrap_or_default().to_string_lossy());
                println!("  Path:       {}", input.display());
                println!("  Size:       {}", file_size);
                println!("  Format:     HWP (OLE Compound Document)");
                println!();
                println!("📊 Document Structure");
                println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
                println!("  Sections:     {}", structure.section_count);
                println!("  Streams:      {}", structure.total_streams);
                println!("  BinData:      {} items", structure.bin_data_count);
                println!("  Compressed:   {}", if structure.compressed { "Yes" } else { "No" });
                println!("  Encrypted:    {}", if structure.encrypted { "Yes ⚠️" } else { "No" });
                println!();
                println!("📁 Streams ({}):", structure.streams.len());
                println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
                for stream in &structure.streams {
                    println!("  • {}", stream);
                }
            }
        }
        Err(e) => eprintln!("❌ Error: {}", e),
    }
}

fn show_hwpx_info(input: &Path, format: &str, file_size: &str) {
    match HwpxParser::open(input) {
        Ok(parser) => {
            let section_count = parser.section_count();
            let encrypted = parser.is_encrypted();
            
            if format == "json" {
                let info = json!({
                    "file": {
                        "name": input.file_name().unwrap_or_default().to_string_lossy(),
                        "path": input.display().to_string(),
                        "size": file_size,
                        "format": "hwpx",
                    },
                    "document": {
                        "sections": section_count,
                        "compressed": true,
                        "encrypted": encrypted,
                    },
                });
                println!("{}", serde_json::to_string_pretty(&info).unwrap());
            } else {
                println!("📄 File Information");
                println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
                println!("  Name:       {}", input.file_name().unwrap_or_default().to_string_lossy());
                println!("  Path:       {}", input.display());
                println!("  Size:       {}", file_size);
                println!("  Format:     HWPX (ZIP-based XML)");
                println!();
                println!("📊 Document Structure");
                println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
                println!("  Sections:     {}", section_count);
                println!("  Compressed:   Yes (ZIP container)");
                println!("  Encrypted:    {}", if encrypted { "Yes ⚠️" } else { "No" });
            }
        }
        Err(e) => eprintln!("❌ Error: {}", e),
    }
}

fn show_docx_info(input: &Path, format: &str, file_size: &str) {
    match DocxParser::open(input) {
        Ok(mut parser) => {
            match parser.parse() {
                Ok(doc) => {
                    if format == "json" {
                        let info = json!({
                            "file": {
                                "name": input.file_name().unwrap_or_default().to_string_lossy(),
                                "path": input.display().to_string(),
                                "size": file_size,
                                "format": "docx",
                            },
                            "document": {
                                "title": doc.metadata.title,
                                "author": doc.metadata.author,
                                "subject": doc.metadata.subject,
                                "pages": doc.metadata.page_count,
                                "words": doc.metadata.word_count,
                                "paragraphs": doc.paragraphs.len(),
                                "tables": doc.tables.len(),
                                "images": doc.images.len(),
                            },
                        });
                        println!("{}", serde_json::to_string_pretty(&info).unwrap());
                    } else {
                        println!("📄 File Information");
                        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
                        println!("  Name:       {}", input.file_name().unwrap_or_default().to_string_lossy());
                        println!("  Path:       {}", input.display());
                        println!("  Size:       {}", file_size);
                        println!("  Format:     DOCX (Office Open XML)");
                        println!();
                        println!("📊 Document Properties");
                        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
                        if let Some(ref title) = doc.metadata.title {
                            println!("  Title:        {}", title);
                        }
                        if let Some(ref author) = doc.metadata.author {
                            println!("  Author:       {}", author);
                        }
                        if let Some(pages) = doc.metadata.page_count {
                            println!("  Pages:        {}", pages);
                        }
                        if let Some(words) = doc.metadata.word_count {
                            println!("  Words:        {}", words);
                        }
                        println!("  Paragraphs:  {}", doc.paragraphs.len());
                        println!("  Tables:       {}", doc.tables.len());
                        println!("  Images:       {}", doc.images.len());
                    }
                }
                Err(e) => eprintln!("❌ Error parsing DOCX: {}", e),
            }
        }
        Err(e) => eprintln!("❌ Error: {}", e),
    }
}

fn show_pdf_info(input: &Path, format: &str, file_size: &str) {
    match PdfParser::open(input) {
        Ok(parser) => {
            match parser.parse() {
                Ok(doc) => {
                    if format == "json" {
                        let info = json!({
                            "file": {
                                "name": input.file_name().unwrap_or_default().to_string_lossy(),
                                "path": input.display().to_string(),
                                "size": file_size,
                                "format": "pdf",
                            },
                            "document": {
                                "version": doc.version,
                                "pages": doc.page_count,
                                "title": doc.metadata.title,
                                "author": doc.metadata.author,
                                "creator": doc.metadata.creator,
                                "producer": doc.metadata.producer,
                            },
                        });
                        println!("{}", serde_json::to_string_pretty(&info).unwrap());
                    } else {
                        println!("📄 File Information");
                        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
                        println!("  Name:       {}", input.file_name().unwrap_or_default().to_string_lossy());
                        println!("  Path:       {}", input.display());
                        println!("  Size:       {}", file_size);
                        println!("  Format:     PDF");
                        println!();
                        println!("📊 Document Properties");
                        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
                        println!("  PDF Version:  {}", doc.version);
                        println!("  Pages:        {}", doc.page_count);
                        if !doc.metadata.title.is_empty() {
                            println!("  Title:        {}", doc.metadata.title);
                        }
                        if !doc.metadata.author.is_empty() {
                            println!("  Author:       {}", doc.metadata.author);
                        }
                        if !doc.metadata.creator.is_empty() {
                            println!("  Creator:      {}", doc.metadata.creator);
                        }
                        if !doc.metadata.producer.is_empty() {
                            println!("  Producer:     {}", doc.metadata.producer);
                        }
                    }
                }
                Err(e) => eprintln!("❌ Error parsing PDF: {}", e),
            }
        }
        Err(e) => eprintln!("❌ Error: {}", e),
    }
}
