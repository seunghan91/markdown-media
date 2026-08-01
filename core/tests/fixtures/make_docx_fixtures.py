#!/usr/bin/env python3
"""
DOCX 테스트 픽스처 제작 스크립트 (C4: 이미지 앵커 파싱 검증용)

core/tests/fixtures/ 에 아래 두 개의 최소 OOXML .docx 를 생성한다.
python-docx 등 외부 의존성 없이 stdlib(zipfile/zlib/struct)만으로 조립하며,
이미지 바이트도 이 스크립트 안에서 직접 생성한다 — 재현 가능성 확보.

- images_basic.docx  : 인라인(wp:inline) 이미지 2개, 동일 바이트(dedup 케이스),
                       이미지 앞뒤로 한국어 문단.
- images_anchor.docx : 플로팅(wp:anchor) 이미지 1개 + drawing 없는 순수 텍스트 문단
                       (앵커 vs 인라인 구분 검증용).

실행: python3 core/tests/fixtures/make_docx_fixtures.py
"""
from __future__ import annotations

import struct
import zlib
import zipfile
from pathlib import Path

FIXTURES_DIR = Path(__file__).resolve().parent

NS_W = "http://schemas.openxmlformats.org/wordprocessingml/2006/main"
NS_WP = "http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing"
NS_A = "http://schemas.openxmlformats.org/drawingml/2006/main"
NS_PIC = "http://schemas.openxmlformats.org/drawingml/2006/picture"
NS_R = "http://schemas.openxmlformats.org/officeDocument/2006/relationships"


def make_solid_png(width: int, height: int, rgb: tuple[int, int, int]) -> bytes:
    """stdlib(zlib/struct)만으로 단색 PNG 바이트를 만든다 (8bit, color type 2=RGB)."""

    def chunk(tag: bytes, data: bytes) -> bytes:
        return (
            struct.pack(">I", len(data))
            + tag
            + data
            + struct.pack(">I", zlib.crc32(tag + data) & 0xFFFFFFFF)
        )

    sig = b"\x89PNG\r\n\x1a\n"
    ihdr = struct.pack(">IIBBBBB", width, height, 8, 2, 0, 0, 0)
    row = b"\x00" + bytes(rgb) * width  # 필터 바이트(0=없음) + RGB 픽셀 반복
    raw = row * height
    idat = zlib.compress(raw, 9)
    return sig + chunk(b"IHDR", ihdr) + chunk(b"IDAT", idat) + chunk(b"IEND", b"")


def content_types_xml() -> str:
    return """<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Default Extension="png" ContentType="image/png"/>
  <Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>
  <Override PartName="/word/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml"/>
  <Override PartName="/docProps/core.xml" ContentType="application/vnd.openxmlformats-package.core-properties+xml"/>
  <Override PartName="/docProps/app.xml" ContentType="application/vnd.openxmlformats-officedocument.extended-properties+xml"/>
</Types>"""


def package_rels_xml() -> str:
    return """<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/>
  <Relationship Id="rId2" Type="http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties" Target="docProps/core.xml"/>
  <Relationship Id="rId3" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/extended-properties" Target="docProps/app.xml"/>
</Relationships>"""


def document_rels_xml(image_rels: list[tuple[str, str]]) -> str:
    rels = "\n".join(
        f'  <Relationship Id="{rid}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="media/{target}"/>'
        for rid, target in image_rels
    )
    return f"""<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
{rels}
  <Relationship Id="rIdStyles" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/>
</Relationships>"""


def styles_xml() -> str:
    return f"""<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:styles xmlns:w="{NS_W}">
  <w:docDefaults/>
  <w:style w:type="paragraph" w:default="1" w:styleId="Normal">
    <w:name w:val="Normal"/>
  </w:style>
</w:styles>"""


def core_props_xml() -> str:
    return """<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties"
                    xmlns:dc="http://purl.org/dc/elements/1.1/">
  <dc:title>DOCX fixture</dc:title>
  <dc:creator>make_docx_fixtures.py</dc:creator>
</cp:coreProperties>"""


def app_props_xml() -> str:
    return """<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Properties xmlns="http://schemas.openxmlformats.org/officeDocument/2006/extended-properties">
  <Application>make_docx_fixtures.py</Application>
</Properties>"""


def text_paragraph(text: str) -> str:
    return f'<w:p><w:r><w:t xml:space="preserve">{text}</w:t></w:r></w:p>'


def inline_drawing_paragraph(rid: str, doc_pr_id: int, name: str, descr: str, cx: int, cy: int) -> str:
    """wp:inline 이미지 문단 — docPr descr(alt text) + 명시적 extent(cx/cy, EMU) 포함."""
    return f"""<w:p><w:r><w:drawing>
    <wp:inline distT="0" distB="0" distL="0" distR="0">
      <wp:extent cx="{cx}" cy="{cy}"/>
      <wp:effectExtent l="0" t="0" r="0" b="0"/>
      <wp:docPr id="{doc_pr_id}" name="{name}" descr="{descr}"/>
      <wp:cNvGraphicFramePr>
        <a:graphicFrameLocks noChangeAspect="1"/>
      </wp:cNvGraphicFramePr>
      <a:graphic>
        <a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/picture">
          <pic:pic>
            <pic:nvPicPr>
              <pic:cNvPr id="{doc_pr_id}" name="{name}" descr="{descr}"/>
              <pic:cNvPicPr/>
            </pic:nvPicPr>
            <pic:blipFill>
              <a:blip r:embed="{rid}"/>
              <a:stretch><a:fillRect/></a:stretch>
            </pic:blipFill>
            <pic:spPr>
              <a:xfrm>
                <a:off x="0" y="0"/>
                <a:ext cx="{cx}" cy="{cy}"/>
              </a:xfrm>
              <a:prstGeom prst="rect"><a:avLst/></a:prstGeom>
            </pic:spPr>
          </pic:pic>
        </a:graphicData>
      </a:graphic>
    </wp:inline>
</w:drawing></w:r></w:p>"""


def anchor_drawing_paragraph(rid: str, doc_pr_id: int, name: str, descr: str, cx: int, cy: int) -> str:
    """wp:anchor(floating) 이미지 문단 — wp:inline 과 달리 simplePos/positionH/positionV/wrap 을 갖는다."""
    return f"""<w:p><w:r><w:drawing>
    <wp:anchor distT="0" distB="0" distL="114300" distR="114300" simplePos="0"
               relativeHeight="1" behindDoc="0" locked="0" layoutInCell="1" allowOverlap="1">
      <wp:simplePos x="0" y="0"/>
      <wp:positionH relativeFrom="column"><wp:posOffset>914400</wp:posOffset></wp:positionH>
      <wp:positionV relativeFrom="paragraph"><wp:posOffset>0</wp:posOffset></wp:positionV>
      <wp:extent cx="{cx}" cy="{cy}"/>
      <wp:effectExtent l="0" t="0" r="0" b="0"/>
      <wp:wrapSquare wrapText="bothSides"/>
      <wp:docPr id="{doc_pr_id}" name="{name}" descr="{descr}"/>
      <wp:cNvGraphicFramePr>
        <a:graphicFrameLocks noChangeAspect="1"/>
      </wp:cNvGraphicFramePr>
      <a:graphic>
        <a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/picture">
          <pic:pic>
            <pic:nvPicPr>
              <pic:cNvPr id="{doc_pr_id}" name="{name}" descr="{descr}"/>
              <pic:cNvPicPr/>
            </pic:nvPicPr>
            <pic:blipFill>
              <a:blip r:embed="{rid}"/>
              <a:stretch><a:fillRect/></a:stretch>
            </pic:blipFill>
            <pic:spPr>
              <a:xfrm>
                <a:off x="0" y="0"/>
                <a:ext cx="{cx}" cy="{cy}"/>
              </a:xfrm>
              <a:prstGeom prst="rect"><a:avLst/></a:prstGeom>
            </pic:spPr>
          </pic:pic>
        </a:graphicData>
      </a:graphic>
    </wp:anchor>
</w:drawing></w:r></w:p>"""


def document_xml(body_paragraphs: list[str]) -> str:
    body = "\n".join(body_paragraphs)
    return f"""<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="{NS_W}" xmlns:wp="{NS_WP}" xmlns:a="{NS_A}" xmlns:pic="{NS_PIC}" xmlns:r="{NS_R}">
  <w:body>
{body}
    <w:sectPr>
      <w:pgSz w:w="11906" w:h="16838"/>
      <w:pgMar w:top="1417" w:right="1417" w:bottom="1417" w:left="1417" w:header="708" w:footer="708" w:gutter="0"/>
    </w:sectPr>
  </w:body>
</w:document>"""


def write_docx(
    path: Path,
    body_paragraphs: list[str],
    media: dict[str, bytes],
    image_rels: list[tuple[str, str]],
) -> None:
    with zipfile.ZipFile(path, "w", zipfile.ZIP_DEFLATED) as zf:
        zf.writestr("[Content_Types].xml", content_types_xml())
        zf.writestr("_rels/.rels", package_rels_xml())
        zf.writestr("word/document.xml", document_xml(body_paragraphs))
        zf.writestr("word/_rels/document.xml.rels", document_rels_xml(image_rels))
        zf.writestr("word/styles.xml", styles_xml())
        zf.writestr("docProps/core.xml", core_props_xml())
        zf.writestr("docProps/app.xml", app_props_xml())
        for name, data in media.items():
            zf.writestr(f"word/media/{name}", data)


def build_images_basic() -> None:
    png = make_solid_png(8, 8, (220, 30, 30))  # 8x8 단색(적색) PNG

    paragraphs = [
        text_paragraph("이미지 삽입 테스트 문서입니다."),
        inline_drawing_paragraph(
            rid="rId1", doc_pr_id=1, name="image1.png",
            descr="첫 번째 그림", cx=304800, cy=304800,
        ),
        text_paragraph("두 그림 사이에 위치한 본문 문단입니다."),
        inline_drawing_paragraph(
            rid="rId2", doc_pr_id=2, name="image2.png",
            descr="중복 그림", cx=228600, cy=228600,
        ),
        text_paragraph("문서의 마지막 문단입니다."),
    ]

    write_docx(
        FIXTURES_DIR / "images_basic.docx",
        paragraphs,
        media={"image1.png": png, "image2.png": png},  # 동일 바이트 — dedup 케이스
        image_rels=[("rId1", "image1.png"), ("rId2", "image2.png")],
    )


def build_images_anchor() -> None:
    png = make_solid_png(8, 8, (30, 120, 220))  # 8x8 단색(청색) PNG

    paragraphs = [
        text_paragraph("이미지가 없는 순수 텍스트 문단입니다."),
        anchor_drawing_paragraph(
            rid="rId1", doc_pr_id=1, name="image1.png",
            descr="플로팅 이미지", cx=457200, cy=457200,
        ),
        text_paragraph("플로팅 이미지 뒤에 이어지는 문단입니다."),
    ]

    write_docx(
        FIXTURES_DIR / "images_anchor.docx",
        paragraphs,
        media={"image1.png": png},
        image_rels=[("rId1", "image1.png")],
    )


if __name__ == "__main__":
    build_images_basic()
    build_images_anchor()
    print(f"생성됨: {FIXTURES_DIR / 'images_basic.docx'}")
    print(f"생성됨: {FIXTURES_DIR / 'images_anchor.docx'}")
