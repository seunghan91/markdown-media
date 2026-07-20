//! Watch a directory and auto-convert supported documents to Markdown.
//!
//! cargo run --example watch --features watch -- <dir> [--out <out_dir>] [--webhook <url>]

use mdm_core::watch::{watch_dir, OutputFormat, WatchOptions};

fn main() {
    let mut args = std::env::args().skip(1);
    let dir = args.next().unwrap_or_else(|| {
        eprintln!("usage: watch <dir> [--out <out_dir>] [--webhook <url>]");
        std::process::exit(1);
    });

    let mut opts = WatchOptions {
        format: OutputFormat::Markdown,
        ..WatchOptions::default()
    };

    let rest: Vec<String> = args.collect();
    let mut i = 0;
    while i < rest.len() {
        match rest[i].as_str() {
            "--out" => {
                opts.out_dir = rest.get(i + 1).map(std::path::PathBuf::from);
                i += 2;
            }
            "--webhook" => {
                opts.webhook = rest.get(i + 1).cloned();
                i += 2;
            }
            _ => {
                i += 1;
            }
        }
    }

    if let Err(e) = watch_dir(&dir, opts, |event| {
        match &event.result {
            Ok(_) => println!("[watch] converted: {}", event.file_name),
            Err(e) => println!("[watch] failed: {} — {}", event.file_name, e),
        }
    }) {
        eprintln!("watch failed: {}", e);
        std::process::exit(1);
    }
}
