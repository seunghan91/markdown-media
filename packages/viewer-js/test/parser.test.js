import { test } from 'node:test';
import assert from 'node:assert';
import { MDMParser, parse } from '../src/parser.js';

test('MDMParser', async (t) => {

  // ─── tokenize() ──────────────────────────────────────────────────────────
  await t.test('tokenize() delegates to Tokenizer', () => {
    const parser = new MDMParser();
    const tokens = parser.tokenize('Hello ![[img.jpg]] World');
    assert.ok(Array.isArray(tokens));
    assert.strictEqual(tokens.length, 3);
    assert.strictEqual(tokens[1].type, 'mdm-reference');
    assert.strictEqual(tokens[1].name, 'img.jpg');
  });

  // ─── setMDMData / getMDMData ─────────────────────────────────────────────
  await t.test('setMDMData and getMDMData', () => {
    const parser = new MDMParser();
    assert.strictEqual(parser.getMDMData(), null);

    const data = {
      version: '1.0',
      resources: { logo: { type: 'image', src: '/logo.png', alt: 'Logo' } },
    };
    parser.setMDMData(data);
    assert.deepStrictEqual(parser.getMDMData(), data);
  });

  // ─── parse() without MDM file ────────────────────────────────────────────
  await t.test('parse() converts direct file references to HTML', async () => {
    const parser = new MDMParser();
    const html = await parser.parse('Look at ![[photo.jpg]] here');
    assert.ok(typeof html === 'string');
    assert.ok(html.includes('<img'));
    assert.ok(html.includes('src="photo.jpg"'));
    assert.ok(html.includes('Look at'));
    assert.ok(html.includes('here'));
  });

  await t.test('parse() handles video files', async () => {
    const parser = new MDMParser();
    const html = await parser.parse('![[intro.mp4 | controls width=720]]');
    assert.ok(html.includes('<video'));
    assert.ok(html.includes('controls'));
    assert.ok(html.includes('width="720"'));
  });

  await t.test('parse() handles audio files', async () => {
    const parser = new MDMParser();
    const html = await parser.parse('![[podcast.mp3 | controls]]');
    assert.ok(html.includes('<audio'));
  });

  await t.test('parse() plain text is preserved', async () => {
    const parser = new MDMParser();
    const html = await parser.parse('No media here, just text.');
    assert.strictEqual(html, 'No media here, just text.');
  });

  await t.test('parse() multiple references in sequence', async () => {
    const parser = new MDMParser();
    const html = await parser.parse('![[a.jpg]] and ![[b.png]]');
    const imgMatches = html.match(/<img/g);
    assert.strictEqual(imgMatches?.length, 2);
  });

  // ─── parse() with inline MDM data ────────────────────────────────────────
  await t.test('parse() uses pre-loaded MDM data', async () => {
    const parser = new MDMParser();
    parser.setMDMData({
      version: '1.0',
      resources: {
        hero: { type: 'image', src: '/images/hero.jpg', alt: 'Hero' },
      },
    });
    const html = await parser.parse('![[hero]]');
    assert.ok(html.includes('src="/images/hero.jpg"'));
    assert.ok(html.includes('alt="Hero"'));
  });

  await t.test('parse() attribute override on named resource', async () => {
    const parser = new MDMParser();
    parser.setMDMData({
      version: '1.0',
      resources: {
        logo: { type: 'image', src: '/logo.png', alt: 'Logo' },
      },
    });
    const html = await parser.parse('![[logo | width=200 align=center]]');
    assert.ok(html.includes('width="200"'));
    assert.ok(html.includes('align-center'));
  });

  // ─── clearCache() ─────────────────────────────────────────────────────────
  await t.test('clearCache() resets mdmData and renderer', () => {
    const parser = new MDMParser();
    parser.setMDMData({ version: '1.0', resources: {} });
    assert.notStrictEqual(parser.getMDMData(), null);
    parser.clearCache();
    assert.strictEqual(parser.getMDMData(), null);
  });

  // ─── parse() convenience function ────────────────────────────────────────
  await t.test('parse() top-level convenience function', async () => {
    const html = await parse('![[sunset.jpg | width=800]]');
    assert.ok(html.includes('<img'));
    assert.ok(html.includes('width="800"'));
  });

  // ─── caption / figure ────────────────────────────────────────────────────
  await t.test('parse() caption attribute wraps in figure', async () => {
    const parser = new MDMParser();
    const html = await parser.parse('![[img.jpg | caption="A nice photo"]]');
    assert.ok(html.includes('<figure>'));
    assert.ok(html.includes('<figcaption>A nice photo</figcaption>'));
  });

  // ─── HTML escaping ────────────────────────────────────────────────────────
  await t.test('parse() escapes XSS in caption', async () => {
    const parser = new MDMParser();
    const html = await parser.parse('![[img.jpg | caption="<script>bad</script>"]]');
    assert.ok(!html.includes('<script>'));
    assert.ok(html.includes('&lt;script&gt;'));
  });
});
