#!/usr/bin/env node

import { Command } from 'commander';
import { convertCommand } from './commands/convert.js';
import { validateCommand } from './commands/validate.js';
import { serveCommand } from './commands/serve.js';
import { watchCommand } from './commands/watch.js';
import { batchCommand } from './commands/batch.js';

const program = new Command();

program
  .name('mdm')
  .description('MDM (Markdown+Media) CLI tool - Convert documents to Markdown+Media bundles')
  .version('0.1.0');

// Convert command
program
  .command('convert')
  .description('Convert HWP/PDF/DOCX/HTML files to MDX format')
  .argument('<input>', 'Input file path')
  .option('-o, --output <dir>', 'Output directory', './output')
  .option('-f, --format <type>', 'Output format (mdx)', 'mdx')
  .action(convertCommand);

// Validate command
program
  .command('validate')
  .description('Validate MDM bundle structure')
  .argument('<path>', 'Path to MDM bundle or file')
  .option('-v, --verbose', 'Verbose output')
  .action(validateCommand);

// Serve command
program
  .command('serve')
  .description('Start local preview server')
  .argument('[path]', 'Path to serve', '.')
  .option('-p, --port <number>', 'Port number', '3000')
  .option('--open', 'Open browser automatically')
  .action(serveCommand);

// Watch command
program
  .command('watch')
  .description('Watch for file changes and auto-convert')
  .argument('<path>', 'File or directory to watch')
  .option('-o, --output <dir>', 'Output directory', './output')
  .action(watchCommand);

// Batch command
program
  .command('batch')
  .description('Batch convert multiple files')
  .argument('<pattern>', 'Glob pattern (e.g., "*.hwp" or "docs/**/*.pdf")')
  .option('-o, --output <dir>', 'Output directory', './output')
  .action(batchCommand);

program.parse();

