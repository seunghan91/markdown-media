/**
 * MDM 토큰을 HTML로 렌더링합니다
 */
export class Renderer {
  constructor(mdmData = null) {
    this.mdmData = mdmData;
  }

  /**
   * 토큰 배열을 HTML로 렌더링합니다
   * @param {Array} tokens - 토큰 배열
   * @returns {string} HTML 문자열
   */
  render(tokens) {
    return tokens.map(token => this.renderToken(token)).join('');
  }

  /**
   * 개별 토큰을 렌더링합니다
   * @param {Object} token - 토큰 객체
   * @returns {string} HTML 문자열
   */
  renderToken(token) {
    if (token.type === 'text') {
      return token.value;
    } else if (token.type === 'mdm-reference') {
      return this.renderMDMReference(token);
    }
    return '';
  }

  /**
   * MDM 참조를 HTML로 렌더링합니다
   * @param {Object} token - MDM 참조 토큰
   * @returns {string} HTML 문자열
   */
  renderMDMReference(token) {
    const { name, preset, attributes } = token;

    // MDM 데이터에서 리소스 찾기
    const resource = this.mdmData?.resources?.[name];
    
    if (!resource) {
      // 리소스를 찾을 수 없으면 파일 경로로 간주
      return this.renderDirectFile(name, attributes);
    }

    // 프리셋 적용
    const presetAttrs = this.getPresetAttributes(resource, preset);
    const mergedAttrs = { ...resource, ...presetAttrs, ...attributes };

    // 리소스 타입별 렌더링
    switch (resource.type) {
      case 'image':
        return this.renderImage(mergedAttrs);
      case 'video':
        return this.renderVideo(mergedAttrs);
      case 'audio':
        return this.renderAudio(mergedAttrs);
      case 'embed':
        return this.renderEmbed(mergedAttrs);
      default:
        return `<!-- Unknown resource type: ${resource.type} -->`;
    }
  }

  /**
   * 프리셋 속성을 가져옵니다
   * @param {Object} resource - 리소스 객체
   * @param {string} preset - 프리셋 이름
   * @returns {Object} 프리셋 속성
   */
  getPresetAttributes(resource, preset) {
    if (!preset) return {};
    
    // 리소스별 프리셋
    if (resource.presets?.[preset]) {
      return resource.presets[preset];
    }
    
    // 전역 프리셋
    if (this.mdmData?.presets?.[preset]) {
      return this.mdmData.presets[preset];
    }
    
    return {};
  }

  /**
   * 이미지를 렌더링합니다
   * @param {Object} attrs - 속성 객체
   * @returns {string} HTML 문자열
   */
  renderImage(attrs) {
    const imgAttrs = [];
    
    // src 속성
    if (attrs.src) {
      imgAttrs.push(`src="${this.escapeHtml(attrs.src)}"`);
    }
    
    // alt 속성
    if (attrs.alt) {
      imgAttrs.push(`alt="${this.escapeHtml(attrs.alt)}"`);
    }
    
    // 크기 속성
    if (attrs.width) {
      imgAttrs.push(`width="${attrs.width}"`);
    }
    if (attrs.height) {
      imgAttrs.push(`height="${attrs.height}"`);
    }
    
    // loading 속성
    if (attrs.loading) {
      imgAttrs.push(`loading="${attrs.loading}"`);
    }
    
    // 스타일 속성
    const styles = this.buildStyles(attrs);
    if (styles) {
      imgAttrs.push(`style="${styles}"`);
    }
    
    // 클래스
    if (attrs.align) {
      imgAttrs.push(`class="align-${attrs.align}"`);
    }
    
    const img = `<img ${imgAttrs.join(' ')}>`;
    
    // 캡션이 있으면 figure로 감싸기
    if (attrs.caption) {
      return `<figure>${img}<figcaption>${this.escapeHtml(attrs.caption)}</figcaption></figure>`;
    }
    
    return img;
  }

  /**
   * 비디오를 렌더링합니다
   * @param {Object} attrs - 속성 객체
   * @returns {string} HTML 문자열
   */
  renderVideo(attrs) {
    const videoAttrs = [];
    
    if (attrs.src) {
      videoAttrs.push(`src="${this.escapeHtml(attrs.src)}"`);
    }
    
    if (attrs.width) {
      videoAttrs.push(`width="${attrs.width}"`);
    }
    if (attrs.height) {
      videoAttrs.push(`height="${attrs.height}"`);
    }
    
    if (attrs.poster) {
      videoAttrs.push(`poster="${this.escapeHtml(attrs.poster)}"`);
    }
    
    // 불린 속성들
    ['controls', 'autoplay', 'muted', 'loop'].forEach(attr => {
      if (attrs[attr]) {
        videoAttrs.push(attr);
      }
    });
    
    return `<video ${videoAttrs.join(' ')}></video>`;
  }

  /**
   * 오디오를 렌더링합니다
   * @param {Object} attrs - 속성 객체
   * @returns {string} HTML 문자열
   */
  renderAudio(attrs) {
    const audioAttrs = [];
    
    if (attrs.src) {
      audioAttrs.push(`src="${this.escapeHtml(attrs.src)}"`);
    }
    
    // 불린 속성들
    ['controls', 'autoplay', 'loop'].forEach(attr => {
      if (attrs[attr]) {
        audioAttrs.push(attr);
      }
    });
    
    return `<audio ${audioAttrs.join(' ')}></audio>`;
  }

  /**
   * 임베드를 렌더링합니다
   * @param {Object} attrs - 속성 객체
   * @returns {string} HTML 문자열
   */
  renderEmbed(attrs) {
    if (attrs.provider === 'youtube') {
      const width = attrs.width || 560;
      const height = attrs.height || 315;
      return `<iframe width="${width}" height="${height}" src="https://www.youtube.com/embed/${attrs.id}" frameborder="0" allowfullscreen></iframe>`;
    }
    
    if (attrs.provider === 'vimeo') {
      const width = attrs.width || 640;
      const height = attrs.height || 360;
      return `<iframe width="${width}" height="${height}" src="https://player.vimeo.com/video/${attrs.id}" frameborder="0" allowfullscreen></iframe>`;
    }
    
    return `<!-- Unsupported embed provider: ${attrs.provider} -->`;
  }

  /**
   * 직접 파일 참조를 렌더링합니다
   * @param {string} filename - 파일명
   * @param {Object} attrs - 속성 객체
   * @returns {string} HTML 문자열
   */
  renderDirectFile(filename, attrs) {
    // 파일 확장자로 타입 추론
    const ext = filename.split('.').pop().toLowerCase();
    const imageExts = ['jpg', 'jpeg', 'png', 'gif', 'webp', 'svg'];
    const videoExts = ['mp4', 'webm', 'ogg'];
    const audioExts = ['mp3', 'wav', 'ogg'];
    
    if (imageExts.includes(ext)) {
      return this.renderImage({ src: filename, ...attrs });
    } else if (videoExts.includes(ext)) {
      return this.renderVideo({ src: filename, ...attrs });
    } else if (audioExts.includes(ext)) {
      return this.renderAudio({ src: filename, ...attrs });
    }
    
    return `<!-- Unknown file type: ${filename} -->`;
  }

  /**
   * 스타일 문자열을 생성합니다
   * @param {Object} attrs - 속성 객체
   * @returns {string} 스타일 문자열
   */
  buildStyles(attrs) {
    const styles = [];
    
    if (attrs['max-width']) {
      styles.push(`max-width: ${attrs['max-width']}px`);
    }
    
    if (attrs['object-fit']) {
      styles.push(`object-fit: ${attrs['object-fit']}`);
    }
    
    if (attrs.margin) {
      styles.push(`margin: ${attrs.margin}`);
    }
    
    if (attrs.opacity) {
      styles.push(`opacity: ${attrs.opacity}`);
    }
    
    if (attrs.float) {
      styles.push(`float: ${attrs.float}`);
    }
    
    return styles.join('; ');
  }

  /**
   * HTML 이스케이프
   * @param {string} str - 이스케이프할 문자열
   * @returns {string} 이스케이프된 문자열
   */
  escapeHtml(str) {
    const htmlEscapes = {
      '&': '&amp;',
      '<': '&lt;',
      '>': '&gt;',
      '"': '&quot;',
      "'": '&#39;'
    };
    
    return str.replace(/[&<>"']/g, char => htmlEscapes[char]);
  }
}