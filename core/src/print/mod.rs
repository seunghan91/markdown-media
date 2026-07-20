//! Print-oriented rendering: `IRBlock[]` / Markdown → print HTML, with an
//! optional best-effort PDF path.
//!
//! Ported from kkdoc (MIT): reference/kkdoc/src/print/{parser.ts, renderer.ts}
//!
//! kkdoc's print pipeline is `IRBlock[]` → Markdown (`blocksToMarkdown`) →
//! HTML (`markdown-it`) → PDF (`puppeteer-core`, a full headless Chromium).
//! The PDF stage doesn't translate to a Rust crate — bundling a browser
//! engine is out of scope here — so this port keeps the HTML side faithful
//! (presets, watermark, page CSS) and adds an independent, `printpdf`-based
//! PDF renderer behind the optional `print-pdf` feature that lays out
//! `IRBlock`s directly instead of rasterizing HTML. See `pdf`'s module doc
//! comment for that renderer's specific limitations (most notably: no
//! Korean/CJK glyph support, since `printpdf`'s built-in fonts are the 14
//! standard Latin-only PDF fonts).
//!
//! - [`render_ir_to_html`] / [`RenderOptions`] — `IRBlock[]` → print HTML.
//! - [`markdown_to_ir`] / [`render_markdown_to_html`] — Markdown (a subset,
//!   see `parser`'s module doc comment) → `IRBlock[]` → print HTML.
//! - [`pdf::render_ir_to_pdf`] (feature `print-pdf`) — `IRBlock[]` → PDF bytes.

mod parser;
#[cfg(feature = "print-pdf")]
mod pdf;
mod renderer;

pub use parser::{markdown_to_ir, render_markdown_to_html};
#[cfg(feature = "print-pdf")]
pub use pdf::render_ir_to_pdf;
pub use renderer::{
    render_ir_to_html, Orientation, PageMargin, PageSize, PrintPreset, RenderOptions,
};
