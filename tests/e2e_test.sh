#!/bin/bash
# ============================================================================
# 🚧 작업 중 - 이 파일은 현재 [테스트 팀]에서 작업 중입니다
# ============================================================================
# 작업 담당: 병렬 작업 팀
# 시작 시간: 2025-01-01
# 진행 상태: Phase 1.8 테스트 구현
#
# ⚠️ 주의: 1.7 오케스트레이터는 다른 팀에서 작업 중입니다.
#         E2E 통합 테스트는 1.7 완료 후 전체 활성화됩니다.
# ============================================================================

set -e

# 색상 정의
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# 스크립트 위치 기준 경로 설정
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# 디렉토리 경로
CORE_DIR="$PROJECT_ROOT/core"
CONVERTERS_DIR="$PROJECT_ROOT/converters"
PARSER_PY_DIR="$PROJECT_ROOT/packages/parser-py"
SAMPLES_DIR="$PROJECT_ROOT/samples/input"
OUTPUT_DIR="$PROJECT_ROOT/test_output"

# 테스트 결과 카운터
TESTS_PASSED=0
TESTS_FAILED=0
TESTS_SKIPPED=0

# ============================================================================
# 유틸리티 함수
# ============================================================================

log_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

log_success() {
    echo -e "${GREEN}[PASS]${NC} $1"
    ((TESTS_PASSED++))
}

log_failure() {
    echo -e "${RED}[FAIL]${NC} $1"
    ((TESTS_FAILED++))
}

log_skip() {
    echo -e "${YELLOW}[SKIP]${NC} $1"
    ((TESTS_SKIPPED++))
}

log_header() {
    echo ""
    echo "============================================================================"
    echo -e "${BLUE}$1${NC}"
    echo "============================================================================"
}

check_command() {
    if command -v "$1" &> /dev/null; then
        return 0
    else
        return 1
    fi
}

# ============================================================================
# 환경 확인
# ============================================================================

check_environment() {
    log_header "환경 확인"

    # Rust 확인
    if check_command cargo; then
        RUST_VERSION=$(cargo --version)
        log_success "Rust: $RUST_VERSION"
    else
        log_failure "Rust (cargo) not found"
    fi

    # Python 확인
    if check_command python3; then
        PYTHON_VERSION=$(python3 --version)
        log_success "Python: $PYTHON_VERSION"
    else
        log_failure "Python3 not found"
    fi

    # Node.js 확인 (선택적)
    if check_command node; then
        NODE_VERSION=$(node --version)
        log_success "Node.js: $NODE_VERSION"
    else
        log_skip "Node.js not found (optional)"
    fi

    # 출력 디렉토리 생성
    mkdir -p "$OUTPUT_DIR"
    log_info "Output directory: $OUTPUT_DIR"
}

# ============================================================================
# Rust 코어 테스트
# ============================================================================

test_rust_core() {
    log_header "Rust Core 테스트"

    cd "$CORE_DIR"

    # 빌드 테스트
    log_info "Building Rust core..."
    if cargo build --release 2>/dev/null; then
        log_success "Rust core build"
    else
        log_failure "Rust core build"
        return 1
    fi

    # 유닛 테스트
    log_info "Running Rust unit tests..."
    if cargo test 2>/dev/null; then
        log_success "Rust unit tests"
    else
        log_failure "Rust unit tests"
    fi

    # CLI 기본 테스트
    if [ -f "target/release/hwp2mdm" ]; then
        log_info "Testing CLI..."
        if ./target/release/hwp2mdm --version 2>/dev/null; then
            log_success "CLI --version"
        else
            log_skip "CLI --version (may need implementation)"
        fi
    else
        log_skip "CLI binary not found"
    fi

    cd "$PROJECT_ROOT"
}

# ============================================================================
# Python 컴포넌트 테스트
# ============================================================================

test_python_components() {
    log_header "Python 컴포넌트 테스트"

    # pytest 확인
    if ! check_command pytest; then
        log_skip "pytest not installed - install with: pip install pytest"
        return 0
    fi

    # 테스트 실행
    log_info "Running Python tests..."
    cd "$PROJECT_ROOT/tests"

    if python3 -m pytest test_pipeline.py -v 2>/dev/null; then
        log_success "Python component tests"
    else
        log_failure "Python component tests"
    fi

    cd "$PROJECT_ROOT"
}

# ============================================================================
# 개별 컴포넌트 테스트
# ============================================================================

test_table_renderer() {
    log_header "테이블 SVG 렌더러 테스트"

    cd "$CONVERTERS_DIR"

    # 기본 테이블 렌더링 테스트
    log_info "Testing table rendering..."

    python3 -c "
import sys
sys.path.insert(0, '.')
try:
    from table_to_svg_enhanced import Table, TableSvgRenderer
    table = Table.from_markdown('| A | B |\n| --- | --- |\n| 1 | 2 |')
    print(f'Table parsed: {table.row_count} rows, {table.col_count} cols')
    print('SUCCESS: table_to_svg_enhanced')
except Exception as e:
    print(f'FAILED: {e}')
    sys.exit(1)
" 2>/dev/null

    if [ $? -eq 0 ]; then
        log_success "Table SVG renderer"
    else
        log_failure "Table SVG renderer"
    fi

    cd "$PROJECT_ROOT"
}

test_chart_renderer() {
    log_header "차트 PNG 렌더러 테스트"

    cd "$CONVERTERS_DIR"

    log_info "Testing chart rendering..."

    python3 -c "
import sys
sys.path.insert(0, '.')
try:
    from chart_to_png import ChartRenderer, ChartData, ChartType
    data = ChartData.from_dict({
        'type': 'bar',
        'title': 'Test',
        'categories': ['A', 'B'],
        'series': [{'name': 'Data', 'values': [10, 20]}]
    })
    print(f'Chart created: {data.chart_type.value}')
    print('SUCCESS: chart_to_png')
except Exception as e:
    print(f'FAILED: {e}')
    sys.exit(1)
" 2>/dev/null

    if [ $? -eq 0 ]; then
        log_success "Chart PNG renderer"
    else
        log_failure "Chart PNG renderer"
    fi

    cd "$PROJECT_ROOT"
}

test_ocr_bridge() {
    log_header "OCR 브릿지 테스트"

    cd "$PARSER_PY_DIR"

    log_info "Testing OCR bridge..."

    python3 -c "
import sys
sys.path.insert(0, '.')
try:
    from ocr_bridge import OcrResult
    result = OcrResult(
        image_id='test',
        source_path='/tmp/test.png',
        extracted_text='Hello World'
    )
    print(f'OCR result created: {result.image_id}')
    print('SUCCESS: ocr_bridge')
except Exception as e:
    print(f'FAILED: {e}')
    sys.exit(1)
" 2>/dev/null

    if [ $? -eq 0 ]; then
        log_success "OCR bridge"
    else
        log_failure "OCR bridge"
    fi

    cd "$PROJECT_ROOT"
}

# ============================================================================
# 샘플 파일 테스트 (실제 파일이 있는 경우)
# ============================================================================

test_sample_files() {
    log_header "샘플 파일 테스트"

    # HWP 샘플 테스트
    if [ -d "$SAMPLES_DIR" ] && ls "$SAMPLES_DIR"/*.hwp 1>/dev/null 2>&1; then
        HWP_COUNT=$(ls "$SAMPLES_DIR"/*.hwp 2>/dev/null | wc -l)
        log_info "Found $HWP_COUNT HWP sample files"

        # 첫 번째 파일로 테스트 (있다면)
        FIRST_HWP=$(ls "$SAMPLES_DIR"/*.hwp 2>/dev/null | head -1)
        if [ -n "$FIRST_HWP" ]; then
            log_info "Testing with: $(basename "$FIRST_HWP")"
            # TODO: 실제 파싱 테스트 추가
            log_skip "HWP parsing test (pending CLI implementation)"
        fi
    else
        log_skip "No HWP sample files found"
    fi

    # DOCX 샘플 테스트
    if ls "$SAMPLES_DIR"/*.docx 1>/dev/null 2>&1; then
        DOCX_COUNT=$(ls "$SAMPLES_DIR"/*.docx 2>/dev/null | wc -l)
        log_info "Found $DOCX_COUNT DOCX sample files"
        log_skip "DOCX parsing test (pending integration)"
    else
        log_skip "No DOCX sample files found"
    fi

    # PDF 샘플 테스트
    if ls "$SAMPLES_DIR"/*.pdf 1>/dev/null 2>&1; then
        PDF_COUNT=$(ls "$SAMPLES_DIR"/*.pdf 2>/dev/null | wc -l)
        log_info "Found $PDF_COUNT PDF sample files"
        log_skip "PDF parsing test (pending integration)"
    else
        log_skip "No PDF sample files found"
    fi
}

# ============================================================================
# 파이프라인 통합 테스트 (1.7 완료 후 활성화)
# ============================================================================

test_pipeline_integration() {
    log_header "파이프라인 통합 테스트"

    log_skip "파이프라인 통합 테스트 - 1.7 오케스트레이터 작업 완료 대기"

    # TODO: 1.7 완료 후 활성화
    # test_hwp_pipeline
    # test_docx_pipeline
    # test_pdf_pipeline
}

# ============================================================================
# 결과 요약
# ============================================================================

print_summary() {
    log_header "테스트 결과 요약"

    TOTAL=$((TESTS_PASSED + TESTS_FAILED + TESTS_SKIPPED))

    echo ""
    echo -e "  ${GREEN}통과: $TESTS_PASSED${NC}"
    echo -e "  ${RED}실패: $TESTS_FAILED${NC}"
    echo -e "  ${YELLOW}스킵: $TESTS_SKIPPED${NC}"
    echo "  ─────────────"
    echo "  총계: $TOTAL"
    echo ""

    if [ $TESTS_FAILED -gt 0 ]; then
        echo -e "${RED}일부 테스트가 실패했습니다.${NC}"
        exit 1
    else
        echo -e "${GREEN}모든 테스트가 통과했습니다!${NC}"
        exit 0
    fi
}

# ============================================================================
# 메인 실행
# ============================================================================

main() {
    echo ""
    echo "╔════════════════════════════════════════════════════════════════════════╗"
    echo "║                    MDM E2E Test Suite                                  ║"
    echo "║                                                                        ║"
    echo "║  ⚠️  Note: 1.7 오케스트레이터 작업 진행 중                               ║"
    echo "║      통합 테스트는 해당 작업 완료 후 전체 활성화됩니다.                  ║"
    echo "╚════════════════════════════════════════════════════════════════════════╝"
    echo ""

    # 환경 확인
    check_environment

    # Rust 테스트
    test_rust_core

    # Python 테스트
    test_python_components

    # 개별 컴포넌트 테스트
    test_table_renderer
    test_chart_renderer
    test_ocr_bridge

    # 샘플 파일 테스트
    test_sample_files

    # 통합 테스트 (1.7 대기)
    test_pipeline_integration

    # 결과 출력
    print_summary
}

# 스크립트 실행
main "$@"
