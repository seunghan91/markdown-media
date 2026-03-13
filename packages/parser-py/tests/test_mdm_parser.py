"""
MDM 파서 종합 테스트.

JS 구현의 tokenizer.test.js, renderer.test.js, parser.test.js,
integration.test.js 를 Python/pytest로 포팅한 테스트입니다.
"""
import sys
import os

# 패키지 경로를 sys.path에 추가
sys.path.insert(0, os.path.join(os.path.dirname(__file__), '..'))

import pytest
from mdm import MDMParser, parse, Tokenizer, Renderer, MDMLoader


# ===========================================================================
# Tokenizer 테스트
# ===========================================================================

class TestTokenizer:
    """Tokenizer 클래스 단위 테스트."""

    def setup_method(self):
        self.tokenizer = Tokenizer()

    def test_simple_reference(self):
        """단순 MDM 참조를 토큰화합니다."""
        tokens = self.tokenizer.tokenize('![[image]]')
        assert len(tokens) == 1
        assert tokens[0]['type'] == 'mdm-reference'
        assert tokens[0]['name'] == 'image'
        assert tokens[0]['preset'] is None
        assert tokens[0]['attributes'] == {}

    def test_reference_with_preset(self):
        """프리셋이 있는 MDM 참조를 토큰화합니다."""
        tokens = self.tokenizer.tokenize('![[logo:small]]')
        assert tokens[0]['name'] == 'logo'
        assert tokens[0]['preset'] == 'small'

    def test_reference_with_attributes(self):
        """속성이 있는 MDM 참조를 토큰화합니다."""
        tokens = self.tokenizer.tokenize('![[image | width=500 align=center]]')
        assert tokens[0]['name'] == 'image'
        assert tokens[0]['attributes']['width'] == 500
        assert tokens[0]['attributes']['align'] == 'center'

    def test_reference_with_preset_and_attributes(self):
        """프리셋과 속성이 모두 있는 MDM 참조를 토큰화합니다."""
        tokens = self.tokenizer.tokenize('![[logo:header | opacity=0.8]]')
        assert tokens[0]['name'] == 'logo'
        assert tokens[0]['preset'] == 'header'
        assert tokens[0]['attributes']['opacity'] == 0.8

    def test_quoted_attribute_values(self):
        """따옴표로 감싼 속성 값을 파싱합니다."""
        tokens = self.tokenizer.tokenize("![[image | caption=\"My Image\" class='highlight']]")
        assert tokens[0]['attributes']['caption'] == 'My Image'
        assert tokens[0]['attributes']['class'] == 'highlight'

    def test_boolean_attributes(self):
        """값 없는 속성은 True로 파싱합니다."""
        tokens = self.tokenizer.tokenize('![[video | controls autoplay muted]]')
        assert tokens[0]['attributes']['controls'] is True
        assert tokens[0]['attributes']['autoplay'] is True
        assert tokens[0]['attributes']['muted'] is True

    def test_mixed_content(self):
        """텍스트와 MDM 참조가 혼합된 내용을 토큰화합니다."""
        tokens = self.tokenizer.tokenize('Before ![[image]] middle ![[video]] after')
        assert len(tokens) == 5
        assert tokens[0]['type'] == 'text'
        assert tokens[0]['value'] == 'Before '
        assert tokens[1]['type'] == 'mdm-reference'
        assert tokens[2]['type'] == 'text'
        assert tokens[2]['value'] == ' middle '
        assert tokens[3]['type'] == 'mdm-reference'
        assert tokens[4]['type'] == 'text'
        assert tokens[4]['value'] == ' after'

    def test_integer_attribute_conversion(self):
        """정수 문자열은 int로 변환합니다."""
        tokens = self.tokenizer.tokenize('![[img | width=800]]')
        val = tokens[0]['attributes']['width']
        assert val == 800
        assert isinstance(val, int)

    def test_float_attribute_conversion(self):
        """소수 문자열은 float으로 변환합니다."""
        tokens = self.tokenizer.tokenize('![[img | opacity=0.5]]')
        val = tokens[0]['attributes']['opacity']
        assert val == 0.5
        assert isinstance(val, float)

    def test_true_false_string_conversion(self):
        """'true'/'false' 문자열은 bool로 변환합니다."""
        tokens = self.tokenizer.tokenize('![[img | lazy=true hidden=false]]')
        assert tokens[0]['attributes']['lazy'] is True
        assert tokens[0]['attributes']['hidden'] is False

    def test_empty_text(self):
        """빈 문자열을 토큰화하면 빈 배열을 반환합니다."""
        tokens = self.tokenizer.tokenize('')
        assert tokens == []

    def test_plain_text_only(self):
        """MDM 참조가 없는 텍스트는 단일 text 토큰으로 반환합니다."""
        tokens = self.tokenizer.tokenize('Just plain text')
        assert len(tokens) == 1
        assert tokens[0]['type'] == 'text'
        assert tokens[0]['value'] == 'Just plain text'

    def test_raw_field_in_token(self):
        """mdm-reference 토큰에 raw 필드가 포함됩니다."""
        tokens = self.tokenizer.tokenize('![[img.jpg]]')
        assert tokens[0]['raw'] == '![[img.jpg]]'


# ===========================================================================
# Renderer 테스트
# ===========================================================================

class TestRendererDirectFile:
    """직접 파일 참조 렌더링 테스트."""

    def setup_method(self):
        self.renderer = Renderer()

    @pytest.mark.parametrize('ext', ['jpg', 'jpeg', 'png', 'gif', 'webp', 'svg'])
    def test_image_extensions(self, ext):
        """이미지 확장자 파일은 <img>로 렌더링합니다."""
        tokens = [{'type': 'mdm-reference', 'name': f'photo.{ext}', 'preset': None, 'attributes': {}}]
        html = self.renderer.render(tokens)
        assert '<img' in html, f"expected <img> for .{ext}, got: {html}"
        assert f'src="photo.{ext}"' in html

    @pytest.mark.parametrize('ext', ['mp4', 'webm', 'ogg'])
    def test_video_extensions(self, ext):
        """비디오 확장자 파일은 <video>로 렌더링합니다."""
        tokens = [{'type': 'mdm-reference', 'name': f'clip.{ext}', 'preset': None, 'attributes': {}}]
        html = self.renderer.render(tokens)
        assert '<video' in html, f"expected <video> for .{ext}"

    @pytest.mark.parametrize('ext', ['mp3', 'wav'])
    def test_audio_extensions(self, ext):
        """오디오 확장자 파일은 <audio>로 렌더링합니다."""
        tokens = [{'type': 'mdm-reference', 'name': f'sound.{ext}', 'preset': None, 'attributes': {}}]
        html = self.renderer.render(tokens)
        assert '<audio' in html, f"expected <audio> for .{ext}"

    def test_unknown_extension_returns_comment(self):
        """알 수 없는 확장자는 HTML 주석을 반환합니다."""
        tokens = [{'type': 'mdm-reference', 'name': 'doc.pdf', 'preset': None, 'attributes': {}}]
        html = self.renderer.render(tokens)
        assert '<!--' in html


class TestRendererImageAttributes:
    """이미지 속성 렌더링 테스트."""

    def setup_method(self):
        self.renderer = Renderer()

    def test_width_attribute(self):
        """width 속성이 렌더링됩니다."""
        tokens = [{'type': 'mdm-reference', 'name': 'img.jpg', 'preset': None, 'attributes': {'width': 500}}]
        assert 'width="500"' in self.renderer.render(tokens)

    def test_height_attribute(self):
        """height 속성이 렌더링됩니다."""
        tokens = [{'type': 'mdm-reference', 'name': 'img.jpg', 'preset': None, 'attributes': {'height': 300}}]
        assert 'height="300"' in self.renderer.render(tokens)

    def test_alt_attribute(self):
        """alt 속성이 렌더링됩니다."""
        tokens = [{'type': 'mdm-reference', 'name': 'img.jpg', 'preset': None, 'attributes': {'alt': 'test image'}}]
        assert 'alt="test image"' in self.renderer.render(tokens)

    def test_align_adds_class(self):
        """align 속성은 class="align-{value}"로 렌더링됩니다."""
        tokens = [{'type': 'mdm-reference', 'name': 'img.jpg', 'preset': None, 'attributes': {'align': 'center'}}]
        assert 'class="align-center"' in self.renderer.render(tokens)

    def test_caption_wraps_in_figure(self):
        """caption 속성이 있으면 <figure>/<figcaption>으로 감쌉니다."""
        tokens = [{'type': 'mdm-reference', 'name': 'img.jpg', 'preset': None, 'attributes': {'caption': 'My Caption'}}]
        html = self.renderer.render(tokens)
        assert '<figure>' in html
        assert '<figcaption>My Caption</figcaption>' in html

    def test_xss_escape_in_alt(self):
        """alt 속성의 HTML 특수문자를 이스케이프합니다."""
        tokens = [{'type': 'mdm-reference', 'name': 'img.jpg', 'preset': None,
                   'attributes': {'alt': '<script>alert(1)</script>'}}]
        html = self.renderer.render(tokens)
        assert '<script>' not in html
        assert '&lt;script&gt;' in html

    def test_xss_escape_in_caption(self):
        """caption 속성의 HTML 특수문자를 이스케이프합니다."""
        tokens = [{'type': 'mdm-reference', 'name': 'img.jpg', 'preset': None,
                   'attributes': {'caption': '<script>bad</script>'}}]
        html = self.renderer.render(tokens)
        assert '<script>' not in html
        assert '&lt;script&gt;' in html


class TestRendererVideoAttributes:
    """비디오 속성 렌더링 테스트."""

    def setup_method(self):
        self.renderer = Renderer()

    def test_controls_attribute(self):
        """controls 속성이 렌더링됩니다."""
        tokens = [{'type': 'mdm-reference', 'name': 'vid.mp4', 'preset': None, 'attributes': {'controls': True}}]
        assert 'controls' in self.renderer.render(tokens)

    def test_autoplay_muted_loop(self):
        """autoplay, muted, loop 속성이 렌더링됩니다."""
        tokens = [{'type': 'mdm-reference', 'name': 'vid.mp4', 'preset': None,
                   'attributes': {'autoplay': True, 'muted': True, 'loop': True}}]
        html = self.renderer.render(tokens)
        assert 'autoplay' in html
        assert 'muted' in html
        assert 'loop' in html

    def test_width_on_video(self):
        """비디오에 width 속성이 렌더링됩니다."""
        tokens = [{'type': 'mdm-reference', 'name': 'vid.mp4', 'preset': None, 'attributes': {'width': 720}}]
        assert 'width="720"' in self.renderer.render(tokens)


class TestRendererAudioAttributes:
    """오디오 속성 렌더링 테스트."""

    def setup_method(self):
        self.renderer = Renderer()

    def test_controls_on_audio(self):
        """오디오에 controls 속성이 렌더링됩니다."""
        tokens = [{'type': 'mdm-reference', 'name': 'track.mp3', 'preset': None, 'attributes': {'controls': True}}]
        html = self.renderer.render(tokens)
        assert '<audio' in html
        assert 'controls' in html


class TestRendererMDMData:
    """MDM 데이터 기반 렌더링 테스트."""

    def setup_method(self):
        self.mdm_data = {
            'version': '1.0',
            'media_root': './',
            'resources': {
                'logo': {'type': 'image', 'src': '/assets/logo.png', 'alt': 'Logo'},
                'demo': {'type': 'video', 'src': '/assets/demo.mp4'},
                'podcast': {'type': 'audio', 'src': '/assets/ep1.mp3'},
                'yt': {'type': 'embed', 'provider': 'youtube', 'id': 'abc123'},
                'vimeo': {'type': 'embed', 'provider': 'vimeo', 'id': '456'},
            },
        }
        self.renderer = Renderer(self.mdm_data)

    def test_image_resource_from_mdm_data(self):
        """MDM 데이터의 이미지 리소스를 렌더링합니다."""
        tokens = [{'type': 'mdm-reference', 'name': 'logo', 'preset': None, 'attributes': {}}]
        html = self.renderer.render(tokens)
        assert 'src="/assets/logo.png"' in html
        assert 'alt="Logo"' in html

    def test_video_resource(self):
        """MDM 데이터의 비디오 리소스를 렌더링합니다."""
        tokens = [{'type': 'mdm-reference', 'name': 'demo', 'preset': None, 'attributes': {}}]
        assert '<video' in self.renderer.render(tokens)

    def test_audio_resource(self):
        """MDM 데이터의 오디오 리소스를 렌더링합니다."""
        tokens = [{'type': 'mdm-reference', 'name': 'podcast', 'preset': None, 'attributes': {}}]
        assert '<audio' in self.renderer.render(tokens)

    def test_youtube_embed(self):
        """YouTube 임베드를 렌더링합니다."""
        tokens = [{'type': 'mdm-reference', 'name': 'yt', 'preset': None, 'attributes': {}}]
        html = self.renderer.render(tokens)
        assert 'youtube.com/embed/abc123' in html

    def test_vimeo_embed(self):
        """Vimeo 임베드를 렌더링합니다."""
        tokens = [{'type': 'mdm-reference', 'name': 'vimeo', 'preset': None, 'attributes': {}}]
        html = self.renderer.render(tokens)
        assert 'vimeo.com/video/456' in html

    def test_unknown_embed_provider_returns_comment(self):
        """지원하지 않는 임베드 제공자는 HTML 주석을 반환합니다."""
        mdm2 = {'resources': {'x': {'type': 'embed', 'provider': 'unknown'}}}
        renderer2 = Renderer(mdm2)
        tokens = [{'type': 'mdm-reference', 'name': 'x', 'preset': None, 'attributes': {}}]
        assert '<!--' in renderer2.render(tokens)

    def test_unknown_resource_type_returns_comment(self):
        """알 수 없는 리소스 타입은 HTML 주석을 반환합니다."""
        mdm_data = {'resources': {'x': {'type': 'unknown', 'src': 'x.bin'}}}
        renderer = Renderer(mdm_data)
        tokens = [{'type': 'mdm-reference', 'name': 'x', 'preset': None, 'attributes': {}}]
        assert '<!--' in renderer.render(tokens)


class TestRendererPresets:
    """프리셋 적용 테스트."""

    def setup_method(self):
        self.mdm_data = {
            'version': '1.0',
            'resources': {
                'banner': {
                    'type': 'image',
                    'src': '/img/banner.jpg',
                    'presets': {'mobile': {'width': 400}},
                },
            },
            'presets': {
                'hero': {'width': 1200},
            },
        }
        self.renderer = Renderer(self.mdm_data)

    def test_resource_level_preset(self):
        """리소스 레벨 프리셋이 적용됩니다."""
        tokens = [{'type': 'mdm-reference', 'name': 'banner', 'preset': 'mobile', 'attributes': {}}]
        assert 'width="400"' in self.renderer.render(tokens)

    def test_global_mdm_preset(self):
        """전역 MDM 프리셋이 적용됩니다."""
        tokens = [{'type': 'mdm-reference', 'name': 'banner', 'preset': 'hero', 'attributes': {}}]
        assert 'width="1200"' in self.renderer.render(tokens)

    def test_inline_attributes_override_preset(self):
        """인라인 속성이 프리셋보다 우선합니다."""
        tokens = [{'type': 'mdm-reference', 'name': 'banner', 'preset': 'mobile', 'attributes': {'width': 999}}]
        assert 'width="999"' in self.renderer.render(tokens)


class TestRendererTextPassthrough:
    """텍스트 토큰 통과 테스트."""

    def test_text_tokens_pass_through(self):
        """text 토큰은 변경 없이 그대로 출력됩니다."""
        renderer = Renderer()
        tokens = [
            {'type': 'text', 'value': 'Hello '},
            {'type': 'mdm-reference', 'name': 'img.png', 'preset': None, 'attributes': {}},
            {'type': 'text', 'value': ' World'},
        ]
        html = renderer.render(tokens)
        assert html.startswith('Hello ')
        assert html.endswith(' World')


class TestEscapeHtml:
    """HTML 이스케이프 테스트."""

    def test_escape_ampersand(self):
        assert Renderer.escape_html('a & b') == 'a &amp; b'

    def test_escape_less_than(self):
        assert Renderer.escape_html('<tag>') == '&lt;tag&gt;'

    def test_escape_quote(self):
        assert Renderer.escape_html('"value"') == '&quot;value&quot;'

    def test_escape_single_quote(self):
        assert Renderer.escape_html("it's") == "it&#39;s"

    def test_no_escape_plain_text(self):
        assert Renderer.escape_html('hello world') == 'hello world'


# ===========================================================================
# MDMParser 테스트
# ===========================================================================

class TestMDMParser:
    """MDMParser 클래스 단위 테스트."""

    def test_tokenize_delegates_to_tokenizer(self):
        """tokenize()가 Tokenizer에 위임합니다."""
        parser = MDMParser()
        tokens = parser.tokenize('Hello ![[img.jpg]] World')
        assert isinstance(tokens, list)
        assert len(tokens) == 3
        assert tokens[1]['type'] == 'mdm-reference'
        assert tokens[1]['name'] == 'img.jpg'

    def test_get_mdm_data_initially_none(self):
        """초기 MDM 데이터는 None입니다."""
        parser = MDMParser()
        assert parser.get_mdm_data() is None

    def test_set_and_get_mdm_data(self):
        """set_mdm_data와 get_mdm_data가 올바르게 동작합니다."""
        parser = MDMParser()
        data = {
            'version': '1.0',
            'resources': {'logo': {'type': 'image', 'src': '/logo.png', 'alt': 'Logo'}},
        }
        parser.set_mdm_data(data)
        assert parser.get_mdm_data() == data

    def test_parse_direct_file_reference(self):
        """직접 파일 참조를 HTML로 변환합니다."""
        parser = MDMParser()
        html = parser.parse('Look at ![[photo.jpg]] here')
        assert isinstance(html, str)
        assert '<img' in html
        assert 'src="photo.jpg"' in html
        assert 'Look at' in html
        assert 'here' in html

    def test_parse_video_file(self):
        """비디오 파일을 HTML로 변환합니다."""
        parser = MDMParser()
        html = parser.parse('![[intro.mp4 | controls width=720]]')
        assert '<video' in html
        assert 'controls' in html
        assert 'width="720"' in html

    def test_parse_audio_file(self):
        """오디오 파일을 HTML로 변환합니다."""
        parser = MDMParser()
        html = parser.parse('![[podcast.mp3 | controls]]')
        assert '<audio' in html

    def test_parse_plain_text_unchanged(self):
        """MDM 참조가 없는 텍스트는 그대로 반환합니다."""
        parser = MDMParser()
        result = parser.parse('No media here, just text.')
        assert result == 'No media here, just text.'

    def test_parse_multiple_references(self):
        """여러 MDM 참조를 순서대로 변환합니다."""
        parser = MDMParser()
        html = parser.parse('![[a.jpg]] and ![[b.png]]')
        import re
        img_matches = re.findall(r'<img', html)
        assert len(img_matches) == 2

    def test_parse_with_pre_loaded_mdm_data(self):
        """사전 로드된 MDM 데이터를 사용합니다."""
        parser = MDMParser()
        parser.set_mdm_data({
            'version': '1.0',
            'resources': {
                'hero': {'type': 'image', 'src': '/images/hero.jpg', 'alt': 'Hero'},
            },
        })
        html = parser.parse('![[hero]]')
        assert 'src="/images/hero.jpg"' in html
        assert 'alt="Hero"' in html

    def test_parse_attribute_override_on_named_resource(self):
        """인라인 속성이 MDM 리소스의 속성을 오버라이드합니다."""
        parser = MDMParser()
        parser.set_mdm_data({
            'version': '1.0',
            'resources': {
                'logo': {'type': 'image', 'src': '/logo.png', 'alt': 'Logo'},
            },
        })
        html = parser.parse('![[logo | width=200 align=center]]')
        assert 'width="200"' in html
        assert 'align-center' in html

    def test_clear_cache_resets_state(self):
        """clear_cache()가 MDM 데이터와 렌더러를 초기화합니다."""
        parser = MDMParser()
        parser.set_mdm_data({'version': '1.0', 'resources': {}})
        assert parser.get_mdm_data() is not None
        parser.clear_cache()
        assert parser.get_mdm_data() is None

    def test_caption_wraps_in_figure(self):
        """caption 속성이 <figure>/<figcaption>을 생성합니다."""
        parser = MDMParser()
        html = parser.parse('![[img.jpg | caption="A nice photo"]]')
        assert '<figure>' in html
        assert '<figcaption>A nice photo</figcaption>' in html

    def test_xss_escape_in_caption(self):
        """caption의 XSS 스크립트를 이스케이프합니다."""
        parser = MDMParser()
        html = parser.parse('![[img.jpg | caption="<script>bad</script>"]]')
        assert '<script>' not in html
        assert '&lt;script&gt;' in html


class TestParseConvenienceFunction:
    """모듈 레벨 parse() 편의 함수 테스트."""

    def test_parse_function(self):
        """parse() 함수가 올바르게 동작합니다."""
        html = parse('![[sunset.jpg | width=800]]')
        assert '<img' in html
        assert 'width="800"' in html

    def test_parse_empty_string(self):
        """빈 문자열을 파싱하면 빈 문자열을 반환합니다."""
        assert parse('') == ''

    def test_parse_plain_text(self):
        """MDM 참조 없는 텍스트는 그대로 반환합니다."""
        result = parse('Just some text, no media.')
        assert result == 'Just some text, no media.'


# ===========================================================================
# Integration 테스트
# ===========================================================================

def make_parser(resources=None, presets=None):
    """테스트용 파서를 생성하는 헬퍼 함수."""
    parser = MDMParser()
    parser.set_mdm_data({
        'version': '1.0',
        'resources': resources or {},
        'presets': presets or {},
    })
    return parser


class TestIntegrationBlogDocument:
    """블로그 스타일 문서 통합 테스트."""

    def setup_method(self):
        self.parser = make_parser({
            'site-logo': {'type': 'image', 'src': '/assets/logo.png', 'alt': 'My Blog Logo'},
            'hero-welcome': {'type': 'image', 'src': '/assets/hero.jpg', 'alt': 'Hero'},
            'intro-video': {
                'type': 'video',
                'src': '/assets/intro.mp4',
                'presets': {
                    'inline': {'width': 800, 'controls': True},
                    'bg': {'autoplay': True, 'muted': True, 'loop': True},
                },
            },
            'youtube-demo': {'type': 'embed', 'provider': 'youtube', 'id': 'dQw4w9WgXcQ'},
        })
        markdown = '\n'.join([
            'Welcome to my blog.',
            '',
            '![[site-logo]]',
            '',
            '![[hero-welcome | width=1200]]',
            '',
            '![[intro-video:inline]]',
            '',
            '![[youtube-demo | width=800 height=450]]',
            '',
            'Thanks for reading!',
        ])
        self.html = self.parser.parse(markdown)

    def test_logo_renders_as_img(self):
        """로고가 img로 렌더링됩니다."""
        assert 'src="/assets/logo.png"' in self.html
        assert 'alt="My Blog Logo"' in self.html

    def test_hero_with_width_override(self):
        """히어로 이미지에 width 오버라이드가 적용됩니다."""
        assert 'width="1200"' in self.html

    def test_video_with_inline_preset(self):
        """인라인 프리셋을 가진 비디오가 controls를 포함합니다."""
        assert 'controls' in self.html
        assert 'width="800"' in self.html

    def test_youtube_iframe_generated(self):
        """YouTube iframe이 생성됩니다."""
        assert 'youtube.com/embed/dQw4w9WgXcQ' in self.html
        assert 'width="800"' in self.html
        assert 'height="450"' in self.html

    def test_plain_text_preserved(self):
        """일반 텍스트가 보존됩니다."""
        assert 'Welcome to my blog.' in self.html
        assert 'Thanks for reading!' in self.html


class TestIntegrationDirectFileReferences:
    """MDM 데이터 없이 직접 파일 참조 통합 테스트."""

    def setup_method(self):
        parser = MDMParser()
        markdown = '\n'.join([
            '![[photo.jpg | width=500 align=center alt="A sunset"]]',
            '![[demo.mp4 | controls width=720]]',
            '![[podcast.mp3 | controls]]',
        ])
        self.html = parser.parse(markdown)

    def test_image_attributes_applied(self):
        """이미지 속성이 올바르게 적용됩니다."""
        assert 'width="500"' in self.html
        assert 'class="align-center"' in self.html
        assert 'alt="A sunset"' in self.html

    def test_video_attributes_applied(self):
        """비디오 속성이 올바르게 적용됩니다."""
        assert '<video' in self.html
        assert 'controls' in self.html
        assert 'width="720"' in self.html

    def test_audio_rendered(self):
        """오디오가 렌더링됩니다."""
        assert '<audio' in self.html


class TestIntegrationGlobalPresets:
    """전역 프리셋 통합 테스트."""

    def test_global_preset_applied(self):
        """전역 프리셋의 속성이 적용됩니다."""
        parser = make_parser(
            {'banner': {'type': 'image', 'src': '/banner.jpg', 'alt': 'Banner'}},
            {'hero': {'width': 1200, 'height': 400}},
        )
        html = parser.parse('![[banner:hero]]')
        assert 'width="1200"' in html
        assert 'height="400"' in html


class TestIntegrationFigureCaption:
    """figure/figcaption 통합 테스트."""

    def test_figure_with_caption(self):
        """caption 속성이 figure와 figcaption을 생성합니다."""
        parser = make_parser({'screenshot': {'type': 'image', 'src': '/shot.png', 'alt': 'App'}})
        html = parser.parse('![[screenshot | caption="Main dashboard"]]')
        assert '<figure>' in html
        assert '<figcaption>Main dashboard</figcaption>' in html


class TestIntegrationXSSPrevention:
    """XSS 방지 통합 테스트."""

    def test_script_in_caption_escaped(self):
        """caption의 스크립트 주입이 이스케이프됩니다."""
        parser = MDMParser()
        html = parser.parse('![[img.jpg | caption="<script>alert(1)</script>"]]')
        assert '<script>' not in html
        assert '&lt;script&gt;' in html

    def test_script_in_alt_escaped(self):
        """alt의 스크립트 주입이 이스케이프됩니다."""
        parser = MDMParser()
        html = parser.parse('![[img.jpg | alt="<img onerror=alert(1)>"]]')
        assert '&lt;img' in html
        assert '&gt;' in html
        # outer <img> 태그에 onerror가 없어야 합니다
        import re
        outer_tags = re.findall(r'<img[^>]*>', html)
        for tag in outer_tags:
            # alt 값 내부가 아닌 태그 레벨에 onerror가 없어야 함
            # alt="..." 안의 내용을 제거한 후 체크
            tag_without_alt = re.sub(r'alt="[^"]*"', '', tag)
            assert 'onerror' not in tag_without_alt


class TestIntegrationUnknownFileType:
    """알 수 없는 파일 타입 통합 테스트."""

    def test_unknown_file_type_returns_comment(self):
        """알 수 없는 파일 타입은 HTML 주석을 반환합니다."""
        parser = MDMParser()
        html = parser.parse('![[document.pdf]]')
        assert '<!--' in html


class TestIntegrationEdgeCases:
    """엣지 케이스 통합 테스트."""

    def test_empty_string(self):
        """빈 문자열은 빈 문자열을 반환합니다."""
        parser = MDMParser()
        assert parser.parse('') == ''

    def test_only_plain_text(self):
        """MDM 참조 없는 텍스트는 그대로 반환합니다."""
        parser = MDMParser()
        result = parser.parse('Just some text, no media.')
        assert result == 'Just some text, no media.'

    def test_multiple_refs_on_same_line(self):
        """같은 줄에 여러 MDM 참조가 있어도 처리합니다."""
        import re
        parser = MDMParser()
        html = parser.parse('![[a.jpg]] and ![[b.jpg]]')
        count = len(re.findall(r'<img', html))
        assert count == 2
