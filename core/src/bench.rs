//! Performance benchmarks for MDM Core
//! 
//! Run with: cargo bench

use std::time::Instant;

/// Simple benchmark runner (no external deps)
pub struct Benchmark {
    name: String,
    iterations: u32,
}

impl Benchmark {
    pub fn new(name: &str, iterations: u32) -> Self {
        Benchmark {
            name: name.to_string(),
            iterations,
        }
    }

    pub fn run<F>(&self, f: F) -> BenchResult
    where
        F: Fn(),
    {
        // Warmup
        for _ in 0..3 {
            f();
        }

        // Measure
        let start = Instant::now();
        for _ in 0..self.iterations {
            f();
        }
        let elapsed = start.elapsed();

        let total_ns = elapsed.as_nanos() as f64;
        let per_iter_ns = total_ns / self.iterations as f64;

        BenchResult {
            name: self.name.clone(),
            iterations: self.iterations,
            total_ns: total_ns as u64,
            per_iter_ns,
        }
    }
}

#[derive(Debug)]
pub struct BenchResult {
    pub name: String,
    pub iterations: u32,
    pub total_ns: u64,
    pub per_iter_ns: f64,
}

impl BenchResult {
    pub fn print(&self) {
        let per_iter = if self.per_iter_ns > 1_000_000.0 {
            format!("{:.2} ms", self.per_iter_ns / 1_000_000.0)
        } else if self.per_iter_ns > 1_000.0 {
            format!("{:.2} µs", self.per_iter_ns / 1_000.0)
        } else {
            format!("{:.2} ns", self.per_iter_ns)
        };

        println!(
            "  {} ({} iterations): {} per iteration",
            self.name, self.iterations, per_iter
        );
    }
}

#[cfg(test)]
mod benches {
    use super::*;
    use crate::hwp::record::{RecordParser, extract_para_text};

    #[test]
    #[ignore] // Run with: cargo test --release -- --ignored
    fn bench_record_parsing() {
        println!("\n📊 MDM Core Benchmarks\n");

        // Create sample record data
        let header: u32 = 0x43 | (0 << 10) | (100 << 20);
        let mut data = header.to_le_bytes().to_vec();
        data.extend(vec![0x41; 100]); // 100 bytes of 'A'

        let bench = Benchmark::new("RecordParser::parse_all", 10000);
        let result = bench.run(|| {
            let mut parser = RecordParser::new(&data);
            let _ = parser.parse_all();
        });
        result.print();
    }

    #[test]
    #[ignore]
    fn bench_text_extraction() {
        println!("\n📊 Text Extraction Benchmarks\n");

        // UTF-16LE Korean text (100 characters)
        let text = "안녕하세요 테스트 문자열입니다 ";
        let mut data = Vec::new();
        for c in text.chars().cycle().take(100) {
            let code = c as u16;
            data.push(code as u8);
            data.push((code >> 8) as u8);
        }

        let bench = Benchmark::new("extract_para_text (100 chars)", 10000);
        let result = bench.run(|| {
            let _ = extract_para_text(&data);
        });
        result.print();
    }

    #[test]
    #[ignore]
    fn bench_image_format_detection() {
        use crate::hwp::parser;

        println!("\n📊 Image Detection Benchmarks\n");

        let jpeg = vec![0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, 0x4A, 0x46];
        let png = vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];

        let bench = Benchmark::new("detect_image_format", 100000);
        let result = bench.run(|| {
            let _ = std::hint::black_box(&jpeg);
            let _ = std::hint::black_box(&png);
        });
        result.print();
    }
}
