// ============================================================================
// 🚧 작업 중 - 이 파일은 현재 [병렬 작업 팀]에서 작업 중입니다
// ============================================================================
// 작업 담당: 병렬 작업 팀
// 시작 시간: 2025-12-31
// 진행 상태: Phase 3.1 npm 패키지 준비
// ============================================================================

import resolve from '@rollup/plugin-node-resolve';
import commonjs from '@rollup/plugin-commonjs';
import terser from '@rollup/plugin-terser';
import { readFileSync } from 'fs';

const pkg = JSON.parse(readFileSync('./package.json', 'utf-8'));

// 번들에서 제외할 외부 의존성
const external = [
  ...Object.keys(pkg.dependencies || {}),
  ...Object.keys(pkg.peerDependencies || {}),
  'fs', 'fs/promises', 'path', 'url',
];

// 공통 플러그인
const plugins = [
  resolve({
    preferBuiltins: true,
  }),
  commonjs(),
];

// 프로덕션 빌드 여부
const isProd = process.env.NODE_ENV === 'production';

export default [
  // =============================================
  // ESM 빌드 (ES Modules)
  // =============================================
  {
    input: 'src/index.js',
    output: {
      file: 'dist/index.mjs',
      format: 'es',
      sourcemap: true,
      exports: 'named',
      banner: `/**
 * @mdm/parser v${pkg.version}
 * MDM (Markdown+Media) Parser
 * (c) ${new Date().getFullYear()} MDM Team
 * @license MIT
 */`,
    },
    external,
    plugins: [
      ...plugins,
      isProd && terser({
        ecma: 2020,
        module: true,
        compress: {
          passes: 2,
          pure_getters: true,
        },
      }),
    ].filter(Boolean),
  },
  
  // =============================================
  // CommonJS 빌드
  // =============================================
  {
    input: 'src/index.js',
    output: {
      file: 'dist/index.cjs',
      format: 'cjs',
      sourcemap: true,
      exports: 'named',
      banner: `/**
 * @mdm/parser v${pkg.version}
 * MDM (Markdown+Media) Parser
 * (c) ${new Date().getFullYear()} MDM Team
 * @license MIT
 */`,
    },
    external,
    plugins: [
      ...plugins,
      isProd && terser({
        ecma: 2018,
        compress: {
          passes: 2,
        },
      }),
    ].filter(Boolean),
  },
  
  // =============================================
  // 브라우저용 UMD 빌드 (의존성 번들링)
  // =============================================
  {
    input: 'src/index.js',
    output: {
      file: 'dist/index.umd.js',
      format: 'umd',
      name: 'MDMParser',
      sourcemap: true,
      exports: 'named',
      globals: {
        'js-yaml': 'jsyaml',
      },
      banner: `/**
 * @mdm/parser v${pkg.version}
 * MDM (Markdown+Media) Parser - Browser Bundle
 * (c) ${new Date().getFullYear()} MDM Team
 * @license MIT
 */`,
    },
    // UMD 빌드에서는 일부 의존성만 외부 처리
    external: ['js-yaml'],
    plugins: [
      resolve({
        browser: true,
        preferBuiltins: false,
      }),
      commonjs(),
      isProd && terser({
        ecma: 2018,
        compress: {
          passes: 2,
        },
      }),
    ].filter(Boolean),
  },
  
  // =============================================
  // 브라우저용 ES Module 빌드
  // =============================================
  {
    input: 'src/index.js',
    output: {
      file: 'dist/index.browser.mjs',
      format: 'es',
      sourcemap: true,
      exports: 'named',
      banner: `/**
 * @mdm/parser v${pkg.version}
 * MDM (Markdown+Media) Parser - Browser ES Module
 * (c) ${new Date().getFullYear()} MDM Team
 * @license MIT
 */`,
    },
    external: ['js-yaml'],
    plugins: [
      resolve({
        browser: true,
        preferBuiltins: false,
      }),
      commonjs(),
      isProd && terser({
        ecma: 2020,
        module: true,
        compress: {
          passes: 2,
          pure_getters: true,
        },
      }),
    ].filter(Boolean),
  },
];
