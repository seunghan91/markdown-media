import { spawn } from 'child_process';
import { resolve, extname, basename } from 'path';
import { globSync } from 'glob';

export function batchCommand(pattern, options) {
  console.log('📦 MDM Batch Conversion');
  console.log(`   Pattern: ${pattern}`);
  console.log(`   Output: ${options.output}\n`);
  
  // Find all matching files
  let files;
  try {
    files = globSync(pattern);
  } catch (e) {
    console.error(`❌ Error: Invalid pattern: ${pattern}`);
    process.exit(1);
  }
  
  if (files.length === 0) {
    console.log('No files found matching the pattern.');
    process.exit(0);
  }
  
  console.log(`Found ${files.length} file(s):\n`);
  
  // Supported extensions
  const supportedExt = ['.hwp', '.hwpx', '.pdf', '.docx', '.html', '.htm'];
  
  const convertFile = (index) => {
    if (index >= files.length) {
      console.log(`\n✅ Batch conversion complete! (${files.length} files)`);
      return;
    }
    
    const file = files[index];
    const ext = extname(file).toLowerCase();
    
    if (!supportedExt.includes(ext)) {
      console.log(`⏭️  Skipping unsupported format: ${file}`);
      convertFile(index + 1);
      return;
    }
    
    console.log(`[${index + 1}/${files.length}] Converting: ${file}`);
    
    // Determine converter
    let converter;
    if (ext === '.hwp') {
      converter = resolve('./converters/hwp_converter.py');
    } else if (ext === '.hwpx') {
      converter = resolve('./converters/hwpx_converter.py');
    } else if (ext === '.pdf') {
      converter = resolve('./converters/pdf_converter.py');
    } else if (ext === '.docx') {
      converter = resolve('./converters/docx_converter.py');
    } else if (ext === '.html' || ext === '.htm') {
      converter = resolve('./converters/html_converter.py');
    }
    
    const outputDir = resolve(options.output, basename(file, ext));
    const python = spawn('python3', [converter, resolve(file), outputDir]);
    
    python.stdout.on('data', (data) => {
      process.stdout.write(`   ${data}`);
    });
    
    python.stderr.on('data', (data) => {
      process.stderr.write(`   ${data}`);
    });
    
    python.on('close', (code) => {
      if (code === 0) {
        console.log(`   ✅ Done\n`);
      } else {
        console.log(`   ❌ Failed\n`);
      }
      convertFile(index + 1);
    });
  };
  
  convertFile(0);
}
