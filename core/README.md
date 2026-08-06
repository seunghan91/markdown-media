# mdm-core

Converts Korean and Western office documents to Markdown, keeping what the
document actually contains — including the parts that usually vanish.

HWP · HWPX · HWPML · DOC97 · DOCX · PDF · XLS/XLSX · PPTX · RTF · EPUB · HTML

```toml
[dependencies]
mdm-core = "0.4"
```

## What it does

```rust
use mdm_core::HwpxParser;

let doc = HwpxParser::parse_file("report.hwpx")?;
println!("{}", doc.to_markdown());
```

Parsers live per format (`mdm_core::hwp`, `::hwpx`, `::pdf`, `::docx`, …) and
each produces a document that renders to Markdown. `hwp2mdm`, the binary in
this crate, is the same thing on the command line:

```bash
hwp2mdm report.hwpx -o out/     # → out/report.mdx + out/report.mdm + out/assets/
```

## What it goes out of its way to keep

Most of the work in this crate is about content that a naive extractor drops
without saying so:

- **Embedded images**, wired back to the place in the body that references them
  — HWP `bin_id`↔BinData streams, HWPX markers, DOCX `w:drawing` anchors
- **Vector shapes** rendered to SVG, including grouped shapes and HWP v5's
  binary `SHAPE_COMPONENT` layout; WMF/EMF metafiles converted, with the
  original preserved when conversion fails
- **Charts** (`chartSpace`) as Markdown data tables rather than as a lost
  picture
- **Text inside images** via OCR, and QR codes decoded to their payload
- **Captions**, used as the image's alt text instead of a generated id
- **PDF tables**, with detection that rejects page borders, banner boxes and
  the spurious grids that scatter prose across cells

Output is standard CommonMark, images included (`![alt](path)`).

## Feature flags

All formats are on by default. Opt out to shed build time or WASM size:

```toml
mdm-core = { version = "0.4", default-features = false, features = ["hwpx", "pdf"] }
```

`hwp` `hwpx` `pdf` `docx` `xls` `rtf` `epub` `image-processing` `wmf` `qr`

## License

MIT. See the [repository](https://github.com/seunghan91/markdown-media) for the
desktop app, the Python and Node bindings, and the benchmark harness.
