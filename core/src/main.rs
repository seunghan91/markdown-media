//! hwp2mdm - HWP to MDM converter CLI tool

mod hwp;
mod pdf;

use clap::{Parser, Subcommand};
use hwp::HwpParser;
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

fn analyze_file(input: &Path) {
    println!("🔍 Analyzing: {}", input.display());
    
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

fn extract_text(input: &Path) {
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
