//! hwp2mdm - HWP/HWPX/PDF to MDM converter CLI tool

#![allow(dead_code, unused_imports, unused_variables, unreachable_patterns, unused_assignments)]

mod docx;
mod hwp;
mod hwpx;
mod hwpx_gen;
mod hwp3;
mod ir;
mod equation;
mod pii;
mod form;
mod lint;
mod chunker;
mod legal;
#[cfg(feature = "watch")]
mod watch;
#[cfg(feature = "ocr")]
mod ocr;
mod manifest;
mod pdf;
mod xlsx;
#[cfg(feature = "xls")]
mod xls;
#[cfg(feature = "rtf")]
mod rtf;
#[cfg(feature = "epub")]
mod epub;
mod pptx;
#[cfg(feature = "url-fetch")]
mod url_fetch;
mod doc97;
mod heic;
#[cfg(feature = "docx-out")]
mod gen_docx;
mod html;
mod csv_parser;
mod txt_parser;
mod utils;

use clap::{Parser, Subcommand};
use std::collections::HashMap;
use std::io::Read as _;
use std::io::Write as _;
use docx::DocxParser;
use hwp::HwpParser;
use hwpx::HwpxParser;
use manifest::{ManifestV2, MediaType, AssetMetadata};
use pdf::PdfParser;
use xlsx::XlsxParser;
use pptx::PptxParser;
use html::HtmlParser;
use csv_parser::CsvParser;
use txt_parser::TxtParser;
use quick_xml::events::Event;
use quick_xml::Reader;
use serde_json::json;
use std::fs;
use std::io;
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

    /// Number of threads for parallel PDF processing (0 = auto-detect CPU cores)
    #[arg(short = 'j', long, default_value = "0")]
    threads: usize,

    /// Enable OCR for scanned/image-based PDF pages
    #[arg(long)]
    ocr: bool,
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

        /// Enable OCR for scanned/image-based pages
        #[arg(long)]
        ocr: bool,
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

    /// Inspect document structure: tables (merged cells, nested), equations, images.
    ///
    /// Shows what the converter extracts and what gets lost in Markdown output.
    /// Useful for debugging conversion quality.
    ///
    /// Example:
    ///   hwp2mdm inspect report.hwp
    ///   hwp2mdm inspect report.hwp --format json
    Inspect {
        /// Input file
        input: PathBuf,

        /// Output format (text, json)
        #[arg(short, long, default_value = "text")]
        format: String,
    },

    /// Dump per-element layout as JSON (y, type, content, font, position).
    ///
    /// This is the structured output the Python triage router uses to
    /// position-merge OCR results on Mixed pages — the manifest gives
    /// it WHERE the image regions are, and this gives it WHERE the
    /// existing text blocks are, so figure captions can be inserted
    /// at the correct Y position in the reading order instead of
    /// appended to the end of the page.
    ///
    /// Example:
    ///   hwp2mdm layout report.pdf -o layout.json
    Layout {
        /// Input PDF file.
        input: PathBuf,

        /// Write layout JSON to this path instead of stdout.
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Restrict to a single 1-indexed page (omit for all pages).
        #[arg(long)]
        page: Option<usize>,
    },

    /// Classify PDF pages before extraction — emit an OCR routing manifest.
    ///
    /// For every page decides: text-native (extract directly), scanned
    /// (full-page OCR), or mixed (extract text + OCR the image regions).
    /// The Python bridge consumes the manifest JSON to route pages to the
    /// appropriate engine (Tesseract / EasyOCR / OpenRouter VLM).
    ///
    /// Example:
    ///   hwp2mdm triage report.pdf                    # pretty text
    ///   hwp2mdm triage report.pdf --format json      # manifest JSON
    ///   hwp2mdm triage report.pdf -o manifest.json   # write to file
    Triage {
        /// Input PDF file.
        input: PathBuf,

        /// Output format: `text` (human-readable table) or `json` (manifest).
        #[arg(short, long, default_value = "text")]
        format: String,

        /// Write output to this path instead of stdout.
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Read a document from stdin and emit Markdown to stdout.
    ///
    /// Designed for MCP/subprocess integration: pipe bytes in, get clean
    /// Markdown out. Internally uses a temp file so every parser works
    /// (HWP needs OLE random access, PDF needs seeking); status messages
    /// are suppressed so stdout contains ONLY the Markdown body.
    ///
    /// Example:
    ///   cat contract.hwp | hwp2mdm stream --ext hwp > out.md
    Stream {
        /// File extension hint (hwp, hwpx, pdf, docx, pptx, xlsx, xls, rtf, epub, html, csv, tsv, txt).
        /// Required because stdin has no filename for auto-detection.
        #[arg(long)]
        ext: String,

        /// Output variant: `mdx` (full Markdown + YAML frontmatter) or
        /// `body` (Markdown content only, no frontmatter).
        #[arg(long, default_value = "mdx")]
        mode: String,
    },

    /// Generate HWPX from Markdown — Korean government document presets included.
    ///
    /// Converts Markdown text to a .hwpx file with proper formatting.
    /// Supports 7 Korean government document presets:
    /// 기안문, 보고서, 계획서, 통지, 회의록, 개조식, 보도자료.
    ///
    /// Example:
    ///   hwp2mdm generate report.md -o report.hwpx
    ///   hwp2mdm generate - --preset 기안문 < draft.md > memo.hwpx
    Generate {
        /// Input Markdown file (use '-' for stdin)
        input: PathBuf,

        /// Output file path (auto-detects format from extension)
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Output format: hwpx (default), docx, pdf — auto-detected from output extension
        #[arg(short = 'F', long, default_value = "hwpx")]
        format: String,

        /// Government document preset (기안문, 보고서, 계획서, 통지, 회의록, 개조식, 보도자료)
        #[arg(short, long)]
        preset: Option<String>,
    },

    /// Detect and mask PII (personal info) in documents.
    ///
    /// Masks resident registration numbers, phone numbers, emails,
    /// credit card numbers, bank account numbers, and more.
    /// Outputs clean text with format-preserving masking (●●●).
    ///
    /// Example:
    ///   hwp2mdm redact document.md
    ///   hwp2mdm redact contract.hwp --rules rrn,phone,email
    Redact {
        /// Input file
        input: PathBuf,

        /// Comma-separated rules: rrn,phone,email,card,account,passport,driver
        /// Default: rrn,phone,email,card,account
        #[arg(short, long, default_value = "rrn,phone,email,card,account")]
        rules: String,

        /// Write output to file instead of stdout
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Compare two documents — cross-format diff.
    ///
    /// Produces a structured diff (added, removed, modified, unchanged blocks)
    /// with similarity scores and cell-level table deltas.
    ///
    /// Example:
    ///   hwp2mdm diff draft_v1.hwp draft_v2.hwp
    ///   hwp2mdm diff original.pdf revised.docx
    Diff {
        /// First document (original)
        input_a: PathBuf,

        /// Second document (revised)
        input_b: PathBuf,

        /// Output format: text (human-readable) or json (structured)
        #[arg(short, long, default_value = "text")]
        format: String,
    },

    /// Fill form fields in a template document.
    ///
    /// Reads a HWPX template, extracts form fields, and fills them
    /// with values from a JSON file. Preserves original formatting 100%.
    ///
    /// Example:
    ///   hwp2mdm fill template.hwpx -j values.json -o filled.hwpx
    Fill {
        /// Input HWPX template file
        input: PathBuf,

        /// JSON file with field values (not required with --dry-run)
        #[arg(short = 'j', long)]
        values: Option<PathBuf>,

        /// Output file path (default: stdout)
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Dry-run: show detected form fields (JSON) without filling
        #[arg(long)]
        dry_run: bool,
    },

    /// Lint Korean government document notation.
    ///
    /// Checks 13 notation rules based on Korean administrative manuals:
    /// date formats, time formats, currency notation, attachment markers, etc.
    ///
    /// Example:
    ///   hwp2mdm lint report.hwpx
    Lint {
        /// Input file
        input: PathBuf,
    },

    /// Split document into RAG-friendly chunks with breadcrumb hierarchy.
    ///
    /// Structural chunking that preserves heading context, list depth,
    /// and table structure. Outputs JSON with breadcrumb-annotated chunks.
    ///
    /// Example:
    ///   hwp2mdm chunks document.hwp
    ///   hwp2mdm chunks document.hwp --granularity block --max-chars 2000
    Chunks {
        /// Input file
        input: PathBuf,

        /// Granularity: section (merge under same heading) or block (1:1)
        #[arg(short, long, default_value = "section")]
        granularity: String,

        /// Max characters per chunk (0 = disabled)
        #[arg(long, default_value = "0")]
        max_chars: usize,

        /// Overlap characters between chunks
        #[arg(long, default_value = "100")]
        overlap: usize,
    },

    /// Watch a directory and auto-convert documents on change.
    ///
    /// Monitors a directory for new/modified documents and converts them
    /// automatically. Optional webhook notification on each conversion.
    ///
    /// Example:
    ///   hwp2mdm watch ./incoming -o ./output
    ///   hwp2mdm watch ./incoming --webhook https://hooks.example/convert
    #[cfg(feature = "watch")]
    Watch {
        /// Directory to watch
        dir: PathBuf,

        /// Output directory for converted files
        #[arg(short, long, default_value = "./output")]
        output: PathBuf,

        /// Webhook URL for conversion notifications
        #[arg(long)]
        webhook: Option<String>,
    },

    /// Parse Korean legal documents with hierarchy detection.
    ///
    /// Detects the full hierarchy: 편(Part) > 장(Chapter) > 절(Section) >
    /// 관(SubSection) > 조(Article) > 항(Paragraph) > 호(Subparagraph) > 목(Item).
    /// Outputs structured JSON chunks with breadcrumb metadata.
    ///
    /// Example:
    ///   hwp2mdm legal law.md
    ///   hwp2mdm legal law.md --format json
    Legal {
        /// Input markdown file
        input: PathBuf,

        /// Output format: text (human-readable) or json (structured)
        #[arg(short, long, default_value = "text")]
        format: String,
    },

    /// Fetch and convert a web page to Markdown.
    ///
    /// Downloads a URL, extracts the main content body (stripping navigation,
    /// ads, sidebars), and outputs clean Markdown. Supports multiple URLs.
    ///
    /// Example:
    ///   hwp2mdm url https://example.com/article
    ///   hwp2mdm url https://a.com https://b.com -o ./output
    #[cfg(feature = "url-fetch")]
    Url {
        /// URLs to fetch
        urls: Vec<String>,

        /// Output directory (default: stdout)
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Validate HWPX file structure (ZIP integrity, manifest, XML well-formedness).
    ///
    /// Example:
    ///   hwp2mdm validate document.hwpx
    Validate {
        /// Input HWPX file
        input: PathBuf,
    },

    /// Convert between HULK (HWP equation script) and LaTeX.
    ///
    /// Reads a file containing HULK equation script and outputs LaTeX.
    ///
    /// Example:
    ///   hwp2mdm equation input.hulk
    Equation {
        /// Input file with HULK script or LaTeX
        input: PathBuf,

        /// Direction: hulk2latex (default) or latex2hulk
        #[arg(short, long, default_value = "hulk2latex")]
        direction: String,
    },
}

fn main() {
    heic::register();
    let cli = Cli::parse();

    // Configure Rayon global thread pool for parallel PDF processing
    let threads = if cli.threads == 0 { num_cpus::get() } else { cli.threads };
    rayon::ThreadPoolBuilder::new()
        .num_threads(threads)
        .build_global()
        .ok(); // Ignore if already initialized

    match cli.command {
        Some(Commands::Convert { input, output, format, extract_images, ocr }) => {
            convert_file(&input, &output, &format, extract_images, true, ocr);
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
        Some(Commands::Inspect { input, format }) => {
            inspect_file(&input, &format);
        }
        Some(Commands::Layout { input, output, page }) => {
            if let Err(e) = dump_layout(&input, output.as_deref(), page) {
                eprintln!("error: {}", e);
                std::process::exit(1);
            }
        }
        Some(Commands::Triage { input, format, output }) => {
            if let Err(e) = triage_pdf(&input, &format, output.as_deref()) {
                eprintln!("error: {}", e);
                std::process::exit(1);
            }
        }
        Some(Commands::Stream { ext, mode }) => {
            if let Err(e) = stream_convert(&ext, &mode) {
                eprintln!("error: {}", e);
                std::process::exit(1);
            }
        }
        Some(Commands::Generate { input, output, format, preset }) => {
            cmd_generate(&input, output.as_deref(), &format, preset.as_deref());
        }
        Some(Commands::Redact { input, rules, output }) => {
            cmd_redact(&input, output.as_deref(), &rules);
        }
        Some(Commands::Diff { input_a, input_b, format }) => {
            cmd_diff(&input_a, &input_b, &format);
        }
        Some(Commands::Fill { input, values, output, dry_run }) => {
            cmd_fill(&input, values.as_deref(), output.as_deref(), dry_run);
        }
        Some(Commands::Lint { input }) => {
            cmd_lint(&input);
        }
        Some(Commands::Chunks { input, granularity, max_chars, overlap }) => {
            cmd_chunks(&input, &granularity, max_chars, overlap);
        }
        #[cfg(feature = "watch")]
        Some(Commands::Watch { dir, output, webhook }) => {
            cmd_watch(&dir, &output, webhook.as_deref());
        }
        Some(Commands::Legal { input, format }) => {
            cmd_legal(&input, &format);
        }
        #[cfg(feature = "url-fetch")]
        Some(Commands::Url { urls, output }) => {
            cmd_url(&urls, output.as_deref());
        }
        Some(Commands::Validate { input }) => {
            cmd_validate(&input);
        }
        Some(Commands::Equation { input, direction }) => {
            cmd_equation(&input, &direction);
        }
        None => {
            // Quick conversion mode
            if let Some(input) = cli.input {
                convert_file(&input, &cli.output, &cli.format, cli.extract_images, cli.verbose, cli.ocr);
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

/// Peek inside a ZIP file to determine if it's DOCX or HWPX.
fn detect_zip_format(path: &Path) -> String {
    let file = match std::fs::File::open(path) {
        Ok(f) => f,
        Err(_) => return "unknown".to_string(),
    };
    let reader = std::io::BufReader::new(file);
    let mut archive = match zip::ZipArchive::new(reader) {
        Ok(a) => a,
        Err(_) => return "unknown".to_string(),
    };
    for i in 0..archive.len().min(30) {
        if let Ok(mut f) = archive.by_index(i) {
            let name = f.name().to_string();
            if name.starts_with("word/") {
                return "docx".to_string();
            }
            if name.starts_with("ppt/") {
                return "pptx".to_string();
            }
            if name.starts_with("xl/") {
                return "xlsx".to_string();
            }
            if name.starts_with("Contents/") || name.starts_with("META-INF/") {
                return "hwpx".to_string();
            }
            if name == "mimetype" {
                let mut buf = Vec::new();
                if f.read_to_end(&mut buf).is_ok() {
                    let mt = String::from_utf8_lossy(&buf);
                    if mt.contains("application/epub+zip") {
                        return "epub".to_string();
                    }
                }
            }
            if name.starts_with("META-INF/container.xml") || name.ends_with(".opf") {
                return "epub".to_string();
            }
        }
    }
    "unknown".to_string()
}

/// Save ManifestV2 JSON as `.mdm` and create the assets directory structure.
fn save_manifest(manifest: &ManifestV2, output_dir: &Path, stem: &str) -> io::Result<()> {
    let mdm_path = output_dir.join(format!("{}.mdm", stem));
    let json = manifest.to_json().map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
    fs::write(&mdm_path, json)?;
    println!("  \u{2713} Created: {}", mdm_path.display());
    Ok(())
}

/// Save a single asset file under `output_dir` using the asset's `src` path.
fn save_asset_file(output_dir: &Path, asset: &manifest::Asset, data: &[u8]) -> io::Result<()> {
    let full_path = output_dir.join(&asset.src);
    if let Some(parent) = full_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&full_path, data)?;
    Ok(())
}

/// RAII guard that redirects stdout to /dev/null for its lifetime.
///
/// Used by `stream_convert` so that the heavily-println-heavy `convert_*`
/// functions can run without polluting the Markdown payload we're writing
/// to the real stdout at the end. Restore happens in Drop.
///
/// Unix-only; on other platforms the struct is a no-op. Windows users
/// should prefer the default `convert` subcommand (file I/O).
#[cfg(unix)]
struct StdoutSilencer {
    saved_fd: libc::c_int,
}

#[cfg(unix)]
impl StdoutSilencer {
    fn new() -> io::Result<Self> {
        use std::os::fd::AsRawFd;
        let saved_fd = unsafe { libc::dup(libc::STDOUT_FILENO) };
        if saved_fd < 0 {
            return Err(io::Error::last_os_error());
        }
        let devnull = fs::File::create("/dev/null")?;
        let rc = unsafe { libc::dup2(devnull.as_raw_fd(), libc::STDOUT_FILENO) };
        if rc < 0 {
            let err = io::Error::last_os_error();
            unsafe { libc::close(saved_fd) };
            return Err(err);
        }
        Ok(Self { saved_fd })
    }
}

#[cfg(unix)]
impl Drop for StdoutSilencer {
    fn drop(&mut self) {
        // Ensure any buffered writes to the silenced stdout are flushed
        // before we restore — otherwise cached bytes land on the real
        // stdout after restore.
        let _ = io::stdout().flush();
        unsafe {
            libc::dup2(self.saved_fd, libc::STDOUT_FILENO);
            libc::close(self.saved_fd);
        }
    }
}

#[cfg(not(unix))]
struct StdoutSilencer;

#[cfg(not(unix))]
impl StdoutSilencer {
    fn new() -> io::Result<Self> { Ok(Self) }
}

/// Read a document from stdin, convert to Markdown, write to stdout.
///
/// Reuses the existing `convert_file` dispatcher internally via a temp
/// file so every supported format (including HWP which needs random
/// access) works without duplicating detection/dispatch logic.
fn stream_convert(ext: &str, mode: &str) -> io::Result<()> {
    // 1. Read stdin bytes.
    let mut bytes = Vec::new();
    io::stdin().read_to_end(&mut bytes)?;
    if bytes.is_empty() {
        return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "stdin was empty"));
    }

    // 2. Materialize as tempfile with the right extension.
    let ext_clean = ext.trim_start_matches('.').to_ascii_lowercase();
    let tmp = tempfile::tempdir()?;
    let in_path = tmp.path().join(format!("input.{}", ext_clean));
    fs::write(&in_path, &bytes)?;

    let out_dir = tmp.path().join("out");
    fs::create_dir(&out_dir)?;

    // 3. Run the existing converter with stdout redirected to /dev/null.
    {
        let _silencer = StdoutSilencer::new()?;
        convert_file(&in_path, &out_dir, "mdx", false, false, false);
    } // stdout restored here

    // 4. Pick up the produced .mdx.
    let mdx_path = fs::read_dir(&out_dir)?
        .flatten()
        .find(|e| e.path().extension().and_then(|s| s.to_str()) == Some("mdx"))
        .map(|e| e.path())
        .ok_or_else(|| io::Error::new(
            io::ErrorKind::NotFound,
            format!("no .mdx produced for ext='{}' (unsupported format or conversion failed)", ext_clean),
        ))?;
    let content = fs::read_to_string(&mdx_path)?;

    // 5. Emit on real stdout — body-only if requested.
    let emitted = match mode {
        "body" | "text" => strip_frontmatter(&content),
        _ => content,
    };
    io::stdout().write_all(emitted.as_bytes())?;
    Ok(())
}

/// Remove a leading `---\n...\n---\n` YAML frontmatter block, if present.
fn strip_frontmatter(s: &str) -> String {
    let bytes = s.as_bytes();
    if !s.starts_with("---\n") { return s.to_string(); }
    // Find the closing delimiter line.
    if let Some(end) = s[4..].find("\n---\n") {
        let body_start = 4 + end + "\n---\n".len();
        if body_start <= bytes.len() {
            return s[body_start..].trim_start().to_string();
        }
    }
    s.to_string()
}

fn convert_file(input: &Path, output: &Path, format: &str, extract_images: bool, verbose: bool, ocr: bool) {
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

    // PDF magic takes priority — some files have wrong extensions (e.g. .hwpx but actually PDF)
    if is_pdf {
        convert_pdf(input, output, format, verbose, ocr);
        return;
    }

    // ZIP-based formats: peek inside to distinguish DOCX vs HWPX vs EPUB
    if is_zip {
        // Check internal structure to determine actual format
        let actual = detect_zip_format(input);
        match actual.as_str() {
            "docx" => { convert_docx(input, output, format, verbose); return; }
            "hwpx" => { convert_hwpx(input, output, format, extract_images, verbose); return; }
            "pptx" => { convert_pptx(input, output, format, verbose); return; }
            "xlsx" => { convert_xlsx(input, output, format, verbose); return; }
            "epub" => { convert_epub(input, output, format, verbose); return; }
            _ => {
                // Fallback to extension for ZIP-based formats
    if ext.eq_ignore_ascii_case("doc") {
        convert_doc97(input, output, format, verbose);
        return;
    }
    if ext.eq_ignore_ascii_case("docx") {
                    convert_docx(input, output, format, verbose);
                } else if ext.eq_ignore_ascii_case("pptx") {
                    convert_pptx(input, output, format, verbose);
                } else if ext.eq_ignore_ascii_case("xlsx") || ext.eq_ignore_ascii_case("xls") {
                    convert_xlsx(input, output, format, verbose);
                } else if ext.eq_ignore_ascii_case("epub") {
                    convert_epub(input, output, format, verbose);
                } else {
                    convert_hwpx(input, output, format, extract_images, verbose);
                }
                return;
            }
        }
    }

    // Extension-based fallback for non-magic-detected files
    if ext.eq_ignore_ascii_case("rtf") || magic.starts_with(b"{\\rtf") {
        convert_rtf(input, output, format, verbose);
        return;
    }
    if ext.eq_ignore_ascii_case("docx") {
        convert_docx(input, output, format, verbose);
        return;
    }
    if ext.eq_ignore_ascii_case("hwpx") {
        convert_hwpx(input, output, format, extract_images, verbose);
        return;
    }
    if ext.eq_ignore_ascii_case("pdf") {
        convert_pdf(input, output, format, verbose, ocr);
        return;
    }
    if ext.eq_ignore_ascii_case("xlsx") || ext.eq_ignore_ascii_case("xls") {
        convert_xlsx(input, output, format, verbose);
        return;
    }
    if ext.eq_ignore_ascii_case("pptx") {
        convert_pptx(input, output, format, verbose);
        return;
    }
    if ext.eq_ignore_ascii_case("epub") {
        convert_epub(input, output, format, verbose);
        return;
    }
    if ext.eq_ignore_ascii_case("html") || ext.eq_ignore_ascii_case("htm") || ext.eq_ignore_ascii_case("mhtml") {
        convert_html(input, output, format, verbose);
        return;
    }
    if ext.eq_ignore_ascii_case("csv") || ext.eq_ignore_ascii_case("tsv") {
        convert_csv(input, output, format, verbose);
        return;
    }
    if ext.eq_ignore_ascii_case("txt") || ext.eq_ignore_ascii_case("text") || ext.eq_ignore_ascii_case("log") {
        convert_txt(input, output, format, verbose);
        return;
    }

    // Neither ZIP nor PDF nor CFB → unknown
    // Detect known unsupported formats with friendly messages BEFORE the
    // confusing 'invalid CFB magic' error fires.
    if is_cfb {
        #[cfg(feature = "xls")]
        {
            if let Ok(data) = std::fs::read(input) {
                if xls::looks_like_xls(&data) {
                    convert_xls(input, output, format, verbose);
                    return;
                }
            }
        }
        // HWP3 detection: same CFB magic, but different internal structure.
        // Check after XLS (which also uses CFB) to avoid false positives.
        if let Ok(data) = std::fs::read(input) {
            if hwp3::is_hwp3(&data) {
                convert_hwp3(input, output, format, verbose);
                return;
            }
            if doc97::looks_like_doc(&data) {
                convert_doc97(input, output, format, verbose);
                return;
            }
        }
    }

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
                    eprintln!("\u{274c} Error extracting content: {}", e);
                    return;
                }
            };

            let stem = input.file_stem().unwrap_or_default().to_string_lossy();

            // Build ManifestV2
            let mut mv2 = ManifestV2::new(input, "hwp");

            // Register and save images via ManifestV2
            let mut image_map: Vec<(String, String)> = Vec::new();
            for img in &mdm.images {
                let ext = Path::new(&img.name)
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("bin");
                let meta = AssetMetadata {
                    format: Some(img.format.clone()),
                    ..Default::default()
                };
                let hash_filename = mv2.add_asset(&img.data, MediaType::Image, ext, meta);
                image_map.push((img.name.clone(), hash_filename));
            }

            // Save images to disk (always when present, not just with --extract-images)
            if !mdm.images.is_empty() {
                let mut saved = 0usize;
                for (idx, img) in mdm.images.iter().enumerate() {
                    if let Some(asset) = mv2.assets.get(idx) {
                        if let Err(e) = save_asset_file(output, asset, &img.data) {
                            eprintln!("  \u{26a0}\u{fe0f}  Failed to save {}: {}", img.name, e);
                        } else {
                            saved += 1;
                            if verbose {
                                println!("  \u{1f4f7} Saved: {} ({} bytes)", asset.src, img.data.len());
                            }
                        }
                    }
                }
                if saved > 0 {
                    println!("  \u{2713} Extracted {} images to assets/images/", saved);
                }
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
                    println!("  \u{2713} Created: {}", json_path.display());
                }
                _ => {
                    // Default: MDX format with @[[]] media references
                    let mut mdx_content = mdm.to_mdx();
                    for (orig_name, _hash_fn) in &image_map {
                        let md_img = format!("![{}](assets/{})", orig_name, orig_name);
                        let replacement = format!("@[[{}]]", orig_name);
                        mdx_content = mdx_content.replace(&md_img, &replacement);
                    }
                    let mdx_path = output.join(format!("{}.mdx", stem));
                    fs::write(&mdx_path, &mdx_content).expect("Failed to write MDX");
                    println!("  \u{2713} Created: {}", mdx_path.display());
                }
            }

            // Update stats and save manifest
            mv2.stats.markdown_lines = mdm.content.lines().count();
            mv2.stats.markdown_chars = mdm.content.len();

            if let Err(e) = save_manifest(&mv2, output, &stem) {
                eprintln!("  \u{26a0}\u{fe0f}  Failed to write manifest: {}", e);
            }

            if verbose {
                println!("\n\u{1f4ca} Summary:");
                println!("  - Sections: {}", mdm.metadata.section_count);
                println!("  - Images: {}", mdm.images.len());
                println!("  - Tables: {}", mdm.tables.len());
                println!("  - Text length: {} chars", mdm.content.len());
            }

            println!("\u{2705} Conversion complete!");
        }
        Err(e) => {
            eprintln!("\u{274c} Error opening file: {}", e);
        }
    }
}

fn convert_hwpml(input: &Path, output: &Path, format: &str, verbose: bool) {
    let xml = match fs::read_to_string(input) {
        Ok(xml) => xml,
        Err(e) => {
            eprintln!("\u{274c} Error reading XML file: {}", e);
            return;
        }
    };

    let (version, title, content, sections) = match parse_hwpml(&xml) {
        Ok(parsed) => parsed,
        Err(e) => {
            eprintln!("\u{274c} Error parsing HWPML: {}", e);
            return;
        }
    };

    fs::create_dir_all(output).expect("Failed to create output directory");
    let stem = input.file_stem().unwrap_or_default().to_string_lossy();

    // Build ManifestV2
    let mut mv2 = ManifestV2::new(input, "hwpml");
    if !title.is_empty() {
        mv2.source.title = Some(title.clone());
    }

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
            println!("  \u{2713} Created: {}", json_path.display());
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
            println!("  \u{2713} Created: {}", mdx_path.display());
        }
    }

    mv2.stats.markdown_lines = content.lines().count();
    mv2.stats.markdown_chars = content.len();

    if let Err(e) = save_manifest(&mv2, output, &stem) {
        eprintln!("  \u{26a0}\u{fe0f}  Failed to write manifest: {}", e);
    }

    if verbose {
        println!("\n\u{1f4ca} Summary:");
        println!("  - Format: HWPML (raw XML)");
        println!("  - Sections: {}", sections);
        println!("  - Text length: {} chars", content.len());
    }

    println!("\u{2705} Conversion complete!");
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
                            version = String::from_utf8_lossy(&attr.value).to_string();
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

fn convert_hwpx(input: &Path, output: &Path, format: &str, _extract_images: bool, verbose: bool) {
    match HwpxParser::open(input) {
        Ok(mut parser) => {
            fs::create_dir_all(output).expect("Failed to create output directory");

            match parser.parse() {
                Ok(doc) => {
                    let stem = input.file_stem().unwrap_or_default().to_string_lossy();

                    // Build ManifestV2
                    let mut mv2 = ManifestV2::new(input, "hwpx");

                    // Extract and save images via ManifestV2 (always, not just when --extract-images)
                    let mut saved_count = 0usize;
                    let mut image_map: Vec<(String, String)> = Vec::new();
                    for img in &doc.image_info {
                        let filename = img.path.split('/').last().unwrap_or(&img.id);
                        let ext = Path::new(filename)
                            .extension()
                            .and_then(|e| e.to_str())
                            .unwrap_or("bin");
                        let meta = AssetMetadata {
                            format: Some(ext.to_string()),
                            ..Default::default()
                        };
                        let hash_filename = mv2.add_asset(&img.data, MediaType::Image, ext, meta);
                        image_map.push((img.id.clone(), hash_filename.clone()));

                        if let Some(asset) = mv2.assets.iter().rev().find(|a| a.src.ends_with(&hash_filename)) {
                            if let Err(e) = save_asset_file(output, asset, &img.data) {
                                eprintln!("  \u{26a0}\u{fe0f}  Failed to save {}: {}", filename, e);
                            } else {
                                saved_count += 1;
                                if verbose {
                                    println!("  \u{1f4f7} Saved: {} ({} bytes)", asset.src, img.data.len());
                                }
                            }
                        }
                    }
                    if saved_count > 0 {
                        println!("  \u{2713} Extracted {} images to assets/images/", saved_count);
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
                            println!("  \u{2713} Created: {}", json_path.display());
                        }
                        _ => {
                            // MDX format with @[[]] image references
                            let mdx_path = output.join(format!("{}.mdx", stem));

                            let mdx_content = format!(
                                "---\nformat: hwpx\nversion: \"{}\"\nsections: {}\nimages: {}\n---\n\n{}",
                                doc.version,
                                doc.sections.len(),
                                doc.image_info.len(),
                                content
                            );

                            fs::write(&mdx_path, &mdx_content).expect("Failed to write MDX");
                            println!("  \u{2713} Created: {}", mdx_path.display());
                        }
                    }

                    mv2.stats.markdown_lines = content.lines().count();
                    mv2.stats.markdown_chars = content.len();

                    if let Err(e) = save_manifest(&mv2, output, &stem) {
                        eprintln!("  \u{26a0}\u{fe0f}  Failed to write manifest: {}", e);
                    }

                    if verbose {
                        println!("\n\u{1f4ca} Summary:");
                        println!("  - Format: HWPX (ZIP-based)");
                        println!("  - Sections: {}", doc.sections.len());
                        println!("  - Tables: {}", doc.tables.len());
                        println!("  - Images: {} (extracted: {})", doc.image_info.len(), saved_count);
                        println!("  - Text length: {} chars", content.len());
                    }

                    println!("\u{2705} Conversion complete!");
                }
                Err(e) => eprintln!("\u{274c} Error parsing HWPX: {}", e),
            }
        }
        Err(e) => eprintln!("\u{274c} Error opening HWPX file: {}", e),
    }
}

fn convert_pdf(input: &Path, output: &Path, format: &str, verbose: bool, ocr: bool) {
    if ocr && !ocr_available() {
        eprintln!("  \u{26a0}\u{fe0f}  OCR requested but OCR engine not available. Build with `--features ocr`.");
        eprintln!("  \u{26a0}\u{fe0f}  Continuing with text-only extraction.");
    }

    match PdfParser::open(input) {
        Ok(parser) => {
            fs::create_dir_all(output).expect("Failed to create output directory");

            match parser.parse() {
                Ok(doc) => {
                    let stem = input.file_stem().unwrap_or_default().to_string_lossy();

                    // Build ManifestV2
                    let mut mv2 = ManifestV2::new(input, "pdf");
                    mv2.source.title = if doc.metadata.title.is_empty() { None } else { Some(doc.metadata.title.clone()) };
                    mv2.source.author = if doc.metadata.author.is_empty() { None } else { Some(doc.metadata.author.clone()) };
                    mv2.source.pages = Some(doc.page_count);

                    // Extract images and register in manifest
                    let mut image_map: Vec<(String, String)> = Vec::new(); // (original_id, hash_filename)
                    for image in &doc.images {
                        let ext = image.format.extension();
                        let meta = AssetMetadata {
                            page: image.page,
                            width: Some(image.width),
                            height: Some(image.height),
                            format: Some(ext.to_string()),
                            ..Default::default()
                        };
                        let hash_filename = mv2.add_asset(&image.data, MediaType::Image, ext, meta);
                        image_map.push((image.id.clone(), hash_filename));
                    }

                    // Save image files to disk
                    for (idx, image) in doc.images.iter().enumerate() {
                        if let Some(asset) = mv2.assets.get(idx) {
                            if let Err(e) = save_asset_file(output, asset, &image.data) {
                                eprintln!("  \u{26a0}\u{fe0f}  Failed to save image {}: {}", image.id, e);
                            } else if verbose {
                                println!("  \u{1f4f7} Saved: {} ({} bytes)", asset.src, image.data.len());
                            }
                        }
                    }
                    if !doc.images.is_empty() {
                        println!("  \u{2713} Extracted {} images to assets/images/", doc.images.len());
                    }

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
                            println!("  \u{2713} Created: {}", json_path.display());
                        }
                        _ => {
                            // MDX format — replace image refs with @[[]] syntax
                            let mut mdx_content = doc.to_mdx();
                            for (orig_id, _hash_fn) in &image_map {
                                // Replace ![image_N](image_N) or similar with @[[image_N]]
                                let md_pattern = format!("![{}]({})", orig_id, orig_id);
                                let replacement = format!("@[[{}]]", orig_id);
                                mdx_content = mdx_content.replace(&md_pattern, &replacement);
                                // Also replace plain - image_id references in ## Images section
                                let list_pattern = format!("- {} (", orig_id);
                                let list_replacement = format!("- @[[{}]] (", orig_id);
                                mdx_content = mdx_content.replace(&list_pattern, &list_replacement);
                            }
                            let mdx_path = output.join(format!("{}.mdx", stem));
                            fs::write(&mdx_path, &mdx_content).expect("Failed to write MDX");
                            println!("  \u{2713} Created: {}", mdx_path.display());
                        }
                    }

                    // Update stats
                    let mdx_content = doc.to_mdx();
                    mv2.stats.markdown_lines = mdx_content.lines().count();
                    mv2.stats.markdown_chars = mdx_content.len();

                    // Save ManifestV2 as .mdm
                    if let Err(e) = save_manifest(&mv2, output, &stem) {
                        eprintln!("  \u{26a0}\u{fe0f}  Failed to write manifest: {}", e);
                    }

                    // OCR: if enabled and available, run OCR on image-heavy pages
                    let ocr_text = if ocr && ocr_available() {
                        let ocr_parts: Vec<String> = Vec::new();
                        for img in &doc.images {
                            let empty = String::new();
                            let page_text = doc.pages.iter()
                                .find(|p| p.page_number == img.page.unwrap_or(0))
                                .map(|p| &p.text)
                                .unwrap_or(&empty);
                            if page_text.len() > 200 {
                                continue;
                            }
                            #[cfg(feature = "ocr")]
                            match ocr::ocr_image(&img.data) {
                                Ok(lines) => {
                                    let text: String = lines.iter()
                                        .map(|l| l.text.clone())
                                        .collect::<Vec<_>>()
                                        .join("\n");
                                    if !text.trim().is_empty() {
                                        let pn = img.page.unwrap_or(0);
                                        ocr_parts.push(format!("## Page {} (OCR)\n\n{}\n", pn, text));
                                    }
                                }
                                Err(_) => {}
                            }
                            #[cfg(not(feature = "ocr"))]
                            let _ = img;
                        }
                        if ocr_parts.is_empty() {
                            String::new()
                        } else {
                            format!("\n\n---\n\n{}", ocr_parts.join("\n"))
                        }
                    } else {
                        String::new()
                    };

                    // Rebuild output with OCR text if needed
                    if !ocr_text.is_empty() {
                        let enriched = doc.to_mdx() + &ocr_text;
                        let mdx_path = output.join(format!("{}.mdx", stem));
                        fs::write(&mdx_path, &enriched).expect("Failed to write MDX");
                        println!("  \u{2713} Created (with OCR): {}", mdx_path.display());
                    }

                    if verbose {
                        println!("\n\u{1f4ca} Summary:");
                        println!("  - Format: PDF");
                        println!("  - Version: {}", doc.version);
                        println!("  - Pages: {}", doc.page_count);
                        if !doc.metadata.title.is_empty() {
                            println!("  - Title: {}", doc.metadata.title);
                        }
                        println!("  - Images: {}", doc.images.len());
                        if !ocr_text.is_empty() {
                            println!("  - OCR: enabled, pages processed");
                        }
                        println!("  - Text length: {} chars", doc.full_text().len());
                        println!("  - Using {} threads", rayon::current_num_threads());
                    }

                    println!("\u{2705} Conversion complete!");
                }
                Err(e) => eprintln!("\u{274c} Error parsing PDF: {}", e),
            }
        }
        Err(e) => eprintln!("\u{274c} Error opening PDF file: {}", e),
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

                    // Build ManifestV2
                    let mut mv2 = ManifestV2::new(input, "docx");
                    mv2.source.title = doc.metadata.title.clone();
                    mv2.source.author = doc.metadata.author.clone();
                    mv2.source.pages = doc.metadata.page_count.map(|p| p as usize);

                    // Extract images via ManifestV2 and save to content-addressed paths
                    let mut image_map: Vec<(String, String)> = Vec::new();
                    let mut saved = 0usize;
                    for image in &doc.images {
                        if let Some(ref data) = image.data {
                            let ext = Path::new(&image.filename)
                                .extension()
                                .and_then(|e| e.to_str())
                                .unwrap_or("bin");
                            let meta = AssetMetadata {
                                width: image.width,
                                height: image.height,
                                format: Some(ext.to_string()),
                                alt_text: image.alt_text.clone(),
                                ..Default::default()
                            };
                            let hash_filename = mv2.add_asset(data, MediaType::Image, ext, meta);
                            image_map.push((image.id.clone(), hash_filename.clone()));

                            // Save image using manifest asset path
                            if let Some(asset) = mv2.assets.iter().rev().find(|a| a.src.ends_with(&hash_filename)) {
                                if let Err(e) = save_asset_file(output, asset, data) {
                                    eprintln!("  \u{26a0}\u{fe0f}  Failed to save {}: {}", image.filename, e);
                                } else {
                                    saved += 1;
                                    if verbose {
                                        println!("  \u{1f4f7} Saved: {} ({} bytes)", asset.src, data.len());
                                    }
                                }
                            }
                        }
                    }
                    if saved > 0 {
                        println!("  \u{2713} Extracted {} images to assets/images/", saved);
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
                            println!("  \u{2713} Created: {}", json_path.display());
                        }
                        _ => {
                            // MDX format — replace image refs with @[[]] syntax
                            let mut mdx_content = doc.to_mdx(&source_name);
                            for (orig_id, _hash_fn) in &image_map {
                                let md_img = format!("![{}](assets/{})", orig_id, _hash_fn);
                                let replacement = format!("@[[{}]]", orig_id);
                                mdx_content = mdx_content.replace(&md_img, &replacement);
                                // Also handle original filename references
                                if let Some(img) = doc.images.iter().find(|i| i.id == *orig_id) {
                                    let orig_ref = format!("![{}](assets/{})", orig_id, img.filename);
                                    mdx_content = mdx_content.replace(&orig_ref, &replacement);
                                }
                            }
                            let mdx_path = output.join(format!("{}.mdx", stem));
                            fs::write(&mdx_path, &mdx_content).expect("Failed to write MDX");
                            println!("  \u{2713} Created: {}", mdx_path.display());
                        }
                    }

                    // Update stats
                    let md_content = doc.to_markdown();
                    mv2.stats.markdown_lines = md_content.lines().count();
                    mv2.stats.markdown_chars = md_content.len();

                    // Save ManifestV2 as .mdm
                    if let Err(e) = save_manifest(&mv2, output, &stem) {
                        eprintln!("  \u{26a0}\u{fe0f}  Failed to write manifest: {}", e);
                    }

                    if verbose {
                        println!("\n\u{1f4ca} Summary:");
                        println!("  - Format: DOCX");
                        if let Some(ref title) = doc.metadata.title {
                            println!("  - Title: {}", title);
                        }
                        println!("  - Paragraphs: {}", doc.paragraphs.len());
                        println!("  - Tables: {}", doc.tables.len());
                        println!("  - Images: {}", doc.images.len());
                        println!("  - Text length: {} chars", doc.text().len());
                    }

                    println!("\u{2705} Conversion complete!");
                }
                Err(e) => eprintln!("\u{274c} Error parsing DOCX: {}", e),
            }
        }
        Err(e) => eprintln!("\u{274c} Error opening DOCX file: {}", e),
    }
}

fn convert_xlsx(input: &Path, output: &Path, format: &str, verbose: bool) {
    match XlsxParser::open(input) {
        Ok(parser) => {
            fs::create_dir_all(output).expect("Failed to create output directory");

            match parser.parse() {
                Ok(doc) => {
                    let stem = input.file_stem().unwrap_or_default().to_string_lossy();
                    let source_name = input.file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| "spreadsheet.xlsx".to_string());

                    // Build ManifestV2 (no images for XLSX)
                    let mut mv2 = ManifestV2::new(input, "xlsx");

                    match format {
                        "json" => {
                            let json_path = output.join(format!("{}.json", stem));
                            let json_data = json!({
                                "version": "1.0",
                                "format": "xlsx",
                                "metadata": {
                                    "sheets": doc.metadata.sheet_count,
                                },
                                "content": doc.to_markdown(),
                            });
                            fs::write(&json_path, serde_json::to_string_pretty(&json_data).unwrap())
                                .expect("Failed to write JSON");
                            println!("  \u{2713} Created: {}", json_path.display());
                        }
                        _ => {
                            let mdx_path = output.join(format!("{}.mdx", stem));
                            fs::write(&mdx_path, doc.to_mdx(&source_name)).expect("Failed to write MDX");
                            println!("  \u{2713} Created: {}", mdx_path.display());
                        }
                    }

                    let md = doc.to_markdown();
                    mv2.stats.markdown_lines = md.lines().count();
                    mv2.stats.markdown_chars = md.len();

                    if let Err(e) = save_manifest(&mv2, output, &stem) {
                        eprintln!("  \u{26a0}\u{fe0f}  Failed to write manifest: {}", e);
                    }

                    if verbose {
                        println!("\n\u{1f4ca} Summary:");
                        println!("  - Format: XLSX");
                        println!("  - Sheets: {}", doc.metadata.sheet_count);
                        for sheet in &doc.sheets {
                            println!("    - {} ({} rows)", sheet.name, sheet.rows.len());
                        }
                    }

                    println!("\u{2705} Conversion complete!");
                }
                Err(e) => eprintln!("\u{274c} Error parsing XLSX: {}", e),
            }
        }
        Err(e) => eprintln!("\u{274c} Error opening XLSX file: {}", e),
    }
}

fn convert_pptx(input: &Path, output: &Path, format: &str, verbose: bool) {
    match PptxParser::open(input) {
        Ok(parser) => {
            fs::create_dir_all(output).expect("Failed to create output directory");

            match parser.parse() {
                Ok(doc) => {
                    let stem = input.file_stem().unwrap_or_default().to_string_lossy();
                    let source_name = input.file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| "presentation.pptx".to_string());

                    // Build ManifestV2 (no images for PPTX yet)
                    let mut mv2 = ManifestV2::new(input, "pptx");

                    match format {
                        "json" => {
                            let json_path = output.join(format!("{}.json", stem));
                            let json_data = json!({
                                "version": "1.0",
                                "format": "pptx",
                                "metadata": {
                                    "slides": doc.metadata.slide_count,
                                },
                                "slides": doc.slides.iter().map(|s| json!({
                                    "number": s.number,
                                    "title": s.title,
                                    "content": s.content,
                                    "notes": s.notes,
                                })).collect::<Vec<_>>(),
                                "content": doc.to_markdown(),
                            });
                            fs::write(&json_path, serde_json::to_string_pretty(&json_data).unwrap())
                                .expect("Failed to write JSON");
                            println!("  \u{2713} Created: {}", json_path.display());
                        }
                        _ => {
                            let mdx_path = output.join(format!("{}.mdx", stem));
                            fs::write(&mdx_path, doc.to_mdx(&source_name)).expect("Failed to write MDX");
                            println!("  \u{2713} Created: {}", mdx_path.display());
                        }
                    }

                    let md = doc.to_markdown();
                    mv2.stats.markdown_lines = md.lines().count();
                    mv2.stats.markdown_chars = md.len();

                    if let Err(e) = save_manifest(&mv2, output, &stem) {
                        eprintln!("  \u{26a0}\u{fe0f}  Failed to write manifest: {}", e);
                    }

                    if verbose {
                        println!("\n\u{1f4ca} Summary:");
                        println!("  - Format: PPTX");
                        println!("  - Slides: {}", doc.metadata.slide_count);
                        for slide in &doc.slides {
                            let title = slide.title.as_deref().unwrap_or("(untitled)");
                            println!("    - Slide {}: {}", slide.number, title);
                        }
                    }

                    println!("\u{2705} Conversion complete!");
                }
                Err(e) => eprintln!("\u{274c} Error parsing PPTX: {}", e),
            }
        }
        Err(e) => eprintln!("\u{274c} Error opening PPTX file: {}", e),
    }
}

fn convert_html(input: &Path, output: &Path, format: &str, verbose: bool) {
    match HtmlParser::open(input) {
        Ok(parser) => {
            fs::create_dir_all(output).expect("Failed to create output directory");

            match parser.parse() {
                Ok(doc) => {
                    let stem = input.file_stem().unwrap_or_default().to_string_lossy();
                    let source_name = input.file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| "page.html".to_string());

                    // Build ManifestV2
                    let mut mv2 = ManifestV2::new(input, "html");
                    mv2.source.title = doc.title.clone();

                    match format {
                        "json" => {
                            let json_path = output.join(format!("{}.json", stem));
                            let json_data = json!({
                                "version": "1.0",
                                "format": "html",
                                "metadata": {
                                    "title": doc.title,
                                },
                                "content": doc.markdown,
                            });
                            fs::write(&json_path, serde_json::to_string_pretty(&json_data).unwrap())
                                .expect("Failed to write JSON");
                            println!("  \u{2713} Created: {}", json_path.display());
                        }
                        _ => {
                            let mdx_path = output.join(format!("{}.mdx", stem));
                            fs::write(&mdx_path, doc.to_mdx(&source_name)).expect("Failed to write MDX");
                            println!("  \u{2713} Created: {}", mdx_path.display());
                        }
                    }

                    mv2.stats.markdown_lines = doc.markdown.lines().count();
                    mv2.stats.markdown_chars = doc.markdown.len();

                    if let Err(e) = save_manifest(&mv2, output, &stem) {
                        eprintln!("  \u{26a0}\u{fe0f}  Failed to write manifest: {}", e);
                    }

                    if verbose {
                        println!("\n\u{1f4ca} Summary:");
                        println!("  - Format: HTML");
                        if let Some(ref title) = doc.title {
                            println!("  - Title: {}", title);
                        }
                        println!("  - Markdown length: {} chars", doc.markdown.len());
                    }

                    println!("\u{2705} Conversion complete!");
                }
                Err(e) => eprintln!("\u{274c} Error parsing HTML: {}", e),
            }
        }
        Err(e) => eprintln!("\u{274c} Error opening HTML file: {}", e),
    }
}

fn convert_csv(input: &Path, output: &Path, format: &str, verbose: bool) {
    match CsvParser::open(input) {
        Ok(parser) => {
            fs::create_dir_all(output).expect("Failed to create output directory");

            match parser.parse() {
                Ok(doc) => {
                    let stem = input.file_stem().unwrap_or_default().to_string_lossy();
                    let source_name = input.file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| "data.csv".to_string());

                    // Build ManifestV2
                    let mut mv2 = ManifestV2::new(input, "csv");

                    match format {
                        "json" => {
                            let json_path = output.join(format!("{}.json", stem));
                            let json_data = json!({
                                "version": "1.0",
                                "format": "csv",
                                "metadata": {
                                    "rows": doc.rows.len(),
                                    "columns": doc.rows.first().map(|r| r.len()).unwrap_or(0),
                                },
                                "content": doc.to_markdown(),
                            });
                            fs::write(&json_path, serde_json::to_string_pretty(&json_data).unwrap())
                                .expect("Failed to write JSON");
                            println!("  \u{2713} Created: {}", json_path.display());
                        }
                        _ => {
                            let mdx_path = output.join(format!("{}.mdx", stem));
                            fs::write(&mdx_path, doc.to_mdx(&source_name)).expect("Failed to write MDX");
                            println!("  \u{2713} Created: {}", mdx_path.display());
                        }
                    }

                    let md = doc.to_markdown();
                    mv2.stats.markdown_lines = md.lines().count();
                    mv2.stats.markdown_chars = md.len();

                    if let Err(e) = save_manifest(&mv2, output, &stem) {
                        eprintln!("  \u{26a0}\u{fe0f}  Failed to write manifest: {}", e);
                    }

                    if verbose {
                        println!("\n\u{1f4ca} Summary:");
                        println!("  - Format: CSV");
                        println!("  - Rows: {}", doc.rows.len());
                        println!("  - Columns: {}", doc.rows.first().map(|r| r.len()).unwrap_or(0));
                    }

                    println!("\u{2705} Conversion complete!");
                }
                Err(e) => eprintln!("\u{274c} Error parsing CSV: {}", e),
            }
        }
        Err(e) => eprintln!("\u{274c} Error opening CSV file: {}", e),
    }
}

fn convert_txt(input: &Path, output: &Path, format: &str, verbose: bool) {
    match TxtParser::open(input) {
        Ok(parser) => {
            fs::create_dir_all(output).expect("Failed to create output directory");

            let stem = input.file_stem().unwrap_or_default().to_string_lossy();
            let source_name = input.file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "file.txt".to_string());
            let markdown = parser.to_markdown();

            // Build ManifestV2
            let mut mv2 = ManifestV2::new(input, "txt");

            match format {
                "json" => {
                    let json_path = output.join(format!("{}.json", stem));
                    let json_data = json!({
                        "version": "1.0",
                        "format": "txt",
                        "metadata": {
                            "lines": markdown.lines().count(),
                        },
                        "content": markdown,
                    });
                    fs::write(&json_path, serde_json::to_string_pretty(&json_data).unwrap())
                        .expect("Failed to write JSON");
                    println!("  \u{2713} Created: {}", json_path.display());
                }
                _ => {
                    let mdx_path = output.join(format!("{}.mdx", stem));
                    fs::write(&mdx_path, parser.to_mdx(&source_name)).expect("Failed to write MDX");
                    println!("  \u{2713} Created: {}", mdx_path.display());
                }
            }

            mv2.stats.markdown_lines = markdown.lines().count();
            mv2.stats.markdown_chars = markdown.len();

            if let Err(e) = save_manifest(&mv2, output, &stem) {
                eprintln!("  \u{26a0}\u{fe0f}  Failed to write manifest: {}", e);
            }

            if verbose {
                println!("\n\u{1f4ca} Summary:");
                println!("  - Format: TXT");
                println!("  - Lines: {}", markdown.lines().count());
                println!("  - Characters: {}", markdown.len());
            }

            println!("\u{2705} Conversion complete!");
        }
        Err(e) => eprintln!("\u{274c} Error opening text file: {}", e),
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
                        convert_file(&path, output, "mdx", true, false, false);
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

fn inspect_file(input: &Path, format: &str) {
    let ext = input.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();

    match ext.as_str() {
        "hwp" => inspect_hwp(input, format),
        "hwpx" => {
            eprintln!("inspect for HWPX: coming soon (use analyze for now)");
        }
        _ => {
            eprintln!("inspect supports HWP/HWPX only (got .{})", ext);
            std::process::exit(1);
        }
    }
}

fn inspect_hwp(input: &Path, format: &str) {
    let mut parser = match HwpParser::open(input) {
        Ok(p) => p,
        Err(e) => { eprintln!("Error: {}", e); return; }
    };

    let tables = match parser.extract_tables() {
        Ok(t) => t,
        Err(e) => { eprintln!("Error extracting tables: {}", e); return; }
    };

    if format == "json" {
        let json_tables: Vec<serde_json::Value> = tables.iter().enumerate().map(|(i, t)| {
            let cells: Vec<serde_json::Value> = t.cell_spans.iter().map(|s| {
                json!({
                    "row": s.row_addr, "col": s.col_addr,
                    "rowspan": s.row_span, "colspan": s.col_span
                })
            }).collect();
            let merged_count = cells.iter().filter(|c| {
                c["rowspan"].as_u64().unwrap_or(1) > 1 || c["colspan"].as_u64().unwrap_or(1) > 1
            }).count();
            json!({
                "id": format!("table_{:03}", i + 1),
                "rows": t.rows, "cols": t.cols,
                "merged_cells": merged_count,
                "cells": cells
            })
        }).collect();

        let tables_with_merges = tables.iter().filter(|t| {
            t.cell_spans.iter().any(|s| s.row_span > 1 || s.col_span > 1)
        }).count();

        println!("{}", serde_json::to_string_pretty(&json!({
            "file": input.display().to_string(),
            "tables": json_tables,
            "summary": {
                "total_tables": tables.len(),
                "tables_with_merges": tables_with_merges
            }
        })).unwrap());
        return;
    }

    println!("MDM Inspect: {}", input.display());
    println!("{}", "=".repeat(60));

    if tables.is_empty() {
        println!("\n  No tables found.");
    } else {
        println!("\n  Tables: {}", tables.len());
        for (i, table) in tables.iter().enumerate() {
            let merged: Vec<_> = table.cell_spans.iter()
                .filter(|s| s.row_span > 1 || s.col_span > 1)
                .collect();

            println!("\n  table_{:03}: {}x{} (rows x cols)", i + 1, table.rows, table.cols);

            if merged.is_empty() {
                println!("    No merged cells");
            } else {
                println!("    Merged cells: {}", merged.len());
                for s in &merged {
                    println!("      [{},{}] span {}x{}", s.row_addr, s.col_addr, s.row_span, s.col_span);
                }
            }
        }
    }

    println!("\n{}", "=".repeat(60));
    println!("Use --format json for machine-readable output");
}

/// Dump per-element layout as JSON — consumed by `pdf_triage_router.py`
/// to place Mixed-page OCR results at their correct Y position.
fn dump_layout(input: &Path, output: Option<&Path>, page: Option<usize>) -> std::io::Result<()> {
    use mdm_core::pdf::PdfParser;

    let parser = PdfParser::open(input)?;
    let elements = parser.extract_layout();
    let filtered: Vec<_> = match page {
        Some(p) => elements.into_iter().filter(|e| e.page == p).collect(),
        None => elements,
    };
    let json = serde_json::to_string_pretty(&filtered)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
    match output {
        Some(p) => std::fs::write(p, json)?,
        None => println!("{}", json),
    }
    Ok(())
}

/// Run PDF page triage and emit either a human-readable table or an
/// OCR-routing manifest (JSON). See `plan/pdf-triage.md`.
fn triage_pdf(input: &Path, format: &str, output: Option<&Path>) -> std::io::Result<()> {
    use mdm_core::pdf::{PdfParser, PdfCategory};
    use mdm_core::pdf::triage::build_manifest;

    let parser = PdfParser::open(input)?;
    let results = parser.triage();

    let payload = match format {
        "json" => {
            let doc_str = input.to_string_lossy().into_owned();
            let manifest = build_manifest(&doc_str, &results);
            serde_json::to_string_pretty(&manifest)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?
        }
        _ => {
            let mut buf = String::new();
            buf.push_str(&format!("{:<60}\n", input.display()));
            buf.push_str(&format!(
                "{:<4} {:<12} {:<5} {:<7} {:<7} {:<5} {:<7} {:<7} {:<5} {:<5}\n",
                "pg", "category", "conf", "text%", "image%", "#img",
                "fontR", "underL", "invis", "cjk"
            ));
            for t in &results {
                let f_opt = |v: Option<f32>| {
                    v.map(|x| format!("{:.2}", x)).unwrap_or_else(|| "-".into())
                };
                let b_opt = |v: Option<bool>| match v {
                    Some(true) => "yes".to_string(),
                    Some(false) => "no".to_string(),
                    None => "-".to_string(),
                };
                buf.push_str(&format!(
                    "{:<4} {:<12?} {:<5.2} {:<7.3} {:<7.3} {:<5} {:<7} {:<7} {:<5} {:<5}\n",
                    t.page,
                    t.category,
                    t.confidence,
                    t.text_coverage,
                    t.image_coverage,
                    t.image_count,
                    f_opt(t.font_reliability),
                    f_opt(t.ocr_underlay_ratio),
                    b_opt(t.has_invisible_text),
                    b_opt(t.contains_cjk),
                ));
            }
            let (scanned, mixed, text_native, unknown) = results.iter().fold(
                (0, 0, 0, 0),
                |(s, m, t, u), r| match r.category {
                    PdfCategory::Scanned => (s + 1, m, t, u),
                    PdfCategory::Mixed => (s, m + 1, t, u),
                    PdfCategory::TextNative => (s, m, t + 1, u),
                    PdfCategory::Unknown => (s, m, t, u + 1),
                },
            );
            buf.push_str(&format!(
                "\nSummary: {} pages — {} text-native, {} scanned, {} mixed, {} unknown\n",
                results.len(), text_native, scanned, mixed, unknown
            ));
            buf
        }
    };

    match output {
        Some(p) => std::fs::write(p, payload)?,
        None => print!("{}", payload),
    }
    Ok(())
}

#[cfg(feature = "rtf")]
fn convert_rtf(input: &Path, output: &Path, format: &str, verbose: bool) {
    match rtf::RtfParser::open(input) {
        Ok(parser) => {
            fs::create_dir_all(output).expect("Failed to create output directory");

            match parser.parse() {
                Ok(doc) => {
                    let stem = input.file_stem().unwrap_or_default().to_string_lossy();
                    let source_name = input.file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| "document.rtf".to_string());

                    let mut mv2 = ManifestV2::new(input, "rtf");

                    match format {
                        "json" => {
                            let json_path = output.join(format!("{}.json", stem));
                            let json_data = json!({
                                "version": "1.0",
                                "format": "rtf",
                                "content": doc.to_markdown(),
                            });
                            fs::write(&json_path, serde_json::to_string_pretty(&json_data).unwrap())
                                .expect("Failed to write JSON");
                            println!("  \u{2713} Created: {}", json_path.display());
                        }
                        _ => {
                            let mdx_path = output.join(format!("{}.mdx", stem));
                            fs::write(&mdx_path, doc.to_mdx(&source_name)).expect("Failed to write MDX");
                            println!("  \u{2713} Created: {}", mdx_path.display());
                        }
                    }

                    let md = doc.to_markdown();
                    mv2.stats.markdown_lines = md.lines().count();
                    mv2.stats.markdown_chars = md.len();

                    if let Err(e) = save_manifest(&mv2, output, &stem) {
                        eprintln!("  \u{26a0}\u{fe0f}  Failed to write manifest: {}", e);
                    }

                    if verbose {
                        println!("\n\u{1f4ca} Summary:");
                        println!("  - Format: RTF");
                        println!("  - Text length: {} chars", md.len());
                    }

                    println!("\u{2705} Conversion complete!");
                }
                Err(e) => eprintln!("\u{274c} Error parsing RTF: {}", e),
            }
        }
        Err(e) => eprintln!("\u{274c} Error opening RTF file: {}", e),
    }
}

#[cfg(not(feature = "rtf"))]
fn convert_rtf(_input: &Path, _output: &Path, _format: &str, _verbose: bool) {
    eprintln!("\u{274c} RTF support disabled. Enable the 'rtf' feature in Cargo.toml.");
}

#[cfg(feature = "epub")]
fn convert_epub(input: &Path, output: &Path, format: &str, verbose: bool) {
    match epub::EpubParser::open(input) {
        Ok(parser) => {
            fs::create_dir_all(output).expect("Failed to create output directory");

            match parser.parse() {
                Ok(doc) => {
                    let stem = input.file_stem().unwrap_or_default().to_string_lossy();
                    let source_name = input.file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| "book.epub".to_string());

                    let mut mv2 = ManifestV2::new(input, "epub");

                    match format {
                        "json" => {
                            let json_path = output.join(format!("{}.json", stem));
                            let json_data = json!({
                                "version": "1.0",
                                "format": "epub",
                                "metadata": {
                                    "title": doc.metadata.title,
                                    "author": doc.metadata.author,
                                    "language": doc.metadata.language,
                                    "chapters": doc.metadata.chapters.len(),
                                },
                                "content": doc.to_markdown(),
                            });
                            fs::write(&json_path, serde_json::to_string_pretty(&json_data).unwrap())
                                .expect("Failed to write JSON");
                            println!("  \u{2713} Created: {}", json_path.display());
                        }
                        _ => {
                            let mdx_path = output.join(format!("{}.mdx", stem));
                            fs::write(&mdx_path, doc.to_mdx(&source_name)).expect("Failed to write MDX");
                            println!("  \u{2713} Created: {}", mdx_path.display());
                        }
                    }

                    let md = doc.to_markdown();
                    mv2.stats.markdown_lines = md.lines().count();
                    mv2.stats.markdown_chars = md.len();

                    if let Err(e) = save_manifest(&mv2, output, &stem) {
                        eprintln!("  \u{26a0}\u{fe0f}  Failed to write manifest: {}", e);
                    }

                    if verbose {
                        println!("\n\u{1f4ca} Summary:");
                        println!("  - Format: EPUB");
                        if let Some(ref title) = doc.metadata.title {
                            println!("  - Title: {}", title);
                        }
                        if let Some(ref author) = doc.metadata.author {
                            println!("  - Author: {}", author);
                        }
                        println!("  - Chapters: {}", doc.metadata.chapters.len());
                        for ch in &doc.metadata.chapters {
                            println!("    - {} ({})", ch.title, ch.path);
                        }
                    }

                    println!("\u{2705} Conversion complete!");
                }
                Err(e) => eprintln!("\u{274c} Error parsing EPUB: {}", e),
            }
        }
        Err(e) => eprintln!("\u{274c} Error opening EPUB file: {}", e),
    }
}

#[cfg(not(feature = "epub"))]
fn convert_epub(_input: &Path, _output: &Path, _format: &str, _verbose: bool) {
    eprintln!("\u{274c} EPUB support disabled. Enable the 'epub' feature in Cargo.toml.");
}

#[cfg(feature = "xls")]
fn convert_xls(input: &Path, output: &Path, format: &str, verbose: bool) {
    match xls::XlsParser::open(input) {
        Ok(parser) => {
            fs::create_dir_all(output).expect("Failed to create output directory");

            match parser.parse() {
                Ok(doc) => {
                    let stem = input.file_stem().unwrap_or_default().to_string_lossy();
                    let source_name = input.file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| "spreadsheet.xls".to_string());

                    let mut mv2 = ManifestV2::new(input, "xls");

                    match format {
                        "json" => {
                            let json_path = output.join(format!("{}.json", stem));
                            let json_data = json!({
                                "version": "1.0",
                                "format": "xls",
                                "metadata": {
                                    "sheets": doc.metadata.sheet_count,
                                },
                                "content": doc.to_markdown(),
                            });
                            fs::write(&json_path, serde_json::to_string_pretty(&json_data).unwrap())
                                .expect("Failed to write JSON");
                            println!("  \u{2713} Created: {}", json_path.display());
                        }
                        _ => {
                            let mdx_path = output.join(format!("{}.mdx", stem));
                            fs::write(&mdx_path, doc.to_mdx(&source_name)).expect("Failed to write MDX");
                            println!("  \u{2713} Created: {}", mdx_path.display());
                        }
                    }

                    let md = doc.to_markdown();
                    mv2.stats.markdown_lines = md.lines().count();
                    mv2.stats.markdown_chars = md.len();

                    if let Err(e) = save_manifest(&mv2, output, &stem) {
                        eprintln!("  \u{26a0}\u{fe0f}  Failed to write manifest: {}", e);
                    }

                    if verbose {
                        println!("\n\u{1f4ca} Summary:");
                        println!("  - Format: XLS (BIFF8)");
                        println!("  - Sheets: {}", doc.metadata.sheet_count);
                        for sheet in &doc.sheets {
                            println!("    - {} ({} rows)", sheet.name, sheet.rows.len());
                        }
                    }

                    println!("\u{2705} Conversion complete!");
                }
                Err(e) => eprintln!("\u{274c} Error parsing XLS: {}", e),
            }
        }
        Err(e) => eprintln!("\u{274c} Error opening XLS file: {}", e),
    }
}

#[cfg(not(feature = "xls"))]
fn convert_xls(_input: &Path, _output: &Path, _format: &str, _verbose: bool) {
    eprintln!("\u{274c} XLS support disabled. Enable the 'xls' feature in Cargo.toml.");
}

// ── New CLI subcommand handlers ────────────────────────────────────────────

fn cmd_generate(input: &Path, output: Option<&Path>, fmt: &str, preset: Option<&str>) {
    let markdown = if input == Path::new("-") {
        let mut s = String::new();
        if io::stdin().read_to_string(&mut s).is_err() {
            eprintln!("\u{274c} Failed to read stdin");
            return;
        }
        s
    } else {
        match fs::read_to_string(input) {
            Ok(s) => s,
            Err(e) => { eprintln!("\u{274c} Failed to read {}: {}", input.display(), e); return; }
        }
    };

    let out_fmt = fmt.to_lowercase();

    match out_fmt.as_str() {
        "docx" => {
            #[cfg(feature = "docx-out")]
            match gen_docx::markdown_to_docx(&markdown) {
                Ok(doc) => write_output(output, &doc.bytes, "docx"),
                Err(e) => eprintln!("\u{274c} DOCX generation failed: {}", e),
            }
            #[cfg(not(feature = "docx-out"))]
            eprintln!("\u{274c} DOCX output disabled. Build with `--features docx-out`.");
        }
        "pdf" => {
            #[cfg(feature = "pdf-out")]
            match mdm_core::gen_pdf::markdown_to_pdf(&markdown) {
                Ok(doc) => write_output(output, &doc.bytes, "pdf"),
                Err(e) => eprintln!("\u{274c} PDF generation failed: {}", e),
            }
            #[cfg(not(feature = "pdf-out"))]
            eprintln!("\u{274c} PDF output disabled. Build with `--features pdf-out`.");
        }
        _ => {
            // Default: HWPX
            let opts = match preset {
                Some(p) => hwpx_gen::GenOptions::with_preset(p),
                None => hwpx_gen::GenOptions::default(),
            };

            match hwpx_gen::markdown_to_hwpx(&markdown, &opts) {
                Ok(data) => write_output(output, &data, "hwpx"),
                Err(e) => eprintln!("\u{274c} HWPX generation failed: {}", e),
            }
        }
    }
}

fn write_output(output: Option<&Path>, data: &[u8], ext: &str) {
    if let Some(out_path) = output {
        if let Err(e) = fs::write(out_path, data) {
            eprintln!("\u{274c} Failed to write {}: {}", out_path.display(), e);
        } else {
            println!("\u{2705} Generated: {} ({} bytes)", out_path.display(), data.len());
        }
    } else {
        io::stdout().write_all(data).ok();
    }
}

/// Read an input as text for the text-oriented tools (redact / lint / chunks /
/// diff). Text files (.md/.markdown/.txt/.text and unknown/extension-less) are
/// read verbatim; binary document formats are converted to Markdown first so
/// masking / linting / chunking operate on real content instead of raw bytes.
///
/// NB: format-preserving in-place masking of HWPX (keeping the original
/// layout) is a follow-up — this path yields Markdown, not a re-serialized doc.
fn input_to_text(path: &Path) -> Option<String> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    match ext.as_str() {
        "hwp" => {
            let mut p = HwpParser::open(path).ok()?;
            p.to_mdm().ok().map(|d| d.to_mdx())
        }
        "hwpx" => convert_to_markdown_inline(path),
        "pdf" => {
            let p = PdfParser::open(path).ok()?;
            p.parse().ok().map(|d| d.to_mdx())
        }
        "docx" => {
            let mut p = DocxParser::open(path).ok()?;
            p.parse().ok().map(|d| d.to_markdown())
        }
        // calamine's auto reader handles both .xls and .xlsx.
        "xls" | "xlsx" => {
            let p = XlsxParser::open(path).ok()?;
            p.parse().ok().map(|d| d.to_markdown())
        }
        // .md / .markdown / .txt / .text / .html / .csv / no-ext → read as text.
        _ => fs::read_to_string(path).ok(),
    }
}

/// Minimal Markdown → IR block parser used by `diff` and `chunks` so both
/// operate on real structure instead of raw lines. Not a full CommonMark
/// parser — it recognizes the block kinds the IR models: ATX headings, GFM
/// pipe tables, ordered/unordered lists, thematic breaks, and paragraphs.
fn markdown_to_ir_blocks(md: &str) -> Vec<ir::IRBlock> {
    fn flush_para(para: &mut Vec<String>, blocks: &mut Vec<ir::IRBlock>) {
        if !para.is_empty() {
            let text = para.join(" ").trim().to_string();
            if !text.is_empty() {
                blocks.push(ir::IRBlock::paragraph(text));
            }
            para.clear();
        }
    }
    fn is_thematic_break(line: &str) -> bool {
        let s: String = line.chars().filter(|c| !c.is_whitespace()).collect();
        s.len() >= 3
            && (s.chars().all(|c| c == '-') || s.chars().all(|c| c == '*') || s.chars().all(|c| c == '_'))
    }
    fn split_table_row(row: &str) -> Vec<String> {
        row.trim()
            .trim_start_matches('|')
            .trim_end_matches('|')
            .split('|')
            .map(|c| c.trim().to_string())
            .collect()
    }
    fn is_table_separator_row(row: &str) -> bool {
        let cells = split_table_row(row);
        !cells.is_empty()
            && cells.iter().all(|c| {
                let t = c.trim();
                !t.is_empty() && t.chars().all(|ch| ch == '-' || ch == ':')
            })
    }
    // Some(true)=ordered, Some(false)=unordered, None=not a list item.
    fn list_marker(line: &str) -> Option<bool> {
        if let Some(rest) = line.strip_prefix(|c| c == '-' || c == '*' || c == '+') {
            if rest.starts_with(' ') {
                return Some(false);
            }
        }
        let digits: String = line.chars().take_while(|c| c.is_ascii_digit()).collect();
        if !digits.is_empty() {
            let after = &line[digits.len()..];
            if (after.starts_with('.') || after.starts_with(')')) && after[1..].starts_with(' ') {
                return Some(true);
            }
        }
        None
    }
    fn strip_list_marker(line: &str) -> String {
        if let Some(rest) = line.strip_prefix(|c| c == '-' || c == '*' || c == '+') {
            return rest.trim_start().to_string();
        }
        let digits: String = line.chars().take_while(|c| c.is_ascii_digit()).collect();
        if !digits.is_empty() {
            let after = &line[digits.len()..];
            if after.starts_with('.') || after.starts_with(')') {
                return after[1..].trim_start().to_string();
            }
        }
        line.to_string()
    }

    let lines: Vec<&str> = md.lines().collect();
    let mut blocks: Vec<ir::IRBlock> = Vec::new();
    let mut para: Vec<String> = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i].trim();

        if line.is_empty() {
            flush_para(&mut para, &mut blocks);
            i += 1;
            continue;
        }

        // ATX heading.
        if line.starts_with('#') {
            let hashes = line.chars().take_while(|c| *c == '#').count();
            let rest = &line[hashes..];
            if (1..=6).contains(&hashes) && (rest.is_empty() || rest.starts_with(' ')) {
                flush_para(&mut para, &mut blocks);
                blocks.push(ir::IRBlock::heading(hashes as u8, rest.trim().to_string()));
                i += 1;
                continue;
            }
        }

        // Thematic break.
        if is_thematic_break(line) {
            flush_para(&mut para, &mut blocks);
            blocks.push(ir::IRBlock::Separator);
            i += 1;
            continue;
        }

        // GFM pipe table: a run of lines starting with '|', or the outer-pipe-less
        // form (`Name | Amount` + `--- | ---`) where the next line is a separator
        // row with a matching column count.
        let pipeless_table = !line.starts_with('|')
            && line.contains('|')
            && i + 1 < lines.len()
            && {
                let next = lines[i + 1].trim();
                next.contains('|')
                    && is_table_separator_row(next)
                    && split_table_row(next).len() == split_table_row(line).len()
            };
        if line.starts_with('|') || pipeless_table {
            flush_para(&mut para, &mut blocks);
            let mut rows: Vec<Vec<ir::IRCell>> = Vec::new();
            while i < lines.len() {
                let row = lines[i].trim();
                let is_row = if pipeless_table {
                    row.contains('|')
                } else {
                    row.starts_with('|')
                };
                if !is_row {
                    break;
                }
                if !is_table_separator_row(row) {
                    let cells = split_table_row(row).into_iter().map(ir::IRCell::new).collect();
                    rows.push(cells);
                }
                i += 1;
            }
            if !rows.is_empty() {
                blocks.push(ir::IRBlock::Table(ir::IRTable::new(rows)));
            }
            continue;
        }

        // Ordered / unordered list.
        if let Some(ordered) = list_marker(line) {
            flush_para(&mut para, &mut blocks);
            let mut items: Vec<String> = Vec::new();
            while i < lines.len() {
                let l = lines[i].trim();
                match list_marker(l) {
                    Some(o) if o == ordered => {
                        items.push(strip_list_marker(l));
                        i += 1;
                    }
                    _ => break,
                }
            }
            blocks.push(ir::IRBlock::List { ordered, items });
            continue;
        }

        para.push(line.to_string());
        i += 1;
    }
    flush_para(&mut para, &mut blocks);
    blocks
}

fn cmd_redact(input: &Path, output: Option<&Path>, rules_str: &str) {
    // Documents (HWP/HWPX/PDF/DOCX/XLS(X)) are converted to Markdown first;
    // .md/.txt pass through unchanged. Format-preserving HWPX masking is a
    // follow-up (see input_to_text).
    let text = match input_to_text(input) {
        Some(t) => t,
        None => {
            eprintln!("\u{274c} Failed to read {} (unsupported or unreadable)", input.display());
            std::process::exit(1);
        }
    };

    let rules: Vec<pii::PiiRule> = rules_str.split(',')
        .filter_map(|s| match s.trim() {
            "rrn" => Some(pii::PiiRule::Rrn),
            "phone" => Some(pii::PiiRule::Phone),
            "email" => Some(pii::PiiRule::Email),
            "card" => Some(pii::PiiRule::Card),
            "account" => Some(pii::PiiRule::Account),
            "passport" => Some(pii::PiiRule::Passport),
            "driver" => Some(pii::PiiRule::Driver),
            _ => None,
        })
        .collect();

    let opts = pii::RedactOptions { rules, ..Default::default() };
    match pii::redact_markdown(&text, &opts) {
        Ok(result) => {
            if let Some(out_path) = output {
                if let Err(e) = fs::write(out_path, &result.text) {
                    eprintln!("\u{274c} Failed to write {}: {}", out_path.display(), e);
                    std::process::exit(1);
                }
                println!("\u{2705} Redacted output written to {} ({} hit(s))", out_path.display(), result.hits.len());
            } else {
                print!("{}", result.text);
            }
        }
        Err(e) => {
            eprintln!("\u{274c} Redaction failed: {}", e);
            std::process::exit(1);
        }
    }
}

fn cmd_diff(input_a: &Path, input_b: &Path, format: &str) {
    // Convert each input to Markdown, parse to IR blocks, then run the
    // structural block diff (added/removed/modified/moved + cell-level table
    // deltas). Default output is a human-readable Markdown report; --format
    // json emits the serde `DiffResult`.
    let read_blocks = |path: &Path| -> Option<Vec<ir::IRBlock>> {
        input_to_text(path).map(|t| markdown_to_ir_blocks(&t))
    };

    let blocks_a = match read_blocks(input_a) {
        Some(b) => b,
        None => { eprintln!("\u{274c} Failed to read {}", input_a.display()); std::process::exit(1); }
    };
    let blocks_b = match read_blocks(input_b) {
        Some(b) => b,
        None => { eprintln!("\u{274c} Failed to read {}", input_b.display()); std::process::exit(1); }
    };

    let result = ir::diff_blocks(&blocks_a, &blocks_b);
    match format {
        "json" => println!("{}", serde_json::to_string_pretty(&result).unwrap_or_default()),
        _ => print!("{}", ir::render_diff_markdown(&result)),
    }
}

fn convert_to_markdown_inline(path: &Path) -> Option<String> {
    let mut parser = HwpxParser::open(path).ok()?;
    let doc = parser.parse().ok()?;
    Some(doc.sections.join("\n\n"))
}

fn cmd_fill(input: &Path, values_path: Option<&Path>, output: Option<&Path>, dry_run: bool) {
    let template_bytes = match fs::read(input) {
        Ok(b) => b,
        Err(e) => { eprintln!("\u{274c} Failed to read {}: {}", input.display(), e); return; }
    };

    if dry_run {
        match form::extract_form_schema(&template_bytes) {
            Ok(schema) => {
                println!("{}", serde_json::to_string_pretty(&schema).unwrap_or_default());
            }
            Err(e) => eprintln!("\u{274c} Form extraction failed: {}", e),
        }
        return;
    }

    let values_path = match values_path {
        Some(p) => p,
        None => {
            eprintln!("\u{274c} fill requires --values/-j <FILE> (or use --dry-run to show the form schema)");
            std::process::exit(1);
        }
    };

    let values_json: serde_json::Value = match fs::read_to_string(values_path) {
        Ok(s) => match serde_json::from_str(&s) {
            Ok(v) => v,
            Err(e) => { eprintln!("\u{274c} Invalid JSON: {}", e); return; }
        },
        Err(e) => { eprintln!("\u{274c} Failed to read {}: {}", values_path.display(), e); return; }
    };

    let values: HashMap<String, form::RawFillInput> = values_json
        .as_object()
        .map(|obj| obj.iter().filter_map(|(k, v)| {
            let s = v.as_str()?.to_string();
            Some((k.clone(), form::RawFillInput { value: form::FillValue::Scalar(s), format: None }))
        }).collect())
        .unwrap_or_default();

    if values.is_empty() {
        eprintln!("\u{274c} No key-value pairs found in JSON");
        return;
    }

    match form::fill_hwpx(&template_bytes, &values) {
        Ok(result) => {
            if let Some(out_path) = output {
                fs::write(out_path, &result.buffer).ok();
                println!("\u{2705} Filled: {} ({} bytes)", out_path.display(), result.buffer.len());
            } else {
                io::stdout().write_all(&result.buffer).ok();
            }
        }
        Err(e) => eprintln!("\u{274c} Fill failed: {}", e),
    }
}

fn cmd_lint(input: &Path) {
    let text = match input_to_text(input) {
        Some(t) => t,
        None => { eprintln!("\u{274c} Failed to read {}", input.display()); std::process::exit(1); }
    };

    let issues = lint::lint_document(&text);
    if issues.is_empty() {
        println!("\u{2705} No issues found.");
    } else {
        for issue in &issues {
            let severity = match issue.severity {
                lint::LintSeverity::Error => "ERROR",
                lint::LintSeverity::Warning => "WARN",
            };
            println!("  {} line {}: {} — {}", severity, issue.line, issue.rule, issue.matched);
        }
        println!("  Found {} issue(s).", issues.len());
    }
}

fn cmd_chunks(input: &Path, granularity: &str, max_chars: usize, overlap: usize) {
    // Convert documents to Markdown then parse to real IR blocks (headings,
    // GFM tables, lists) instead of reconstructing one paragraph per raw line.
    let text = match input_to_text(input) {
        Some(t) => t,
        None => { eprintln!("\u{274c} Failed to read {}", input.display()); std::process::exit(1); }
    };
    let blocks = markdown_to_ir_blocks(&text);

    let opts = chunker::ChunkOptions {
        granularity: match granularity {
            "block" => chunker::Granularity::Block,
            _ => chunker::Granularity::Section,
        },
        max_chars: if max_chars == 0 { None } else { Some(max_chars) },
        overlap,
        include_table_cells: false,
    };

    let chunks = chunker::chunk(&blocks, &opts);
    println!("{}", serde_json::to_string_pretty(&chunks).unwrap_or_default());
}

#[cfg(feature = "watch")]
fn cmd_watch(dir: &Path, output: &Path, webhook: Option<&str>) {
    use watch::{OutputFormat, WatchOptions};

    let opts = WatchOptions {
        out_dir: Some(output.to_path_buf()),
        webhook: webhook.map(|s| s.to_string()),
        format: OutputFormat::Markdown,
        silent: false,
    };

    println!("\u{1f4c2} Watching {} ...", dir.display());
    println!("  Output: {}", output.display());
    if webhook.is_some() {
        println!("  Webhook: enabled");
    }
    println!("  Supported: {:?}", watch::supported_extensions());

    // `FileEvent` is a struct: { path, file_name, result: Result<String, String>, out_path }.
    let result = watch::watch_dir(dir, opts, |event| match event.result {
        Ok(_) => {
            let out = event
                .out_path
                .as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| "(stdout)".to_string());
            println!("  \u{2705} {} -> {}", event.path.display(), out);
        }
        Err(err) => eprintln!("  \u{274c} {}: {}", event.file_name, err),
    });
    if let Err(e) = result {
        eprintln!("\u{274c} Watch failed: {}", e);
    }
}

fn convert_hwp3(input: &Path, output: &Path, format: &str, verbose: bool) {
    let data = match fs::read(input) {
        Ok(d) => d,
        Err(e) => { eprintln!("\u{274c} Failed to read {}: {}", input.display(), e); return; }
    };

    match hwp3::parse_hwp3_document(&data) {
        Ok(doc) => {
            fs::create_dir_all(output).expect("Failed to create output directory");
            let stem = input.file_stem().unwrap_or_default().to_string_lossy();
            let source_name = input.file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "document.hwp".to_string());

            let mut mv2 = ManifestV2::new(input, "hwp3");

            match format {
                "json" => {
                    let json_path = output.join(format!("{}.json", stem));
                    let json_data = json!({
                        "version": "1.0",
                        "format": "hwp3",
                        "metadata": {
                            "title": doc.metadata.title,
                            "author": doc.metadata.author,
                            "subject": doc.metadata.subject,
                            "date": doc.metadata.date,
                        },
                        "content": doc.markdown,
                        "warnings": doc.warnings,
                    });
                    fs::write(&json_path, serde_json::to_string_pretty(&json_data).unwrap())
                        .expect("Failed to write JSON");
                    println!("  \u{2713} Created: {}", json_path.display());
                }
                _ => {
                    let mdx_content = format!(
                        "---\nformat: hwp3\nsource: \"{}\"---\n\n{}",
                        source_name.replace('"', "\\\""),
                        doc.markdown,
                    );
                    let mdx_path = output.join(format!("{}.mdx", stem));
                    fs::write(&mdx_path, &mdx_content).expect("Failed to write MDX");
                    println!("  \u{2713} Created: {}", mdx_path.display());
                }
            }

            mv2.stats.markdown_lines = doc.markdown.lines().count();
            mv2.stats.markdown_chars = doc.markdown.len();

            if let Err(e) = save_manifest(&mv2, output, &stem) {
                eprintln!("  \u{26a0}\u{fe0f}  Failed to write manifest: {}", e);
            }

            if verbose {
                println!("\n\u{1f4ca} Summary:");
                println!("  - Format: HWP 3.0 (1996-2002)");
                if let Some(ref t) = doc.metadata.title {
                    println!("  - Title: {}", t);
                }
                println!("  - Blocks: {}", doc.blocks.len());
                println!("  - Text length: {} chars", doc.markdown.len());
                if !doc.warnings.is_empty() {
                    println!("  - Warnings: {}", doc.warnings.len());
                }
            }

            println!("\u{2705} Conversion complete!");
        }
        Err(e) => eprintln!("\u{274c} Error parsing HWP3: {}", e),
    }
}

fn cmd_legal(input: &Path, format: &str) {
    let mut chunker = legal::KoreanLegalChunker::new();
    match chunker.parse_markdown(input) {
        Ok(chunks) => {
            match format {
                "json" => {
                    println!("{}", serde_json::to_string_pretty(&chunks).unwrap_or_default());
                }
                _ => {
                    for chunk in &chunks {
                        println!("{}", chunk.to_json());
                    }
                }
            }
        }
        Err(e) => eprintln!("\u{274c} Legal parsing failed: {}", e),
    }
}

#[cfg(feature = "ocr")]
fn ocr_available() -> bool {
    ocr::ocr_available()
}

#[cfg(not(feature = "ocr"))]
fn ocr_available() -> bool {
    false
}

fn convert_doc97(input: &Path, output: &Path, format: &str, verbose: bool) {
    match doc97::DocParser::open(input) {
        Ok(parser) => {
            fs::create_dir_all(output).expect("Failed to create output directory");

            match parser.parse() {
                Ok(doc) => {
                    let stem = input.file_stem().unwrap_or_default().to_string_lossy();
                    let source_name = input.file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| "document.doc".to_string());

                    let mut mv2 = ManifestV2::new(input, "doc");

                    match format {
                        "json" => {
                            let json_path = output.join(format!("{}.json", stem));
                            let json_data = json!({
                                "version": "1.0",
                                "format": "doc",
                                "content": doc.to_markdown(),
                            });
                            fs::write(&json_path, serde_json::to_string_pretty(&json_data).unwrap())
                                .expect("Failed to write JSON");
                            println!("  \u{2713} Created: {}", json_path.display());
                        }
                        _ => {
                            let mdx_path = output.join(format!("{}.mdx", stem));
                            fs::write(&mdx_path, doc.to_mdx(&source_name)).expect("Failed to write MDX");
                            println!("  \u{2713} Created: {}", mdx_path.display());
                        }
                    }

                    let md = doc.to_markdown();
                    mv2.stats.markdown_lines = md.lines().count();
                    mv2.stats.markdown_chars = md.len();

                    if let Err(e) = save_manifest(&mv2, output, &stem) {
                        eprintln!("  \u{26a0}\u{fe0f}  Failed to write manifest: {}", e);
                    }

                    if verbose {
                        println!("\n\u{1f4ca} Summary:");
                        println!("  - Format: DOC (Word 97-2003)");
                        println!("  - Text length: {} chars", md.len());
                    }

                    println!("\u{2705} Conversion complete!");
                }
                Err(e) => eprintln!("\u{274c} Error parsing DOC: {}", e),
            }
        }
        Err(e) => eprintln!("\u{274c} Error opening DOC file: {}", e),
    }
}

#[cfg(feature = "url-fetch")]
fn cmd_url(urls: &[String], output: Option<&Path>) {
    let url_refs: Vec<&str> = urls.iter().map(|s| s.as_str()).collect();
    let results = url_fetch::fetch_urls(&url_refs);

    for (url, result) in urls.iter().zip(results.iter()) {
        match result {
            Ok(doc) => {
                if let Some(out_dir) = output {
                    fs::create_dir_all(out_dir).ok();
                    let slug = url
                        .replace("https://", "")
                        .replace("http://", "")
                        .replace('/', "_")
                        .chars()
                        .take(60)
                        .collect::<String>();
                    let mdx_path = out_dir.join(format!("{}.mdx", slug));
                    fs::write(&mdx_path, doc.to_mdx()).ok();
                    println!("\u{2705} {} -> {}", url, mdx_path.display());
                } else {
                    println!("\n## {}\n\n{}", url, doc.markdown);
                }
            }
            Err(e) => eprintln!("\u{274c} {}: {}", url, e),
        }
    }
}

fn cmd_validate(input: &Path) {
    match fs::read(input) {
        Ok(data) => {
            let result = hwpx_gen::validate::validate_hwpx(&data);
            if result.ok {
                println!("\u{2705} HWPX validation passed ({} entries checked).",
                    result.entry_count);
            } else {
                eprintln!("\u{274c} Validation failed:");
                for issue in &result.issues {
                    let path = issue.path.as_deref().unwrap_or("(root)");
                    eprintln!("  - {}: {}", path, issue.message);
                }
            }
        }
        Err(e) => eprintln!("\u{274c} Failed to read {}: {}", input.display(), e),
    }
}

fn cmd_equation(input: &Path, direction: &str) {
    let text = match fs::read_to_string(input) {
        Ok(s) => s,
        Err(e) => { eprintln!("\u{274c} Failed to read {}: {}", input.display(), e); return; }
    };

    match direction {
        "latex2hulk" => println!("{}", equation::latex_to_hulk(&text)),
        _ => println!("{}", equation::hulk_to_latex(&text)),
    }
}

#[cfg(test)]
mod md_ir_tests {
    use super::*;

    fn table_count(blocks: &[ir::IRBlock]) -> usize {
        blocks.iter().filter(|b| matches!(b, ir::IRBlock::Table(_))).count()
    }

    #[test]
    fn md_ir_parses_piped_gfm_table() {
        let md = "| Name | Amount |\n| --- | --- |\n| A | 1 |\n";
        let blocks = markdown_to_ir_blocks(md);
        assert_eq!(table_count(&blocks), 1);
    }

    #[test]
    fn md_ir_parses_gfm_table_without_outer_pipes() {
        let md = "Name | Amount\n--- | ---\nA | 1\nB | 2\n";
        let blocks = markdown_to_ir_blocks(md);
        assert_eq!(table_count(&blocks), 1, "outer-pipe-less GFM table must parse as IRBlock::Table");
        if let Some(ir::IRBlock::Table(t)) = blocks.iter().find(|b| matches!(b, ir::IRBlock::Table(_))) {
            assert_eq!(t.rows, 3); // header + 2 body rows (separator dropped)
            assert_eq!(t.cols, 2);
        }
    }

    #[test]
    fn md_ir_pipe_in_paragraph_is_not_a_table() {
        // No separator row on the next line → must stay a paragraph.
        let md = "either | or is fine here\njust prose\n";
        let blocks = markdown_to_ir_blocks(md);
        assert_eq!(table_count(&blocks), 0);
    }

    #[test]
    fn md_ir_column_count_mismatch_is_not_a_table() {
        // Separator row with a different column count → not a table header.
        let md = "a | b | c\n--- | ---\n";
        let blocks = markdown_to_ir_blocks(md);
        assert_eq!(table_count(&blocks), 0);
    }
}
