/**
 * MDMLoader tests — ManifestV2(JSON) 정본 파싱 + legacy YAML 폴백 + media_type pass-through
 */
import { test } from 'node:test';
import assert from 'node:assert';
import fs from 'fs/promises';
import os from 'os';
import path from 'path';
import { fileURLToPath } from 'url';
import { MDMLoader } from '../src/mdm-loader.js';

const __dirname = path.dirname(fileURLToPath(import.meta.url));

// CLI(Rust)가 생산하는 실물 ManifestV2 산출물 (core/src/manifest.rs)
// output/ 은 저장소 루트 .gitignore(72행)에 걸려 CI에 존재하지 않으므로,
// 계약 검증의 정본은 추적 파일인 아래 FIXTURE_MDM_PATH 로 한다.
const REAL_MDM_PATH = path.join(__dirname, '../../../output/보도_125091_0.mdm');

// 실물 산출물의 매니페스트 구조(자산 3개)를 그대로 복사한 추적 픽스처.
// 실제 asset 바이너리(assets/images/*)는 담지 않는다 — 저장소 루트 .gitignore
// 의 `assets/`(75행) 규칙이 임의 깊이의 assets/ 디렉터리를 전부 무시해
// 바이너리를 넣어도 git add 시 조용히 누락되는 동일한 함정을 재현하기 때문.
// 파일 존재 검증은 아래 "실물 CLI 산출물 검증" 보너스 테스트가 전담한다.
const FIXTURE_MDM_PATH = path.join(__dirname, 'fixtures/manifest-v2-sample/보도_125091_0.mdm');

async function writeTmpFile(name, content) {
  const dir = await fs.mkdtemp(path.join(os.tmpdir(), 'mdm-loader-test-'));
  const filePath = path.join(dir, name);
  await fs.writeFile(filePath, content, 'utf8');
  return filePath;
}

// ─── ManifestV2 JSON (CLI 정본) ────────────────────────────────────────────────
test('MDMLoader: ManifestV2 JSON fixture converts assets[] to resources map', async (t) => {
  const manifestV2 = {
    version: '2.0',
    source: {
      filename: 'test.hwpx',
      format: 'hwpx',
      size_bytes: 100,
      hash: 'abc123',
      title: null,
      author: null,
      pages: null,
    },
    assets: [
      {
        id: 'image_001',
        media_type: 'image',
        src: 'assets/images/a1b2c3.png',
        content_hash: 'a1b2c3',
        original_name: null,
        metadata: {
          page: 1,
          width: 800,
          height: 600,
          format: 'png',
          caption: '표지 이미지',
          alt_text: 'cover image',
        },
      },
      {
        id: 'table_001',
        media_type: 'table',
        src: 'assets/tables/d4e5f6.html',
        content_hash: 'd4e5f6',
        original_name: null,
        metadata: { page: 2, format: 'html' },
      },
    ],
    stats: {
      total_assets: 2,
      images: 1,
      tables: 1,
      charts: 0,
      equations: 0,
      markdown_lines: 10,
      markdown_chars: 200,
      conversion_ms: 5,
    },
  };

  const filePath = await writeTmpFile('doc.mdm', JSON.stringify(manifestV2));
  const loader = new MDMLoader();
  const data = await loader.load(filePath);

  await t.test('version preserved', () => {
    assert.strictEqual(data.version, '2.0');
  });

  await t.test('resources map keyed by asset id', () => {
    assert.ok(data.resources.image_001);
    assert.ok(data.resources.table_001);
  });

  await t.test('media_type passed through as type', () => {
    assert.strictEqual(data.resources.image_001.type, 'image');
    assert.strictEqual(data.resources.table_001.type, 'table');
  });

  await t.test('metadata mapped to alt/caption/width/height', () => {
    assert.strictEqual(data.resources.image_001.alt, 'cover image');
    assert.strictEqual(data.resources.image_001.caption, '표지 이미지');
    assert.strictEqual(data.resources.image_001.width, 800);
    assert.strictEqual(data.resources.image_001.height, 600);
  });

  await t.test('src path normalized relative to mdm file location (no media_root needed)', () => {
    assert.ok(data.resources.image_001.src.endsWith(path.join('assets', 'images', 'a1b2c3.png')));
  });
});

// ─── Unknown media_type pass-through (whitelist 없음) ──────────────────────────
test('MDMLoader: unknown media_type passes through without whitelist', async (t) => {
  const manifestV2 = {
    version: '2.0',
    source: { filename: 'x.pdf', format: 'pdf', size_bytes: 1, hash: 'h', title: null, author: null, pages: null },
    assets: [
      {
        id: 'shape_001',
        media_type: 'shape', // MediaType enum에 아직 없는 미래 타입 가정 (예: Shape)
        src: 'assets/shapes/f00.svg',
        content_hash: 'f00',
        original_name: null,
        metadata: {},
      },
    ],
    stats: {
      total_assets: 1,
      images: 0,
      tables: 0,
      charts: 0,
      equations: 0,
      markdown_lines: 0,
      markdown_chars: 0,
      conversion_ms: 0,
    },
  };

  const filePath = await writeTmpFile('shape.mdm', JSON.stringify(manifestV2));
  const loader = new MDMLoader();
  const data = await loader.load(filePath);

  await t.test('unknown type preserved as-is (no whitelist rejection)', () => {
    assert.strictEqual(data.resources.shape_001.type, 'shape');
  });
});

// ─── Legacy YAML은 여전히 로드됨 (deprecated fallback) ─────────────────────────
test('MDMLoader: legacy YAML .mdm still loads (deprecated fallback)', async (t) => {
  const legacyYaml = [
    'version: "1.0"',
    'media_root: ./assets',
    'resources:',
    '  logo:',
    '    type: image',
    '    src: logo.png',
    '    alt: Logo',
  ].join('\n');

  const filePath = await writeTmpFile('legacy.mdm', legacyYaml);
  const loader = new MDMLoader();
  const data = await loader.load(filePath);

  await t.test('version parsed', () => {
    assert.strictEqual(data.version, '1.0');
  });

  await t.test('resources.logo loaded from YAML', () => {
    assert.strictEqual(data.resources.logo.type, 'image');
    assert.ok(data.resources.logo.src.endsWith(path.join('assets', 'logo.png')));
  });
});

// ─── 실물 ManifestV2 계약 검증 (추적 픽스처 — CI에서 항상 실행) ────────────────
test('MDMLoader: tracked fixture (실물 구조 복사본) loads as 3 assets', async (t) => {
  const loader = new MDMLoader();
  const data = await loader.load(FIXTURE_MDM_PATH);

  await t.test('resources map has exactly 3 assets', () => {
    assert.strictEqual(Object.keys(data.resources).length, 3);
  });

  await t.test('all three are image type with bundle-relative src paths', () => {
    for (const id of ['image_001', 'image_002', 'image_003']) {
      assert.strictEqual(data.resources[id].type, 'image');
      assert.ok(data.resources[id].src.includes('assets'));
    }
  });

  await t.test('content_hash-derived filenames preserved in src', () => {
    assert.ok(data.resources.image_001.src.endsWith('635dec1fdfc6.PNG'));
    assert.ok(data.resources.image_002.src.endsWith('f8e0c88413ce.JPG'));
    assert.ok(data.resources.image_003.src.endsWith('721ec72f2317.PNG'));
  });
});

// ─── 실물 CLI 산출물 검증 (output/ 존재 시 추가 검증하는 보너스) ────────────────
test('MDMLoader: real CLI output (output/보도_125091_0.mdm) loads as 3 assets', async (t) => {
  const stat = await fs.stat(REAL_MDM_PATH).catch(() => null);

  if (!stat) {
    t.skip('output/보도_125091_0.mdm not present in this checkout (gitignored — 로컬 생성물)');
    return;
  }

  const loader = new MDMLoader();
  const data = await loader.load(REAL_MDM_PATH);

  await t.test('resources map has exactly 3 assets', () => {
    assert.strictEqual(Object.keys(data.resources).length, 3);
  });

  await t.test('all three are image type with resolved src paths', () => {
    for (const id of ['image_001', 'image_002', 'image_003']) {
      assert.strictEqual(data.resources[id].type, 'image');
      assert.ok(data.resources[id].src.includes('assets'));
    }
  });

  await t.test('resolved asset files actually exist on disk', async () => {
    for (const id of Object.keys(data.resources)) {
      await assert.doesNotReject(fs.access(data.resources[id].src));
    }
  });
});
