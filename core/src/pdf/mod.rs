pub mod parser;
pub mod table_detect;
pub mod triage;

pub use table_detect::{
    build_table_grids, detect_line_tables, extract_cells, extract_ruling_lines,
    merge_line_and_cluster, normalize_undersegmented, preprocess_lines, BBox, DetectedTable,
    ExtractedCell, LineSegment, TableGrid,
};

pub use triage::{PageTriage, PdfCategory, TriageConfig, BoundingBox as PdfBoundingBox};

pub use parser::{
    PdfParser,
    PdfDocument,
    PdfError,
    EncryptionInfo,
    LayoutElement,
    LayoutElementType,
    TextAlignment,
    PdfImage,
    PdfMetadata,
    PdfFont,
    PdfTable,
    PageContent,
    ImageFormat,
};
