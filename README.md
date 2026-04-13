# MDM — 한국 문서를 마크다운으로 변환하는 가장 빠른 엔진

![License](https://img.shields.io/badge/license-MIT-blue.svg)
![Rust](https://img.shields.io/badge/Core-Rust_21K_LOC-orange)
![Python](https://img.shields.io/badge/pip_install-mdm--core-blue)
![Tests](https://img.shields.io/badge/tests-159_passed-green)

**HWP, HWPX, PDF, DOCX** 파일을 깨끗한 **Markdown**으로 변환합니다.

Rust로 작성되어 **Pandoc보다 23%p, AI 기반 Marker보다 17%p 높은 품질**을 달성하면서도 10-100배 빠릅니다.

```
계약서.hwp  ──┐
보고서.pdf  ──┼──▶  MDM Engine (Rust)  ──▶  깨끗한 Markdown + 메타데이터
제안서.docx ──┘
```

---

## 5분 만에 시작하기

### 방법 1: Python (가장 쉬움)

```bash
pip install mdm-core
```

```python
import mdm_core

# 파일 하나를 마크다운으로 변환
md = mdm_core.convert("공고문.hwp")
print(md)

# PDF도 됩니다
md = mdm_core.convert("보고서.pdf")

# DOCX도 됩니다
md = mdm_core.convert("제안서.docx")

# HWPX도 됩니다
md = mdm_core.convert("채용공고.hwpx")
```

끝입니다. 이게 전부입니다.

### 방법 2: 커맨드 라인 (CLI)

```bash
# Rust 빌드 (최초 1회)
cd core && cargo build --release

# 변환
./target/release/hwp2mdm 계약서.hwp -o output/
./target/release/hwp2mdm 보고서.pdf -o output/
./target/release/hwp2mdm 제안서.docx -o output/
```

`output/` 폴더에 `.mdx` (마크다운) + `.mdm` (메타데이터 JSON) 파일이 생성됩니다.

### 방법 3: 웹 뷰어 (설치 없이)

`viewer/index.html`을 브라우저에서 열고, 파일을 드래그 앤 드롭하세요.

---

## 어떤 파일을 변환할 수 있나요?

| 형식 | 확장자 | 설명 | 지원 기능 |
|------|--------|------|-----------|
| **HWP** | `.hwp` | 한글 워드프로세서 | 텍스트, 표, 볼드/이탤릭, 각주, 이미지, 암호화 해제, 법률문서 구조 |
| **HWPX** | `.hwpx` | 한글 (XML 기반) | 텍스트, 표, 서식, 개요 제목 |
| **PDF** | `.pdf` | 범용 문서 | 텍스트, 제목 계층(H1-H4), 표, 볼드/이탤릭, 2단 레이아웃, 머리글/바닥글 제거 |
| **DOCX** | `.docx` | 마이크로소프트 워드 | 텍스트, 제목, 리스트, 표(병합 셀), 하이퍼링크, 각주, 인용문, 이미지 |

---

## 다른 도구와 비교

### DOCX 변환 품질 (39개 기능 테스트)

```
MDM (Rust)  ████████████████████████████████████████ 100% (39/39)
Pandoc      ██████████████████████████████           77% (30/39)
```

MDM만 지원하는 기능: GFM 테이블, 하이퍼링크, 중첩 리스트, 한글 넘버링(가나다)

### PDF 변환 품질 (29개 기능 테스트)

```
MDM (Rust)  ███████████████████████████████████████  93% (27/29)
Marker (AI) ████████████████████████████             76% (22/29)
pdftotext   █████████████████                        45% (13/29)
```

MDM이 AI 기반 Marker보다 높은 이유: 정확한 H1-H4 제목 감지, 인라인 볼드/이탤릭, 메타데이터 보존

### HWP 변환

```
MDM (Rust)  ████████████████████████████████████████ 경쟁자 없음
(세계 유일의 오픈소스 HWP→Markdown 변환기)
```

### 속도

| 도구 | DOCX | PDF |
|------|:----:|:---:|
| **MDM** | **14ms** | **20ms** |
| Pandoc | 64ms | - |
| Marker (AI+GPU) | - | ~7,000ms |

---

## AI 파이프라인에서 사용하기

### LangChain과 함께

```bash
pip install mdm-core[langchain]
```

```python
from mdm_core.langchain import MDMLoader

# 하나의 파일 로드
loader = MDMLoader("계약서.hwp")
docs = loader.load()

# 폴더 전체 로드 (HWP, PDF, DOCX 자동 감지)
loader = MDMLoader("./문서함/")
docs = loader.load()

# LangChain RAG 파이프라인에 바로 연결
from langchain_openai import ChatOpenAI
from langchain.chains import RetrievalQA
from langchain_community.vectorstores import FAISS
from langchain_openai import OpenAIEmbeddings

vectorstore = FAISS.from_documents(docs, OpenAIEmbeddings())
qa = RetrievalQA.from_chain_type(ChatOpenAI(), retriever=vectorstore.as_retriever())
answer = qa.invoke("이 계약서의 해지 조건은?")
```

### LlamaIndex와 함께

```bash
pip install mdm-core[llamaindex]
```

```python
from mdm_core.llamaindex import MDMReader
from llama_index.core import VectorStoreIndex

reader = MDMReader()
docs = reader.load_data(["공고.hwpx", "법률.pdf", "계약서.docx"])

index = VectorStoreIndex.from_documents(docs)
engine = index.as_query_engine()
response = engine.query("채용 자격 요건은?")
```

---

## 변환 결과는 어떻게 생겼나요?

### 입력: 행정안전부 청년인턴 채용 공고.hwpx

### 출력:

```markdown
---
format: hwpx
version: "1.0"
sections: 2
---

**행정안전부 공고 제2025 – 2377호**

2026년 제1기 행정안전부 청년인턴 채용 공고

| **근무기관(지역)** | **지원코드** | **채용분야** | **선발인원** |
| --- | --- | --- | --- |
| 행정안전부 본부(세종) | **인턴01** | **행정** | **16** |
| | **인턴02** | **홍보** | **7** |
| 지방자치인재개발원(전북 완주) | **인턴06** | **행정** | **12** |
...
```

표, 볼드, 구조가 그대로 보존됩니다.

---

## 프로젝트 구조

```
markdown-media/
├── core/                    # [Rust] 핵심 파서 엔진 (21,000+ LOC)
│   └── src/
│       ├── hwp/             #   HWP 파서 (OLE, 암호화, 법률문서)
│       ├── hwpx/            #   HWPX 파서 (XML)
│       ├── pdf/             #   PDF 파서 (레이아웃, 제목감지)
│       ├── docx/            #   DOCX 파서 (하이퍼링크, 각주)
│       ├── wasm.rs          #   WASM 바인딩 (브라우저용)
│       └── main.rs          #   CLI 도구
├── packages/
│   └── python/              # [Python] pip install mdm-core
│       └── python/mdm_core/ #   LangChain, LlamaIndex 로더
├── viewer/
│   └── index.html           # 웹 뷰어 (44KB, 설치 불필요)
├── tests/
│   ├── docx_benchmark/      # DOCX 벤치마크 (vs Pandoc)
│   ├── pdf_benchmark/       # PDF 벤치마크 (vs Marker)
│   └── benchmark_engine.py  # 정량 메트릭 (BLEU, edit distance)
└── samples/input/           # 테스트용 HWP/HWPX 파일
```

---

## 직접 빌드하기

### 필요한 것

- **Rust** 1.70+ ([설치](https://rustup.rs/))
- **Python** 3.8+ (Python 패키지 빌드 시)

### Rust 코어 빌드

```bash
git clone https://github.com/seunghan91/markdown-media.git
cd markdown-media

# 빌드
cd core && cargo build --release

# 테스트 (159개 전부 통과해야 합니다)
cargo test
```

### Python 패키지 빌드 (개발용)

```bash
pip install maturin
cd packages/python
maturin build --release
pip install target/wheels/mdm_core-*.whl
```

---

## 벤치마크 직접 돌려보기

```bash
# 테스트 파일 생성
python3 tests/docx_benchmark/generate_test_docx.py
python3 tests/pdf_benchmark/generate_test_pdfs.py

# DOCX: MDM vs Pandoc 비교
python3 tests/docx_benchmark/compare_quality.py

# PDF: MDM vs Marker vs pdftotext 비교
python3 tests/pdf_benchmark/compare_quality.py

# 정량 메트릭 (BLEU, edit distance)
python3 tests/benchmark_engine.py
```

---

## 이 프로젝트는 왜 만들었나요?

대한민국 정부 문서의 90%는 **HWP** 형식입니다. 하지만:

- AI(LLM)에 한국 공문서를 넣으려면 먼저 텍스트로 변환해야 합니다
- 기존 도구(Marker, Docling, MinerU)는 **HWP를 전혀 지원하지 않습니다**
- Pandoc은 DOCX 테이블을 망가뜨립니다
- Python 기반 도구는 느립니다

MDM은 이 문제를 해결합니다:
- **HWP 네이티브 파싱** — 세계 유일
- **Rust 성능** — Python 대비 10-100배 빠름
- **AI-Ready** — LangChain/LlamaIndex 즉시 연결

---

## 기여하기

모든 기여를 환영합니다! `CONTRIBUTING.md`를 참고하세요.

특히 다음 영역에 도움이 필요합니다:
- HWP 수식(equation) 파싱
- PDF OCR (스캔 문서)
- WASM 빌드 최적화 (C 의존성 제거)
- 실제 한국 공문서 테스트 케이스

---

## 라이선스

MIT License

---

**Author**: [seunghan91](https://github.com/seunghan91)
**Last Updated**: 2026.04.13
