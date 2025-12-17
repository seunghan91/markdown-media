# MDM Parser (Rust/WASM)

Rust-based MDM parser compiled to WebAssembly for browser use.

## Building

```bash
# Install wasm-pack
curl https://rustwasm.github.io/wasm-pack/installer/init.sh -sSf | sh

# Build for web
wasm-pack build --target web

# Build for Node.js
wasm-pack build --target nodejs
```

## Usage

### Browser

```html
<script type="module">
  import init, { parse_mdm } from "./pkg/mdm_parser_rs.js";

  await init();

  const result = parse_mdm("Hello ![[image.jpg]] world");
  console.log(result);
</script>
```

### Node.js

```javascript
const { parse_mdm } = require("./pkg/mdm_parser_rs");

const result = parse_mdm("Hello ![[image.jpg]] world");
console.log(result);
```

## Features

- Fast MDM syntax parsing
- WASM-optimized performance
- Browser and Node.js compatible
