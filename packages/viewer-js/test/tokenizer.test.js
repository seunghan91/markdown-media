import { test } from 'node:test';
import assert from 'node:assert';
import { Tokenizer } from '../src/tokenizer.js';

test('Tokenizer', async (t) => {
  const tokenizer = new Tokenizer();

  await t.test('should tokenize simple MDM reference', () => {
    const input = '![[image]]';
    const tokens = tokenizer.tokenize(input);
    
    assert.strictEqual(tokens.length, 1);
    assert.strictEqual(tokens[0].type, 'mdm-reference');
    assert.strictEqual(tokens[0].name, 'image');
    assert.strictEqual(tokens[0].preset, null);
    assert.deepStrictEqual(tokens[0].attributes, {});
  });

  await t.test('should tokenize MDM reference with preset', () => {
    const input = '![[logo:small]]';
    const tokens = tokenizer.tokenize(input);
    
    assert.strictEqual(tokens[0].name, 'logo');
    assert.strictEqual(tokens[0].preset, 'small');
  });

  await t.test('should tokenize MDM reference with attributes', () => {
    const input = '![[image | width=500 align=center]]';
    const tokens = tokenizer.tokenize(input);
    
    assert.strictEqual(tokens[0].name, 'image');
    assert.strictEqual(tokens[0].attributes.width, 500);
    assert.strictEqual(tokens[0].attributes.align, 'center');
  });

  await t.test('should tokenize MDM reference with preset and attributes', () => {
    const input = '![[logo:header | opacity=0.8]]';
    const tokens = tokenizer.tokenize(input);
    
    assert.strictEqual(tokens[0].name, 'logo');
    assert.strictEqual(tokens[0].preset, 'header');
    assert.strictEqual(tokens[0].attributes.opacity, 0.8);
  });

  await t.test('should handle quoted attribute values', () => {
    const input = '![[image | caption="My Image" class=\'highlight\']]';
    const tokens = tokenizer.tokenize(input);
    
    assert.strictEqual(tokens[0].attributes.caption, 'My Image');
    assert.strictEqual(tokens[0].attributes.class, 'highlight');
  });

  await t.test('should handle boolean attributes', () => {
    const input = '![[video | controls autoplay muted]]';
    const tokens = tokenizer.tokenize(input);
    
    assert.strictEqual(tokens[0].attributes.controls, true);
    assert.strictEqual(tokens[0].attributes.autoplay, true);
    assert.strictEqual(tokens[0].attributes.muted, true);
  });

  await t.test('should handle mixed content', () => {
    const input = 'Before ![[image]] middle ![[video]] after';
    const tokens = tokenizer.tokenize(input);

    assert.strictEqual(tokens.length, 5);
    assert.strictEqual(tokens[0].type, 'text');
    assert.strictEqual(tokens[0].value, 'Before ');
    assert.strictEqual(tokens[1].type, 'mdm-reference');
    assert.strictEqual(tokens[2].type, 'text');
    assert.strictEqual(tokens[2].value, ' middle ');
    assert.strictEqual(tokens[3].type, 'mdm-reference');
    assert.strictEqual(tokens[4].type, 'text');
    assert.strictEqual(tokens[4].value, ' after');
  });

  // ─── 표준 CommonMark 이미지 (문법 단일화 — CLI 정본 산출물의 실제 문법) ───────
  await t.test('should tokenize standard markdown image', () => {
    const input = '![첫 번째 그림](assets/images/635dec1fdfc6.png)';
    const tokens = tokenizer.tokenize(input);

    assert.strictEqual(tokens.length, 1);
    assert.strictEqual(tokens[0].type, 'image-reference');
    assert.strictEqual(tokens[0].alt, '첫 번째 그림');
    assert.strictEqual(tokens[0].src, 'assets/images/635dec1fdfc6.png');
  });

  await t.test('should tokenize standard markdown image with empty alt', () => {
    const input = '![](assets/images/foo.png)';
    const tokens = tokenizer.tokenize(input);

    assert.strictEqual(tokens[0].type, 'image-reference');
    assert.strictEqual(tokens[0].alt, '');
    assert.strictEqual(tokens[0].src, 'assets/images/foo.png');
  });

  await t.test('should not confuse standard image with MDM bracket syntax', () => {
    const input = '![[bracket]] and ![standard](path.png)';
    const tokens = tokenizer.tokenize(input);

    const refs = tokens.filter(t => t.type !== 'text');
    assert.strictEqual(refs.length, 2);
    assert.strictEqual(refs[0].type, 'mdm-reference');
    assert.strictEqual(refs[1].type, 'image-reference');
    assert.strictEqual(refs[1].src, 'path.png');
  });

  await t.test('should preserve document order across mixed MDM and standard refs', () => {
    const input = 'A ![std1](a.png) B ![[mdm1]] C ![std2](b.png)';
    const tokens = tokenizer.tokenize(input);
    const refs = tokens.filter(t => t.type !== 'text');

    assert.deepStrictEqual(refs.map(t => t.type), ['image-reference', 'mdm-reference', 'image-reference']);
  });
});