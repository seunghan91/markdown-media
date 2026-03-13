import { test } from 'node:test';
import assert from 'node:assert';
import { Renderer } from '../src/renderer.js';

test('Renderer', async (t) => {

  // ─── 직접 파일 참조 ───────────────────────────────────────────────────────
  await t.test('renderDirectFile: image by extension', async (inner) => {
    const renderer = new Renderer();

    for (const ext of ['jpg', 'jpeg', 'png', 'gif', 'webp', 'svg']) {
      await inner.test(`renders .${ext} as <img>`, () => {
        const tokens = [{ type: 'mdm-reference', name: `photo.${ext}`, preset: null, attributes: {} }];
        const html = renderer.render(tokens);
        assert.ok(html.includes('<img'), `expected <img> for .${ext}, got: ${html}`);
        assert.ok(html.includes(`src="photo.${ext}"`));
      });
    }

    for (const ext of ['mp4', 'webm', 'ogg']) {
      await inner.test(`renders .${ext} as <video>`, () => {
        const tokens = [{ type: 'mdm-reference', name: `clip.${ext}`, preset: null, attributes: {} }];
        const html = renderer.render(tokens);
        assert.ok(html.includes('<video'), `expected <video> for .${ext}`);
      });
    }

    for (const ext of ['mp3', 'wav']) {
      await inner.test(`renders .${ext} as <audio>`, () => {
        const tokens = [{ type: 'mdm-reference', name: `sound.${ext}`, preset: null, attributes: {} }];
        const html = renderer.render(tokens);
        assert.ok(html.includes('<audio'), `expected <audio> for .${ext}`);
      });
    }
  });

  // ─── 이미지 속성 ───────────────────────────────────────────────────────────
  await t.test('image attributes', async (inner) => {
    const renderer = new Renderer();

    await inner.test('width attribute', () => {
      const tokens = [{ type: 'mdm-reference', name: 'img.jpg', preset: null, attributes: { width: 500 } }];
      assert.ok(renderer.render(tokens).includes('width="500"'));
    });

    await inner.test('height attribute', () => {
      const tokens = [{ type: 'mdm-reference', name: 'img.jpg', preset: null, attributes: { height: 300 } }];
      assert.ok(renderer.render(tokens).includes('height="300"'));
    });

    await inner.test('alt attribute', () => {
      const tokens = [{ type: 'mdm-reference', name: 'img.jpg', preset: null, attributes: { alt: 'test image' } }];
      assert.ok(renderer.render(tokens).includes('alt="test image"'));
    });

    await inner.test('align attribute adds class', () => {
      const tokens = [{ type: 'mdm-reference', name: 'img.jpg', preset: null, attributes: { align: 'center' } }];
      assert.ok(renderer.render(tokens).includes('class="align-center"'));
    });

    await inner.test('caption wraps in <figure>', () => {
      const tokens = [{ type: 'mdm-reference', name: 'img.jpg', preset: null, attributes: { caption: 'My Caption' } }];
      const html = renderer.render(tokens);
      assert.ok(html.includes('<figure>'));
      assert.ok(html.includes('<figcaption>My Caption</figcaption>'));
    });

    await inner.test('escapes HTML in alt', () => {
      const tokens = [{ type: 'mdm-reference', name: 'img.jpg', preset: null, attributes: { alt: '<script>alert(1)</script>' } }];
      const html = renderer.render(tokens);
      assert.ok(!html.includes('<script>'));
      assert.ok(html.includes('&lt;script&gt;'));
    });
  });

  // ─── 비디오 속성 ───────────────────────────────────────────────────────────
  await t.test('video attributes', async (inner) => {
    const renderer = new Renderer();

    await inner.test('controls attribute', () => {
      const tokens = [{ type: 'mdm-reference', name: 'vid.mp4', preset: null, attributes: { controls: true } }];
      assert.ok(renderer.render(tokens).includes('controls'));
    });

    await inner.test('autoplay + muted + loop', () => {
      const tokens = [{ type: 'mdm-reference', name: 'vid.mp4', preset: null, attributes: { autoplay: true, muted: true, loop: true } }];
      const html = renderer.render(tokens);
      assert.ok(html.includes('autoplay'));
      assert.ok(html.includes('muted'));
      assert.ok(html.includes('loop'));
    });

    await inner.test('width attribute on video', () => {
      const tokens = [{ type: 'mdm-reference', name: 'vid.mp4', preset: null, attributes: { width: 720 } }];
      assert.ok(renderer.render(tokens).includes('width="720"'));
    });
  });

  // ─── 오디오 속성 ───────────────────────────────────────────────────────────
  await t.test('audio attributes', async (inner) => {
    const renderer = new Renderer();

    await inner.test('controls on audio', () => {
      const tokens = [{ type: 'mdm-reference', name: 'track.mp3', preset: null, attributes: { controls: true } }];
      const html = renderer.render(tokens);
      assert.ok(html.includes('<audio'));
      assert.ok(html.includes('controls'));
    });
  });

  // ─── MDM 데이터 기반 렌더링 ───────────────────────────────────────────────
  await t.test('renders resources from MDM data', async (inner) => {
    const mdmData = {
      version: '1.0',
      media_root: './',
      resources: {
        logo: { type: 'image', src: '/assets/logo.png', alt: 'Logo' },
        demo: { type: 'video', src: '/assets/demo.mp4' },
        podcast: { type: 'audio', src: '/assets/ep1.mp3' },
        yt: { type: 'embed', provider: 'youtube', id: 'abc123' },
        vimeo: { type: 'embed', provider: 'vimeo', id: '456' },
      },
    };
    const renderer = new Renderer(mdmData);

    await inner.test('image resource renders src from mdmData', () => {
      const tokens = [{ type: 'mdm-reference', name: 'logo', preset: null, attributes: {} }];
      const html = renderer.render(tokens);
      assert.ok(html.includes('src="/assets/logo.png"'));
      assert.ok(html.includes('alt="Logo"'));
    });

    await inner.test('video resource', () => {
      const tokens = [{ type: 'mdm-reference', name: 'demo', preset: null, attributes: {} }];
      assert.ok(renderer.render(tokens).includes('<video'));
    });

    await inner.test('audio resource', () => {
      const tokens = [{ type: 'mdm-reference', name: 'podcast', preset: null, attributes: {} }];
      assert.ok(renderer.render(tokens).includes('<audio'));
    });

    await inner.test('youtube embed', () => {
      const tokens = [{ type: 'mdm-reference', name: 'yt', preset: null, attributes: {} }];
      const html = renderer.render(tokens);
      assert.ok(html.includes('youtube.com/embed/abc123'));
    });

    await inner.test('vimeo embed', () => {
      const tokens = [{ type: 'mdm-reference', name: 'vimeo', preset: null, attributes: {} }];
      const html = renderer.render(tokens);
      assert.ok(html.includes('vimeo.com/video/456'));
    });

    await inner.test('unknown embed provider returns comment', () => {
      const mdm2 = { resources: { x: { type: 'embed', provider: 'unknown' } } };
      const r2 = new Renderer(mdm2);
      const tokens = [{ type: 'mdm-reference', name: 'x', preset: null, attributes: {} }];
      const html = r2.render(tokens);
      assert.ok(html.includes('<!--'));
    });
  });

  // ─── 프리셋 적용 ──────────────────────────────────────────────────────────
  await t.test('preset resolution', async (inner) => {
    const mdmData = {
      version: '1.0',
      resources: {
        banner: {
          type: 'image',
          src: '/img/banner.jpg',
          presets: { mobile: { width: 400 } },
        },
      },
      presets: {
        hero: { width: 1200 },
      },
    };
    const renderer = new Renderer(mdmData);

    await inner.test('resource-level preset applied', () => {
      const tokens = [{ type: 'mdm-reference', name: 'banner', preset: 'mobile', attributes: {} }];
      assert.ok(renderer.render(tokens).includes('width="400"'));
    });

    await inner.test('global mdmData preset applied', () => {
      const tokens = [{ type: 'mdm-reference', name: 'banner', preset: 'hero', attributes: {} }];
      assert.ok(renderer.render(tokens).includes('width="1200"'));
    });

    await inner.test('inline attributes override preset', () => {
      const tokens = [{ type: 'mdm-reference', name: 'banner', preset: 'mobile', attributes: { width: 999 } }];
      assert.ok(renderer.render(tokens).includes('width="999"'));
    });
  });

  // ─── 텍스트 토큰 통과 ─────────────────────────────────────────────────────
  await t.test('text tokens pass through unchanged', () => {
    const renderer = new Renderer();
    const tokens = [
      { type: 'text', value: 'Hello ' },
      { type: 'mdm-reference', name: 'img.png', preset: null, attributes: {} },
      { type: 'text', value: ' World' },
    ];
    const html = renderer.render(tokens);
    assert.ok(html.startsWith('Hello '));
    assert.ok(html.endsWith(' World'));
  });

  // ─── 알 수 없는 리소스 타입 ──────────────────────────────────────────────
  await t.test('unknown resource type returns comment', () => {
    const mdmData = { resources: { x: { type: 'unknown', src: 'x.bin' } } };
    const renderer = new Renderer(mdmData);
    const tokens = [{ type: 'mdm-reference', name: 'x', preset: null, attributes: {} }];
    assert.ok(renderer.render(tokens).includes('<!--'));
  });
});
