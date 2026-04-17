import { strict as assert } from 'node:assert';
import { readFileSync, existsSync } from 'node:fs';
import {
  parseAnnexText,
  parseDate,
  parseDateWithReference,
  createChainPlan,
  aggregateChainResults,
  getVersion,
  detectFormat,
  convertBytes,
  convertFile,
  convertToJson,
} from '../index.js';

// --- Annex Parser ---
console.log('Testing parseAnnexText...');
const annexes = parseAnnexText('[별표 1] 위반행위의 종류와 과태료\n내용 첫 줄\n\n[별표 2] 수수료\n수수료 내용');
assert(Array.isArray(annexes), 'Should return array');
assert(annexes.length >= 1, `Expected >=1 annexes, got ${annexes.length}`);
if (annexes.length > 0) {
  assert.equal(typeof annexes[0].annexType, 'string');
  assert.equal(typeof annexes[0].number, 'number');
  assert.equal(typeof annexes[0].markdown, 'string');
  console.log(`  Found ${annexes.length} annex(es): ${annexes.map(a => a.annexType + ' ' + a.number).join(', ')}`);
}

// --- Date Parser ---
console.log('Testing parseDate...');
const today = parseDate('오늘');
assert(today !== null && today !== undefined, 'parseDate("오늘") should return result');
assert.equal(typeof today.date, 'string');
assert.equal(today.date.length, 8, 'Date should be YYYYMMDD format');
assert.equal(today.format, 'relative');
console.log(`  오늘 = ${today.date}`);

const abs = parseDate('2024년 3월 1일');
assert(abs !== null);
assert.equal(abs.date, '20240301');
assert.equal(abs.format, 'absolute');
console.log(`  2024년 3월 1일 = ${abs.date}`);

console.log('Testing parseDateWithReference...');
const refResult = parseDateWithReference('내일', '20260402');
assert(refResult !== null);
assert.equal(refResult.date, '20260403');
console.log(`  내일 (ref 20260402) = ${refResult.date}`);

const garbage = parseDate('아무말대잔치');
assert(garbage === null || garbage === undefined, 'Garbage input should return null');
console.log('  Garbage input correctly returned null');

// --- Chain Planner ---
console.log('Testing createChainPlan...');
const plan = createChainPlan('full_research', '음주운전 처벌');
assert.equal(typeof plan.chainType, 'string');
assert.equal(typeof plan.description, 'string');
assert(Array.isArray(plan.steps), 'steps should be array');
assert.equal(plan.steps.length, 4, `FullResearch should have 4 steps, got ${plan.steps.length}`);
assert.equal(plan.steps[0].toolName, 'search_law_names');
assert(Array.isArray(plan.executableGroups), 'executableGroups should be array');
console.log(`  Chain: ${plan.chainType}, ${plan.steps.length} steps, ${plan.executableGroups.length} groups`);

console.log('Testing aggregateChainResults...');
const md = aggregateChainResults('full_research', ['법령 검색 결과', '조문 내용', '판례 목록']);
assert(md.includes('포괄적 법률 조사'), 'Should contain chain description');
assert(md.includes('법령 검색 결과'), 'Should contain result text');
console.log('  Aggregation OK');

// --- Error handling ---
console.log('Testing error handling...');
try {
  createChainPlan('invalid_type', 'test');
  assert.fail('Should have thrown for invalid chain type');
} catch (e) {
  assert(e.message.includes('Unknown chain type'), `Expected "Unknown chain type", got: ${e.message}`);
  console.log('  Invalid chain type correctly throws');
}

// --- Unified Document API (HWP/HWPX/PDF/DOCX) ---
console.log('Testing getVersion...');
const v = getVersion();
assert(typeof v === 'string' && v.length > 0, 'getVersion should return non-empty string');
console.log(`  version = ${v}`);

console.log('Testing detectFormat...');
assert.equal(detectFormat(Buffer.from('%PDF-1.7'), 'x.pdf'), 'pdf');
assert.equal(detectFormat(Buffer.from('%PDF-1.7'), 'no-ext'), 'pdf'); // magic-byte fallback
assert.equal(detectFormat(Buffer.from('junk'), 'doc.hwpx'), 'hwpx'); // extension wins
assert.equal(detectFormat(Buffer.from('junk'), 'unknown.xyz'), 'unknown');
console.log('  format detection OK (ext + magic bytes)');

import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';
const __dirname = dirname(fileURLToPath(import.meta.url));
const sampleHwpx = join(__dirname, '..', '..', '..', 'samples', 'input', '2026년 제1기 행정안전부 청년인턴 채용 공고(최종).hwpx');
if (existsSync(sampleHwpx)) {
  console.log('Testing convertBytes / convertFile / convertToJson on HWPX...');
  const data = readFileSync(sampleHwpx);
  const md = convertBytes(data, sampleHwpx);
  assert(typeof md === 'string' && md.length > 100, 'convertBytes should return non-trivial markdown');
  assert.equal(convertFile(sampleHwpx), md, 'convertFile must match convertBytes');
  const json = JSON.parse(convertToJson(data, sampleHwpx));
  assert.equal(json.format, 'hwpx');
  assert(typeof json.markdown === 'string' && json.markdown.length > 0);
  assert(json.metadata && typeof json.metadata.section_count === 'number');
  console.log(`  HWPX: ${md.length} chars, ${json.metadata.section_count} sections, ${json.metadata.table_count} tables`);
} else {
  console.log('  (skipped HWPX sample — file not present)');
}

console.log('Testing convertBytes error on unknown format...');
try {
  convertBytes(Buffer.from('some garbage'), 'foo.xyz');
  assert.fail('Should throw on unknown format');
} catch (e) {
  assert(e.message.includes('Unknown document format'), `Expected unknown-format error, got: ${e.message}`);
  console.log('  unknown format correctly throws');
}

console.log('\n=== ALL SMOKE TESTS PASSED ===');
