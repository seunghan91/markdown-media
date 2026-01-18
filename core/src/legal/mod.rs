//! Korean Legal Document Parser Module
//!
//! 한국 법령 문서의 계층 구조를 파싱하여 벡터화에 적합한 청크를 생성합니다.
//!
//! 법령 계층 구조:
//! - 편(Part) > 장(Chapter) > 절(Section) > 관(Sub-Section) > 조(Article) > 항(Paragraph) > 호(Subparagraph) > 목(Item)
//!
//! # Example
//! ```rust,ignore
//! use mdm_core::legal::{KoreanLegalChunker, WeKnoraExporter};
//!
//! let chunker = KoreanLegalChunker::new();
//! let chunks = chunker.parse_markdown("path/to/law.md")?;
//!
//! let exporter = WeKnoraExporter::new();
//! exporter.export_to_jsonl(&chunks, "output.jsonl")?;
//! ```

pub mod types;
pub mod patterns;
pub mod chunker;
pub mod exporter;

pub use types::*;
pub use patterns::*;
pub use chunker::KoreanLegalChunker;
pub use exporter::WeKnoraExporter;
