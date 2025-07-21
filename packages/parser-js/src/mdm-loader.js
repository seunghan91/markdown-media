import fs from 'fs/promises';
import path from 'path';
import yaml from 'js-yaml';

/**
 * MDM 파일을 로드하고 파싱합니다
 */
export class MDMLoader {
  constructor() {
    this.cache = new Map();
  }

  /**
   * MDM 파일을 로드합니다
   * @param {string} mdmPath - MDM 파일 경로
   * @returns {Promise<Object>} 파싱된 MDM 데이터
   */
  async load(mdmPath) {
    // 캐시 확인
    if (this.cache.has(mdmPath)) {
      return this.cache.get(mdmPath);
    }

    try {
      const content = await fs.readFile(mdmPath, 'utf8');
      const data = this.parse(content, path.extname(mdmPath));
      
      // 유효성 검증
      this.validate(data);
      
      // 경로 정규화
      const normalized = this.normalizePaths(data, path.dirname(mdmPath));
      
      // 캐시 저장
      this.cache.set(mdmPath, normalized);
      
      return normalized;
    } catch (error) {
      throw new Error(`Failed to load MDM file: ${error.message}`);
    }
  }

  /**
   * MDM 콘텐츠를 파싱합니다
   * @param {string} content - 파일 내용
   * @param {string} ext - 파일 확장자
   * @returns {Object} 파싱된 데이터
   */
  parse(content, ext) {
    if (ext === '.json') {
      return JSON.parse(content);
    } else if (ext === '.yaml' || ext === '.yml' || ext === '.mdm') {
      return yaml.load(content);
    } else {
      throw new Error(`Unsupported MDM file extension: ${ext}`);
    }
  }

  /**
   * MDM 데이터 유효성을 검증합니다
   * @param {Object} data - MDM 데이터
   */
  validate(data) {
    if (!data.version) {
      throw new Error('MDM file must have a version field');
    }
    
    if (!data.resources) {
      data.resources = {};
    }
    
    // 각 리소스 유효성 검증
    for (const [name, resource] of Object.entries(data.resources)) {
      if (!resource.type) {
        throw new Error(`Resource "${name}" must have a type`);
      }
      
      if (!resource.src && resource.type !== 'embed') {
        throw new Error(`Resource "${name}" must have a src`);
      }
    }
  }

  /**
   * 경로를 정규화합니다
   * @param {Object} data - MDM 데이터
   * @param {string} basePath - 기준 경로
   * @returns {Object} 정규화된 데이터
   */
  normalizePaths(data, basePath) {
    const normalized = { ...data };
    const mediaRoot = data.media_root || './';
    
    // 리소스 경로 정규화
    if (normalized.resources) {
      for (const [name, resource] of Object.entries(normalized.resources)) {
        if (resource.src && !this.isAbsoluteUrl(resource.src)) {
          resource.src = path.join(basePath, mediaRoot, resource.src);
        }
        
        // 포스터 이미지 경로
        if (resource.poster && !this.isAbsoluteUrl(resource.poster)) {
          resource.poster = path.join(basePath, mediaRoot, resource.poster);
        }
        
        // variants 경로
        if (resource.variants) {
          for (const [variant, src] of Object.entries(resource.variants)) {
            if (!this.isAbsoluteUrl(src)) {
              resource.variants[variant] = path.join(basePath, mediaRoot, src);
            }
          }
        }
      }
    }
    
    return normalized;
  }

  /**
   * URL이 절대 경로인지 확인합니다
   * @param {string} url - 확인할 URL
   * @returns {boolean}
   */
  isAbsoluteUrl(url) {
    return /^https?:\/\//.test(url) || path.isAbsolute(url);
  }

  /**
   * 캐시를 초기화합니다
   */
  clearCache() {
    this.cache.clear();
  }
}