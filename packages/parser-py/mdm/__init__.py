"""
MDM (Markdown+Media) Python 파서 패키지.

![[name:preset | attr=val]] 형식의 MDM 참조를 HTML로 변환합니다.

기본 사용법::

    from mdm import parse

    html = parse("Here is an image: ![[photo.jpg | width=800]]")

클래스 기반 사용법::

    from mdm import MDMParser

    parser = MDMParser()
    parser.set_mdm_data({
        "version": "1.0",
        "resources": {
            "hero": {"type": "image", "src": "/img/hero.jpg", "alt": "Hero"},
        },
    })
    html = parser.parse("![[hero | width=1200]]")
"""

from .loader import MDMLoader
from .parser import MDMParser, parse
from .renderer import Renderer
from .tokenizer import Tokenizer

__all__ = [
    "MDMParser",
    "parse",
    "Tokenizer",
    "Renderer",
    "MDMLoader",
]

__version__ = "0.1.0"
