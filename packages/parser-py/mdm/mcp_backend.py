"""MDM MCP 서버가 사용하는 문서 파싱 백엔드.

우선순위:
1. ``mdm_core`` (PyO3 네이티브 확장, PyPI ``mdm-core``) — 설치돼 있으면 in-process 호출.
2. ``hwp2mdm`` Rust CLI 바이너리 (``core/`` 빌드 산출물) — 서브프로세스로 호출.
3. 둘 다 없으면 :class:`BackendUnavailableError`.

이 모듈은 ``mcp`` SDK에 의존하지 않는다 — MCP 서버 없이도(예: 단독 스크립트) 재사용 가능하고,
``mcp`` 패키지가 설치되지 않은 환경에서도 단위 테스트가 가능하도록 분리했다.
"""

from __future__ import annotations

import difflib
import json
import os
import shutil
import subprocess
import tempfile
from pathlib import Path
from typing import Any

ALLOWED_EXTENSIONS = {".hwp", ".hwpx", ".hml", ".pdf", ".xls", ".xlsx", ".docx"}
MAX_FILE_SIZE = 500 * 1024 * 1024  # 500MB — kordoc mcp.ts와 동일 상한


class BackendUnavailableError(RuntimeError):
    """mdm-core 네이티브 모듈도 hwp2mdm CLI 바이너리도 찾을 수 없을 때."""


class NotYetAvailableError(RuntimeError):
    """core에 아직 구현되지 않은 기능(스텁 도구)을 호출했을 때."""

    def __init__(self, tool: str, depends_on: str, message: str) -> None:
        self.tool = tool
        self.depends_on = depends_on
        super().__init__(f"[{tool}] 아직 사용할 수 없습니다 — {message} (연결 대기: {depends_on})")


def _try_import_mdm_core() -> Any:
    try:
        import mdm_core  # type: ignore[import-not-found]
    except ImportError:
        return None
    return mdm_core


_MDM_CORE = _try_import_mdm_core()


def mdm_core_available() -> bool:
    return _MDM_CORE is not None


def _find_core_binary() -> Path | None:
    env_override = os.environ.get("MDM_CORE_BIN")
    if env_override and Path(env_override).is_file():
        return Path(env_override)
    # packages/parser-py/mdm/mcp_backend.py -> repo root
    repo_root = Path(__file__).resolve().parents[3]
    for candidate in (
        repo_root / "core" / "target" / "release" / "hwp2mdm",
        repo_root / "core" / "target" / "debug" / "hwp2mdm",
    ):
        if candidate.is_file():
            return candidate
    which = shutil.which("hwp2mdm")
    return Path(which) if which else None


_CORE_BIN = _find_core_binary()


def core_binary_available() -> bool:
    return _CORE_BIN is not None


def backend_status() -> dict[str, Any]:
    """진단용 — 어떤 백엔드가 활성인지."""
    return {
        "mdm_core_native": mdm_core_available(),
        "hwp2mdm_cli": str(_CORE_BIN) if _CORE_BIN else None,
    }


def safe_path(file_path: str, allowed_exts: set[str] | None = None) -> Path:
    """경로 정규화 + 존재/확장자/크기 검증 (kordoc mcp.ts의 safePath 대응)."""
    if not file_path:
        raise ValueError("파일 경로가 비어있습니다")
    exts = ALLOWED_EXTENSIONS if allowed_exts is None else allowed_exts
    resolved = Path(file_path).expanduser().resolve()
    if not resolved.exists():
        raise FileNotFoundError(f"파일을 찾을 수 없습니다: {resolved}")
    if not resolved.is_file():
        raise ValueError(f"파일이 아니라 디렉토리입니다: {resolved}")
    ext = resolved.suffix.lower()
    if exts and ext not in exts:
        raise ValueError(f"지원하지 않는 확장자입니다: {ext} (허용: {', '.join(sorted(exts))})")
    size = resolved.stat().st_size
    if size > MAX_FILE_SIZE:
        raise ValueError(
            f"파일이 너무 큽니다: {size / 1024 / 1024:.1f}MB (최대 {MAX_FILE_SIZE // 1024 // 1024}MB)"
        )
    return resolved


def _run_cli(args: list[str], input_bytes: bytes | None = None, timeout: int = 120) -> subprocess.CompletedProcess:
    if _CORE_BIN is None:
        raise BackendUnavailableError(
            "mdm-core 네이티브 모듈과 hwp2mdm CLI 바이너리를 모두 찾을 수 없습니다. "
            "core/ 에서 `cargo build`를 실행하거나 MDM_CORE_BIN 환경변수로 바이너리 경로를 지정하세요."
        )
    return subprocess.run(
        [str(_CORE_BIN), *args],
        input=input_bytes,
        capture_output=True,
        timeout=timeout,
    )


def convert_to_markdown(file_path: str) -> str:
    """문서를 마크다운 본문으로 변환 (frontmatter 없음)."""
    resolved = safe_path(file_path)
    ext = resolved.suffix.lstrip(".").lower()
    if _MDM_CORE is not None:
        data = resolved.read_bytes()
        return str(_MDM_CORE.convert_bytes(data, resolved.name))
    proc = _run_cli(["stream", "--ext", ext, "--mode", "body"], input_bytes=resolved.read_bytes())
    if proc.returncode != 0:
        raise RuntimeError(f"변환 실패: {proc.stderr.decode('utf-8', 'replace').strip()}")
    return proc.stdout.decode("utf-8", "replace")


def get_document_info(file_path: str) -> dict[str, Any]:
    """포맷 감지 + 메타데이터. hwp2mdm CLI의 `info -f json`을 우선 사용하고,
    바이너리가 없으면 확장자 기반 최소 정보로 폴백한다."""
    resolved = safe_path(file_path)
    if _CORE_BIN is not None:
        proc = _run_cli(["info", str(resolved), "-f", "json"])
        if proc.returncode == 0:
            try:
                return dict(json.loads(proc.stdout.decode("utf-8", "replace")))
            except json.JSONDecodeError:
                pass  # 아래 폴백으로
    ext = resolved.suffix.lstrip(".").lower()
    stat = resolved.stat()
    return {
        "file": {
            "name": resolved.name,
            "path": str(resolved),
            "size": f"{stat.st_size / 1024:.2f} KB",
        },
        "document": {"format_guess": ext},
        "warning": "hwp2mdm CLI 바이너리를 찾을 수 없어 확장자 기반 추정치만 제공합니다 (매직바이트 미검증).",
    }


def detect_format(file_path: str) -> str:
    info = get_document_info(file_path)
    fmt = info.get("file", {}).get("format") or info.get("document", {}).get("format_guess")
    return str(fmt) if fmt else "unknown"


def extract_media(file_path: str, output_dir: str) -> list[dict[str, Any]]:
    """문서에서 이미지를 추출해 output_dir에 저장하고 파일 목록을 반환."""
    resolved = safe_path(file_path)
    out_dir = Path(output_dir).expanduser().resolve()
    out_dir.mkdir(parents=True, exist_ok=True)

    # mdm_core 0.1.0의 __init__.py는 extract_images를 top-level로 re-export하지 않는다
    # (packages/python/python/mdm_core/__init__.py — 이 패키지는 우리 작업 범위 밖).
    # 있으면 쓰고, 없으면 CLI로 폴백한다.
    extract_fn = getattr(_MDM_CORE, "extract_images", None)
    if extract_fn is None:
        native = getattr(_MDM_CORE, "_mdm_native", None)
        extract_fn = getattr(native, "extract_images", None) if native else None
    if extract_fn is not None:
        data = resolved.read_bytes()
        images: dict[str, bytes] = extract_fn(data, resolved.name)
        results = []
        for name, content in images.items():
            path = out_dir / name
            path.write_bytes(content)
            results.append({"filename": name, "path": str(path), "size": len(content)})
        return results

    proc = _run_cli(["images", str(resolved), "-o", str(out_dir)])
    if proc.returncode != 0:
        raise RuntimeError(f"이미지 추출 실패: {proc.stderr.decode('utf-8', 'replace').strip()}")
    results = []
    for path in sorted(out_dir.iterdir()):
        if path.is_file():
            results.append({"filename": path.name, "path": str(path), "size": path.stat().st_size})
    return results


def compare_documents(file_path_a: str, file_path_b: str) -> dict[str, Any]:
    """두 문서를 마크다운으로 변환 후 라인 단위로 비교.

    주의: kordoc의 compare_documents는 IRBlock 단위 구조적 diff(추가/삭제/변경 블록)를
    반환하지만, core Rust에는 아직 그런 API가 없다(gap-diff 작업 이후 대체 예정).
    현재는 difflib 기반 라인 unified diff로 즉시 동작하는 근사치를 제공한다.
    """
    md_a = convert_to_markdown(file_path_a)
    md_b = convert_to_markdown(file_path_b)
    lines_a = md_a.splitlines()
    lines_b = md_b.splitlines()

    diff_lines = list(
        difflib.unified_diff(lines_a, lines_b, fromfile=file_path_a, tofile=file_path_b, lineterm="")
    )

    matcher = difflib.SequenceMatcher(a=lines_a, b=lines_b)
    added = removed = unchanged = 0
    for tag, i1, i2, j1, j2 in matcher.get_opcodes():
        if tag == "equal":
            unchanged += i2 - i1
        elif tag == "insert":
            added += j2 - j1
        elif tag == "delete":
            removed += i2 - i1
        elif tag == "replace":
            removed += i2 - i1
            added += j2 - j1

    return {
        "stats": {
            "added": added,
            "removed": removed,
            "unchanged": unchanged,
            "similarity": round(matcher.ratio(), 4),
        },
        "diff": "\n".join(diff_lines),
    }


def _write_temp_markdown(text: str) -> str:
    """redact/lint/chunks 서브커맨드는 텍스트 입력만 받으므로 임시 .md 파일에 써서 넘긴다.
    호출자가 반드시 finally에서 삭제해야 한다."""
    with tempfile.NamedTemporaryFile(mode="w", suffix=".md", delete=False, encoding="utf-8") as f:
        f.write(text)
        return f.name


def redact_document(
    file_path: str,
    rules: str | None = None,
    output_path: str | None = None,
    dry_run: bool = False,
) -> str:
    """문서의 개인정보(주민번호·전화·이메일·카드·계좌 등)를 탐지해 마스킹한다.

    redact 서브커맨드는 텍스트 입력만 받으므로, 문서를 먼저 마크다운으로 변환한 뒤
    임시 파일로 넘긴다.
    """
    md = convert_to_markdown(file_path)
    tmp_md = _write_temp_markdown(md)
    try:
        proc = _run_cli(["redact", tmp_md, "-r", rules or "rrn,phone,email,card,account"])
        if proc.returncode != 0:
            raise RuntimeError(f"PII 마스킹 실패: {proc.stderr.decode('utf-8', 'replace').strip()}")
        masked = proc.stdout.decode("utf-8", "replace")
        if output_path and not dry_run:
            Path(output_path).write_text(masked, encoding="utf-8")
        return masked
    finally:
        Path(tmp_md).unlink(missing_ok=True)


def lint_document(file_path: str) -> str:
    """공문서 표기법(날짜·시간·금액·붙임 등)을 검수해 경고 목록을 반환한다."""
    md = convert_to_markdown(file_path)
    tmp_md = _write_temp_markdown(md)
    try:
        proc = _run_cli(["lint", tmp_md])
        if proc.returncode != 0:
            raise RuntimeError(f"공문서 표기법 린트 실패: {proc.stderr.decode('utf-8', 'replace').strip()}")
        return proc.stdout.decode("utf-8", "replace")
    finally:
        Path(tmp_md).unlink(missing_ok=True)


def parse_chunks(file_path: str, granularity: str = "section") -> str:
    """문서를 RAG용 구조 청크 JSON(문자열)으로 파싱한다 (헤딩/개조식 위계 breadcrumb 보존)."""
    md = convert_to_markdown(file_path)
    tmp_md = _write_temp_markdown(md)
    try:
        proc = _run_cli(["chunks", tmp_md, "-g", granularity])
        if proc.returncode != 0:
            raise RuntimeError(f"구조 청킹 실패: {proc.stderr.decode('utf-8', 'replace').strip()}")
        return proc.stdout.decode("utf-8", "replace")
    finally:
        Path(tmp_md).unlink(missing_ok=True)


def generate_document(
    markdown: str,
    output_path: str,
    preset: str | None = None,
    profile_path: str | None = None,
) -> str:
    """마크다운을 HWPX 한글 문서로 생성한다 (공문서 프리셋: 기안문/보고서/계획서/통지/회의록/개조식/보도자료).

    profile_path(표 서식 프로필)는 현재 hwp2mdm generate가 지원하지 않아 무시된다
    (연결 대기: gap-hwpxgen).
    """
    args = ["generate", "-", "-o", output_path]
    if preset:
        args += ["-p", preset]
    proc = _run_cli(args, input_bytes=markdown.encode("utf-8"))
    if proc.returncode != 0:
        raise RuntimeError(f"HWPX 생성 실패: {proc.stderr.decode('utf-8', 'replace').strip()}")
    size = os.path.getsize(output_path)
    note = ""
    if profile_path:
        note = " (profile_path는 현재 core CLI가 지원하지 않아 무시되었습니다 — 연결 대기: gap-hwpxgen)"
    return f"HWPX 생성 완료 → {output_path} ({size:,} bytes){note}"


def fill_form(
    file_path: str,
    fields: str,
    output_path: str | None = None,
    output_format: str = "markdown",
) -> str:
    """HWPX 서식 문서의 빈칸을 fields(JSON 문자열, 라벨→값 맵)로 채워 새 문서를 만든다.

    output_format은 현재 core CLI가 hwpx 바이너리 출력만 지원하므로 무시된다.
    """
    try:
        field_values = json.loads(fields)
    except json.JSONDecodeError as e:
        raise ValueError(f"fields는 유효한 JSON 문자열이어야 합니다: {e}") from e
    if not isinstance(field_values, dict):
        raise ValueError("fields는 JSON 객체(라벨→값 맵)여야 합니다")

    resolved = safe_path(file_path, {".hwpx"})

    with tempfile.NamedTemporaryFile(mode="w", suffix=".json", delete=False, encoding="utf-8") as f:
        json.dump(field_values, f, ensure_ascii=False)
        tmp_json = f.name

    try:
        args = ["fill", str(resolved), "-j", tmp_json]
        if output_path:
            args += ["-o", output_path]
        proc = _run_cli(args)
        if proc.returncode != 0:
            raise RuntimeError(f"서식 채우기 실패: {proc.stderr.decode('utf-8', 'replace').strip()}")

        if output_path:
            return f"서식 채우기 완료 → {output_path} ({os.path.getsize(output_path):,} bytes)"

        # -o 미지정 시 CLI가 바이너리 hwpx를 stdout으로 출력하므로 임시 파일에 저장해 경로를 알려준다.
        with tempfile.NamedTemporaryFile(suffix=".hwpx", delete=False) as out_f:
            out_f.write(proc.stdout)
            out_path = out_f.name
        return f"서식 채우기 완료 → {out_path} ({os.path.getsize(out_path):,} bytes)"
    finally:
        Path(tmp_json).unlink(missing_ok=True)


def parse_form(file_path: str) -> str:
    """HWPX 서식 문서에서 레이블-값 필드를 구조화된 JSON으로 추출한다.

    core CLI의 `fill --dry-run`이 감지된 폼 스키마(fields, confidence)를 JSON으로
    출력한다(문서를 채우지 않고 스키마만 반환).
    """
    resolved = safe_path(file_path, {".hwpx"})
    proc = _run_cli(["fill", str(resolved), "--dry-run"])
    if proc.returncode != 0:
        raise RuntimeError(f"서식 추출 실패: {proc.stderr.decode('utf-8', 'replace').strip()}")
    return proc.stdout.decode("utf-8", "replace")

def parse_table(file_path: str, table_index: int) -> str:
    """hwp2mdm inspect --format json 으로 표 구조 추출."""
    resolved = safe_path(file_path, {".hwp", ".hwpx"})
    proc = _run_cli(["inspect", str(resolved), "--format", "json"])
    if proc.returncode != 0:
        raise RuntimeError(f"표 추출 실패: {proc.stderr.decode('utf-8', 'replace').strip()}")
    data = json.loads(proc.stdout)
    tables = data.get("tables", [])
    if table_index < 0 or table_index >= len(tables):
        return json.dumps({"error": f"table_index {table_index} out of range (0-{len(tables)-1})", "total_tables": len(tables)}, ensure_ascii=False, indent=2)
    return json.dumps(tables[table_index], ensure_ascii=False, indent=2)


def validate_hwpx(file_path: str) -> str:
    """hwp2mdm validate 로 HWPX 검증."""
    resolved = safe_path(file_path, {".hwpx"})
    proc = _run_cli(["validate", str(resolved)])
    if proc.returncode != 0:
        raise RuntimeError(f"HWPX 검증 실패: {proc.stderr.decode('utf-8', 'replace').strip()}")
    return proc.stdout.decode("utf-8", "replace")


def hulk_to_latex(file_path: str) -> str:
    """hwp2mdm equation 으로 HULK→LaTeX 변환."""
    resolved = safe_path(file_path, {".hulk", ".txt", ".xml"})
    proc = _run_cli(["equation", str(resolved)])
    if proc.returncode != 0:
        raise RuntimeError(f"수식 변환 실패: {proc.stderr.decode('utf-8', 'replace').strip()}")
    return proc.stdout.decode("utf-8", "replace")


def ocr_document(file_path: str, output_path: str | None = None) -> str:
    """hwp2mdm convert --ocr 로 OCR 포함 변환."""
    resolved = safe_path(file_path, {".pdf"})
    args = ["convert", str(resolved), "--ocr"]
    if output_path:
        args += ["-o", output_path]
    else:
        with tempfile.TemporaryDirectory() as tmpdir:
            args += ["-o", tmpdir]
            proc = _run_cli(args)
            if proc.returncode != 0:
                raise RuntimeError(f"OCR 변환 실패: {proc.stderr.decode('utf-8', 'replace').strip()}")
            mdx_files = list(Path(tmpdir).glob("*.mdx"))
            if mdx_files:
                return mdx_files[0].read_text(encoding="utf-8")
            return "OCR 변환 완료 (출력 파일을 찾을 수 없습니다)"
    proc = _run_cli(args)
    if proc.returncode != 0:
        raise RuntimeError(f"OCR 변환 실패: {proc.stderr.decode('utf-8', 'replace').strip()}")
    return f"OCR 변환 완료 → {output_path}" if output_path else proc.stdout.decode("utf-8", "replace")
