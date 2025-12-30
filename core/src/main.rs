//! hwp2mdm - HWP/HWPX/PDF to MDM converter CLI tool

mod hwp;
mod hwpx;
mod pdf;

use clap::{Parser, Subcommand};
use hwp::HwpParser;
use hwpx::HwpxParser;
use pdf::PdfParser;
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

    // Handle HWPX (ZIP-based)
    if ext.eq_ignore_ascii_case("hwpx") {
        convert_hwpx(input, output, format, extract_images, verbose);
        return;
    }

    // Handle PDF
    if ext.eq_ignore_ascii_case("pdf") {
        convert_pdf(input, output, format, verbose);
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
