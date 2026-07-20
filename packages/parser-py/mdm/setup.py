"""AI 클라이언트 자동 설치 — mdm MCP 서버를 감지된 클라이언트 설정에 등록.

Ported from kkdoc (MIT): reference/kkdoc/src/setup.ts

`mdm-setup` 콘솔 스크립트로 실행. API 키 불필요, macOS / Linux / Windows 공용.
등록 대상은 8종 클라이언트: Claude Desktop, Claude Code(현재 디렉토리), Cursor,
VS Code(현재 디렉토리), Windsurf, Gemini CLI, Zed, Antigravity.

kkdoc 원본과의 차이:
- 서버 실행 커맨드는 `npx -y kordoc mcp` 대신 `uvx --from mdm-parser mdm-mcp` —
  mdm-mcp는 PyPI 배포 시 별도 설치 없이 uvx로 실행 가능한 파이썬 콘솔 스크립트다.
  `--from mdm-parser`가 필요한 이유: PyPI 배포명(``mdm-parser``, pyproject.toml
  [project].name)과 콘솔 스크립트명(``mdm-mcp``)이 다르기 때문 — uvx는 기본적으로
  배포명으로 조회한다.
- 배너/타자기 애니메이션 등 장식적 CLI 연출은 이식하지 않았다(핵심 기능이 아님).
- kkdoc에는 없는 안전장치로 백업(``<path>.bak``) 생성과 ``--dry-run``을 추가했다.
"""
from __future__ import annotations

import argparse
import json
import os
import platform
import shutil
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import cast

SERVER_NAME = "mdm"


@dataclass(frozen=True)
class ClientConfig:
    """감지된 MCP 클라이언트 하나. `format`은 그 클라이언트의 설정 파일에서
    MCP 서버 목록이 저장되는 최상위 키다."""

    name: str
    config_path: Path
    format: str  # "mcpServers" | "servers" | "context_servers"


def _appdata_dir(home: Path) -> Path:
    appdata = os.environ.get("APPDATA")
    return Path(appdata) if appdata else home / "AppData/Roaming"


def detect_clients(home: Path | None = None, system: str | None = None, cwd: Path | None = None) -> list[ClientConfig]:
    """설치 가능한 MCP 클라이언트와 설정 파일 경로를 감지한다.

    `home`/`system`/`cwd`는 테스트에서 실제 홈 디렉토리/OS 없이 검증하기 위한
    주입 지점이다 — 기본값은 각각 `Path.home()`, `platform.system()`, `Path.cwd()`.
    """
    home = home or Path.home()
    system = system or platform.system()  # "Darwin" | "Windows" | "Linux"
    cwd = cwd or Path.cwd()

    clients: list[ClientConfig] = []

    claude_desktop_paths = {
        "Darwin": home / "Library/Application Support/Claude/claude_desktop_config.json",
        "Windows": _appdata_dir(home) / "Claude/claude_desktop_config.json",
        "Linux": home / ".config/Claude/claude_desktop_config.json",
    }
    claude_desktop_path = claude_desktop_paths.get(system)
    if claude_desktop_path is not None:
        clients.append(ClientConfig("Claude Desktop", claude_desktop_path, "mcpServers"))

    clients.append(ClientConfig("Claude Code (현재 디렉토리)", cwd / ".mcp.json", "mcpServers"))
    clients.append(ClientConfig("Cursor", home / ".cursor/mcp.json", "mcpServers"))
    clients.append(ClientConfig("VS Code (현재 디렉토리)", cwd / ".vscode/mcp.json", "servers"))
    clients.append(ClientConfig("Windsurf", home / ".codeium/windsurf/mcp_config.json", "mcpServers"))
    clients.append(ClientConfig("Gemini CLI", home / ".gemini/settings.json", "mcpServers"))

    zed_paths = {
        "Darwin": home / ".zed/settings.json",
        "Linux": home / ".config/zed/settings.json",
        "Windows": home / ".zed/settings.json",
    }
    zed_path = zed_paths.get(system)
    if zed_path is not None:
        clients.append(ClientConfig("Zed", zed_path, "context_servers"))

    clients.append(ClientConfig("Antigravity", home / ".gemini/antigravity/mcp_config.json", "mcpServers"))

    return clients


def build_server_entry(system: str | None = None) -> dict:
    """mcpServers/servers 키에 들어갈 서버 등록 항목.

    Windows에서는 Claude Desktop 등이 `uvx.exe`/`uvx.cmd`를 직접 해석하지 못해
    `cmd /c` 래핑이 필요하다 (kkdoc의 npx.cmd 이슈와 동일한 근본 원인)."""
    system = system or platform.system()
    args = ["--from", "mdm-parser", "mdm-mcp"]
    if system == "Windows":
        return {"command": "cmd", "args": ["/c", "uvx", *args]}
    return {"command": "uvx", "args": args}


def build_zed_entry(system: str | None = None) -> dict:
    """Zed의 `context_servers`는 `{command: {path, args}}` 형태로 감싼다."""
    system = system or platform.system()
    args = ["--from", "mdm-parser", "mdm-mcp"]
    if system == "Windows":
        inner = {"path": "cmd", "args": ["/c", "uvx", *args]}
    else:
        inner = {"path": "uvx", "args": args}
    return {"command": inner}


def read_json_file(path: Path) -> dict:
    if not path.exists():
        return {}
    return cast(dict, json.loads(path.read_text(encoding="utf-8")))


def backup_path_for(path: Path) -> Path:
    return path.with_name(path.name + ".bak")


def write_json_file(path: Path, data: dict, *, backup: bool = True, dry_run: bool = False) -> Path | None:
    """설정 파일을 쓴다.

    `backup=True`이고 기존 파일이 있으면 덮어쓰기 전에 `<path>.bak`으로 복사한다
    (반환값이 백업 경로). `dry_run=True`면 아무것도 쓰지 않고 항상 `None`을 반환한다.
    """
    if dry_run:
        return None

    path.parent.mkdir(parents=True, exist_ok=True)

    backup_file: Path | None = None
    if backup and path.exists():
        backup_file = backup_path_for(path)
        shutil.copy2(path, backup_file)

    path.write_text(json.dumps(data, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
    return backup_file


def inject_client(
    client: ClientConfig, *, backup: bool = True, dry_run: bool = False, system: str | None = None
) -> tuple[bool, str | None]:
    """단일 클라이언트 설정 파일에 mdm MCP 서버를 등록한다.

    기존 설정(다른 MCP 서버 등록, 기타 키)은 그대로 보존하고 `SERVER_NAME` 키만
    추가/갱신한다(merge). `(성공 여부, 실패 시 에러 메시지)`를 반환한다."""
    try:
        config = read_json_file(client.config_path)
        key = client.format
        entry = build_zed_entry(system) if key == "context_servers" else build_server_entry(system)

        servers = config.get(key)
        if not isinstance(servers, dict):
            servers = {}
        servers[SERVER_NAME] = entry
        config[key] = servers

        write_json_file(client.config_path, config, backup=backup, dry_run=dry_run)
        return True, None
    except Exception as e:  # noqa: BLE001 — surfaced to the caller as a report line, not swallowed
        return False, str(e)


def print_manual_config(system: str | None = None) -> None:
    entry = build_server_entry(system)
    print()
    print("  아래 JSON을 설정 파일의 mcpServers에 추가하세요:")
    print()
    print(f'  "{SERVER_NAME}": {json.dumps(entry, indent=4, ensure_ascii=False)}')
    print()


def parse_indices(raw: str, count: int) -> list[int]:
    """"1,3" 같은 쉼표구분 입력을 유효한 0-기반 인덱스 목록으로 변환한다.
    범위 밖/숫자가 아닌 항목은 조용히 걸러진다(kkdoc와 동일한 관용)."""
    indices: list[int] = []
    for part in raw.split(","):
        part = part.strip()
        if not part:
            continue
        try:
            idx = int(part) - 1
        except ValueError:
            continue
        if 0 <= idx < count:
            indices.append(idx)
    return indices


def run_setup(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(prog="mdm-setup", description="mdm MCP 서버를 감지된 AI 클라이언트에 자동 등록")
    parser.add_argument("--clients", help="쉼표로 구분된 클라이언트 번호 (예: 1,3) — 미지정 시 대화형 프롬프트")
    parser.add_argument("--dry-run", action="store_true", help="실제로 파일을 쓰지 않고 계획만 출력")
    parser.add_argument("--no-backup", action="store_true", help="기존 설정 파일 백업(.bak)을 생략")
    args = parser.parse_args(argv)

    clients = detect_clients()

    print()
    print("[1/2] MCP 클라이언트 선택")
    print()
    for i, c in enumerate(clients, start=1):
        badge = " [감지됨]" if c.config_path.exists() else ""
        print(f"  {i:>2}) {c.name}{badge}")

    if args.clients is not None:
        raw = args.clients
    else:
        try:
            raw = input("\n> 번호 (예: 1,3): ").strip()
        except EOFError:
            raw = ""

    if not raw:
        print("\n선택 없음 — 수동 설정 안내:")
        print_manual_config()
        return 0

    indices = parse_indices(raw, len(clients))
    if not indices:
        print("\n유효한 선택 없음 — 수동 설정 안내:")
        print_manual_config()
        return 0

    print()
    print("[2/2] 설정 파일 업데이트")
    print()

    any_failed = False
    for idx in indices:
        client = clients[idx]
        ok, err = inject_client(client, backup=not args.no_backup, dry_run=args.dry_run)
        if ok:
            suffix = " (dry-run)" if args.dry_run else ""
            print(f"  + {client.name}{suffix} — {client.config_path}")
        else:
            any_failed = True
            print(f"  x {client.name} — {err}")

    print()
    if not args.dry_run:
        print("  클라이언트를 재시작하면 'mdm' MCP 서버가 활성화됩니다.")
    print()

    return 1 if any_failed else 0


def main() -> None:
    sys.exit(run_setup())


if __name__ == "__main__":
    main()
