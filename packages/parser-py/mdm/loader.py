"""
MDM 사이드카 파일을 로드하고 파싱합니다.
"""
import json
import os
from typing import Any, Dict, Optional

try:
    import yaml
    _YAML_AVAILABLE = True
except ImportError:
    _YAML_AVAILABLE = False


class MDMLoader:
    """MDM 파일 로더.

    YAML(.mdm, .yaml, .yml) 및 JSON(.json) 형식의 MDM 사이드카 파일을
    로드하고 캐싱합니다.
    """

    def __init__(self) -> None:
        self._cache: Dict[str, Dict[str, Any]] = {}

    def load(self, mdm_path: str) -> Dict[str, Any]:
        """MDM 파일을 로드합니다.

        캐시에 있으면 캐시된 값을 반환합니다.

        Args:
            mdm_path: MDM 파일 경로

        Returns:
            파싱된 MDM 데이터 딕셔너리

        Raises:
            OSError: 파일을 읽을 수 없을 때
            ValueError: 파일 형식이 잘못되었거나 유효성 검증 실패 시
        """
        # 캐시 확인
        if mdm_path in self._cache:
            return self._cache[mdm_path]

        try:
            with open(mdm_path, 'r', encoding='utf-8') as f:
                content = f.read()

            _, ext = os.path.splitext(mdm_path)
            data = self._parse(content, ext)

            # 유효성 검증
            self.validate(data)

            # 경로 정규화
            base_path = os.path.dirname(os.path.abspath(mdm_path))
            normalized = self.normalize_paths(data, base_path)

            # 캐시 저장
            self._cache[mdm_path] = normalized

            return normalized

        except (OSError, json.JSONDecodeError, ValueError) as exc:
            raise ValueError(f"Failed to load MDM file: {exc}") from exc

    def _parse(self, content: str, ext: str) -> Dict[str, Any]:
        """MDM 콘텐츠를 파싱합니다.

        Args:
            content: 파일 내용
            ext: 파일 확장자 (점 포함)

        Returns:
            파싱된 데이터 딕셔너리

        Raises:
            ValueError: 지원하지 않는 확장자이거나 파싱 실패 시
        """
        if ext == '.json':
            return json.loads(content)
        elif ext in ('.yaml', '.yml', '.mdm'):
            if not _YAML_AVAILABLE:
                raise ValueError("PyYAML is required for YAML/MDM files. Run: pip install pyyaml")
            result = yaml.safe_load(content)
            if not isinstance(result, dict):
                raise ValueError("MDM YAML file must contain a mapping at the top level")
            return result
        else:
            raise ValueError(f"Unsupported MDM file extension: {ext}")

    def validate(self, data: Dict[str, Any]) -> None:
        """MDM 데이터 유효성을 검증합니다.

        Args:
            data: MDM 데이터 딕셔너리

        Raises:
            ValueError: 필수 필드가 없거나 리소스가 잘못된 경우
        """
        if not data.get('version'):
            raise ValueError("MDM file must have a version field")

        if 'resources' not in data:
            data['resources'] = {}

        # 각 리소스 유효성 검증
        for name, resource in (data.get('resources') or {}).items():
            if not resource.get('type'):
                raise ValueError(f'Resource "{name}" must have a type')

            if not resource.get('src') and resource.get('type') != 'embed':
                raise ValueError(f'Resource "{name}" must have a src')

    def normalize_paths(self, data: Dict[str, Any], base_path: str) -> Dict[str, Any]:
        """리소스 경로를 정규화합니다.

        상대 경로를 base_path + media_root 기준으로 절대 경로로 변환합니다.

        Args:
            data: MDM 데이터 딕셔너리
            base_path: MDM 파일이 위치한 디렉터리 경로

        Returns:
            정규화된 데이터 딕셔너리 (원본 딕셔너리 수정)
        """
        normalized = dict(data)
        media_root = data.get('media_root', './')

        if normalized.get('resources'):
            for resource in normalized['resources'].values():
                if resource.get('src') and not self._is_absolute_url(resource['src']):
                    resource['src'] = os.path.join(base_path, media_root, resource['src'])

                if resource.get('poster') and not self._is_absolute_url(resource['poster']):
                    resource['poster'] = os.path.join(base_path, media_root, resource['poster'])

                if resource.get('variants'):
                    for variant_key, src in resource['variants'].items():
                        if not self._is_absolute_url(src):
                            resource['variants'][variant_key] = os.path.join(
                                base_path, media_root, src
                            )

        return normalized

    @staticmethod
    def _is_absolute_url(url: str) -> bool:
        """URL이 절대 경로인지 확인합니다.

        Args:
            url: 확인할 URL 문자열

        Returns:
            http(s):// 로 시작하거나 절대 파일 경로이면 True
        """
        return url.startswith('http://') or url.startswith('https://') or os.path.isabs(url)

    def clear_cache(self) -> None:
        """캐시를 초기화합니다."""
        self._cache.clear()
