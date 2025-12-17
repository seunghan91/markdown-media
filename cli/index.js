#!/usr/bin/env node

import { Command } from 'commander';
import { convertCommand } from './commands/convert.js';
import { validateCommand } from './commands/validate.js';
import { serveCommand } from './commands/serve.js';

const program = new Command();

program
  .name('mdm')
  .description('MDM (Markdown+Media) CLI tool')
  .version('0.1.0');

// Convert command
program
  .command('convert')
  .description('Convert HWP/PDF files to MDX format')
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

program.parse();
