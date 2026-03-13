"""
MDM 내장 프리셋 정의.

프리셋은 자주 사용되는 이미지/미디어 변환 설정을 미리 정의한 것입니다.
"""
from typing import Any, Dict, Optional

# 이미지 크기 프리셋
SIZE_PRESETS: Dict[str, Dict[str, Any]] = {
    # 썸네일용 작은 이미지 (150x150)
    'thumb': {
        'width': 150,
        'height': 150,
        'fit': 'cover',
        'quality': 80,
        'format': 'webp',
    },
    # 작은 이미지 (320px 너비)
    'small': {
        'width': 320,
        'height': None,
        'fit': 'contain',
        'quality': 85,
    },
    # 중간 크기 이미지 (640px 너비)
    'medium': {
        'width': 640,
        'height': None,
        'fit': 'contain',
        'quality': 85,
    },
    # 큰 이미지 (1024px 너비)
    'large': {
        'width': 1024,
        'height': None,
        'fit': 'contain',
        'quality': 90,
    },
    # 전체 화면 이미지 (1920px 너비)
    'full': {
        'width': 1920,
        'height': None,
        'fit': 'contain',
        'quality': 90,
    },
    # 정사각형 (1:1)
    'square': {
        'width': 500,
        'height': 500,
        'fit': 'cover',
        'quality': 85,
    },
    # 와이드스크린 (16:9)
    'widescreen': {
        'width': 1280,
        'height': 720,
        'fit': 'cover',
        'quality': 90,
    },
    # 시네마 (21:9)
    'cinema': {
        'width': 1680,
        'height': 720,
        'fit': 'cover',
        'quality': 90,
    },
    # 세로 (9:16) - 모바일/스토리용
    'portrait': {
        'width': 720,
        'height': 1280,
        'fit': 'cover',
        'quality': 85,
    },
    # 아바타/프로필 이미지
    'avatar': {
        'width': 200,
        'height': 200,
        'fit': 'cover',
        'quality': 80,
        'format': 'webp',
        'borderRadius': '50%',
    },
}

# 포맷별 기본 옵션
FORMAT_DEFAULTS: Dict[str, Dict[str, Any]] = {
    'jpeg': {'quality': 85, 'progressive': True},
    'png': {'compressionLevel': 9, 'interlace': True},
    'webp': {'quality': 85, 'lossless': False},
    'avif': {'quality': 80, 'speed': 5},
    'gif': {'colors': 256, 'dither': True},
    'svg': {'cleanupIds': True, 'removeComments': True, 'minifyStyles': True},
}

# 반응형 이미지 프리셋
RESPONSIVE_PRESETS: Dict[str, Dict[str, Any]] = {
    'article': {
        'widths': [320, 640, 960, 1280],
        'sizes': '(max-width: 640px) 100vw, (max-width: 1024px) 90vw, 800px',
        'format': 'webp',
        'fallbackFormat': 'jpeg',
    },
    'hero': {
        'widths': [640, 960, 1280, 1920, 2560],
        'sizes': '100vw',
        'format': 'webp',
        'fallbackFormat': 'jpeg',
        'quality': 90,
    },
    'card': {
        'widths': [200, 400, 600],
        'sizes': '(max-width: 640px) 50vw, 300px',
        'format': 'webp',
        'fallbackFormat': 'jpeg',
    },
    'gallery': {
        'widths': [320, 640, 960],
        'sizes': '(max-width: 480px) 100vw, (max-width: 768px) 50vw, 33vw',
        'format': 'webp',
        'fallbackFormat': 'jpeg',
    },
}

# 비디오 프리셋
VIDEO_PRESETS: Dict[str, Dict[str, Any]] = {
    'background': {
        'autoplay': True,
        'loop': True,
        'muted': True,
        'playsinline': True,
        'preload': 'auto',
        'controls': False,
    },
    'presentation': {
        'autoplay': False,
        'loop': False,
        'muted': False,
        'controls': True,
        'preload': 'metadata',
    },
    'clip': {
        'autoplay': True,
        'loop': True,
        'muted': True,
        'playsinline': True,
        'preload': 'auto',
        'controls': False,
        'maxDuration': 30,
    },
}

# 오디오 프리셋
AUDIO_PRESETS: Dict[str, Dict[str, Any]] = {
    'background': {
        'autoplay': True,
        'loop': True,
        'volume': 0.3,
        'controls': False,
        'preload': 'auto',
    },
    'podcast': {
        'autoplay': False,
        'loop': False,
        'controls': True,
        'preload': 'metadata',
    },
}

# 스타일 프리셋
STYLE_PRESETS: Dict[str, Dict[str, Any]] = {
    'default': {'border': 'none', 'borderRadius': '0', 'shadow': 'none'},
    'card': {
        'border': '1px solid #e0e0e0',
        'borderRadius': '8px',
        'shadow': '0 2px 8px rgba(0,0,0,0.1)',
        'padding': '16px',
    },
    'rounded': {'borderRadius': '12px', 'overflow': 'hidden'},
    'elevated': {'shadow': '0 4px 16px rgba(0,0,0,0.15)', 'borderRadius': '8px'},
    'bordered': {'border': '2px solid #333', 'borderRadius': '4px'},
    'polaroid': {
        'border': '10px solid white',
        'borderBottom': '40px solid white',
        'shadow': '0 4px 12px rgba(0,0,0,0.2)',
    },
}

# 레이지 로딩 프리셋
LOADING_PRESETS: Dict[str, Dict[str, Any]] = {
    'eager': {'loading': 'eager', 'decoding': 'sync', 'fetchpriority': 'high'},
    'lazy': {'loading': 'lazy', 'decoding': 'async', 'fetchpriority': 'auto'},
    'progressive': {
        'loading': 'lazy',
        'decoding': 'async',
        'placeholder': 'blur',
        'blurDataURL': True,
    },
}

# 모든 프리셋 통합
PRESETS: Dict[str, Dict[str, Any]] = {
    'size': SIZE_PRESETS,
    'format': FORMAT_DEFAULTS,
    'responsive': RESPONSIVE_PRESETS,
    'video': VIDEO_PRESETS,
    'audio': AUDIO_PRESETS,
    'style': STYLE_PRESETS,
    'loading': LOADING_PRESETS,
}


def get_preset(preset_name: str) -> Optional[Dict[str, Any]]:
    """프리셋 이름으로 설정을 가져옵니다.

    Args:
        preset_name: 프리셋 이름 (예: "large", "size:large", "responsive:hero")

    Returns:
        프리셋 설정 딕셔너리 또는 None
    """
    if not preset_name or not isinstance(preset_name, str):
        return None

    # 카테고리:이름 형식 지원 (예: "responsive:hero")
    if ':' in preset_name:
        category, name = preset_name.split(':', 1)
        category_presets = PRESETS.get(category)
        return category_presets.get(name) if category_presets else None

    # 단순 이름으로 검색 (SIZE_PRESETS 우선)
    if preset_name in SIZE_PRESETS:
        return SIZE_PRESETS[preset_name]

    # 모든 카테고리에서 검색
    for category in PRESETS.values():
        if preset_name in category:
            return category[preset_name]

    return None


def apply_preset(
    base_config: Dict[str, Any],
    preset: Any,
) -> Dict[str, Any]:
    """프리셋을 베이스 설정과 병합합니다.

    Args:
        base_config: 기본 설정
        preset: 프리셋 이름 또는 딕셔너리

    Returns:
        병합된 설정 (사용자 설정이 프리셋보다 우선)
    """
    if isinstance(preset, str):
        preset_config = get_preset(preset)
    else:
        preset_config = preset

    if not preset_config:
        return base_config

    return {**preset_config, **base_config}


def list_presets() -> Dict[str, list]:
    """사용 가능한 모든 프리셋 이름을 반환합니다.

    Returns:
        카테고리별 프리셋 이름 목록
    """
    return {category: list(presets.keys()) for category, presets in PRESETS.items()}
