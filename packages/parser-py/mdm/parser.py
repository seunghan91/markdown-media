"""
MDM 파서 메인 모듈.

마크다운 텍스트의 ![[...]] 참조를 HTML로 변환하는 파이프라인을 제공합니다.
"""
from typing import Any, Dict, List, Optional

from .loader import MDMLoader
from .renderer import Renderer
from .tokenizer import Tokenizer


class MDMParser:
    """MDM 마크다운 파서.

    tokenize → render 파이프라인으로 MDM 마크다운을 HTML로 변환합니다.

    Args:
        options: 파서 옵션 딕셔너리 (현재 미사용, 확장용)
    """

    def __init__(self, options: Optional[Dict[str, Any]] = None) -> None:
        self.options: Dict[str, Any] = {'mdm_path': None, **(options or {})}
        self.loader = MDMLoader()
        self.tokenizer = Tokenizer()
        self._renderer: Optional[Renderer] = None
        self._mdm_data: Optional[Dict[str, Any]] = None

    def load_mdm(self, mdm_path: str) -> Dict[str, Any]:
        """MDM 사이드카 파일을 로드합니다.

        Args:
            mdm_path: .mdm/.yaml/.json 파일 경로

        Returns:
            파싱된 MDM 데이터 딕셔너리
        """
        self._mdm_data = self.loader.load(mdm_path)
        self._renderer = Renderer(self._mdm_data)
        return self._mdm_data

    def parse(self, markdown: str, mdm_path: Optional[str] = None) -> str:
        """마크다운 텍스트를 HTML로 변환합니다.

        MDM 참조(![[...]])를 해당 HTML 태그로 교체합니다.
        나머지 텍스트는 그대로 유지됩니다.

        Args:
            markdown: 파싱할 마크다운 텍스트
            mdm_path: MDM 사이드카 파일 경로 (선택적)

        Returns:
            변환된 HTML 문자열
        """
        # MDM 파일 로드 (아직 로드되지 않았으면)
        if mdm_path and self._mdm_data is None:
            self.load_mdm(mdm_path)

        # 토큰화
        tokens = self.tokenizer.tokenize(markdown)

        # 렌더링
        if self._renderer is None:
            self._renderer = Renderer(self._mdm_data)

        return self._renderer.render(tokens)

    def tokenize(self, markdown: str) -> List[Dict[str, Any]]:
        """마크다운 텍스트를 토큰 배열로 변환합니다 (디버깅용).

        Args:
            markdown: 파싱할 마크다운 텍스트

        Returns:
            토큰 배열
        """
        return self.tokenizer.tokenize(markdown)

    def set_mdm_data(self, mdm_data: Dict[str, Any]) -> None:
        """MDM 데이터를 직접 설정합니다.

        파일 로드 없이 MDM 데이터를 주입할 때 사용합니다.

        Args:
            mdm_data: MDM 데이터 딕셔너리
        """
        self._mdm_data = mdm_data
        self._renderer = Renderer(mdm_data)

    def get_mdm_data(self) -> Optional[Dict[str, Any]]:
        """현재 로드된 MDM 데이터를 반환합니다.

        Returns:
            MDM 데이터 딕셔너리 또는 None
        """
        return self._mdm_data

    def clear_cache(self) -> None:
        """캐시 및 상태를 초기화합니다.

        MDM 데이터, 렌더러, 로더 캐시를 모두 초기화합니다.
        """
        self.loader.clear_cache()
        self._mdm_data = None
        self._renderer = None


def parse(markdown: str, mdm_path: Optional[str] = None) -> str:
    """MDM 마크다운을 HTML로 변환하는 편의 함수.

    Args:
        markdown: 파싱할 마크다운 텍스트
        mdm_path: MDM 사이드카 파일 경로 (선택적)

    Returns:
        변환된 HTML 문자열
    """
    parser = MDMParser()
    return parser.parse(markdown, mdm_path=mdm_path)
