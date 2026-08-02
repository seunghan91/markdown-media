#!/usr/bin/env python3
"""
DOCX 테스트 픽스처 제작 스크립트 (C4: 이미지 앵커 파싱 검증용 / P2-M1: 메타파일 보존 검증용)

core/tests/fixtures/ 에 아래 세 개의 최소 OOXML .docx 를 생성한다.
python-docx 등 외부 의존성 없이 stdlib(zipfile/zlib/struct)만으로 조립하며,
이미지 바이트도 이 스크립트 안에서 직접 생성한다 — 재현 가능성 확보.

- images_basic.docx    : 인라인(wp:inline) 이미지 2개, 동일 바이트(dedup 케이스),
                         이미지 앞뒤로 한국어 문단.
- images_anchor.docx   : 플로팅(wp:anchor) 이미지 1개 + drawing 없는 순수 텍스트 문단
                         (앵커 vs 인라인 구분 검증용).
- images_metafile.docx : WMF 1개(본문 wp:inline 참조) + EMF 1개(관계만 등록,
                         본문 미참조 — '## 이미지' 목록행 케이스 검증용).
                         변환 없이 원본 확장자 그대로 보존되는지 확인하는 용도.

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


def make_minimal_wmf() -> bytes:
    """Standard (non-placeable) WMF with actual drawable content — no
    D7CDC69A placeable-header magic (that's a separate, Windows-added
    wrapper the parser's WMF sniff doesn't require).

    METAHEADER (18 bytes): mtType(WORD) mtHeaderSize(WORD) mtVersion(WORD)
    mtSize(DWORD, in WORDs) mtNoObjects(WORD) mtMaxRecord(DWORD, in WORDs)
    mtNoParameters(WORD). Records: [rdSize(DWORD, WORDs)][rdFunction(WORD)]
    [params...]. mtType=1 시작을 유지해 기존 바이트-보존 assert와 호환.

    P2-M2: SETWINDOWEXT + SETWINDOWORG + RECTANGLE 레코드를 넣어
    WMF→SVG 변환기가 실제 요소를 그리도록 함 (헤더+EOF 뿐이면 퇴화
    SVG 가드가 None으로 강등해 변환 없이 원본 보존됨).
    """

    def record(function: int, params: list[int]) -> bytes:
        size_words = 3 + len(params)  # rdSize(2 WORDs) + rdFunction + params
        return struct.pack("<IH", size_words, function) + b"".join(
            struct.pack("<H", p) for p in params
        )

    records = (
        record(0x020C, [200, 300])          # SETWINDOWEXT (y, x)
        + record(0x020B, [0, 0])            # SETWINDOWORG
        + record(0x041B, [150, 100, 50, 20])  # RECTANGLE (bottom right top left)
        + record(0x0000, [])                # META_EOF
    )
    max_record_words = 7  # RECTANGLE: 3 + 4 params
    total_words = 9 + len(records) // 2
    header = struct.pack(
        "<HHHIHIH",
        1,                 # mtType = memory metafile
        9,                 # mtHeaderSize (WORDs)
        0x0300,            # mtVersion = Windows 3.0
        total_words,       # mtSize (WORDs)
        0,                 # mtNoObjects
        max_record_words,  # mtMaxRecord (WORDs)
        0,                 # mtNoParameters (reserved)
    )
    return header + records


def make_minimal_emf() -> bytes:
    """Minimal ENHMETAHEADER: iType=1 (EMR_HEADER) at offset 0, dSignature
    (" EMF") at offset 40 — iType(4)+nSize(4)+rclBounds(16)+rclFrame(16)=40.
    Matches core/src/hwp/parser.rs's detect_image_format offset-40 check
    (added in this same P2-M1 change) so this fixture also doubles as an
    EMF-detection regression input.
    """
    buf = bytearray(88)  # minimal ENHMETAHEADER size
    struct.pack_into("<I", buf, 0, 1)  # iType = EMR_HEADER
    struct.pack_into("<I", buf, 4, len(buf))  # nSize
    buf[40:44] = b" EMF"  # dSignature
    struct.pack_into("<I", buf, 44, 0x00010000)  # nVersion
    return bytes(buf)


def content_types_xml(extra_parts: list[str] | None = None) -> str:
    overrides = ""
    for part in extra_parts or []:
        if "chart" in part:
            overrides += (
                f'\n  <Override PartName="/{part}" ContentType='
                '"application/vnd.openxmlformats-officedocument.drawingml.chart+xml"/>'
            )
    return """<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Default Extension="png" ContentType="image/png"/>
  <Default Extension="wmf" ContentType="image/x-wmf"/>
  <Default Extension="emf" ContentType="image/x-emf"/>
  <Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>
  <Override PartName="/word/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml"/>
  <Override PartName="/docProps/core.xml" ContentType="application/vnd.openxmlformats-package.core-properties+xml"/>
  <Override PartName="/docProps/app.xml" ContentType="application/vnd.openxmlformats-officedocument.extended-properties+xml"/>{overrides}
</Types>""".replace("{overrides}", overrides)


def package_rels_xml() -> str:
    return """<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/>
  <Relationship Id="rId2" Type="http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties" Target="docProps/core.xml"/>
  <Relationship Id="rId3" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/extended-properties" Target="docProps/app.xml"/>
</Relationships>"""


def document_rels_xml(
    image_rels: list[tuple[str, str]],
    extra_rels: list[tuple[str, str, str]] | None = None,
) -> str:
    base = "http://schemas.openxmlformats.org/officeDocument/2006/relationships"
    rels = "\n".join(
        f'  <Relationship Id="{rid}" Type="{base}/image" Target="media/{target}"/>'
        for rid, target in image_rels
    )
    for rid, kind, target in extra_rels or []:
        rels += f'\n  <Relationship Id="{rid}" Type="{base}/{kind}" Target="{target}"/>'
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
    extra_parts: dict[str, str] | None = None,
    extra_rels: list[tuple[str, str, str]] | None = None,
) -> None:
    """extra_parts: ZIP 경로 → XML 본문. extra_rels: (rId, 관계타입 접미사, Target)."""
    with zipfile.ZipFile(path, "w", zipfile.ZIP_DEFLATED) as zf:
        zf.writestr("[Content_Types].xml", content_types_xml(list((extra_parts or {}).keys())))
        zf.writestr("_rels/.rels", package_rels_xml())
        zf.writestr("word/document.xml", document_xml(body_paragraphs))
        zf.writestr("word/_rels/document.xml.rels", document_rels_xml(image_rels, extra_rels))
        for part_path, part_xml in (extra_parts or {}).items():
            zf.writestr(part_path, part_xml)
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


def build_images_metafile() -> None:
    """WMF(본문 wp:inline 참조) + EMF(관계만 등록, 본문 미참조) 조합.

    EMF는 어떤 <w:drawing>에서도 r:embed="rId2"를 참조하지 않는다 — 본문에
    등장하지 않는 임베디드 이미지가 여전히 매니페스트/'## 이미지' 목록으로
    보존되는지 확인하는 케이스 (HWPX의 미참조-이미지 목록 관례와 동형).
    """
    wmf = make_minimal_wmf()
    emf = make_minimal_emf()

    paragraphs = [
        text_paragraph("메타파일(WMF/EMF) 보존 테스트 문서입니다."),
        inline_drawing_paragraph(
            rid="rId1", doc_pr_id=1, name="image1.wmf",
            descr="WMF 그림", cx=304800, cy=304800,
        ),
        text_paragraph("EMF는 본문에 참조되지 않고 관계로만 존재합니다."),
    ]

    write_docx(
        FIXTURES_DIR / "images_metafile.docx",
        paragraphs,
        media={"image1.wmf": wmf, "image2.emf": emf},
        image_rels=[("rId1", "image1.wmf"), ("rId2", "image2.emf")],
    )


def chart_space_xml() -> str:
    """OOXML DrawingML chartSpace — HWPX의 Chart/chartN.xml과 동일 스키마.
    (hwpx_gen이 생성하는 실물 chart1.xml 구조를 그대로 축약)"""
    NS_C = "http://schemas.openxmlformats.org/drawingml/2006/chart"

    def ser(idx: int, col: str, name: str, values: list[int]) -> str:
        cats = ["상반기", "하반기"]
        cat_pts = "".join(
            f'<c:pt idx="{i}"><c:v>{c}</c:v></c:pt>' for i, c in enumerate(cats)
        )
        val_pts = "".join(
            f'<c:pt idx="{i}"><c:v>{v}</c:v></c:pt>' for i, v in enumerate(values)
        )
        return (
            f'<c:ser><c:idx val="{idx}"/><c:order val="{idx}"/>'
            f'<c:tx><c:strRef><c:f>Sheet1!${col}$1</c:f><c:strCache>'
            f'<c:ptCount val="1"/><c:pt idx="0"><c:v>{name}</c:v></c:pt>'
            f"</c:strCache></c:strRef></c:tx>"
            f'<c:cat><c:strRef><c:f>Sheet1!$A$2:$A$3</c:f><c:strCache>'
            f'<c:ptCount val="{len(cats)}"/>{cat_pts}</c:strCache></c:strRef></c:cat>'
            f'<c:val><c:numRef><c:f>Sheet1!${col}$2:${col}$3</c:f><c:numCache>'
            f'<c:formatCode>General</c:formatCode><c:ptCount val="{len(values)}"/>'
            f"{val_pts}</c:numCache></c:numRef></c:val></c:ser>"
        )

    return (
        '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
        f'<c:chartSpace xmlns:c="{NS_C}" xmlns:a="{NS_A}" xmlns:r="{NS_R}">'
        "<c:chart>"
        '<c:title><c:tx><c:rich><a:p><a:r><a:t>부서별 집계</a:t></a:r></a:p>'
        "</c:rich></c:tx></c:title>"
        '<c:plotArea><c:layout/><c:barChart><c:barDir val="col"/>'
        '<c:grouping val="clustered"/><c:varyColors val="0"/>'
        + ser(0, "B", "영업부", [340, 410])
        + ser(1, "C", "기술부", [280, 305])
        + "</c:barChart></c:plotArea></c:chart></c:chartSpace>"
    )


def chart_drawing_paragraph(rid: str, doc_pr_id: int, cx: int, cy: int) -> str:
    """<w:drawing> 안에 a:graphicData/c:chart(r:id) 를 담은 차트 참조 문단."""
    NS_C = "http://schemas.openxmlformats.org/drawingml/2006/chart"
    return (
        "<w:p><w:r><w:drawing>"
        f'<wp:inline distT="0" distB="0" distL="0" distR="0">'
        f'<wp:extent cx="{cx}" cy="{cy}"/>'
        f'<wp:docPr id="{doc_pr_id}" name="차트 {doc_pr_id}" descr="분기 집계 차트"/>'
        "<a:graphic><a:graphicData "
        f'uri="{NS_C}">'
        f'<c:chart xmlns:c="{NS_C}" r:id="{rid}"/>'
        "</a:graphicData></a:graphic>"
        "</wp:inline></w:drawing></w:r></w:p>"
    )


def build_chart_basic() -> None:
    """차트 1개(본문 wp:inline 참조) + 앞뒤 텍스트 문단 (P2-M4).

    word/charts/chart1.xml 은 HWPX Chart/chartN.xml 과 같은 OOXML chartSpace
    스키마라, 같은 chart_data 파서로 데이터 표가 나오는지 검증하는 픽스처.
    """
    paragraphs = [
        text_paragraph("차트 데이터 추출 테스트 문서입니다."),
        chart_drawing_paragraph(rid="rIdChart1", doc_pr_id=1, cx=5486400, cy=3200400),
        text_paragraph("차트 뒤에 이어지는 문단입니다."),
    ]
    write_docx(
        FIXTURES_DIR / "chart_basic.docx",
        paragraphs,
        media={},
        image_rels=[],
        extra_parts={"word/charts/chart1.xml": chart_space_xml()},
        extra_rels=[("rIdChart1", "chart", "charts/chart1.xml")],
    )


if __name__ == "__main__":
    build_images_basic()
    build_images_anchor()
    build_images_metafile()
    build_chart_basic()
    print(f"생성됨: {FIXTURES_DIR / 'images_basic.docx'}")
    print(f"생성됨: {FIXTURES_DIR / 'images_anchor.docx'}")
    print(f"생성됨: {FIXTURES_DIR / 'images_metafile.docx'}")
    print(f"생성됨: {FIXTURES_DIR / 'chart_basic.docx'}")
