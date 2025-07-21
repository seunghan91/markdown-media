import { MDMLoader } from './mdm-loader.js';
import { Tokenizer } from './tokenizer.js';
import { Renderer } from './renderer.js';

/**
 * MDM 파서 메인 클래스
 */
export class MDMParser {
  constructor(options = {}) {
    this.options = {
      mdmPath: null,
      ...options
    };
    
    this.loader = new MDMLoader();
    this.tokenizer = new Tokenizer();
    this.renderer = null;
    this.mdmData = null;
  }

  /**
   * MDM 파일을 로드합니다
   * @param {string} mdmPath - MDM 파일 경로
   */
  async loadMDM(mdmPath) {
    this.mdmData = await this.loader.load(mdmPath);
    this.renderer = new Renderer(this.mdmData);
    return this.mdmData;
  }

  /**
   * 마크다운 텍스트를 파싱하여 HTML로 변환합니다
   * @param {string} markdown - 마크다운 텍스트
   * @param {Object} options - 파싱 옵션
   * @returns {Promise<string>} HTML 문자열
   */
  async parse(markdown, options = {}) {
    // MDM 파일 로드 (아직 로드되지 않았으면)
    if (options.mdmPath && !this.mdmData) {
      await this.loadMDM(options.mdmPath);
    }

    // 토큰화
    const tokens = this.tokenizer.tokenize(markdown);

    // 렌더링
    if (!this.renderer) {
      this.renderer = new Renderer(this.mdmData);
    }
    
    return this.renderer.render(tokens);
  }

  /**
   * 마크다운 텍스트를 토큰으로만 변환합니다 (디버깅용)
   * @param {string} markdown - 마크다운 텍스트
   * @returns {Array} 토큰 배열
   */
  tokenize(markdown) {
    return this.tokenizer.tokenize(markdown);
  }

  /**
   * MDM 데이터를 직접 설정합니다
   * @param {Object} mdmData - MDM 데이터
   */
  setMDMData(mdmData) {
    this.mdmData = mdmData;
    this.renderer = new Renderer(mdmData);
  }

  /**
   * 현재 로드된 MDM 데이터를 반환합니다
   * @returns {Object|null} MDM 데이터
   */
  getMDMData() {
    return this.mdmData;
  }

  /**
   * 캐시를 초기화합니다
   */
  clearCache() {
    this.loader.clearCache();
    this.mdmData = null;
    this.renderer = null;
  }
}

/**
 * 간편 사용을 위한 함수
 * @param {string} markdown - 마크다운 텍스트
 * @param {Object} options - 파싱 옵션
 * @returns {Promise<string>} HTML 문자열
 */
export async function parse(markdown, options = {}) {
  const parser = new MDMParser();
  return parser.parse(markdown, options);
}