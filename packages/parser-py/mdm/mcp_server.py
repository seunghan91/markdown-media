"""MDM MCP 서버 — Claude Desktop/Claude Code 등에서 문서 파싱 도구로 사용.

`mcp` 공식 Python SDK(FastMCP)로 구현. reference/kkdoc/src/mcp.ts(TypeScript, 15개 도구)와
도구 이름·스키마 호환을 목표로 하되, 아직 core(Rust)에 없는 기능은 명확한 오류 메시지를
반환하는 스텁으로 둔다 (연결 지점은 각 스텁 docstring의 "연결 대기" 참조).

실행:
    mdm-mcp                    # 콘솔 스크립트 (pyproject [project.scripts])
    python -m mdm.mcp_server   # 모듈 직접 실행

설치: pip install "mdm-parser[mcp]"
"""

import json
from typing import Optional

from mcp.server.fastmcp import FastMCP

from . import mcp_backend as backend

mcp = FastMCP("mdm-parser")


# ─── 실제 동작하는 도구 (mdm-core 네이티브 또는 hwp2mdm CLI 백엔드) ────────


@mcp.tool()
def parse_document(file_path: str, ocr: Optional[bool] = None) -> str:
    """한국 문서 파일(HWP, HWPX, PDF, XLSX, DOCX)을 마크다운으로 변환합니다.
    파일 경로를 입력하면 포맷을 자동 감지하여 텍스트를 추출합니다.

    ocr: 스캔/이미지 PDF 텍스트 OCR 요청 — 현재 백엔드(core CLI)는 OCR을 지원하지
    않아 True로 지정해도 무시되고 경고가 붙습니다 (연결 대기: gap-ocr).
    """
    markdown = backend.convert_to_markdown(file_path)
    info = backend.get_document_info(file_path)
    fmt = (info.get("file", {}).get("format") or info.get("document", {}).get("format_guess") or "unknown")
    doc = info.get("document", {})
    meta_parts = [f"포맷: {str(fmt).upper()}"]
    if doc.get("pages"):
        meta_parts.append(f"페이지: {doc['pages']}")
    if doc.get("title"):
        meta_parts.append(f"제목: {doc['title']}")
    if doc.get("author"):
        meta_parts.append(f"작성자: {doc['author']}")
    header = f"[{' | '.join(meta_parts)}]"
    warn = "\n\n⚠️ ocr: true가 지정되었지만 현재 백엔드는 OCR을 지원하지 않습니다 (연결 대기: gap-ocr)." if ocr else ""
    return f"{header}{warn}\n\n{markdown}"


@mcp.tool()
def convert_to_markdown(file_path: str) -> str:
    """문서를 마크다운 본문만으로 변환합니다 (메타데이터/경고 없이 순수 본문)."""
    return backend.convert_to_markdown(file_path)


@mcp.tool()
def detect_format(file_path: str) -> str:
    """파일의 포맷을 감지합니다 (hwp, hwpx, pdf, xlsx, docx, unknown)."""
    fmt = backend.detect_format(file_path)
    return f"{file_path}: {fmt}"


@mcp.tool()
def get_document_info(file_path: str) -> str:
    """문서의 파일 정보 + 메타데이터(제목/작성자/페이지 수 등)를 JSON으로 반환합니다."""
    info = backend.get_document_info(file_path)
    return json.dumps(info, ensure_ascii=False, indent=2)


@mcp.tool()
def parse_metadata(file_path: str) -> str:
    """문서의 메타데이터(제목, 작성자, 페이지 등)만 빠르게 추출합니다.
    (kordoc mcp.ts의 parse_metadata와 이름 호환 — get_document_info와 동일 백엔드)
    """
    info = backend.get_document_info(file_path)
    fmt = backend.detect_format(file_path)
    return json.dumps({"format": fmt, **info}, ensure_ascii=False, indent=2)


@mcp.tool()
def extract_media(file_path: str, output_dir: str) -> str:
    """문서에서 이미지를 추출해 output_dir에 저장하고 파일 목록을 반환합니다."""
    files = backend.extract_media(file_path, output_dir)
    if not files:
        return "추출된 이미지가 없습니다."
    lines = [f"이미지 {len(files)}개 추출 → {output_dir}"]
    lines += [f"  - {f['filename']} ({f['size']:,} bytes)" for f in files]
    return "\n".join(lines)


@mcp.tool()
def compare_documents(file_path_a: str, file_path_b: str) -> str:
    """두 한국 문서 파일을 비교하여 추가/삭제/변경 라인을 표시합니다 (신구대조표 용도).

    주의: 현재는 마크다운 변환 후 라인 단위 diff(difflib)입니다 — kordoc처럼 표/블록
    단위 구조적 비교는 아직 지원하지 않습니다 (연결 대기: gap-diff의 core 구조적 diff API).
    """
    result = backend.compare_documents(file_path_a, file_path_b)
    stats = result["stats"]
    header = (
        f"## 문서 비교 결과 (라인 단위, 유사도 {stats['similarity'] * 100:.1f}%)\n"
        f"추가: {stats['added']} | 삭제: {stats['removed']} | 동일: {stats['unchanged']}\n"
    )
    return f"{header}\n{result['diff']}"


# ─── 스텁 도구 — 다른 에이전트가 병렬로 core(Rust)에 구현 중, 백엔드 연결 전까지 명확한 오류 반환 ──


@mcp.tool()
def parse_pages(file_path: str, pages: str) -> str:
    """문서의 특정 페이지/섹션 범위만 파싱합니다 (예: '1-3', '1,3,5-7').
    연결 대기: core CLI/바인딩의 페이지 범위 파라미터 (현재 hwp2mdm은 전체 문서만 변환)."""
    raise backend.NotYetAvailableError(
        "parse_pages", "core 페이지 범위 API (미배정 — general core 확장)",
        "hwp2mdm CLI/바인딩에 페이지 범위 옵션이 없어 부분 파싱을 지원하지 않습니다.",
    )


@mcp.tool()
def parse_table(file_path: str, table_index: int) -> str:
    """문서에서 N번째 테이블만 추출합니다 (0-based index).
    core CLI `hwp2mdm inspect --format json`을 통해 표 구조를 추출합니다."""
    try:
        from . import mcp_backend as b
        return b.parse_table(file_path, table_index)
    except Exception as e:
        raise backend.NotYetAvailableError(
            "parse_table", "hwp2mdm inspect 연동",
            f"표 추출 실패: {e}",
        ) from e


@mcp.tool()
def parse_form(file_path: str) -> str:
    """한국 서식 문서(HWPX)에서 레이블-값 쌍을 구조화된 JSON으로 추출합니다.
    (core CLI `fill --dry-run`의 폼 스키마)"""
    return backend.parse_form(file_path)


@mcp.tool()
def fill_form(
    file_path: str,
    fields: str,
    output_path: Optional[str] = None,
    output_format: str = "markdown",
) -> str:
    """한국 서식(HWPX) 문서의 빈칸을 채워서 새 문서로 출력합니다.
    fields: JSON 문자열 (라벨 → 값 맵, 예: '{"성명": "홍길동"}').
    output_path를 지정하지 않으면 임시 파일에 저장하고 경로를 반환합니다.
    """
    return backend.fill_form(file_path, fields, output_path=output_path, output_format=output_format)


@mcp.tool()
def place_seal(file_path: str, image_path: str, anchor: str = "(인)", output_path: str = "") -> str:
    """도장/서명 이미지를 앵커 문구 위에 배치합니다 (HWPX 전용).
    연결 대기: 미배정 — HWPX 생성/편집 파이프라인(gap-hwpxgen) 완료 후 연동 예정."""
    raise backend.NotYetAvailableError(
        "place_seal", "gap-hwpxgen (미배정 후속)", "도장 배치 API가 아직 core에 없습니다."
    )


@mcp.tool()
def patch_document(file_path: str, edited_markdown: str, output_path: str) -> str:
    """원본 문서의 서식을 보존한 채 바뀐 텍스트만 제자리 치환합니다.
    연결 대기: 미배정 — gap-diff의 구조적 diff API 이후 무손실 패치로 확장 예정."""
    raise backend.NotYetAvailableError(
        "patch_document", "gap-diff (미배정 후속)", "무손실 패치 API가 아직 core에 없습니다."
    )


@mcp.tool()
def render_document(file_path: str, output_format: str = "png", output_path: Optional[str] = None) -> str:
    """HWPX 문서를 조판 그대로 렌더링해 PNG/SVG로 반환합니다.
    연결 대기: gap-hwpxgen."""
    raise backend.NotYetAvailableError(
        "render_document", "gap-hwpxgen", "HWPX 렌더링(SVG/PNG) API가 아직 core에 없습니다."
    )


@mcp.tool()
def redact_document(
    file_path: str,
    rules: Optional[str] = None,
    output_path: Optional[str] = None,
    dry_run: bool = False,
) -> str:
    """문서의 개인정보(주민번호·전화·이메일·카드·계좌)를 탐지해 마스킹합니다.
    rules: 콤마로 구분된 규칙 목록 (rrn,phone,email,card,account,passport,driver;
    기본값 rrn,phone,email,card,account). output_path 지정 시 마스킹된 텍스트를 저장합니다
    (dry_run=True면 저장하지 않고 결과만 반환).
    """
    masked = backend.redact_document(file_path, rules=rules, output_path=output_path, dry_run=dry_run)
    if output_path and not dry_run:
        return f"PII 마스킹 완료 → {output_path}\n\n{masked}"
    return masked


@mcp.tool()
def parse_chunks(file_path: str, granularity: str = "section") -> str:
    """문서를 RAG용 구조 청크 JSON으로 파싱합니다 (헤딩/개조식 위계 breadcrumb 보존).
    granularity: 'section'(동일 헤딩 아래 병합) 또는 'block'(1:1)."""
    return backend.parse_chunks(file_path, granularity=granularity)


@mcp.tool()
def extract_profile(hwpx_path: str, output_path: str) -> str:
    """참조 HWPX 문서에서 표 서식 프로필(테두리·음영·열 너비 등)을 JSON으로 추출합니다.
    연결 대기: gap-hwpxgen."""
    raise backend.NotYetAvailableError(
        "extract_profile", "gap-hwpxgen", "서식 프로필 추출 API가 아직 core에 없습니다."
    )


@mcp.tool()
def generate_document(
    markdown: str,
    output_path: str,
    preset: Optional[str] = None,
    profile_path: Optional[str] = None,
) -> str:
    """마크다운을 HWPX 한글 문서로 생성합니다.
    preset: 공문서 프리셋 (기안문/보고서/계획서/통지/회의록/개조식/보도자료).
    profile_path: 표 서식 프로필 — 현재 core CLI가 지원하지 않아 무시됩니다 (연결 대기: gap-hwpxgen).
    """
    return backend.generate_document(markdown, output_path, preset=preset, profile_path=profile_path)


@mcp.tool()
def validate_hwpx(file_path: str) -> str:
    """HWPX 파일의 구조 유효성(OWPML 스키마·manifest 정합성)을 검증합니다.
    core CLI `hwp2mdm validate`를 호출합니다."""
    return backend.validate_hwpx(file_path)


@mcp.tool()
def hulk_to_latex(file_path: str) -> str:
    """HWPX 수식(HULK script)을 LaTeX로 변환합니다.
    core CLI `hwp2mdm equation`을 호출합니다."""
    return backend.hulk_to_latex(file_path)


@mcp.tool()
def ocr_document(file_path: str, output_path: Optional[str] = None) -> str:
    """스캔/이미지 기반 문서(PDF 등)에 OCR을 적용해 텍스트를 추출합니다.
    core CLI `hwp2mdm convert --ocr`를 호출합니다. (--features ocr 빌드 필요)"""
    return backend.ocr_document(file_path, output_path=output_path)


@mcp.tool()
def lint_document(file_path: str) -> str:
    """공문서 표기법(날짜·시간·금액·붙임 등)을 검수해 경고 목록을 반환합니다."""
    result = backend.lint_document(file_path)
    return result if result.strip() else "표기법 위반 사항이 발견되지 않았습니다."


def main() -> None:
    """콘솔 스크립트 엔트리포인트 (`mdm-mcp`) — stdio transport로 서버 기동."""
    mcp.run(transport="stdio")


if __name__ == "__main__":
    main()
