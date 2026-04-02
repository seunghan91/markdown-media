import { strict as assert } from 'node:assert';
import {
  parseAnnexText,
  parseDate,
  parseDateWithReference,
  createChainPlan,
  aggregateChainResults,
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

console.log('\n=== ALL SMOKE TESTS PASSED ===');
