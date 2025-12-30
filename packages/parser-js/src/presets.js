// ============================================================================
// 🚧 작업 중 - 이 파일은 현재 [병렬 작업 팀]에서 작업 중입니다
// ============================================================================
// 작업 담당: 병렬 작업 팀
// 시작 시간: 2025-12-31
// 진행 상태: Phase 2.2 Sidecar 파일 완전 구현
//
// ⚠️ 주의: 3.4/3.5 CI/CD는 C팀에서 작업 중입니다.
// ============================================================================

/**
 * MDM 내장 프리셋 정의
 * 
 * 프리셋은 자주 사용되는 이미지/미디어 변환 설정을 미리 정의한 것입니다.
 * 사용자는 직접 속성을 지정하거나 프리셋을 참조할 수 있습니다.
 * 
 * @example
 * // MDM 파일에서 프리셋 사용
 * resources:
 *   hero-image:
 *     src: ./hero.jpg
 *     preset: large  # 내장 프리셋 사용
 */

/**
 * 이미지 크기 프리셋
 * 일반적인 사용 사례에 맞춘 크기 설정
 */
export const SIZE_PRESETS = {
  /** 썸네일용 작은 이미지 (150x150) */
  thumb: {
    width: 150,
    height: 150,
    fit: 'cover',
    quality: 80,
    format: 'webp',
  },
  
  /** 작은 이미지 (320px 너비) */
  small: {
    width: 320,
    height: null,  // 비율 유지
    fit: 'contain',
    quality: 85,
  },
  
  /** 중간 크기 이미지 (640px 너비) */
  medium: {
    width: 640,
    height: null,
    fit: 'contain',
    quality: 85,
  },
  
  /** 큰 이미지 (1024px 너비) */
  large: {
    width: 1024,
    height: null,
    fit: 'contain',
    quality: 90,
  },
  
  /** 전체 화면 이미지 (1920px 너비) */
  full: {
    width: 1920,
    height: null,
    fit: 'contain',
    quality: 90,
  },
  
  /** 정사각형 (1:1) */
  square: {
    width: 500,
    height: 500,
    fit: 'cover',
    quality: 85,
  },
  
  /** 와이드스크린 (16:9) */
  widescreen: {
    width: 1280,
    height: 720,
    fit: 'cover',
    quality: 90,
  },
  
  /** 시네마 (21:9) */
  cinema: {
    width: 1680,
    height: 720,
    fit: 'cover',
    quality: 90,
  },
  
  /** 세로 (9:16) - 모바일/스토리 용 */
  portrait: {
    width: 720,
    height: 1280,
    fit: 'cover',
    quality: 85,
  },
  
  /** 아바타/프로필 이미지 */
  avatar: {
    width: 200,
    height: 200,
    fit: 'cover',
    quality: 80,
    format: 'webp',
    borderRadius: '50%',  // CSS용 힌트
  },
};

/**
 * 포맷별 기본 옵션
 */
export const FORMAT_DEFAULTS = {
  jpeg: {
    quality: 85,
    progressive: true,
  },
  
  png: {
    compressionLevel: 9,
    interlace: true,
  },
  
  webp: {
    quality: 85,
    lossless: false,
  },
  
  avif: {
    quality: 80,
    speed: 5,  // 0-10, 높을수록 빠름
  },
  
  gif: {
    colors: 256,
    dither: true,
  },
  
  svg: {
    cleanupIds: true,
    removeComments: true,
    minifyStyles: true,
  },
};

/**
 * 반응형 이미지 프리셋
 * srcset/sizes 생성에 사용
 */
export const RESPONSIVE_PRESETS = {
  /** 블로그 본문 이미지 */
  article: {
    widths: [320, 640, 960, 1280],
    sizes: '(max-width: 640px) 100vw, (max-width: 1024px) 90vw, 800px',
    format: 'webp',
    fallbackFormat: 'jpeg',
  },
  
  /** 전체 너비 히어로 이미지 */
  hero: {
    widths: [640, 960, 1280, 1920, 2560],
    sizes: '100vw',
    format: 'webp',
    fallbackFormat: 'jpeg',
    quality: 90,
  },
  
  /** 카드/그리드 썸네일 */
  card: {
    widths: [200, 400, 600],
    sizes: '(max-width: 640px) 50vw, 300px',
    format: 'webp',
    fallbackFormat: 'jpeg',
  },
  
  /** 갤러리 이미지 */
  gallery: {
    widths: [320, 640, 960],
    sizes: '(max-width: 480px) 100vw, (max-width: 768px) 50vw, 33vw',
    format: 'webp',
    fallbackFormat: 'jpeg',
  },
};

/**
 * 비디오 프리셋
 */
export const VIDEO_PRESETS = {
  /** 자동 재생 배경 비디오 */
  background: {
    autoplay: true,
    loop: true,
    muted: true,
    playsinline: true,
    preload: 'auto',
    controls: false,
  },
  
  /** 프레젠테이션/튜토리얼 비디오 */
  presentation: {
    autoplay: false,
    loop: false,
    muted: false,
    controls: true,
    preload: 'metadata',
  },
  
  /** 짧은 클립/GIF 대체 */
  clip: {
    autoplay: true,
    loop: true,
    muted: true,
    playsinline: true,
    preload: 'auto',
    controls: false,
    maxDuration: 30,  // 초
  },
};

/**
 * 오디오 프리셋
 */
export const AUDIO_PRESETS = {
  /** 배경 음악 */
  background: {
    autoplay: true,
    loop: true,
    volume: 0.3,
    controls: false,
    preload: 'auto',
  },
  
  /** 팟캐스트/음성 녹음 */
  podcast: {
    autoplay: false,
    loop: false,
    controls: true,
    preload: 'metadata',
  },
};

/**
 * 테마별 스타일 프리셋
 */
export const STYLE_PRESETS = {
  /** 기본 스타일 */
  default: {
    border: 'none',
    borderRadius: '0',
    shadow: 'none',
  },
  
  /** 카드 스타일 */
  card: {
    border: '1px solid #e0e0e0',
    borderRadius: '8px',
    shadow: '0 2px 8px rgba(0,0,0,0.1)',
    padding: '16px',
  },
  
  /** 둥근 모서리 */
  rounded: {
    borderRadius: '12px',
    overflow: 'hidden',
  },
  
  /** 그림자 효과 */
  elevated: {
    shadow: '0 4px 16px rgba(0,0,0,0.15)',
    borderRadius: '8px',
  },
  
  /** 테두리 강조 */
  bordered: {
    border: '2px solid #333',
    borderRadius: '4px',
  },
  
  /** 폴라로이드 스타일 */
  polaroid: {
    border: '10px solid white',
    borderBottom: '40px solid white',
    shadow: '0 4px 12px rgba(0,0,0,0.2)',
  },
};

/**
 * 레이지 로딩 프리셋
 */
export const LOADING_PRESETS = {
  /** 즉시 로딩 (above the fold) */
  eager: {
    loading: 'eager',
    decoding: 'sync',
    fetchpriority: 'high',
  },
  
  /** 레이지 로딩 (below the fold) */
  lazy: {
    loading: 'lazy',
    decoding: 'async',
    fetchpriority: 'auto',
  },
  
  /** 점진적 표시 (placeholder → blur → full) */
  progressive: {
    loading: 'lazy',
    decoding: 'async',
    placeholder: 'blur',
    blurDataURL: true,  // 자동 생성 플래그
  },
};

/**
 * 모든 프리셋 통합
 */
export const PRESETS = {
  size: SIZE_PRESETS,
  format: FORMAT_DEFAULTS,
  responsive: RESPONSIVE_PRESETS,
  video: VIDEO_PRESETS,
  audio: AUDIO_PRESETS,
  style: STYLE_PRESETS,
  loading: LOADING_PRESETS,
};

/**
 * 프리셋 이름으로 설정을 가져옵니다
 * @param {string} presetName - 프리셋 이름 (예: "large", "size:large", "responsive:hero")
 * @returns {Object|null} 프리셋 설정 또는 null
 */
export function getPreset(presetName) {
  if (!presetName || typeof presetName !== 'string') {
    return null;
  }
  
  // 카테고리:이름 형식 지원 (예: "responsive:hero")
  if (presetName.includes(':')) {
    const [category, name] = presetName.split(':');
    const categoryPresets = PRESETS[category];
    return categoryPresets ? categoryPresets[name] || null : null;
  }
  
  // 단순 이름으로 검색 (SIZE_PRESETS 우선)
  if (SIZE_PRESETS[presetName]) {
    return SIZE_PRESETS[presetName];
  }
  
  // 모든 카테고리에서 검색
  for (const category of Object.values(PRESETS)) {
    if (category[presetName]) {
      return category[presetName];
    }
  }
  
  return null;
}

/**
 * 프리셋을 베이스 설정과 병합합니다
 * @param {Object} baseConfig - 기본 설정
 * @param {string|Object} preset - 프리셋 이름 또는 객체
 * @returns {Object} 병합된 설정
 */
export function applyPreset(baseConfig, preset) {
  const presetConfig = typeof preset === 'string' 
    ? getPreset(preset) 
    : preset;
  
  if (!presetConfig) {
    return baseConfig;
  }
  
  return {
    ...presetConfig,
    ...baseConfig,  // 사용자 설정이 프리셋보다 우선
  };
}

/**
 * 사용 가능한 모든 프리셋 이름을 반환합니다
 * @returns {Object} 카테고리별 프리셋 이름 목록
 */
export function listPresets() {
  return {
    size: Object.keys(SIZE_PRESETS),
    format: Object.keys(FORMAT_DEFAULTS),
    responsive: Object.keys(RESPONSIVE_PRESETS),
    video: Object.keys(VIDEO_PRESETS),
    audio: Object.keys(AUDIO_PRESETS),
    style: Object.keys(STYLE_PRESETS),
    loading: Object.keys(LOADING_PRESETS),
  };
}

export default PRESETS;
