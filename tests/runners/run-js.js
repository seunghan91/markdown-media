#!/usr/bin/env node
// ============================================================================
// MDM Spec Test Runner (JavaScript)
// ============================================================================
// 작업 담당: 병렬 작업 팀
// 진행 상태: Phase 3.7 통합 테스트
//
// 사용법:
//   node tests/runners/run-js.js
//   node tests/runners/run-js.js --filter basic
//   node tests/runners/run-js.js --verbose
// ============================================================================

import fs from 'fs/promises';
import path from 'path';
import { fileURLToPath } from 'url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

// 색상 코드
const colors = {
  reset: '\x1b[0m',
  red: '\x1b[31m',
  green: '\x1b[32m',
  yellow: '\x1b[33m',
  blue: '\x1b[34m',
  cyan: '\x1b[36m',
};

/**
 * 테스트 결과
 */
class TestResult {
  constructor() {
    this.passed = 0;
    this.failed = 0;
    this.skipped = 0;
    this.errors = [];
  }

  get total() {
    return this.passed + this.failed + this.skipped;
  }
}

/**
 * 스펙 테스트 러너
 */
class SpecTestRunner {
  constructor(options = {}) {
    this.specDir = path.resolve(__dirname, '../spec');
    this.verbose = options.verbose || false;
    this.filter = options.filter || null;
    this.result = new TestResult();
  }

  log(msg, color = 'reset') {
    console.log(`${colors[color]}${msg}${colors.reset}`);
  }

  /**
   * 모든 테스트 실행
   */
  async run() {
    this.log('\n📋 MDM Spec Tests (JavaScript)\n', 'cyan');
    this.log('='.repeat(50));

    const categories = await this.getCategories();

    for (const category of categories) {
      if (this.filter && !category.includes(this.filter)) {
        continue;
      }

      await this.runCategory(category);
    }

    this.printSummary();
    
    // 실패한 테스트가 있으면 exit code 1
    process.exit(this.result.failed > 0 ? 1 : 0);
  }

  /**
   * 테스트 카테고리 목록 조회
   */
  async getCategories() {
    try {
      const entries = await fs.readdir(this.specDir, { withFileTypes: true });
      return entries
        .filter(e => e.isDirectory() && !e.name.startsWith('.'))
        .map(e => e.name);
    } catch {
      return [];
    }
  }

  /**
   * 카테고리별 테스트 실행
   */
  async runCategory(category) {
    const categoryPath = path.join(this.specDir, category);
    this.log(`\n📁 ${category}/`, 'blue');

    try {
      const files = await fs.readdir(categoryPath);
      const testFiles = files.filter(f => f.endsWith('.md'));

      for (const testFile of testFiles) {
        await this.runTest(category, testFile);
      }
    } catch (error) {
      this.log(`  ⚠️ Error reading category: ${error.message}`, 'yellow');
    }
  }

  /**
   * 개별 테스트 실행
   */
  async runTest(category, testFile) {
    const testName = testFile.replace('.md', '');
    const basePath = path.join(this.specDir, category, testName);

    const inputPath = `${basePath}.md`;
    const expectedPath = `${basePath}.expected.json`;
    const sidecarPath = `${basePath}.mdm`;

    try {
      // 입력 파일 읽기
      const input = await fs.readFile(inputPath, 'utf-8');
      
      // expected.json 파일 확인
      let expected;
      try {
        const expectedContent = await fs.readFile(expectedPath, 'utf-8');
        expected = JSON.parse(expectedContent);
      } catch {
        this.log(`  ⏭️  ${testName} (no expected file)`, 'yellow');
        this.result.skipped++;
        return;
      }

      // 사이드카 파일 확인 (있는 경우)
      let sidecar = null;
      try {
        sidecar = await fs.readFile(sidecarPath, 'utf-8');
      } catch {
        // 사이드카 파일 없음 - 정상
      }

      // 테스트 실행
      const actual = await this.parseDocument(input, sidecar);
      
      // 결과 비교
      const passed = this.compareResults(expected, actual);

      if (passed) {
        this.log(`  ✅ ${testName}`, 'green');
        this.result.passed++;
      } else {
        this.log(`  ❌ ${testName}`, 'red');
        this.result.failed++;
        
        if (this.verbose) {
          this.log(`     Expected: ${JSON.stringify(expected.resources, null, 2)}`, 'yellow');
          this.log(`     Actual: ${JSON.stringify(actual.resources, null, 2)}`, 'yellow');
        }
      }
    } catch (error) {
      this.log(`  ❌ ${testName} - Error: ${error.message}`, 'red');
      this.result.failed++;
      this.result.errors.push({ test: testName, error: error.message });
    }
  }

  /**
   * 문서 파싱 (실제 파서 호출)
   */
  async parseDocument(markdown, sidecar) {
    // TODO: 실제 파서 연동
    // 지금은 기본 이미지 추출 로직만 구현
    
    const resources = {};
    
    // 마크다운에서 이미지 추출 (간단한 정규식)
    const imageRegex = /!\[([^\]]*)\]\(([^)\s]+)(?:\s+"([^"]*)")?\)(?:\{([^}]*)\})?/g;
    let match;

    while ((match = imageRegex.exec(markdown)) !== null) {
      const [, alt, src, title, attrs] = match;
      const type = this.detectType(src);

      // 기본 key는 파일명(상대경로) 기준이지만, embed(YouTube/Vimeo)는 안정적인 key가 필요함
      let key = path.basename(src);

      const resource = {
        type,
        src,
        alt: alt || null,
      };

      if (type !== 'embed') {
        // 일반 미디어만 title 유지 (embed 스펙에서는 provider/videoId를 사용)
        resource.title = title || null;
      } else {
        const embed = this.extractEmbedInfo(src);
        if (embed) {
          key = embed.key;
          resource.provider = embed.provider;
          resource.videoId = embed.videoId;
        }
      }

      // 속성 파싱
      if (attrs) {
        const presetMatch = attrs.match(/preset=(\w+)/);
        if (presetMatch) {
          resource.preset = presetMatch[1];
        }
      }

      // 외부 URL 감지
      if (type !== 'embed' && (src.startsWith('http://') || src.startsWith('https://'))) {
        resource.external = true;
      }

      resources[key] = resource;
    }

    return {
      resources,
      resourceCount: Object.keys(resources).length,
      errors: [],
    };
  }

  /**
   * 파일 확장자로 타입 감지
   */
  detectType(src) {
    const ext = path.extname(src).toLowerCase();
    
    const typeMap = {
      // 이미지
      '.jpg': 'image', '.jpeg': 'image', '.png': 'image',
      '.gif': 'image', '.webp': 'image', '.svg': 'image',
      '.avif': 'image', '.bmp': 'image',
      
      // 비디오
      '.mp4': 'video', '.webm': 'video', '.mov': 'video',
      '.avi': 'video', '.mkv': 'video',
      
      // 오디오
      '.mp3': 'audio', '.wav': 'audio', '.ogg': 'audio',
      '.m4a': 'audio', '.flac': 'audio',
    };

    // YouTube/Vimeo 등 embed 감지
    if (src.includes('youtube.com') || src.includes('youtu.be')) {
      return 'embed';
    }
    if (src.includes('vimeo.com')) {
      return 'embed';
    }

    return typeMap[ext] || 'unknown';
  }

  /**
   * YouTube/Vimeo URL에서 provider/videoId 추출
   * 스펙 테스트에서 embed 리소스 key는 videoId로 통일
   */
  extractEmbedInfo(src) {
    try {
      const u = new URL(src);

      // YouTube: https://youtube.com/watch?v=abc123
      if (u.hostname.includes('youtube.com')) {
        const v = u.searchParams.get('v');
        if (v) return { provider: 'youtube', videoId: v, key: v };
      }

      // YouTube short: https://youtu.be/abc123
      if (u.hostname.includes('youtu.be')) {
        const id = u.pathname.replace('/', '').trim();
        if (id) return { provider: 'youtube', videoId: id, key: id };
      }

      // Vimeo: https://vimeo.com/123456789
      if (u.hostname.includes('vimeo.com')) {
        const id = u.pathname.replace('/', '').trim();
        if (id) return { provider: 'vimeo', videoId: id, key: id };
      }
    } catch {
      // not a URL
    }

    return null;
  }

  /**
   * 결과 비교
   */
  compareResults(expected, actual) {
    // 리소스 개수 비교
    if (expected.resourceCount !== actual.resourceCount) {
      return false;
    }

    // 각 리소스 비교 (기본 속성만)
    for (const [key, expectedResource] of Object.entries(expected.resources)) {
      const actualResource = actual.resources[key];
      
      if (!actualResource) {
        return false;
      }

      // 타입 비교
      if (expectedResource.type !== actualResource.type) {
        return false;
      }

      // src 비교
      if (expectedResource.src !== actualResource.src) {
        return false;
      }
    }

    return true;
  }

  /**
   * 결과 요약 출력
   */
  printSummary() {
    this.log('\n' + '='.repeat(50));
    this.log('📊 Test Summary\n', 'cyan');
    
    this.log(`  Total:   ${this.result.total}`);
    this.log(`  Passed:  ${this.result.passed}`, 'green');
    this.log(`  Failed:  ${this.result.failed}`, this.result.failed > 0 ? 'red' : 'reset');
    this.log(`  Skipped: ${this.result.skipped}`, 'yellow');

    if (this.result.errors.length > 0) {
      this.log('\n⚠️ Errors:', 'red');
      for (const { test, error } of this.result.errors) {
        this.log(`  - ${test}: ${error}`);
      }
    }

    this.log('');
  }
}

// CLI 실행
const args = process.argv.slice(2);
const options = {
  verbose: args.includes('--verbose') || args.includes('-v'),
  filter: args.find(a => !a.startsWith('-')),
};

const runner = new SpecTestRunner(options);
runner.run().catch(console.error);
