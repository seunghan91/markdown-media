#!/usr/bin/env python3
# ============================================================================
# MDM Spec Test Runner (Python)
# ============================================================================
# 작업 담당: 병렬 작업 팀
# 진행 상태: Phase 3.7 통합 테스트
#
# 사용법:
#   python tests/runners/run-py.py
#   python tests/runners/run-py.py --filter basic
#   python tests/runners/run-py.py --verbose
# ============================================================================

import argparse
import json
import re
import sys
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any, Dict, List, Optional


# 색상 코드
class Colors:
    RESET = "\033[0m"
    RED = "\033[31m"
    GREEN = "\033[32m"
    YELLOW = "\033[33m"
    BLUE = "\033[34m"
    CYAN = "\033[36m"


@dataclass
class TestResult:
    """테스트 결과"""
    passed: int = 0
    failed: int = 0
    skipped: int = 0
    errors: List[Dict[str, str]] = field(default_factory=list)

    @property
    def total(self) -> int:
        return self.passed + self.failed + self.skipped


class SpecTestRunner:
    """스펙 테스트 러너"""

    def __init__(self, verbose: bool = False, filter_pattern: Optional[str] = None):
        self.spec_dir = Path(__file__).parent.parent / "spec"
        self.verbose = verbose
        self.filter = filter_pattern
        self.result = TestResult()

    def log(self, msg: str, color: str = "RESET") -> None:
        """컬러 로깅"""
        color_code = getattr(Colors, color, Colors.RESET)
        print(f"{color_code}{msg}{Colors.RESET}")

    def run(self) -> int:
        """모든 테스트 실행"""
        self.log("\n📋 MDM Spec Tests (Python)\n", "CYAN")
        self.log("=" * 50)

        categories = self.get_categories()

        for category in categories:
            if self.filter and self.filter not in category:
                continue
            self.run_category(category)

        self.print_summary()
        return 1 if self.result.failed > 0 else 0

    def get_categories(self) -> List[str]:
        """테스트 카테고리 목록"""
        if not self.spec_dir.exists():
            return []
        return [
            d.name
            for d in self.spec_dir.iterdir()
            if d.is_dir() and not d.name.startswith(".")
        ]

    def run_category(self, category: str) -> None:
        """카테고리별 테스트 실행"""
        category_path = self.spec_dir / category
        self.log(f"\n📁 {category}/", "BLUE")

        test_files = sorted(category_path.glob("*.md"))
        for test_file in test_files:
            self.run_test(category, test_file.stem)

    def run_test(self, category: str, test_name: str) -> None:
        """개별 테스트 실행"""
        base_path = self.spec_dir / category / test_name

        input_path = base_path.with_suffix(".md")
        expected_path = Path(str(base_path) + ".expected.json")
        sidecar_path = base_path.with_suffix(".mdm")

        try:
            # 입력 파일 읽기
            input_content = input_path.read_text(encoding="utf-8")

            # expected.json 확인
            if not expected_path.exists():
                self.log(f"  ⏭️  {test_name} (no expected file)", "YELLOW")
                self.result.skipped += 1
                return

            expected = json.loads(expected_path.read_text(encoding="utf-8"))

            # 사이드카 파일 (있는 경우)
            sidecar = None
            if sidecar_path.exists():
                sidecar = sidecar_path.read_text(encoding="utf-8")

            # 테스트 실행
            actual = self.parse_document(input_content, sidecar)

            # 결과 비교
            passed = self.compare_results(expected, actual)

            if passed:
                self.log(f"  ✅ {test_name}", "GREEN")
                self.result.passed += 1
            else:
                self.log(f"  ❌ {test_name}", "RED")
                self.result.failed += 1

                if self.verbose:
                    self.log(
                        f"     Expected: {json.dumps(expected.get('resources', {}), indent=2)}",
                        "YELLOW",
                    )
                    self.log(
                        f"     Actual: {json.dumps(actual.get('resources', {}), indent=2)}",
                        "YELLOW",
                    )

        except Exception as e:
            self.log(f"  ❌ {test_name} - Error: {e}", "RED")
            self.result.failed += 1
            self.result.errors.append({"test": test_name, "error": str(e)})

    def parse_document(
        self, markdown: str, sidecar: Optional[str] = None
    ) -> Dict[str, Any]:
        """문서 파싱 (실제 파서 호출)"""
        # TODO: 실제 파서 연동
        # 지금은 기본 이미지 추출 로직만 구현

        resources: Dict[str, Dict[str, Any]] = {}

        # 마크다운에서 이미지 추출
        pattern = r'!\[([^\]]*)\]\(([^)\s]+)(?:\s+"([^"]*)")?\)(?:\{([^}]*)\})?'

        for match in re.finditer(pattern, markdown):
            alt, src, title, attrs = match.groups()
            filename = Path(src).name

            resource: Dict[str, Any] = {
                "type": self.detect_type(src),
                "src": src,
                "alt": alt or None,
                "title": title or None,
            }

            # 속성 파싱
            if attrs:
                preset_match = re.search(r"preset=(\w+)", attrs)
                if preset_match:
                    resource["preset"] = preset_match.group(1)

            # 외부 URL 감지
            if src.startswith(("http://", "https://")):
                resource["external"] = True

            resources[filename] = resource

        return {
            "resources": resources,
            "resourceCount": len(resources),
            "errors": [],
        }

    def detect_type(self, src: str) -> str:
        """파일 확장자로 타입 감지"""
        ext = Path(src).suffix.lower()

        type_map = {
            # 이미지
            ".jpg": "image", ".jpeg": "image", ".png": "image",
            ".gif": "image", ".webp": "image", ".svg": "image",
            ".avif": "image", ".bmp": "image",
            # 비디오
            ".mp4": "video", ".webm": "video", ".mov": "video",
            ".avi": "video", ".mkv": "video",
            # 오디오
            ".mp3": "audio", ".wav": "audio", ".ogg": "audio",
            ".m4a": "audio", ".flac": "audio",
        }

        # YouTube/Vimeo 등 embed 감지
        if "youtube.com" in src or "youtu.be" in src:
            return "embed"
        if "vimeo.com" in src:
            return "embed"

        return type_map.get(ext, "unknown")

    def compare_results(
        self, expected: Dict[str, Any], actual: Dict[str, Any]
    ) -> bool:
        """결과 비교"""
        # 리소스 개수 비교
        if expected.get("resourceCount") != actual.get("resourceCount"):
            return False

        expected_resources = expected.get("resources", {})
        actual_resources = actual.get("resources", {})

        # 각 리소스 비교
        for key, expected_resource in expected_resources.items():
            actual_resource = actual_resources.get(key)

            if not actual_resource:
                return False

            # 타입 비교
            if expected_resource.get("type") != actual_resource.get("type"):
                return False

            # src 비교
            if expected_resource.get("src") != actual_resource.get("src"):
                return False

        return True

    def print_summary(self) -> None:
        """결과 요약"""
        self.log("\n" + "=" * 50)
        self.log("📊 Test Summary\n", "CYAN")

        self.log(f"  Total:   {self.result.total}")
        self.log(f"  Passed:  {self.result.passed}", "GREEN")
        
        failed_color = "RED" if self.result.failed > 0 else "RESET"
        self.log(f"  Failed:  {self.result.failed}", failed_color)
        self.log(f"  Skipped: {self.result.skipped}", "YELLOW")

        if self.result.errors:
            self.log("\n⚠️ Errors:", "RED")
            for err in self.result.errors:
                self.log(f"  - {err['test']}: {err['error']}")

        print()


def main():
    parser = argparse.ArgumentParser(description="MDM Spec Test Runner (Python)")
    parser.add_argument("--verbose", "-v", action="store_true", help="Verbose output")
    parser.add_argument("--filter", "-f", help="Filter tests by category name")

    args = parser.parse_args()

    runner = SpecTestRunner(verbose=args.verbose, filter_pattern=args.filter)
    sys.exit(runner.run())


if __name__ == "__main__":
    main()
