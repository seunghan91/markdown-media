import fs from 'node:fs';
import path from 'node:path';

const root = process.cwd();
const scanRoot = path.join(root, 'src');
const allowedFiles = new Set([
  path.join(scanRoot, 'lib', 'styles', 'tokens.css')
]);
const allowedExtensions = new Set(['.css', '.svelte', '.ts', '.js']);
const colorLiteralPattern = /(#[0-9a-fA-F]{3,8}\b|rgba?\(|hsla?\()/g;

const violations = [];

function walk(dir) {
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    const fullPath = path.join(dir, entry.name);

    if (entry.isDirectory()) {
      walk(fullPath);
      continue;
    }

    if (!allowedExtensions.has(path.extname(entry.name))) {
      continue;
    }

    if (allowedFiles.has(fullPath)) {
      continue;
    }

    const content = fs.readFileSync(fullPath, 'utf8');
    const lines = content.split('\n');

    lines.forEach((line, index) => {
      const match = line.match(colorLiteralPattern);
      if (match) {
        violations.push({
          file: path.relative(root, fullPath),
          line: index + 1,
          value: match[0]
        });
      }
    });
  }
}

walk(scanRoot);

if (violations.length > 0) {
  console.error('Hard-coded color literals are not allowed outside src/lib/styles/tokens.css');
  for (const violation of violations) {
    console.error(`${violation.file}:${violation.line} -> ${violation.value}`);
  }
  process.exit(1);
}

console.log('No hard-coded color literals found outside tokens.css.');
