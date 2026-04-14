# HTML 변환 대조 매트릭스 + Findings: MDM vs MarkItDown

**Date**: 2026-04-14
**MDM ref**: `core/src/html/mod.rs` (regex-based, 434 LOC)
**MarkItDown ref**: `_html_converter.py` (BeautifulSoup + markdownify wrapper)

## 아키텍처

| 측면 | MDM | MarkItDown |
|-----|-----|-----------|
| 파서 | 정규식 (regex crate + lazy_static) | BeautifulSoup (html.parser) |
| MD 변환 | 직접 치환 | markdownify (CustomMarkdownify 상속) |
| body 추출 | `<head>` 스트립 | `soup.find("body")` |

## 실측 비교 (tests/html_benchmark/test_basic.html, 사이클 시작 시점)

| 피처 | MDM (이전) | MDM (현재) | MarkItDown |
|-----|:--------:|:---------:|:---------:|
| Heading H1-H6 | ✅ | ✅ | ✅ |
| 볼드/이탤릭 | ✅ | ✅ | ✅ |
| 안전 하이퍼링크 | ✅ | ✅ | ✅ |
| 테이블 (`<th>`/`<td>`) | ✅ | ✅ | ✅ |
| Unordered 리스트 | ✅ `- item` | ✅ | ✅ `* item` |
| `<script>`/`<style>` 제거 | ✅ | ✅ | ✅ |
| **이미지 alt 보존** | ❌ `![]()` | ✅ `![A cat](photo.jpg)` | ✅ |
| **data: URI 잘라내기** | ❌ 풀 base64 덤프 | ✅ `data:mime;base64,...` | ✅ |
| **체크박스** | ❌ 완전 누락 | ✅ `[x]` / `[ ]` | ✅ |
| **javascript: URL 스트립** | ❌ 통과(XSS 위험) | ✅ 텍스트만 | ✅ |
| mailto: URL 보존 | ✅ | ✅ | ⚠️ **스트립 (MI 버그)** |
| tel: URL 보존 | ✅ | ✅ | ⚠️ 스트립 |
| HTML 엔티티 디코딩 | ✅ | ✅ | ✅ |
| 인코딩 감지 (UTF-8/EUC-KR) | ✅ | ✅ | ⚠️ UTF-8만 |

## 결정 테이블

| 후보 | 채용? | 이유 |
|-----|:---:|-----|
| 이미지 alt 보존 | ✅ 채용 | 정보 손실 제로 비용. MarkItDown도 보존 |
| data: URI 잘라내기 | ✅ 채용 | 인라인 base64 이미지로 인한 출력 비대 방지. 64자 이상일 때만 적용 |
| 체크박스 `[x]`/`[ ]` | ✅ 채용 | GFM 표준, HTML `<input type=checkbox>`의 의미 보존 |
| javascript:/vbscript:/data: 스트립 | ✅ 채용 | XSS 방어. **mailto:/tel:/#anchor는 유지** (MarkItDown이 틀리게 스트립하는 영역) |
| markdownify의 URL 퍼센트 이스케이프 | ❌ 기각 | 현재 regex 기반 구조와 충돌, 복잡도 대비 ROI 낮음 |
| MarkItDown의 공격적 스킴 화이트리스트(http/https/file만) | ❌ 기각 | **역채용**. mailto/tel 스트립은 MarkItDown 측 버그 |

## 구현 요약 (core/src/html/mod.rs)

### 추가된 regex
- `RE_IMG` (src 캡처 제거) + `RE_IMG_SRC`, `RE_IMG_ALT` — 속성 순서 무관하게 개별 추출
- `RE_INPUT`, `RE_INPUT_TYPE`, `RE_INPUT_CHECKED` — 체크박스 감지

### 추가된 헬퍼
- `truncate_data_uri(src)` — `data:*` 64자 초과 시 `data:mime;base64,...`로 축약
- `is_dangerous_url(href)` — `javascript:`/`vbscript:`/`data:` 탐지 (공백 허용, 대소문자 무시)

### 치환 규칙 변경
- 링크 치환: `is_dangerous_url()` 분기 — 위험 스킴은 텍스트만 남김
- 이미지 치환: src + alt 모두 추출 → `![alt](src-clean)`
- input 치환 (링크 앞 선행): checkbox면 `[x]`/`[ ]`, 그 외면 제거

## 변환 예시

입력:
```html
<a href="javascript:alert('xss')">Click</a>
<img src="data:image/png;base64,iVBOR...500자...Kg==" alt="inline">
<input type="checkbox" checked> Done
```

MDM 출력 (이전 → 이후):
```
이전: [Click](javascript:alert('xss'))        ← XSS 페이로드 그대로
이후: Click                                    ← 안전

이전: ![](data:image/png;base64,iVBOR...500자까지)   ← 비대
이후: ![inline](data:image/png;base64,...)            ← 축약 + alt 보존

이전: (완전 누락)
이후: [x] Done
```

## 회귀 검증

- 라이브러리 유닛 테스트: 218 → 228 passed (+10 신규 HTML 테스트)
- 기존 HTML 테스트 8개 모두 통과

## ROI 결론

- 4개 실질 개선 (alt 보존 / data URI 축약 / 체크박스 / 보안 URL 스트립)
- mailto/tel/`#anchor`는 MarkItDown이 **틀리게 스트립**하는 지점 — MDM이 명시적 화이트리스트 대신 블랙리스트를 택해 MarkItDown보다도 정확

스펙 목표 "1%+" 대폭 초과 달성.

다음 사이클: PDF (가장 복잡, 별도 파이프라인).
