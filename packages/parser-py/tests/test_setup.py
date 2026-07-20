"""Tests for mdm.setup — AI client MCP registration.

Uses a fake home directory (tmp_path) and an explicit `system=` override so
these run identically on macOS/Linux/Windows CI without touching the real
user's actual client configs.
"""
from __future__ import annotations

import json
import sys
from pathlib import Path

import pytest

# Put parser-py on sys.path (no package install needed for CI smoke tests) —
# same pattern as tests/test_triage_router.py.
REPO_ROOT = Path(__file__).resolve().parent.parent.parent.parent
sys.path.insert(0, str(REPO_ROOT / "packages" / "parser-py"))

from mdm import setup as mdm_setup  # noqa: E402

# ─── detect_clients ─────────────────────────────────────────────────────


def test_detect_clients_darwin_count_and_paths(tmp_path):
    clients = mdm_setup.detect_clients(home=tmp_path, system="Darwin", cwd=tmp_path)
    names = [c.name for c in clients]

    assert len(clients) == 8
    assert "Claude Desktop" in names
    assert "Zed" in names

    claude = next(c for c in clients if c.name == "Claude Desktop")
    assert claude.config_path == tmp_path / "Library/Application Support/Claude/claude_desktop_config.json"
    assert claude.format == "mcpServers"

    zed = next(c for c in clients if c.name == "Zed")
    assert zed.config_path == tmp_path / ".zed/settings.json"
    assert zed.format == "context_servers"


def test_detect_clients_windows_uses_appdata(tmp_path, monkeypatch):
    monkeypatch.setenv("APPDATA", str(tmp_path / "Roaming"))
    clients = mdm_setup.detect_clients(home=tmp_path, system="Windows", cwd=tmp_path)
    claude = next(c for c in clients if c.name == "Claude Desktop")
    assert claude.config_path == tmp_path / "Roaming/Claude/claude_desktop_config.json"


def test_detect_clients_windows_without_appdata_falls_back(tmp_path, monkeypatch):
    monkeypatch.delenv("APPDATA", raising=False)
    clients = mdm_setup.detect_clients(home=tmp_path, system="Windows", cwd=tmp_path)
    claude = next(c for c in clients if c.name == "Claude Desktop")
    assert claude.config_path == tmp_path / "AppData/Roaming/Claude/claude_desktop_config.json"


def test_detect_clients_current_dir_clients_use_cwd(tmp_path):
    fake_cwd = tmp_path / "project"
    fake_cwd.mkdir()
    clients = mdm_setup.detect_clients(home=tmp_path, system="Linux", cwd=fake_cwd)
    claude_code = next(c for c in clients if c.name.startswith("Claude Code"))
    assert claude_code.config_path == fake_cwd / ".mcp.json"


# ─── build_server_entry / build_zed_entry ──────────────────────────────


def test_build_server_entry_unix_uses_uvx_from():
    entry = mdm_setup.build_server_entry(system="Linux")
    assert entry == {"command": "uvx", "args": ["--from", "mdm-parser", "mdm-mcp"]}


def test_build_server_entry_windows_wraps_with_cmd():
    entry = mdm_setup.build_server_entry(system="Windows")
    assert entry["command"] == "cmd"
    assert entry["args"] == ["/c", "uvx", "--from", "mdm-parser", "mdm-mcp"]


def test_build_zed_entry_wraps_command_in_path_key():
    entry = mdm_setup.build_zed_entry(system="Darwin")
    assert entry == {"command": {"path": "uvx", "args": ["--from", "mdm-parser", "mdm-mcp"]}}


# ─── inject_client: fresh file, merge, backup ──────────────────────────


def test_inject_client_creates_new_config(tmp_path):
    client = mdm_setup.ClientConfig("Cursor", tmp_path / "cursor" / "mcp.json", "mcpServers")
    ok, err = mdm_setup.inject_client(client, system="Linux")

    assert ok is True
    assert err is None
    data = json.loads(client.config_path.read_text(encoding="utf-8"))
    assert data["mcpServers"]["mdm"]["command"] == "uvx"


def test_inject_client_preserves_existing_keys_and_other_servers(tmp_path):
    config_path = tmp_path / "claude_desktop_config.json"
    config_path.write_text(
        json.dumps(
            {
                "mcpServers": {"other-tool": {"command": "other", "args": []}},
                "unrelatedTopLevelKey": "keep-me",
            }
        ),
        encoding="utf-8",
    )
    client = mdm_setup.ClientConfig("Claude Desktop", config_path, "mcpServers")

    ok, err = mdm_setup.inject_client(client, system="Darwin")

    assert ok is True
    data = json.loads(config_path.read_text(encoding="utf-8"))
    assert data["unrelatedTopLevelKey"] == "keep-me"
    assert data["mcpServers"]["other-tool"] == {"command": "other", "args": []}
    assert data["mcpServers"]["mdm"]["command"] == "uvx"


def test_inject_client_creates_backup_of_existing_file(tmp_path):
    config_path = tmp_path / "mcp.json"
    config_path.write_text(json.dumps({"mcpServers": {}}), encoding="utf-8")

    ok, _ = mdm_setup.inject_client(
        mdm_setup.ClientConfig("Cursor", config_path, "mcpServers"), system="Linux"
    )

    assert ok is True
    backup = config_path.with_name(config_path.name + ".bak")
    assert backup.exists()
    assert json.loads(backup.read_text(encoding="utf-8")) == {"mcpServers": {}}


def test_inject_client_no_backup_flag_skips_backup(tmp_path):
    config_path = tmp_path / "mcp.json"
    config_path.write_text(json.dumps({"mcpServers": {}}), encoding="utf-8")

    ok, _ = mdm_setup.inject_client(
        mdm_setup.ClientConfig("Cursor", config_path, "mcpServers"), backup=False, system="Linux"
    )

    assert ok is True
    backup = config_path.with_name(config_path.name + ".bak")
    assert not backup.exists()


def test_inject_client_dry_run_does_not_write(tmp_path):
    config_path = tmp_path / "mcp.json"

    ok, _ = mdm_setup.inject_client(
        mdm_setup.ClientConfig("Cursor", config_path, "mcpServers"), dry_run=True, system="Linux"
    )

    assert ok is True
    assert not config_path.exists()


def test_inject_client_reports_error_on_invalid_existing_json(tmp_path):
    config_path = tmp_path / "mcp.json"
    config_path.write_text("{not valid json", encoding="utf-8")

    ok, err = mdm_setup.inject_client(
        mdm_setup.ClientConfig("Cursor", config_path, "mcpServers"), system="Linux"
    )

    assert ok is False
    assert err is not None


# ─── parse_indices ──────────────────────────────────────────────────────


@pytest.mark.parametrize(
    ("raw", "count", "expected"),
    [
        ("1,3", 5, [0, 2]),
        (" 1 , 3 ", 5, [0, 2]),
        ("0,99", 5, []),  # out of range, both dropped
        ("abc,2", 5, [1]),  # non-numeric dropped
        ("", 5, []),
    ],
)
def test_parse_indices(raw, count, expected):
    assert mdm_setup.parse_indices(raw, count) == expected


# ─── run_setup: end-to-end via --clients (non-interactive) ────────────


def test_run_setup_non_interactive_registers_selected_clients(tmp_path, monkeypatch):
    monkeypatch.setattr(
        mdm_setup,
        "detect_clients",
        lambda: [
            mdm_setup.ClientConfig("A", tmp_path / "a.json", "mcpServers"),
            mdm_setup.ClientConfig("B", tmp_path / "b.json", "mcpServers"),
        ],
    )

    exit_code = mdm_setup.run_setup(["--clients", "1"])

    assert exit_code == 0
    assert (tmp_path / "a.json").exists()
    assert not (tmp_path / "b.json").exists()


def test_run_setup_dry_run_writes_nothing(tmp_path, monkeypatch):
    monkeypatch.setattr(
        mdm_setup,
        "detect_clients",
        lambda: [mdm_setup.ClientConfig("A", tmp_path / "a.json", "mcpServers")],
    )

    exit_code = mdm_setup.run_setup(["--clients", "1", "--dry-run"])

    assert exit_code == 0
    assert not (tmp_path / "a.json").exists()


def test_run_setup_empty_selection_prints_manual_config_and_succeeds(tmp_path, monkeypatch, capsys):
    monkeypatch.setattr(
        mdm_setup,
        "detect_clients",
        lambda: [mdm_setup.ClientConfig("A", tmp_path / "a.json", "mcpServers")],
    )

    exit_code = mdm_setup.run_setup(["--clients", ""])

    assert exit_code == 0
    captured = capsys.readouterr()
    assert "수동 설정" in captured.out
