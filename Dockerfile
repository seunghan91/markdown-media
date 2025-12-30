# ============================================================================
# MDM (Markdown+Media) Docker Container
# ============================================================================
# 작업 담당: 병렬 작업 팀
# 작업 상태: Phase 3.6 Docker 컨테이너
# 시작 시간: 2025-12-31
#
# 멀티스테이지 빌드:
# 1. rust-builder: Rust CLI 바이너리 빌드
# 2. python-deps: Python 의존성 설치
# 3. final: 최종 슬림 이미지
#
# 사용법:
#   docker build -t mdm .
#   docker run -v $(pwd)/docs:/input -v $(pwd)/output:/output mdm convert /input/doc.hwp -o /output
# ============================================================================

# =============================================================================
# Stage 1: Rust Builder
# =============================================================================
FROM rust:1.75-slim-bookworm AS rust-builder

# 빌드에 필요한 시스템 패키지
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# 작업 디렉토리 설정
WORKDIR /build

# Cargo.toml과 Cargo.lock 먼저 복사 (캐시 활용)
COPY core/Cargo.toml core/Cargo.lock ./

# 더미 소스 생성으로 의존성 캐시
RUN mkdir src && \
    echo "fn main() {}" > src/main.rs && \
    echo "pub fn lib() {}" > src/lib.rs && \
    cargo build --release && \
    rm -rf src

# 실제 소스 복사 및 빌드
COPY core/src ./src
RUN touch src/main.rs src/lib.rs && \
    cargo build --release --bin hwp2mdm

# =============================================================================
# Stage 2: Python Dependencies
# =============================================================================
FROM python:3.11-slim-bookworm AS python-deps

# 시스템 의존성 (Cairo, Tesseract 등)
RUN apt-get update && apt-get install -y \
    libcairo2 \
    libcairo2-dev \
    libpango1.0-dev \
    libgdk-pixbuf2.0-dev \
    libffi-dev \
    tesseract-ocr \
    tesseract-ocr-kor \
    tesseract-ocr-eng \
    && rm -rf /var/lib/apt/lists/*

# pip 업그레이드
RUN pip install --no-cache-dir --upgrade pip

# 작업 디렉토리
WORKDIR /app

# Python 의존성 설치 (캐시 활용)
COPY packages/parser-py/pyproject.toml ./packages/parser-py/
RUN pip install --no-cache-dir \
    pdfplumber>=0.10.0 \
    pillow>=9.0.0 \
    svgwrite>=1.4.0 \
    matplotlib>=3.8.0 \
    cairosvg>=2.7.0 \
    pytesseract>=0.3.10

# =============================================================================
# Stage 3: Final Image
# =============================================================================
FROM python:3.11-slim-bookworm AS final

# 메타데이터
LABEL org.opencontainers.image.title="MDM - Markdown+Media Converter"
LABEL org.opencontainers.image.description="Convert HWP, PDF, DOCX documents to MDX with media support"
LABEL org.opencontainers.image.source="https://github.com/seunghan91/markdown-media"
LABEL org.opencontainers.image.licenses="MIT"
LABEL org.opencontainers.image.version="0.1.0"

# 런타임 시스템 의존성
RUN apt-get update && apt-get install -y \
    libcairo2 \
    libpango-1.0-0 \
    libpangocairo-1.0-0 \
    libgdk-pixbuf-2.0-0 \
    tesseract-ocr \
    tesseract-ocr-kor \
    tesseract-ocr-eng \
    fonts-nanum \
    fonts-noto-cjk \
    && rm -rf /var/lib/apt/lists/*

# 비root 사용자 생성
RUN useradd --create-home --shell /bin/bash mdm

# 작업 디렉토리
WORKDIR /app

# Python 사이트 패키지 복사
COPY --from=python-deps /usr/local/lib/python3.11/site-packages /usr/local/lib/python3.11/site-packages

# Rust 바이너리 복사
COPY --from=rust-builder /build/target/release/hwp2mdm /usr/local/bin/hwp2mdm

# Python 컨버터들 복사
COPY converters/ ./converters/
COPY packages/parser-py/ ./packages/parser-py/
COPY pipeline/ ./pipeline/

# 실행 권한
RUN chmod +x /usr/local/bin/hwp2mdm

# 디렉토리 생성
RUN mkdir -p /input /output && \
    chown -R mdm:mdm /app /input /output

# 환경 변수
ENV PYTHONPATH="/app:/app/packages/parser-py:/app/converters:/app/pipeline"
ENV PYTHONUNBUFFERED=1
ENV TESSDATA_PREFIX=/usr/share/tesseract-ocr/5/tessdata/

# 사용자 전환
USER mdm

# 엔트리포인트 스크립트
COPY --chown=mdm:mdm <<'EOF' /app/entrypoint.sh
#!/bin/bash
set -e

# 명령어가 없으면 도움말 표시
if [ $# -eq 0 ]; then
    echo "MDM - Markdown+Media Converter"
    echo ""
    echo "Usage:"
    echo "  docker run mdm convert <input> -o <output>  # Convert document"
    echo "  docker run mdm text <input>                 # Extract text"
    echo "  docker run mdm images <input> -o <dir>      # Extract images"
    echo "  docker run mdm info <input>                 # Show document info"
    echo "  docker run mdm pipeline <input> -o <output> # Run full pipeline"
    echo ""
    echo "Examples:"
    echo "  docker run -v \$(pwd):/data mdm convert /data/doc.hwp -o /data/output"
    echo "  docker run -v \$(pwd):/data mdm pipeline /data/doc.pdf -o /data/output --ocr"
    exit 0
fi

# pipeline 명령 처리
if [ "$1" = "pipeline" ]; then
    shift
    python -m pipeline.orchestrator "$@"
else
    # Rust CLI로 전달
    hwp2mdm "$@"
fi
EOF

RUN chmod +x /app/entrypoint.sh

# 기본 볼륨
VOLUME ["/input", "/output"]

# 엔트리포인트
ENTRYPOINT ["/app/entrypoint.sh"]

# 헬스체크
HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 \
    CMD hwp2mdm --version || exit 1
