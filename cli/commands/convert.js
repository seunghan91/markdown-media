import { spawn } from 'child_process';
import { existsSync } from 'fs';
import { resolve, extname } from 'path';

export async function convertCommand(input, options) {
  console.log('🔄 Converting file...');
  console.log(`  Input: ${input}`);
  console.log(`  Output: ${options.output}`);
  console.log(`  Format: ${options.format}`);

  const inputPath = resolve(input);
  
  if (!existsSync(inputPath)) {
    console.error(`❌ Error: File not found: ${input}`);
    process.exit(1);
  }

  const ext = extname(inputPath).toLowerCase();
  
  // Determine converter based on file extension
  let converter, converterArgs;
  if (ext === '.hwp') {
    converter = resolve('./converters/hwp_converter.py');
    converterArgs = [converter, inputPath, options.output];
  } else if (ext === '.hwpx') {
    converter = resolve('./converters/hwpx_converter.py');
    converterArgs = [converter, inputPath, options.output];
  } else if (ext === '.pdf') {
    converter = resolve('./converters/pdf_converter.py');
    converterArgs = [converter, inputPath, options.output];
  } else if (ext === '.docx') {
    converter = resolve('./converters/docx_converter.py');
    converterArgs = [converter, inputPath, options.output];
  } else if (ext === '.html' || ext === '.htm') {
    converter = resolve('./converters/html_converter.py');
    converterArgs = [converter, inputPath, options.output];
  } else {
    console.error(`❌ Error: Unsupported file format: ${ext}`);
    console.error('   Supported formats: .hwp, .hwpx, .pdf, .docx, .html, .htm');
    process.exit(1);
  }

  // Call Python converter
  const python = spawn('python3', converterArgs);

  python.stdout.on('data', (data) => {
    process.stdout.write(data);
  });

  python.stderr.on('data', (data) => {
    process.stderr.write(data);
  });

  python.on('close', (code) => {
    if (code === 0) {
      console.log('\n✅ Conversion complete!');
    } else {
      console.error(`\n❌ Conversion failed with code ${code}`);
      process.exit(code);
    }
  });
}
