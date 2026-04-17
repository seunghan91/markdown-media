# @markdown-media/core

Native Node.js bindings for [MDM](https://github.com/seunghan91/markdown-media) — convert HWP / HWPX / PDF / DOCX documents to Markdown using the Rust core engine, with zero filesystem access when you already have bytes in memory.

```bash
npm install @markdown-media/core
```

## Unified document API

Four formats, three entry points, auto-detection from filename extension and/or magic bytes. Same behavior as the WASM build (used in the Chrome extension) and the Python bindings — one Rust core, multiple languages.

```js
const {
  convertFile,
  convertBytes,
  convertToJson,
  detectFormat,
  getVersion,
} = require('@markdown-media/core');

// Path API — reads the file for you
const md = convertFile('./report.hwpx');

// Bytes API — for uploads, streams, serverless
const fs = require('node:fs');
const data = fs.readFileSync('./report.pdf');
const md2 = convertBytes(data, 'report.pdf');

// Format detection — extension first, magic bytes as fallback
detectFormat(data, 'report.pdf');    // => 'pdf'
detectFormat(data, 'no-extension');  // => 'pdf' (from %PDF magic)
detectFormat(Buffer.from('junk'), 'x.xyz'); // => 'unknown'

// Structured output with metadata
const json = JSON.parse(convertToJson(data, 'report.pdf'));
// {
//   format: 'pdf',
//   version: '1.7',
//   markdown: '...',
//   metadata: { page_count, title, author, image_count, ... }
// }

getVersion(); // '1.0.0'
```

### Supported formats

| Ext | Detection | Notes |
|---|---|---|
| `.hwp`  | extension, OLE magic `D0 CF 11 E0` | Hancom HWP5 (CFB) |
| `.hwpx` | extension, ZIP + `Contents/` entry | Hancom HWPX (OWPML) |
| `.pdf`  | extension, `%PDF` magic | Layout-aware extraction |
| `.docx` | extension, ZIP + `word/` entry | OOXML |

`convertBytes` and `convertToJson` throw on unknown formats rather than guessing.

### When to use which entry point

- **`convertFile(path)`** — CLI tools, batch scripts. Reads the file for you.
- **`convertBytes(buffer, filename)`** — HTTP upload handlers, serverless functions (Lambda/Render), in-memory pipelines. No temp file round-trip, no cleanup.
- **`convertToJson(buffer, filename)`** — when you also want structured metadata (page counts, tables, images, author). Returns a JSON string.
- **`detectFormat(buffer, filename)`** — cheap pre-flight check; decide routing or reject early without parsing.

## Low-level API

The unified functions are built on top of format-specific parsers, which remain exported for callers that need the raw structures:

```js
const {
  parseHwpFile,     // { text, tables }
  parseHwpxFile,    // string (joined sections)
  parseAnnexText,   // Korean legal "[별표 N]" extraction
  parseAnnexHwp,
  parseAnnexHwpx,
  parseDate,        // Korean date parsing → YYYYMMDD
  parseDateWithReference,
  createChainPlan,  // Legal research chain planner
  aggregateChainResults,
} = require('@markdown-media/core');
```

See `index.d.ts` for full TypeScript definitions.

## Platform support

Native binaries are distributed as optional dependencies; npm picks the right one for your platform:

- `darwin-arm64` (Apple Silicon)
- `darwin-x64` (Intel Mac)
- `linux-x64-gnu`
- `linux-arm64-gnu`

## License

MIT
