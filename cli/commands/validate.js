import { existsSync, statSync, readdirSync } from 'fs';
import { resolve, join, extname } from 'path';
import { readFile } from 'fs/promises';
import yaml from 'js-yaml';

export async function validateCommand(path, options) {
  console.log('🔍 Validating MDM bundle...');
  console.log(`  Path: ${path}\n`);

  const targetPath = resolve(path);
  
  if (!existsSync(targetPath)) {
    console.error(`❌ Error: Path not found: ${path}`);
    process.exit(1);
  }

  const stats = statSync(targetPath);
  let errors = [];
  let warnings = [];

  if (stats.isDirectory()) {
    // Validate directory as bundle
    validateBundle(targetPath, errors, warnings, options.verbose);
  } else {
    // Validate single file
    await validateFile(targetPath, errors, warnings, options.verbose);
  }

  // Print results
  console.log('\n📊 Validation Results:');
  
  if (errors.length === 0 && warnings.length === 0) {
    console.log('✅ All checks passed!');
  } else {
    if (warnings.length > 0) {
      console.log(`\n⚠️  ${warnings.length} warning(s):`);
      warnings.forEach(w => console.log(`  - ${w}`));
    }
    
    if (errors.length > 0) {
      console.log(`\n❌ ${errors.length} error(s):`);
      errors.forEach(e => console.log(`  - ${e}`));
      process.exit(1);
    }
  }
}

function validateBundle(dirPath, errors, warnings, verbose) {
  const files = readdirSync(dirPath);
  
  const mdxFiles = files.filter(f => f.endsWith('.mdx'));
  const mdmFiles = files.filter(f => f.endsWith('.mdm'));
  
  if (mdxFiles.length === 0) {
    warnings.push('No .mdx files found in bundle');
  }
  
  if (verbose) {
    console.log(`  Found ${mdxFiles.length} MDX file(s)`);
    console.log(`  Found ${mdmFiles.length} MDM file(s)`);
  }
  
  // Check for matching pairs
  mdxFiles.forEach(mdx => {
    const baseName = mdx.replace('.mdx', '');
    const mdmFile = `${baseName}.mdm`;
    
    if (!mdmFiles.includes(mdmFile)) {
      warnings.push(`Missing sidecar file: ${mdmFile}`);
    }
  });
}

async function validateFile(filePath, errors, warnings, verbose) {
  const ext = extname(filePath);
  
  if (ext === '.mdx') {
    if (verbose) console.log('  Validating MDX file...');
    // Basic MDX validation
    try {
      const content = await readFile(filePath, 'utf-8');
      if (content.length === 0) {
        errors.push('MDX file is empty');
      }
    } catch (e) {
      errors.push(`Failed to read file: ${e.message}`);
    }
  } else if (ext === '.mdm') {
    if (verbose) console.log('  Validating MDM file...');
    // Validate JSON format
    try {
      const content = await readFile(filePath, 'utf-8');
      const data = JSON.parse(content);
      
      if (!data.version) {
        warnings.push('Missing version field');
      }
      if (!data.resources) {
        warnings.push('Missing resources field');
      }
    } catch (e) {
      errors.push(`Invalid JSON: ${e.message}`);
    }
  } else {
    warnings.push(`Unknown file type: ${ext}`);
  }
}
