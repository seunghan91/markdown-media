#!/usr/bin/env python3
"""Download 수능 기출문제 corpus from suneung.re.kr.

Usage:
  python3 scripts/fetch-suneung-corpus.py --sample         # Page 1 only (2026 수능)
  python3 scripts/fetch-suneung-corpus.py --board 1500234  # All pages of a board
  python3 scripts/fetch-suneung-corpus.py --all            # All 4 boards

Boards:
  1500234  수능 (본시험)
  1500235  수능 2004학년도 이전
  1500236  수능 모의평가
  1500237  2028학년도 예시문항

Output layout:
  assets/conformance/suneung/pdf/{board_slug}/{year}/seq_{seq}_{subject}/{filename}
  assets/conformance/suneung/manifest.json

License: 공공누리 제4유형 (출처표시·비상업·변경금지). 개인 테스트용 로컬 캐시.
"""
from __future__ import annotations

import argparse
import json
import re
import ssl
import sys
import time
import zipfile
from concurrent.futures import ThreadPoolExecutor, as_completed
from html.parser import HTMLParser
from pathlib import Path
from urllib.parse import unquote, urljoin
from urllib.request import Request, urlopen

try:
    import certifi
    _SSL_CTX = ssl.create_default_context(cafile=certifi.where())
except ImportError:
    _SSL_CTX = ssl.create_default_context()
    try:
        _SSL_CTX.load_default_certs()
    except Exception:
        pass

BASE = "https://www.suneung.re.kr"
UA = "Mozilla/5.0 (markdown-media test corpus fetcher; contact: theqwe2000@gmail.com)"

BOARDS = {
    1500234: "suneung",
    1500235: "suneung_pre2004",
    1500236: "mopyeongga",
    1500237: "sample_2028",
}

ROOT = Path(__file__).resolve().parent.parent
OUT_DIR = ROOT / "assets" / "conformance" / "suneung"
PDF_DIR = OUT_DIR / "pdf"
MANIFEST = OUT_DIR / "manifest.json"


def fetch(url: str) -> bytes:
    req = Request(url, headers={"User-Agent": UA})
    with urlopen(req, timeout=30, context=_SSL_CTX) as r:
        return r.read()


def fetch_text(url: str) -> str:
    return fetch(url).decode("utf-8", errors="replace")


GOVIEW_RE = re.compile(r"goView\(\s*'(\d+)'\s*,\s*'(\d+)'")
TR_RE = re.compile(r"<tr[^>]*>(.*?)</tr>", re.S)
TD_RE = re.compile(r"<td[^>]*>(.*?)</td>", re.S)
TAG_RE = re.compile(r"<[^>]+>")


def clean(s: str) -> str:
    return TAG_RE.sub("", s).replace("&nbsp;", " ").strip()


def parse_list_page(html: str) -> list[dict]:
    rows = []
    for row_html in TR_RE.findall(html):
        m = GOVIEW_RE.search(row_html)
        if not m:
            continue
        board_id, seq = m.groups()
        tds = [clean(td) for td in TD_RE.findall(row_html)]
        if len(tds) < 5:
            continue
        rows.append({
            "board_id": int(board_id),
            "seq": int(seq),
            "year": tds[1] if len(tds) > 1 else "",
            "subject": tds[2] if len(tds) > 2 else "",
            "kind": tds[3] if len(tds) > 3 else "",
            "date": tds[4] if len(tds) > 4 else "",
        })
    # Deduplicate by seq (mobile+desktop render duplicates)
    seen: dict[int, dict] = {}
    for r in rows:
        seen.setdefault(r["seq"], r)
    return list(seen.values())


class AttachmentExtractor(HTMLParser):
    def __init__(self) -> None:
        super().__init__()
        self.files: list[tuple[str, str]] = []
        self._cur_href: str | None = None
        self._cur_text: list[str] = []

    def handle_starttag(self, tag, attrs):
        if tag == "a":
            d = dict(attrs)
            href = d.get("href", "")
            if "fileDown.do" in href:
                self._cur_href = href
                self._cur_text = []

    def handle_data(self, data):
        if self._cur_href is not None:
            self._cur_text.append(data)

    def handle_endtag(self, tag):
        if tag == "a" and self._cur_href is not None:
            text = "".join(self._cur_text).strip()
            self.files.append((text, self._cur_href))
            self._cur_href = None
            self._cur_text = []


def parse_view_page(html: str) -> list[tuple[str, str]]:
    p = AttachmentExtractor()
    p.feed(html)
    return p.files


CD_FILENAME_RE = re.compile(r"filename=([^;\r\n]+)")


def download_file(url: str, out_path: Path) -> tuple[bool, str]:
    if out_path.exists() and out_path.stat().st_size > 0:
        return True, "exists"
    req = Request(url, headers={"User-Agent": UA})
    try:
        with urlopen(req, timeout=60, context=_SSL_CTX) as r:
            cd = r.headers.get("Content-Disposition", "")
            m = CD_FILENAME_RE.search(cd)
            actual_name = unquote(m.group(1).strip().strip('"')) if m else out_path.name
            out_path.parent.mkdir(parents=True, exist_ok=True)
            out_path.write_bytes(r.read())
            return True, actual_name
    except Exception as e:
        return False, str(e)


def extract_zip(zip_path: Path) -> list[Path]:
    """Unzip to sibling dir named after the zip stem. Fix CP949/EUC-KR filenames."""
    if not zipfile.is_zipfile(zip_path):
        return []
    out_dir = zip_path.with_suffix("")
    out_dir.mkdir(parents=True, exist_ok=True)
    extracted: list[Path] = []
    try:
        with zipfile.ZipFile(zip_path) as zf:
            for info in zf.infolist():
                if info.is_dir():
                    continue
                # If UTF-8 flag set, zipfile already decoded correctly
                if info.flag_bits & 0x800:
                    name = info.filename
                else:
                    # Legacy CP437-decoded name → recover original bytes → try Korean codecs
                    raw = info.filename.encode("cp437", errors="replace")
                    for enc in ("cp949", "euc-kr", "utf-8"):
                        try:
                            name = raw.decode(enc)
                            break
                        except UnicodeDecodeError:
                            continue
                    else:
                        name = info.filename
                safe_name = name.replace("..", "_").lstrip("/")
                target = out_dir / safe_name
                target.parent.mkdir(parents=True, exist_ok=True)
                with zf.open(info) as src, open(target, "wb") as dst:
                    dst.write(src.read())
                extracted.append(target)
    except Exception as e:
        print(f"    [unzip-FAIL] {zip_path.name}: {e}")
    return extracted


def sanitize(s: str) -> str:
    return re.sub(r"[^\w\uac00-\ud7af.-]+", "_", s).strip("_")[:80]


def process_entry(board_id: int, slug: str, page: int, row: dict) -> dict:
    """Fetch view page, download all attachments, unzip if needed. Thread-safe."""
    seq = row["seq"]
    view_url = (
        f"{BASE}/boardCnts/view.do?boardID={board_id}&boardSeq={seq}"
        f"&lev=0&m=0403&statusYN=W&page={page}&s=suneung"
    )
    try:
        view_html = fetch_text(view_url)
    except Exception as e:
        return {"board_id": board_id, "board_slug": slug, "seq": seq, "error": str(e), "files": []}
    files = parse_view_page(view_html)
    year = row["year"] or "unknown"
    subject = row["subject"] or "unknown"
    subject_slug = sanitize(subject)
    entry_dir = PDF_DIR / slug / year / f"seq_{seq}_{subject_slug}"
    downloaded = []
    for label, href in files:
        if re.search(r"(듣기.*음원|mp3)", label, re.I):
            continue
        url = urljoin(BASE, href)
        label_clean = sanitize(label) or f"file_{len(downloaded)}"
        ext_match = re.search(r"\.(pdf|zip|hwp|hwpx)$", label, re.I)
        ext = ext_match.group(0).lower() if ext_match else ""
        fname = label_clean if ext else f"{label_clean}.bin"
        out_path = entry_dir / fname
        ok, info = download_file(url, out_path)
        extracted = []
        if ok and ext == ".zip":
            extracted = extract_zip(out_path)
        downloaded.append({
            "label": label,
            "url": url,
            "path": str(out_path.relative_to(ROOT)),
            "ok": ok,
            "info": info,
            "extracted": [str(p.relative_to(ROOT)) for p in extracted],
        })
    return {
        "board_id": board_id,
        "board_slug": slug,
        "seq": seq,
        "year": year,
        "subject": subject,
        "kind": row["kind"],
        "date": row["date"],
        "view_url": view_url,
        "files": downloaded,
    }


def crawl_board(board_id: int, max_pages: int | None = None, workers: int = 16) -> list[dict]:
    slug = BOARDS.get(board_id, f"board_{board_id}")
    # Collect all rows from all list pages first (sequential — cheap)
    all_rows: list[tuple[int, dict]] = []
    page = 1
    while True:
        if max_pages and page > max_pages:
            break
        list_url = f"{BASE}/boardCnts/list.do?boardID={board_id}&m=0403&page={page}&s=suneung"
        print(f"[list] {slug} page={page}", flush=True)
        try:
            html = fetch_text(list_url)
        except Exception as e:
            print(f"  [list-FAIL] {e}")
            break
        rows = parse_list_page(html)
        if not rows:
            break
        for row in rows:
            all_rows.append((page, row))
        page += 1

    print(f"[plan] {slug}: {len(all_rows)} entries across {page - 1} pages → {workers} workers", flush=True)

    entries: list[dict] = []
    done_count = 0
    total = len(all_rows)
    with ThreadPoolExecutor(max_workers=workers) as ex:
        futures = [ex.submit(process_entry, board_id, slug, pg, row) for pg, row in all_rows]
        for fut in as_completed(futures):
            entry = fut.result()
            entries.append(entry)
            done_count += 1
            ok_files = sum(1 for f in entry.get("files", []) if f.get("ok"))
            tot_files = len(entry.get("files", []))
            err = entry.get("error", "")
            tag = f"ERR: {err}" if err else f"{ok_files}/{tot_files} files"
            print(f"[{done_count}/{total}] {entry.get('year','?')} {entry.get('subject','?')} seq={entry['seq']} ({tag})", flush=True)
    return entries


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--sample", action="store_true", help="Page 1 of 수능 board only (~8 entries)")
    ap.add_argument("--board", type=int, action="append", help="Specific board ID (repeatable)")
    ap.add_argument("--all", action="store_true", help="All 4 boards")
    ap.add_argument("--max-pages", type=int, default=None)
    ap.add_argument("--workers", type=int, default=16, help="Parallel download workers")
    args = ap.parse_args()

    if args.sample:
        targets = [(1500234, 1)]
    elif args.all:
        targets = [(bid, args.max_pages) for bid in BOARDS]
    elif args.board:
        targets = [(bid, args.max_pages) for bid in args.board]
    else:
        ap.print_help()
        return 1

    OUT_DIR.mkdir(parents=True, exist_ok=True)
    all_entries: list[dict] = []
    for board_id, max_pages in targets:
        all_entries.extend(crawl_board(board_id, max_pages=max_pages, workers=args.workers))

    manifest = {
        "source": BASE,
        "license": "공공누리 제4유형 (출처표시·비상업·변경금지)",
        "note": "Local test corpus — do not redistribute.",
        "entries": all_entries,
    }
    MANIFEST.write_text(json.dumps(manifest, ensure_ascii=False, indent=2))
    ok = sum(1 for e in all_entries for f in e["files"] if f["ok"])
    total = sum(len(e["files"]) for e in all_entries)
    print(f"\n[done] entries={len(all_entries)} files={ok}/{total} manifest={MANIFEST}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
