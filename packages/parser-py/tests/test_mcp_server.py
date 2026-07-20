"""mdm.mcp_server 테스트 — 도구 등록, 실동작 도구, 스텁 도구의 에러 응답."""
from __future__ import annotations

import asyncio
import sys
from pathlib import Path

import pytest

pytest.importorskip("mcp", reason="mcp SDK 미설치 — [mcp] extra 필요 (pip install mdm-parser[mcp])")

REPO_ROOT = Path(__file__).resolve().parent.parent.parent.parent
sys.path.insert(0, str(REPO_ROOT / "packages" / "parser-py"))

from mcp.server.fastmcp.exceptions import ToolError  # noqa: E402

from mdm import mcp_backend as backend  # noqa: E402
from mdm import mcp_server as server  # noqa: E402

SAMPLE_PDF = REPO_ROOT / "tests" / "pdf_benchmark" / "test_headings.pdf"

REAL_TOOLS = {
    "parse_document",
    "convert_to_markdown",
    "detect_format",
    "get_document_info",
    "parse_metadata",
    "extract_media",
    "compare_documents",
    "redact_document",
    "lint_document",
    "parse_chunks",
    "generate_document",
    "fill_form",
    "parse_form",
    "validate_hwpx",
    "hulk_to_latex",
    "ocr_document",
}

STUB_TOOLS_KWARGS = {
    "parse_pages": {"file_path": str(SAMPLE_PDF), "pages": "1-2"},
    "parse_table": {"file_path": str(SAMPLE_PDF), "table_index": 0},
    "place_seal": {"file_path": str(SAMPLE_PDF), "image_path": "/tmp/seal.png"},
    "patch_document": {"file_path": str(SAMPLE_PDF), "edited_markdown": "# x", "output_path": "/tmp/out.hwpx"},
    "render_document": {"file_path": str(SAMPLE_PDF)},
    "extract_profile": {"hwpx_path": str(SAMPLE_PDF), "output_path": "/tmp/profile.json"},
}


def _require_fixture():
    if not SAMPLE_PDF.exists():
        pytest.skip(f"fixture 없음: {SAMPLE_PDF}")


def _require_backend():
    if not backend.mdm_core_available() and not backend.core_binary_available():
        pytest.skip("백엔드(mdm_core/hwp2mdm) 미구축 환경")


class TestToolRegistration:
    def test_tool_count_and_names(self):
        tools = asyncio.run(server.mcp.list_tools())
        names = {t.name for t in tools}
        assert len(tools) == 22
        assert names >= REAL_TOOLS
        assert set(STUB_TOOLS_KWARGS.keys()) <= names
        # 레퍼런스(kkdoc/src/mcp.ts) 15개 도구 이름과 호환
        reference_names = {
            "parse_document", "detect_format", "parse_metadata", "parse_pages",
            "parse_table", "compare_documents", "parse_form", "fill_form",
            "place_seal", "patch_document", "render_document", "redact_document",
            "parse_chunks", "extract_profile", "generate_document",
        }
        assert reference_names <= names

    def test_every_tool_has_description(self):
        tools = asyncio.run(server.mcp.list_tools())
        for t in tools:
            assert t.description, f"{t.name} 도구에 설명(docstring)이 없습니다"


class TestRealTools:
    def test_convert_to_markdown(self):
        _require_backend()
        _require_fixture()
        md = server.convert_to_markdown(str(SAMPLE_PDF))
        assert isinstance(md, str) and md.strip()

    def test_detect_format(self):
        _require_fixture()
        result = server.detect_format(str(SAMPLE_PDF))
        assert str(SAMPLE_PDF) in result

    def test_get_document_info_is_json(self):
        _require_fixture()
        import json

        result = server.get_document_info(str(SAMPLE_PDF))
        parsed = json.loads(result)
        assert isinstance(parsed, dict)

    def test_parse_metadata_includes_format_key(self):
        _require_fixture()
        import json

        result = server.parse_metadata(str(SAMPLE_PDF))
        parsed = json.loads(result)
        assert "format" in parsed

    def test_parse_document_has_meta_header(self):
        _require_backend()
        _require_fixture()
        result = server.parse_document(str(SAMPLE_PDF))
        assert result.startswith("[포맷:")

    def test_parse_document_ocr_true_adds_warning(self):
        _require_backend()
        _require_fixture()
        result = server.parse_document(str(SAMPLE_PDF), ocr=True)
        assert "OCR" in result and "gap-ocr" in result

    def test_extract_media_no_crash(self, tmp_path):
        _require_backend()
        _require_fixture()
        result = server.extract_media(str(SAMPLE_PDF), str(tmp_path))
        assert isinstance(result, str)

    def test_compare_documents_identical(self):
        _require_backend()
        _require_fixture()
        result = server.compare_documents(str(SAMPLE_PDF), str(SAMPLE_PDF))
        assert "100.0%" in result

    def test_real_tool_missing_file_raises(self):
        with pytest.raises(FileNotFoundError):
            server.convert_to_markdown("/no/such/file.pdf")


class TestWiredCliTools:
    """redact_document/lint_document/parse_chunks/generate_document/fill_form —
    hwp2mdm CLI 서브커맨드에 새로 연결된 5개 도구. 바이너리가 있어야 실행되므로 스킵 가능."""

    def test_redact_document(self):
        _require_backend()
        _require_fixture()
        result = server.redact_document(str(SAMPLE_PDF))
        assert isinstance(result, str)

    def test_lint_document(self):
        _require_backend()
        _require_fixture()
        result = server.lint_document(str(SAMPLE_PDF))
        assert isinstance(result, str) and result.strip()

    def test_parse_chunks(self):
        _require_backend()
        _require_fixture()
        result = server.parse_chunks(str(SAMPLE_PDF))
        assert isinstance(result, str)
        import json

        json.loads(result)  # chunks 서브커맨드는 JSON 배열을 출력해야 함

    def test_generate_document(self, tmp_path):
        _require_backend()
        out = tmp_path / "out.hwpx"
        result = server.generate_document("# 제목\n\n본문", str(out))
        assert out.exists()
        assert str(out) in result

    def test_fill_form_not_a_hwpx_raises(self):
        # safe_path 검증이 CLI 호출보다 먼저 실패하므로 바이너리 없이도 검증 가능
        _require_fixture()
        with pytest.raises(ValueError, match="확장자"):
            server.fill_form(str(SAMPLE_PDF), "{}")


class TestStubTools:
    @pytest.mark.parametrize("tool_name", sorted(STUB_TOOLS_KWARGS))
    def test_stub_raises_not_yet_available(self, tool_name):
        fn = getattr(server, tool_name)
        kwargs = STUB_TOOLS_KWARGS[tool_name]
        with pytest.raises(backend.NotYetAvailableError) as exc_info:
            fn(**kwargs)
        message = str(exc_info.value)
        assert tool_name in message
        assert "연결 대기" in message

    def test_stub_tool_via_mcp_call_tool_surfaces_tool_error(self):
        with pytest.raises(ToolError):
            asyncio.run(server.mcp.call_tool("render_document", {"file_path": str(SAMPLE_PDF)}))

    def test_real_tool_via_mcp_call_tool_succeeds(self):
        _require_fixture()
        result = asyncio.run(server.mcp.call_tool("detect_format", {"file_path": str(SAMPLE_PDF)}))
        content, _structured = result
        assert content[0].text.endswith(": pdf") or "unknown" in content[0].text


class TestEntryPoint:
    def test_main_is_callable(self):
        assert callable(server.main)
