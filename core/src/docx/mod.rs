pub mod parser;
pub mod math;

pub use parser::{
    DocxParser,
    DocxDocument,
    DocxMetadata,
    DocxImage,
    DocxTable,
    Paragraph,
    TextRun,
    TableCell,
};
