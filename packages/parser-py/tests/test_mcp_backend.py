"""mdm.mcp_backend 단위 테스트 — mcp SDK에 의존하지 않는 순수 백엔드 로직."""
from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path

import pytest

REPO_ROOT = Path(__file__).resolve().parent.parent.parent.parent
sys.path.insert(0, str(REPO_ROOT / "packages" / "parser-py"))

from mdm import mcp_backend as backend  # noqa: E402

SAMPLE_PDF = REPO_ROOT / "tests" / "pdf_benchmark" / "test_headings.pdf"
SAMPLE_PDF_2 = REPO_ROOT / "tests" / "pdf_benchmark" / "test_twocolumn.pdf"


def _require_backend():
    if not backend.mdm_core_available() and not backend.core_binary_available():
        pytest.skip("mdm_core 네이티브 모듈도 hwp2mdm CLI 바이너리도 없음 — 백엔드 미구축 환경")


def _require_fixture(path: Path):
    if not path.exists():
        pytest.skip(f"fixture 없음: {path}")


class TestSafePath:
    def test_missing_file_raises(self):
        with pytest.raises(FileNotFoundError):
            backend.safe_path("/no/such/file.pdf")

    def test_empty_path_raises(self):
        with pytest.raises(ValueError):
            backend.safe_path("")

    def test_disallowed_extension_raises(self, tmp_path):
        f = tmp_path / "note.txt"
        f.write_text("hi")
        with pytest.raises(ValueError, match="확장자"):
            backend.safe_path(str(f))

    def test_allowed_extension_ok(self, tmp_path):
        f = tmp_path / "doc.pdf"
        f.write_bytes(b"%PDF-1.4\n")
        resolved = backend.safe_path(str(f))
        assert resolved == f.resolve()

    def test_directory_raises(self, tmp_path):
        with pytest.raises(ValueError, match="디렉토리"):
            backend.safe_path(str(tmp_path), allowed_exts=set())


class TestBackendStatus:
    def test_backend_status_shape(self):
        status = backend.backend_status()
        assert "mdm_core_native" in status
        assert "hwp2mdm_cli" in status
        assert isinstance(status["mdm_core_native"], bool)


class TestConvertAndInfo:
    def test_convert_to_markdown(self):
        _require_backend()
        _require_fixture(SAMPLE_PDF)
        md = backend.convert_to_markdown(str(SAMPLE_PDF))
        assert "Main Title" in md or "# " in md
        assert isinstance(md, str) and md.strip()

    def test_get_document_info(self):
        _require_fixture(SAMPLE_PDF)
        info = backend.get_document_info(str(SAMPLE_PDF))
        assert isinstance(info, dict)
        # 백엔드가 전혀 없으면 폴백 dict(warning 필드)라도 반환해야 한다
        assert "file" in info or "warning" in info

    def test_detect_format(self):
        _require_fixture(SAMPLE_PDF)
        fmt = backend.detect_format(str(SAMPLE_PDF))
        assert fmt in {"pdf", "unknown"}
        if backend.core_binary_available():
            assert fmt == "pdf"

    def test_convert_unreadable_path_raises(self):
        with pytest.raises(FileNotFoundError):
            backend.convert_to_markdown("/no/such/file.pdf")


class TestExtractMedia:
    def test_extract_media_no_crash(self, tmp_path):
        _require_backend()
        _require_fixture(SAMPLE_PDF)
        results = backend.extract_media(str(SAMPLE_PDF), str(tmp_path))
        assert isinstance(results, list)
        for item in results:
            assert {"filename", "path", "size"} <= item.keys()


class TestCompareDocuments:
    def test_compare_identical(self):
        _require_backend()
        _require_fixture(SAMPLE_PDF)
        result = backend.compare_documents(str(SAMPLE_PDF), str(SAMPLE_PDF))
        assert result["stats"]["similarity"] == 1.0
        assert result["stats"]["added"] == 0
        assert result["stats"]["removed"] == 0

    def test_compare_different(self):
        _require_backend()
        _require_fixture(SAMPLE_PDF)
        _require_fixture(SAMPLE_PDF_2)
        result = backend.compare_documents(str(SAMPLE_PDF), str(SAMPLE_PDF_2))
        assert result["stats"]["similarity"] < 1.0
        assert "diff" in result and result["diff"]
        json.dumps(result)  # JSON 직렬화 가능해야 함 (MCP 응답 조립에 사용)


class TestNotYetAvailableError:
    def test_message_contains_tool_and_dependency(self):
        err = backend.NotYetAvailableError("foo_tool", "gap-foo", "설명")
        assert "foo_tool" in str(err)
        assert "gap-foo" in str(err)
        assert "설명" in str(err)


# ─── 새로 연결된 5개 도구의 backend 함수 — _run_cli를 가짜로 바꿔치기해서
#     바이너리 없이도 CLI 인자 구성이 옳은지 검증한다 ────────────────────────


def _fake_completed(args, returncode=0, stdout=b"", stderr=b""):
    return subprocess.CompletedProcess(args, returncode, stdout=stdout, stderr=stderr)


class TestRedactDocumentWiring:
    def test_args_and_default_rules(self, monkeypatch, tmp_path):
        captured = {}

        def fake_run_cli(args, input_bytes=None, timeout=120):
            captured["args"] = args
            assert Path(args[1]).exists()  # temp .md 파일이 실제로 존재해야 함
            return _fake_completed(args, stdout=b"masked-text")

        monkeypatch.setattr(backend, "convert_to_markdown", lambda p: "주민번호 900101-1234567")
        monkeypatch.setattr(backend, "_run_cli", fake_run_cli)

        result = backend.redact_document(str(tmp_path))
        assert captured["args"][0] == "redact"
        assert captured["args"][2:4] == ["-r", "rrn,phone,email,card,account"]
        assert result == "masked-text"
        assert not Path(captured["args"][1]).exists()  # 임시파일 정리 확인

    def test_custom_rules_passed_through(self, monkeypatch, tmp_path):
        captured = {}

        def fake_run_cli(args, input_bytes=None, timeout=120):
            captured["args"] = args
            return _fake_completed(args, stdout=b"masked")

        monkeypatch.setattr(backend, "convert_to_markdown", lambda p: "text")
        monkeypatch.setattr(backend, "_run_cli", fake_run_cli)

        backend.redact_document(str(tmp_path), rules="rrn,phone")
        assert captured["args"][2:4] == ["-r", "rrn,phone"]

    def test_output_path_written_unless_dry_run(self, monkeypatch, tmp_path):
        monkeypatch.setattr(backend, "convert_to_markdown", lambda p: "text")
        monkeypatch.setattr(backend, "_run_cli", lambda args, input_bytes=None, timeout=120: _fake_completed(args, stdout=b"masked-out"))

        out = tmp_path / "out.txt"
        backend.redact_document(str(tmp_path), output_path=str(out))
        assert out.read_text(encoding="utf-8") == "masked-out"

    def test_dry_run_skips_output_write(self, monkeypatch, tmp_path):
        monkeypatch.setattr(backend, "convert_to_markdown", lambda p: "text")
        monkeypatch.setattr(backend, "_run_cli", lambda args, input_bytes=None, timeout=120: _fake_completed(args, stdout=b"masked-out"))

        out = tmp_path / "out.txt"
        backend.redact_document(str(tmp_path), output_path=str(out), dry_run=True)
        assert not out.exists()

    def test_nonzero_returncode_raises(self, monkeypatch, tmp_path):
        monkeypatch.setattr(backend, "convert_to_markdown", lambda p: "text")
        monkeypatch.setattr(
            backend, "_run_cli",
            lambda args, input_bytes=None, timeout=120: _fake_completed(args, returncode=1, stderr=b"boom"),
        )
        with pytest.raises(RuntimeError, match="boom"):
            backend.redact_document(str(tmp_path))


class TestLintDocumentWiring:
    def test_args(self, monkeypatch, tmp_path):
        captured = {}

        def fake_run_cli(args, input_bytes=None, timeout=120):
            captured["args"] = args
            return _fake_completed(args, stdout="문제 없음".encode())

        monkeypatch.setattr(backend, "convert_to_markdown", lambda p: "본문")
        monkeypatch.setattr(backend, "_run_cli", fake_run_cli)

        result = backend.lint_document(str(tmp_path))
        assert captured["args"][0] == "lint"
        assert result == "문제 없음"
        assert not Path(captured["args"][1]).exists()

    def test_nonzero_returncode_raises(self, monkeypatch, tmp_path):
        monkeypatch.setattr(backend, "convert_to_markdown", lambda p: "본문")
        monkeypatch.setattr(
            backend, "_run_cli",
            lambda args, input_bytes=None, timeout=120: _fake_completed(args, returncode=1, stderr=b"lint-fail"),
        )
        with pytest.raises(RuntimeError, match="lint-fail"):
            backend.lint_document(str(tmp_path))


class TestParseChunksWiring:
    def test_args_default_granularity(self, monkeypatch, tmp_path):
        captured = {}

        def fake_run_cli(args, input_bytes=None, timeout=120):
            captured["args"] = args
            return _fake_completed(args, stdout=b"[]")

        monkeypatch.setattr(backend, "convert_to_markdown", lambda p: "본문")
        monkeypatch.setattr(backend, "_run_cli", fake_run_cli)

        result = backend.parse_chunks(str(tmp_path))
        assert captured["args"][0] == "chunks"
        assert captured["args"][2:4] == ["-g", "section"]
        assert result == "[]"

    def test_granularity_passed_through(self, monkeypatch, tmp_path):
        captured = {}

        def fake_run_cli(args, input_bytes=None, timeout=120):
            captured["args"] = args
            return _fake_completed(args, stdout=b"[]")

        monkeypatch.setattr(backend, "convert_to_markdown", lambda p: "본문")
        monkeypatch.setattr(backend, "_run_cli", fake_run_cli)

        backend.parse_chunks(str(tmp_path), granularity="block")
        assert captured["args"][2:4] == ["-g", "block"]


class TestGenerateDocumentWiring:
    def test_args_without_preset(self, monkeypatch, tmp_path):
        captured = {}
        out = tmp_path / "out.hwpx"
        out.write_bytes(b"x" * 10)  # os.path.getsize용

        def fake_run_cli(args, input_bytes=None, timeout=120):
            captured["args"] = args
            captured["input_bytes"] = input_bytes
            return _fake_completed(args)

        monkeypatch.setattr(backend, "_run_cli", fake_run_cli)

        result = backend.generate_document("# 제목", str(out))
        assert captured["args"] == ["generate", "-", "-o", str(out)]
        assert captured["input_bytes"] == "# 제목".encode()
        assert str(out) in result

    def test_preset_appended(self, monkeypatch, tmp_path):
        captured = {}
        out = tmp_path / "out.hwpx"
        out.write_bytes(b"x")

        def fake_run_cli(args, input_bytes=None, timeout=120):
            captured["args"] = args
            return _fake_completed(args)

        monkeypatch.setattr(backend, "_run_cli", fake_run_cli)
        backend.generate_document("# 제목", str(out), preset="기안문")
        assert captured["args"] == ["generate", "-", "-o", str(out), "-p", "기안문"]

    def test_nonzero_returncode_raises(self, monkeypatch, tmp_path):
        out = tmp_path / "out.hwpx"
        monkeypatch.setattr(
            backend, "_run_cli",
            lambda args, input_bytes=None, timeout=120: _fake_completed(args, returncode=1, stderr=b"gen-fail"),
        )
        with pytest.raises(RuntimeError, match="gen-fail"):
            backend.generate_document("# x", str(out))


class TestFillFormWiring:
    def _make_hwpx(self, tmp_path) -> Path:
        f = tmp_path / "template.hwpx"
        f.write_bytes(b"PK\x03\x04fake-hwpx")
        return f

    def test_invalid_json_raises_value_error(self, tmp_path):
        template = self._make_hwpx(tmp_path)
        with pytest.raises(ValueError):
            backend.fill_form(str(template), "not json")

    def test_non_object_json_raises_value_error(self, tmp_path):
        template = self._make_hwpx(tmp_path)
        with pytest.raises(ValueError):
            backend.fill_form(str(template), "[1, 2, 3]")

    def test_args_with_output_path(self, monkeypatch, tmp_path):
        template = self._make_hwpx(tmp_path)
        out = tmp_path / "out.hwpx"
        captured = {}

        def fake_run_cli(args, input_bytes=None, timeout=120):
            captured["args"] = args
            out.write_bytes(b"filled")  # CLI가 -o로 직접 파일을 씀
            return _fake_completed(args)

        monkeypatch.setattr(backend, "_run_cli", fake_run_cli)
        result = backend.fill_form(str(template), '{"성명": "홍길동"}', output_path=str(out))
        assert captured["args"][0] == "fill"
        assert captured["args"][1] == str(template)
        assert captured["args"][2] == "-j"
        assert Path(captured["args"][3]).name.endswith(".json")
        assert captured["args"][4:6] == ["-o", str(out)]
        assert str(out) in result
        assert not Path(captured["args"][3]).exists()  # 임시 json 정리 확인

    def test_no_output_path_writes_temp_hwpx(self, monkeypatch, tmp_path):
        template = self._make_hwpx(tmp_path)

        def fake_run_cli(args, input_bytes=None, timeout=120):
            return _fake_completed(args, stdout=b"binary-hwpx-bytes")

        monkeypatch.setattr(backend, "_run_cli", fake_run_cli)
        result = backend.fill_form(str(template), "{}")
        assert "bytes" in result

    def test_non_hwpx_template_raises(self, tmp_path):
        f = tmp_path / "doc.pdf"
        f.write_bytes(b"%PDF-1.4\n")
        with pytest.raises(ValueError, match="확장자"):
            backend.fill_form(str(f), "{}")

    def test_nonzero_returncode_raises(self, monkeypatch, tmp_path):
        template = self._make_hwpx(tmp_path)
        monkeypatch.setattr(
            backend, "_run_cli",
            lambda args, input_bytes=None, timeout=120: _fake_completed(args, returncode=1, stderr=b"fill-fail"),
        )
        with pytest.raises(RuntimeError, match="fill-fail"):
            backend.fill_form(str(template), "{}")


class TestParseFormWiring:
    def _make_hwpx(self, tmp_path) -> Path:
        f = tmp_path / "template.hwpx"
        f.write_bytes(b"PK\x03\x04fake-hwpx")
        return f

    def test_args_dry_run(self, monkeypatch, tmp_path):
        template = self._make_hwpx(tmp_path)
        captured = {}

        def fake_run_cli(args, input_bytes=None, timeout=120):
            captured["args"] = args
            return _fake_completed(args, stdout=b'{"fields": [], "confidence": 1.0}')

        monkeypatch.setattr(backend, "_run_cli", fake_run_cli)
        result = backend.parse_form(str(template))
        assert captured["args"] == ["fill", str(template), "--dry-run"]
        assert "fields" in result

    def test_non_hwpx_raises(self, tmp_path):
        f = tmp_path / "doc.pdf"
        f.write_bytes(b"%PDF-1.4\n")
        with pytest.raises(ValueError, match="확장자"):
            backend.parse_form(str(f))

    def test_nonzero_returncode_raises(self, monkeypatch, tmp_path):
        template = self._make_hwpx(tmp_path)
        monkeypatch.setattr(
            backend, "_run_cli",
            lambda args, input_bytes=None, timeout=120: _fake_completed(args, returncode=1, stderr=b"extract-fail"),
        )
        with pytest.raises(RuntimeError, match="extract-fail"):
            backend.parse_form(str(template))
