import { watch } from 'fs';
import { spawn } from 'child_process';
import { resolve, extname, dirname, basename } from 'path';
import { existsSync, statSync } from 'fs';

export function watchCommand(input, options) {
  const inputPath = resolve(input);
  
  if (!existsSync(inputPath)) {
    console.error(`❌ Error: Path not found: ${inputPath}`);
    process.exit(1);
  }
  
  const isDir = statSync(inputPath).isDirectory();
  const watchPath = isDir ? inputPath : dirname(inputPath);
  
  console.log('👀 MDM Watch Mode');
  console.log(`   Watching: ${watchPath}`);
  console.log(`   Output: ${options.output}`);
  console.log('\n   Press Ctrl+C to stop\n');
  
  // Supported extensions
  const supportedExt = ['.hwp', '.hwpx', '.pdf', '.docx', '.html', '.htm'];
  
  // Debounce map to prevent multiple triggers
  const debounceMap = new Map();
  
  watch(watchPath, { recursive: true }, (eventType, filename) => {
    if (!filename) return;
    
    const ext = extname(filename).toLowerCase();
    if (!supportedExt.includes(ext)) return;
    
    // Debounce - wait 500ms before processing
    const key = filename;
    if (debounceMap.has(key)) {
      clearTimeout(debounceMap.get(key));
    }
    
    debounceMap.set(key, setTimeout(() => {
      debounceMap.delete(key);
      
      const fullPath = isDir ? resolve(watchPath, filename) : inputPath;
      
      if (!existsSync(fullPath)) return; // File might have been deleted
      
      console.log(`\n📄 Change detected: ${filename}`);
      console.log(`   Converting...`);
      
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
      
      if (!converter) return;
      
      // Create output directory based on filename
      const outputDir = resolve(options.output, basename(filename, ext));
      
      // Run converter
      const python = spawn('python3', [converter, fullPath, outputDir]);
      
      python.stdout.on('data', (data) => {
        process.stdout.write(`   ${data}`);
      });
      
      python.stderr.on('data', (data) => {
        process.stderr.write(`   ${data}`);
      });
      
      python.on('close', (code) => {
        if (code === 0) {
          console.log(`   ✅ Converted: ${outputDir}`);
        } else {
          console.log(`   ❌ Conversion failed (code: ${code})`);
        }
        console.log('\n   Watching for changes...');
      });
    }, 500));
  });
  
  console.log('   Watching for changes...');
}
