//! MDM Core Library
//! 
//! High-performance document parsing and conversion engine for MDM format.

pub mod hwp;
pub mod pdf;
pub mod docx;
pub mod optimizer;
pub mod bench;

pub use hwp::HwpParser;
pub use optimizer::{ImageOptimizer, OptimizeSettings, FileCache};
pub use bench::{Benchmark, BenchResult};
