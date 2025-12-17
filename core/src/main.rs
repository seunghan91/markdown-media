mod hwp;

use hwp::HwpParser;
use std::env;

fn main() {
    let args: Vec<String> = env::args().collect();
    
    if args.len() < 2 {
        println!("MDM Core - HWP Parser");
        println!("Usage: {} <file.hwp>", args[0]);
        return;
    }
    
    let file_path = &args[1];
    
    match HwpParser::open(file_path) {
        Ok(mut parser) => {
            println!("✓ Opened HWP file: {}", file_path);
            
            let structure = parser.analyze();
            println!("\n📊 File Structure:");
            println!("  Total streams: {}", structure.total_streams);
            println!("\n📁 Streams:");
            for stream in &structure.streams {
                println!("  - {}", stream);
            }
            
            println!("\n📄 Extracting text...");
            match parser.extract_text() {
                Ok(text) => println!("  {}", text),
                Err(e) => println!("  Error: {}", e),
            }
        }
        Err(e) => {
            eprintln!("✗ Error opening file: {}", e);
        }
    }
}
