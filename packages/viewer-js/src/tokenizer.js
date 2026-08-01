/**
 * MDM 참조 문법을 토큰화합니다
 */
export class Tokenizer {
  constructor() {
    // 참조 패턴: MDM 브래킷 문법 ![[name:preset | attributes]] (그룹 1) 또는
    // 표준 CommonMark 이미지 ![alt](src) (그룹 2/3) — CLI 정본 산출물이 실제로
    // emit하는 문법(문법 단일화 이후). 하나의 정규식으로 같이 스캔해 문서 내
    // 등장 순서를 그대로 보존한다.
    this.patterns = {
      reference: /!\[\[([^\]]+)\]\]|!\[([^\]]*)\]\(([^)]+)\)/g,
      resourceParts: /^([^:|]+)(?::([^|]+))?(?:\s*\|\s*(.+))?$/,
      attribute: /(\w+)(?:=(?:"([^"]*)"|'([^']*)'|([^\s]+)))?/g
    };
  }

  /**
   * 텍스트에서 MDM 참조와 표준 마크다운 이미지를 찾아 토큰화합니다
   * @param {string} text - 파싱할 텍스트
   * @returns {Array} 토큰 배열
   */
  tokenize(text) {
    const tokens = [];
    let lastIndex = 0;
    let match;

    while ((match = this.patterns.reference.exec(text)) !== null) {
      // 이전 텍스트 추가
      if (match.index > lastIndex) {
        tokens.push({
          type: 'text',
          value: text.slice(lastIndex, match.index)
        });
      }

      if (match[1] !== undefined) {
        // MDM 브래킷 참조: ![[name:preset | attrs]]
        const parsed = this.parseReference(match[1]);
        tokens.push({
          type: 'mdm-reference',
          raw: match[0],
          ...parsed
        });
      } else {
        // 표준 마크다운 이미지: ![alt](src) — 속성/프리셋 없이 src를 그대로 사용
        tokens.push({
          type: 'image-reference',
          raw: match[0],
          alt: match[2] || '',
          src: (match[3] || '').trim()
        });
      }

      lastIndex = match.index + match[0].length;
    }

    // 나머지 텍스트 추가
    if (lastIndex < text.length) {
      tokens.push({
        type: 'text',
        value: text.slice(lastIndex)
      });
    }

    return tokens;
  }

  /**
   * MDM 참조를 파싱합니다
   * @param {string} reference - 참조 문자열
   * @returns {Object} 파싱된 참조 정보
   */
  parseReference(reference) {
    const match = reference.match(this.patterns.resourceParts);
    
    if (!match) {
      throw new Error(`Invalid MDM reference: ${reference}`);
    }

    const [, name, preset, attributesStr] = match;
    const attributes = attributesStr ? this.parseAttributes(attributesStr) : {};

    return {
      name: name.trim(),
      preset: preset ? preset.trim() : null,
      attributes
    };
  }

  /**
   * 속성 문자열을 파싱합니다
   * @param {string} attributesStr - 속성 문자열
   * @returns {Object} 파싱된 속성 객체
   */
  parseAttributes(attributesStr) {
    const attributes = {};
    let match;

    this.patterns.attribute.lastIndex = 0;
    while ((match = this.patterns.attribute.exec(attributesStr)) !== null) {
      const [, key, doubleQuoted, singleQuoted, unquoted] = match;
      const value = doubleQuoted || singleQuoted || unquoted || true;
      
      // 숫자로 변환 가능한 경우 변환
      if (typeof value === 'string' && /^\d+$/.test(value)) {
        attributes[key] = parseInt(value, 10);
      } else if (typeof value === 'string' && /^\d+\.\d+$/.test(value)) {
        attributes[key] = parseFloat(value);
      } else if (value === 'true') {
        attributes[key] = true;
      } else if (value === 'false') {
        attributes[key] = false;
      } else {
        attributes[key] = value;
      }
    }

    return attributes;
  }

  /**
   * 토큰을 다시 텍스트로 변환합니다 (디버깅용)
   * @param {Array} tokens - 토큰 배열
   * @returns {string} 재구성된 텍스트
   */
  reconstruct(tokens) {
    return tokens.map(token => {
      if (token.type === 'text') {
        return token.value;
      } else if (token.type === 'mdm-reference' || token.type === 'image-reference') {
        return token.raw;
      }
      return '';
    }).join('');
  }
}