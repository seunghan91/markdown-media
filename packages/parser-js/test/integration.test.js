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
