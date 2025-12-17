# MDM CLI

Command-line tool for MDM (Markdown+Media) format.

## Installation

```bash
npm install -g @mdm/cli
```

## Usage

### Convert Files

Convert HWP or PDF files to MDX format:

```bash
mdm convert input.hwp -o output/
mdm convert document.pdf -o output/
```

### Validate Bundle

Validate MDM bundle structure:

```bash
mdm validate ./bundle
mdm validate document.mdx -v
```

### Preview Server

Start a local preview server:

```bash
mdm serve ./output --port 3000
mdm serve ./bundle --open
```

## Commands

- `mdm convert <input> [options]` - Convert documents to MDX
- `mdm validate <path> [options]` - Validate MDM structure
- `mdm serve [path] [options]` - Start preview server

## Options

### Convert

- `-o, --output <dir>` - Output directory (default: ./output)
- `-f, --format <type>` - Output format (default: mdx)

### Validate

- `-v, --verbose` - Verbose output

### Serve

- `-p, --port <number>` - Port number (default: 3000)
- `--open` - Open browser automatically
