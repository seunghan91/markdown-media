//! MDM Core Parser Benchmarks
//! 
//! Run with: cargo bench

use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn record_parsing_benchmark(c: &mut Criterion) {
    // Create sample record data
    // tag=0x43 (PARA_TEXT), level=0, size=100
    let header: u32 = 0x43 | (0 << 10) | (100 << 20);
    let mut data = header.to_le_bytes().to_vec();
    data.extend(vec![0x41; 100]); // 100 bytes of 'A'

    c.bench_function("record_parse", |b| {
        b.iter(|| {
            // Simple record header parsing simulation
            let h = u32::from_le_bytes([
                black_box(data[0]),
                black_box(data[1]),
                black_box(data[2]),
                black_box(data[3]),
            ]);
            let _tag = (h & 0x3FF) as u16;
            let _level = ((h >> 10) & 0x3FF) as u16;
            let _size = (h >> 20) & 0xFFF;
        })
    });
}

fn text_extraction_benchmark(c: &mut Criterion) {
    // UTF-16LE Korean text (100 characters)
    let text = "안녕하세요 테스트 문자열입니다 ";
    let mut data = Vec::new();
    for ch in text.chars().cycle().take(100) {
        let code = ch as u16;
        data.push(code as u8);
        data.push((code >> 8) as u8);
    }

    c.bench_function("text_extract_100chars", |b| {
        b.iter(|| {
            let mut result = String::new();
            let mut i = 0;
            while i + 1 < data.len() {
                let char_code = u16::from_le_bytes([data[i], data[i + 1]]);
                if let Some(c) = char::from_u32(char_code as u32) {
                    result.push(c);
                }
                i += 2;
            }
            black_box(result)
        })
    });
}

fn image_detection_benchmark(c: &mut Criterion) {
    let jpeg = vec![0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, 0x4A, 0x46];
    let png = vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
    let gif = vec![0x47, 0x49, 0x46, 0x38, 0x39, 0x61, 0x00, 0x00];

    c.bench_function("image_format_detect", |b| {
        b.iter(|| {
            let j = &jpeg[0..4];
            let p = &png[0..4];
            let g = &gif[0..3];
            
            let _ = black_box(j[0] == 0xFF && j[1] == 0xD8);
            let _ = black_box(p[0] == 0x89 && p[1] == 0x50);
            let _ = black_box(g[0] == 0x47 && g[1] == 0x49);
        })
    });
}

criterion_group!(
    benches,
    record_parsing_benchmark,
    text_extraction_benchmark,
    image_detection_benchmark
);

criterion_main!(benches);
