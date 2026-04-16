/**
 * MDM 참조 문법을 토큰화합니다
 */
export class Tokenizer {
  constructor() {
    // MDM 참조 패턴: ![[name:preset | attributes]]
    this.patterns = {
      mdmReference: /!\[\[([^\]]+)\]\]/g,
      resourceParts: /^([^:|]+)(?::([^|]+))?(?:\s*\|\s*(.+))?$/,
      attribute: /(\w+)(?:=(?:"([^"]*)"|'([^']*)'|([^\s]+)))?/g
    };
  }

  /**
   * 텍스트에서 MDM 참조를 찾아 토큰화합니다
   * @param {string} text - 파싱할 텍스트
   * @returns {Array} 토큰 배열
   */
  tokenize(text) {
    const tokens = [];
    let lastIndex = 0;
    let match;

    // MDM 참조 찾기
    while ((match = this.patterns.mdmReference.exec(text)) !== null) {
      // 이전 텍스트 추가
      if (match.index > lastIndex) {
        tokens.push({
          type: 'text',
          value: text.slice(lastIndex, match.index)
        });
      }

      // MDM 참조 파싱
      const reference = match[1];
      const parsed = this.parseReference(reference);
      
      tokens.push({
        type: 'mdm-reference',
        raw: match[0],
        ...parsed
      });

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
      } else if (token.type === 'mdm-reference') {
        return token.raw;
      }
      return '';
    }).join('');
  }
}