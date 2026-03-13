"""
MDM 토큰을 HTML로 렌더링합니다.
"""
from typing import Any, Dict, List, Optional

from .presets import SIZE_PRESETS


# 파일 확장자별 타입 매핑
IMAGE_EXTENSIONS = {'jpg', 'jpeg', 'png', 'gif', 'webp', 'svg'}
VIDEO_EXTENSIONS = {'mp4', 'webm', 'ogg'}
AUDIO_EXTENSIONS = {'mp3', 'wav', 'ogg'}

# HTML 이스케이프 매핑
_HTML_ESCAPE_TABLE = {
    '&': '&amp;',
    '<': '&lt;',
    '>': '&gt;',
    '"': '&quot;',
    "'": '&#39;',
}


class Renderer:
    """MDM 토큰을 HTML로 렌더링하는 클래스.

    Args:
        mdm_data: 로드된 MDM 사이드카 데이터 (선택적)
    """

    def __init__(self, mdm_data: Optional[Dict[str, Any]] = None) -> None:
        self.mdm_data = mdm_data

    def render(self, tokens: List[Dict[str, Any]]) -> str:
        """토큰 배열을 HTML 문자열로 렌더링합니다.

        Args:
            tokens: 토큰 배열

        Returns:
            결합된 HTML 문자열
        """
        return ''.join(self.render_token(token) for token in tokens)

    def render_token(self, token: Dict[str, Any]) -> str:
        """개별 토큰을 렌더링합니다.

        Args:
            token: 토큰 딕셔너리

        Returns:
            HTML 문자열
        """
        if token['type'] == 'text':
            return token['value']
        elif token['type'] == 'mdm-reference':
            return self.render_mdm_reference(token)
        return ''

    def render_mdm_reference(self, token: Dict[str, Any]) -> str:
        """MDM 참조 토큰을 HTML로 렌더링합니다.

        MDM 데이터에서 리소스를 찾아 적절한 HTML을 생성합니다.
        리소스를 찾을 수 없으면 직접 파일 참조로 처리합니다.

        Args:
            token: MDM 참조 토큰

        Returns:
            HTML 문자열
        """
        name = token['name']
        preset = token['preset']
        attributes = token['attributes']

        # MDM 데이터에서 리소스 찾기
        resources = (self.mdm_data or {}).get('resources') or {}
        resource = resources.get(name)

        if resource is None:
            # 리소스를 찾을 수 없으면 파일 경로로 간주
            return self.render_direct_file(name, attributes)

        # 프리셋 속성 적용
        preset_attrs = self._get_preset_attributes(resource, preset)
        merged_attrs = {**resource, **preset_attrs, **attributes}

        # 리소스 타입별 렌더링
        resource_type = resource.get('type')
        if resource_type == 'image':
            return self.render_image(merged_attrs)
        elif resource_type == 'video':
            return self.render_video(merged_attrs)
        elif resource_type == 'audio':
            return self.render_audio(merged_attrs)
        elif resource_type == 'embed':
            return self.render_embed(merged_attrs)
        else:
            return f'<!-- Unknown resource type: {resource_type} -->'

    def _get_preset_attributes(
        self,
        resource: Dict[str, Any],
        preset: Optional[str],
    ) -> Dict[str, Any]:
        """프리셋 속성을 가져옵니다.

        우선순위:
        1. 리소스별 프리셋
        2. 전역 MDM 데이터 프리셋
        3. 내장 SIZE_PRESETS

        Args:
            resource: 리소스 딕셔너리
            preset: 프리셋 이름

        Returns:
            프리셋 속성 딕셔너리
        """
        if not preset:
            return {}

        # 1. 리소스별 프리셋
        resource_presets = resource.get('presets') or {}
        if preset in resource_presets:
            return resource_presets[preset]

        # 2. 전역 프리셋 (사용자 정의)
        global_presets = (self.mdm_data or {}).get('presets') or {}
        if preset in global_presets:
            return global_presets[preset]

        # 3. 내장 기본 프리셋
        if preset in SIZE_PRESETS:
            return SIZE_PRESETS[preset]

        return {}

    def render_image(self, attrs: Dict[str, Any]) -> str:
        """이미지 HTML을 생성합니다.

        caption이 있으면 <figure>/<figcaption>으로 감쌉니다.

        Args:
            attrs: 이미지 속성 딕셔너리

        Returns:
            <img> 또는 <figure> HTML 문자열
        """
        img_attrs = []

        if attrs.get('src'):
            img_attrs.append(f'src="{self.escape_html(str(attrs["src"]))}"')

        if attrs.get('alt'):
            img_attrs.append(f'alt="{self.escape_html(str(attrs["alt"]))}"')

        if attrs.get('width') is not None:
            img_attrs.append(f'width="{attrs["width"]}"')

        if attrs.get('height') is not None:
            img_attrs.append(f'height="{attrs["height"]}"')

        if attrs.get('loading'):
            img_attrs.append(f'loading="{attrs["loading"]}"')

        styles = self.build_styles(attrs)
        if styles:
            img_attrs.append(f'style="{styles}"')

        if attrs.get('align'):
            img_attrs.append(f'class="align-{attrs["align"]}"')

        img = f'<img {" ".join(img_attrs)}>'

        # 캡션이 있으면 figure로 감싸기
        if attrs.get('caption'):
            caption_escaped = self.escape_html(str(attrs['caption']))
            return f'<figure>{img}<figcaption>{caption_escaped}</figcaption></figure>'

        return img

    def render_video(self, attrs: Dict[str, Any]) -> str:
        """비디오 HTML을 생성합니다.

        Args:
            attrs: 비디오 속성 딕셔너리

        Returns:
            <video> HTML 문자열
        """
        video_attrs = []

        if attrs.get('src'):
            video_attrs.append(f'src="{self.escape_html(str(attrs["src"]))}"')

        if attrs.get('width') is not None:
            video_attrs.append(f'width="{attrs["width"]}"')

        if attrs.get('height') is not None:
            video_attrs.append(f'height="{attrs["height"]}"')

        if attrs.get('poster'):
            video_attrs.append(f'poster="{self.escape_html(str(attrs["poster"]))}"')

        # 불린 속성들
        for bool_attr in ('controls', 'autoplay', 'muted', 'loop'):
            if attrs.get(bool_attr):
                video_attrs.append(bool_attr)

        return f'<video {" ".join(video_attrs)}></video>'

    def render_audio(self, attrs: Dict[str, Any]) -> str:
        """오디오 HTML을 생성합니다.

        Args:
            attrs: 오디오 속성 딕셔너리

        Returns:
            <audio> HTML 문자열
        """
        audio_attrs = []

        if attrs.get('src'):
            audio_attrs.append(f'src="{self.escape_html(str(attrs["src"]))}"')

        # 불린 속성들
        for bool_attr in ('controls', 'autoplay', 'loop'):
            if attrs.get(bool_attr):
                audio_attrs.append(bool_attr)

        return f'<audio {" ".join(audio_attrs)}></audio>'

    def render_embed(self, attrs: Dict[str, Any]) -> str:
        """임베드 HTML을 생성합니다 (YouTube, Vimeo).

        Args:
            attrs: 임베드 속성 딕셔너리

        Returns:
            <iframe> HTML 문자열 또는 미지원 주석
        """
        provider = attrs.get('provider')
        embed_id = attrs.get('id')

        if provider == 'youtube':
            width = attrs.get('width', 560)
            height = attrs.get('height', 315)
            return (
                f'<iframe width="{width}" height="{height}" '
                f'src="https://www.youtube.com/embed/{embed_id}" '
                f'frameborder="0" allowfullscreen></iframe>'
            )

        if provider == 'vimeo':
            width = attrs.get('width', 640)
            height = attrs.get('height', 360)
            return (
                f'<iframe width="{width}" height="{height}" '
                f'src="https://player.vimeo.com/video/{embed_id}" '
                f'frameborder="0" allowfullscreen></iframe>'
            )

        return f'<!-- Unsupported embed provider: {provider} -->'

    def render_direct_file(self, filename: str, attrs: Dict[str, Any]) -> str:
        """직접 파일 참조를 렌더링합니다.

        파일 확장자로 타입을 추론합니다.

        Args:
            filename: 파일명 (확장자 포함)
            attrs: 속성 딕셔너리

        Returns:
            HTML 문자열 또는 미지원 주석
        """
        ext = filename.rsplit('.', 1)[-1].lower() if '.' in filename else ''

        if ext in IMAGE_EXTENSIONS:
            return self.render_image({'src': filename, **attrs})
        elif ext in VIDEO_EXTENSIONS:
            return self.render_video({'src': filename, **attrs})
        elif ext in AUDIO_EXTENSIONS:
            return self.render_audio({'src': filename, **attrs})

        return f'<!-- Unknown file type: {filename} -->'

    def build_styles(self, attrs: Dict[str, Any]) -> str:
        """인라인 스타일 문자열을 생성합니다.

        Args:
            attrs: 속성 딕셔너리

        Returns:
            CSS 스타일 문자열 (세미콜론 구분)
        """
        styles = []

        if attrs.get('max-width') is not None:
            styles.append(f'max-width: {attrs["max-width"]}px')

        if attrs.get('object-fit'):
            styles.append(f'object-fit: {attrs["object-fit"]}')

        if attrs.get('margin'):
            styles.append(f'margin: {attrs["margin"]}')

        if attrs.get('opacity') is not None:
            styles.append(f'opacity: {attrs["opacity"]}')

        if attrs.get('float'):
            styles.append(f'float: {attrs["float"]}')

        return '; '.join(styles)

    @staticmethod
    def escape_html(s: str) -> str:
        """HTML 특수문자를 이스케이프합니다.

        XSS 방지를 위해 &, <, >, ", ' 를 엔티티로 변환합니다.

        Args:
            s: 이스케이프할 문자열

        Returns:
            이스케이프된 문자열
        """
        result = []
        for char in s:
            result.append(_HTML_ESCAPE_TABLE.get(char, char))
        return ''.join(result)
