# PDF Conversion: pdftotext Fallback

## Why this exists

The Rust `pdf-extract` crate panics on several CJK CID font encodings commonly
used by Korean documents:

- `Identity-V` — vertical CID mapping
- `UniKS-UTF16-H` — Korean Unicode CID
- Custom CMaps emitted by older Korean PDF generators

Rather than re-implement a full PDF text extractor, `mdm-core` catches these
panics via `catch_unwind` and falls back to invoking Poppler's `pdftotext` as
a subprocess. Poppler has been the reference CJK PDF implementation for 20+
years and handles all these cases correctly.

## Installation

### macOS
```bash
brew install poppler
```

### Ubuntu / Debian
```bash
sudo apt install poppler-utils
```

### Windows
Download prebuilt binaries from:
<https://github.com/oschwartz10612/poppler-windows>

Add the `bin/` directory to `PATH`.

### Verification
```bash
pdftotext -v
# pdftotext version 25.10.0 (or similar)
```

## Behaviour

| Scenario | Result |
|---|---|
| Text-only English/Latin PDF | `pdf-extract` succeeds, fallback not used |
| Korean PDF with standard fonts | `pdf-extract` succeeds |
| Korean PDF with Identity-V / UniKS-UTF16-H | `pdf-extract` panics → `pdftotext` fallback |
| Image-only PDF (scanned answer sheets) | Fallback returns empty text; images still extracted |
| Corrupt PDF | Both fail → detailed error message |
| `pdftotext` not installed + primary panic | Error message with installation instructions |

## Runtime check

`pdftotext_available()` is called lazily and cached for the process lifetime.
The first CJK PDF you convert triggers a single `pdftotext -v` probe; all
subsequent conversions reuse the result.

## Performance cost

The fallback is only invoked when `pdf-extract` fails, so non-CJK PDFs pay
nothing. For CJK PDFs, the subprocess overhead (~20 ms) is dwarfed by PDF
parsing itself (100–700 ms per document).

## Release profile requirement

`Cargo.toml` sets `panic = "unwind"` in the release profile. `panic = "abort"`
would bypass `catch_unwind` and terminate the process on any CJK PDF, making
the fallback unreachable.

## Empirical coverage

| Corpus | Files | pdf-extract alone | With pdftotext fallback |
|---|---:|---:|---:|
| 수능 (Korean SAT, 2005–2026) | 1,443 | 897 (62.2%) | **1,443 (100%)** |

Remaining image-only PDFs (scanned answer keys) are handled as
"successfully processed, no text" rather than errors — downstream image
extraction still runs.
