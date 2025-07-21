import { MDMParser } from './parser.js';
import fs from 'fs/promises';
import path from 'path';
import { fileURLToPath } from 'url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

async function demo() {
  try {
    console.log('=== MDM Parser Demo ===\n');

    // 파서 생성
    const parser = new MDMParser();

    // MDM 파일 로드
    const mdmPath = path.join(__dirname, '../example/blog.mdm');
    console.log('1. Loading MDM file:', mdmPath);
    const mdmData = await parser.loadMDM(mdmPath);
    console.log('   Loaded resources:', Object.keys(mdmData.resources).join(', '));
    console.log('   Global presets:', Object.keys(mdmData.presets).join(', '));

    // 마크다운 파일 읽기
    const mdPath = path.join(__dirname, '../example/test.md');
    console.log('\n2. Reading markdown file:', mdPath);
    const markdown = await fs.readFile(mdPath, 'utf8');

    // 토큰화 테스트
    console.log('\n3. Tokenizing markdown...');
    const tokens = parser.tokenize(markdown);
    
    // MDM 참조 토큰 출력
    const mdmTokens = tokens.filter(t => t.type === 'mdm-reference');
    console.log(`   Found ${mdmTokens.length} MDM references:`);
    mdmTokens.forEach((token, i) => {
      console.log(`   ${i + 1}. ${token.name}${token.preset ? ':' + token.preset : ''}`);
      if (Object.keys(token.attributes).length > 0) {
        console.log(`      Attributes:`, token.attributes);
      }
    });

    // HTML 렌더링
    console.log('\n4. Rendering to HTML...');
    const html = await parser.parse(markdown);

    // 결과 저장
    const outputPath = path.join(__dirname, '../example/output.html');
    const fullHtml = `<!DOCTYPE html>
<html>
<head>
  <meta charset="UTF-8">
  <title>MDM Parser Demo</title>
  <style>
    body { 
      font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
      max-width: 1200px; 
      margin: 0 auto; 
      padding: 20px;
      line-height: 1.6;
    }
    img { max-width: 100%; height: auto; }
    figure { margin: 20px 0; text-align: center; }
    figcaption { margin-top: 10px; font-style: italic; color: #666; }
    .align-center { display: block; margin: 0 auto; }
    video { max-width: 100%; }
    h1, h2 { color: #333; }
    h1 { border-bottom: 2px solid #eee; padding-bottom: 10px; }
    h2 { margin-top: 30px; }
  </style>
</head>
<body>
${html}
</body>
</html>`;

    await fs.writeFile(outputPath, fullHtml);
    console.log('   Output saved to:', outputPath);

    // 일부 HTML 미리보기
    console.log('\n5. Sample HTML output:');
    const sampleHtml = html.split('\n').slice(0, 10).join('\n');
    console.log(sampleHtml);
    console.log('   ...');

    console.log('\n=== Demo Complete ===');
    console.log('Open example/output.html in a browser to see the result.');

  } catch (error) {
    console.error('Error:', error.message);
    console.error(error.stack);
  }
}

// 데모 실행
demo();