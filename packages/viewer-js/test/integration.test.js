/**
 * Integration tests — full pipeline: tokenize → render → HTML
 * No file I/O; uses setMDMData() to inject MDM config.
 */
import { test } from 'node:test';
import assert from 'node:assert';
import { MDMParser } from '../src/parser.js';

// ─── Helpers ──────────────────────────────────────────────────────────────────
function makeParser(resources = {}, presets = {}) {
  const parser = new MDMParser();
  parser.setMDMData({ version: '1.0', resources, presets });
  return parser;
}

// ─── Blog-style document ──────────────────────────────────────────────────────
test('Integration: blog-style document', async (t) => {
  const parser = makeParser({
    'site-logo': { type: 'image', src: '/assets/logo.png', alt: 'My Blog Logo' },
    'hero-welcome': { type: 'image', src: '/assets/hero.jpg', alt: 'Hero' },
    'intro-video': {
      type: 'video',
      src: '/assets/intro.mp4',
      presets: {
        inline: { width: 800, controls: true },
        bg: { autoplay: true, muted: true, loop: true },
      },
    },
    'youtube-demo': { type: 'embed', provider: 'youtube', id: 'dQw4w9WgXcQ' },
  });

  const markdown = [
    'Welcome to my blog.',
    '',
    '![[site-logo]]',
    '',
    '![[hero-welcome | width=1200]]',
    '',
    '![[intro-video:inline]]',
    '',
    '![[youtube-demo | width=800 height=450]]',
    '',
    'Thanks for reading!',
  ].join('\n');

  const html = await parser.parse(markdown);

  await t.test('logo renders as img', () => {
    assert.ok(html.includes('src="/assets/logo.png"'));
    assert.ok(html.includes('alt="My Blog Logo"'));
  });

  await t.test('hero rendered with width override', () => {
    assert.ok(html.includes('width="1200"'));
  });

  await t.test('video with inline preset has controls', () => {
    assert.ok(html.includes('controls'));
    assert.ok(html.includes('width="800"'));
  });

  await t.test('youtube iframe generated', () => {
    assert.ok(html.includes('youtube.com/embed/dQw4w9WgXcQ'));
    assert.ok(html.includes('width="800"'));
    assert.ok(html.includes('height="450"'));
  });

  await t.test('plain text is preserved', () => {
    assert.ok(html.includes('Welcome to my blog.'));
    assert.ok(html.includes('Thanks for reading!'));
  });
});

// ─── Direct file references (no MDM data) ─────────────────────────────────────
test('Integration: direct file references without MDM', async (t) => {
  const parser = new MDMParser();

  const markdown = [
    '![[photo.jpg | width=500 align=center alt="A sunset"]]',
    '![[demo.mp4 | controls width=720]]',
    '![[podcast.mp3 | controls]]',
  ].join('\n');

  const html = await parser.parse(markdown);

  await t.test('image attributes applied', () => {
    assert.ok(html.includes('width="500"'));
    assert.ok(html.includes('class="align-center"'));
    assert.ok(html.includes('alt="A sunset"'));
  });

  await t.test('video attributes applied', () => {
    assert.ok(html.includes('<video'));
    assert.ok(html.includes('controls'));
    assert.ok(html.includes('width="720"'));
  });

  await t.test('audio rendered', () => {
    assert.ok(html.includes('<audio'));
  });
});

// ─── Global presets ───────────────────────────────────────────────────────────
test('Integration: global presets', async (t) => {
  const parser = makeParser(
    { banner: { type: 'image', src: '/banner.jpg', alt: 'Banner' } },
    { hero: { width: 1200, height: 400 } },
  );

  const html = await parser.parse('![[banner:hero]]');

  await t.test('global hero preset width applied', () => {
    assert.ok(html.includes('width="1200"'));
  });

  await t.test('global hero preset height applied', () => {
    assert.ok(html.includes('height="400"'));
  });
});

// ─── Captions & figures ───────────────────────────────────────────────────────
test('Integration: figure with caption', async (t) => {
  const parser = makeParser({
    screenshot: { type: 'image', src: '/shot.png', alt: 'App' },
  });

  const html = await parser.parse('![[screenshot | caption="Main dashboard"]]');

  await t.test('wraps in <figure>', () => {
    assert.ok(html.includes('<figure>'));
  });

  await t.test('figcaption present', () => {
    assert.ok(html.includes('<figcaption>Main dashboard</figcaption>'));
  });
});

// ─── Security: XSS prevention ─────────────────────────────────────────────────
test('Integration: XSS prevention', async (t) => {
  const parser = new MDMParser();

  await t.test('script injection in caption is escaped', async () => {
    const html = await parser.parse('![[img.jpg | caption="<script>alert(1)</script>"]]');
    assert.ok(!html.includes('<script>'));
    assert.ok(html.includes('&lt;script&gt;'));
  });

  await t.test('script injection in alt is escaped', async () => {
    const html = await parser.parse('![[img.jpg | alt="<img onerror=alert(1)>"]]');
    // The injected < > are HTML-escaped so the tag cannot become a real element
    assert.ok(html.includes('&lt;img'));
    assert.ok(html.includes('&gt;'));
    // The outer <img> tag itself must not have an onerror attribute
    // (onerror= only appears inside the properly-escaped alt value)
    const outerTag = html.match(/<img[^>]*>/)?.[0] ?? '';
    assert.ok(!outerTag.match(/^<img[^"]*onerror/));
  });
});

// ─── Unknown file type ────────────────────────────────────────────────────────
test('Integration: unknown file type returns comment', async () => {
  const parser = new MDMParser();
  const html = await parser.parse('![[document.pdf]]');
  assert.ok(html.includes('<!--'));
});

// ─── Empty / edge cases ───────────────────────────────────────────────────────
test('Integration: edge cases', async (t) => {
  const parser = new MDMParser();

  await t.test('empty string returns empty string', async () => {
    assert.strictEqual(await parser.parse(''), '');
  });

  await t.test('only plain text returns text unchanged', async () => {
    const result = await parser.parse('Just some text, no media.');
    assert.strictEqual(result, 'Just some text, no media.');
  });

  await t.test('multiple MDM refs on same line', async () => {
    const html = await parser.parse('![[a.jpg]] and ![[b.jpg]]');
    const count = (html.match(/<img/g) || []).length;
    assert.strictEqual(count, 2);
  });
});

// ─── CLI 산출 mdx 계약: 표준 ![](assets/images/…) 참조 (문법 단일화, codex P1) ──
// core/src/main.rs 의 convert_hwp/hwpx/docx/pdf 가 실제로 emit하는 형태를
// 그대로 재현한다 — MDM 브래킷 문법(![[ ]])이 아니라 표준 CommonMark 이미지.
// 뷰어가 이 문법을 <img> 로 렌더링하지 못하면 CLI 산출물이 뷰어에서 안 보이는
// 회귀가 발생한다 (tokenizer.js 가 ![[ ]] 만 인식하던 상태에서 확인된 갭).
test('Integration: CLI-style standard markdown image references', async (t) => {
  await t.test('single standard image ref renders as <img> with real src/alt', async () => {
    const parser = new MDMParser();
    const html = await parser.parse('![첫 번째 그림](assets/images/635dec1fdfc6.png)');
    assert.ok(html.includes('<img'));
    assert.ok(html.includes('src="assets/images/635dec1fdfc6.png"'));
    assert.ok(html.includes('alt="첫 번째 그림"'));
  });

  await t.test('full CLI-shaped mdx body (text + heading + inline image + dedup pair)', async () => {
    const parser = new MDMParser();
    const mdx = [
      '이미지 삽입 테스트 문서입니다.',
      '',
      '![첫 번째 그림](assets/images/3cf87ebd8dae.png)',
      '',
      '두 그림 사이에 위치한 본문 문단입니다.',
      '',
      '![중복 그림](assets/images/3cf87ebd8dae.png)',
      '',
      '문서의 마지막 문단입니다.',
    ].join('\n');
    const html = await parser.parse(mdx);

    const imgCount = (html.match(/<img/g) || []).length;
    assert.strictEqual(imgCount, 2, 'both refs (same dedup hash) should render as <img>');
    assert.ok(html.includes('src="assets/images/3cf87ebd8dae.png"'));
    assert.ok(html.includes('alt="첫 번째 그림"'));
    assert.ok(html.includes('alt="중복 그림"'));
    assert.ok(html.includes('이미지 삽입 테스트 문서입니다.'));
    assert.ok(html.includes('문서의 마지막 문단입니다.'));
  });

  await t.test('standard image ref does not require an .mdm manifest', async () => {
    // No setMDMData() call — CLI output is self-contained; the viewer must not
    // need a sidecar resource lookup to render a path-based image reference.
    const parser = new MDMParser();
    const html = await parser.parse('![alt](assets/images/hash12.jpg)');
    assert.ok(html.includes('<img'));
    assert.ok(!html.includes('<!--'));
  });

  await t.test('standard and MDM bracket syntax coexist without cross-contamination', async () => {
    const parser = makeParser({ logo: { type: 'image', src: '/logo.png', alt: 'Logo' } });
    const html = await parser.parse('![[logo]] and ![photo](assets/images/x.png)');
    assert.ok(html.includes('src="/logo.png"'));
    assert.ok(html.includes('src="assets/images/x.png"'));
    const count = (html.match(/<img/g) || []).length;
    assert.strictEqual(count, 2);
  });
});
