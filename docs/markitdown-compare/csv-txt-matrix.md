# CSV + TXT 변환 대조 매트릭스 + Findings: MDM vs MarkItDown

**Date**: 2026-04-14
**MDM refs**: `core/src/csv_parser.rs` (190 LOC), `core/src/txt_parser.rs` (148 LOC)
**MarkItDown refs**: `_csv_converter.py` (78 LOC), `_plain_text_converter.py` (72 LOC)

## CSV 피처 대조

| 피처 | MDM | MarkItDown | 비고 |
|-----|:---:|:---------:|-----|
| CSV 기본 파싱 | ✅ `csv` crate (RFC 4180) | ✅ Python `csv` 모듈 | 동등 |
| 따옴표 내 콤마 | ✅ | ✅ | 동등 |
| **`.tsv` 자동 인식 (탭 구분자)** | ✅ auto-detect | ❌ `.tsv`는 plain text 처리 | **MDM 우세** |
| **셀 내 파이프 `\|` 이스케이프** | ✅ `\|` | ❌ **표 구조 파손** | **MDM 우세** (XLSX와 동일 버그) |
| **셀 내 개행 평탄화 (신규)** | ✅ 공백으로 변환 | ❌ 원본 유지 → GFM 파손 | **MDM 우세** (개선됨) |
| 짧은 행 패딩 | ✅ | ✅ | 동등 |
| 긴 행 자르기 | ❌ | ✅ (헤더 열 수로) | MarkItDown 우세 (미미, 거의 발생 안 함) |
| 문자 인코딩 감지 | ❌ UTF-8 가정 | ✅ `charset-normalizer` | MarkItDown 우세 |
| 프론트매터(행/열 수) | ✅ | ❌ | MDM 우세 |
| 열 폭 정렬 패딩 | ✅ | ❌ | MDM 우세 (가독성) |

## TXT 피처 대조

| 피처 | MDM | MarkItDown | 비고 |
|-----|:---:|:---------:|-----|
| UTF-8 pass-through | ✅ | ✅ | 동등 |
| UTF-8 BOM strip | ✅ | ⚠️ charset-normalizer가 일부 처리 | 동등 |
| **UTF-16 LE/BE BOM 감지 (신규)** | ✅ | ⚠️ charset-normalizer 의존 | MDM이 명시적 |
| EUC-KR 폴백 | ✅ | ⚠️ charset-normalizer | 동등 (MDM은 명시, MI는 감지) |
| Shift_JIS / Big5 등 | ❌ | ⚠️ charset-normalizer 지원 | MarkItDown 우세 |
| CRLF 정규화 | ✅ | ❌ | MDM 우세 |
| 후행 공백 트리밍 | ✅ | ❌ | MDM 우세 |
| 빈 줄 3연속 이상 축약 | ✅ (최대 2) | ❌ | MDM 우세 |
| 프론트매터 | ✅ | ❌ | MDM 우세 |

## 이번 사이클 구현

### CSV: 셀 내 개행 평탄화 (core/src/csv_parser.rs)

`escape_pipe`를 확장하여 GFM 파이프 표의 line-oriented 특성에 맞게 셀 내부 `\r\n` / `\n` / `\r`을 공백으로 치환. MarkItDown은 이 처리를 하지 않아 따옴표 CSV에서 표 구조가 실제로 파손됨.

입력:
```csv
A,B
1,"line1
line2"
```

MarkItDown 출력 (파손):
```
| A | B |
| --- | --- |
| 1 | line1
line2 |
```

MDM 출력 (정상):
```
| A | B |
| - | ----------- |
| 1 | line1 line2 |
```

### TXT: UTF-16 BOM 감지 (core/src/txt_parser.rs)

`decode_text`에 UTF-16 LE (`FF FE`) / BE (`FE FF`) BOM 감지 추가. Windows에서 메모장으로 저장한 유니코드 텍스트 파일이 대표적으로 UTF-16 LE.

## 결정 테이블

| 후보 | 채용? | 이유 |
|-----|:---:|-----|
| CSV 셀 개행 평탄화 | ✅ 채용 | GFM 정합성 — MDM이 MarkItDown을 명확히 앞섬 |
| UTF-16 BOM 감지 | ✅ 채용 | Windows 텍스트 파일 호환성, 낮은 리스크 |
| 긴 CSV 행 자르기 | ❌ 기각 | 실제 데이터 손실 위험, 드묾 |
| 풀 `charset_normalizer` 포팅 (Shift_JIS 등) | ❌ 기각 | 별도 crate 의존성 + 잘못 감지 위험, 저ROI |

## 회귀 검증

- 라이브러리 유닛 테스트: 228 → 233 passed (+5)
  - CSV: `test_escape_pipe_flattens_newlines`, `test_quoted_multiline_field_flattened_in_output`
  - TXT: `test_decode_utf16_le_bom`, `test_decode_utf16_be_bom`, `test_decode_utf16_korean`

## ROI 결론

- CSV에서 실제 데이터 손실 케이스 하나 제거(개행 평탄화)
- TXT에서 Windows 유니코드 파일 호환성 추가
- 전체적으로 CSV는 **MDM이 완연히 우세**, TXT는 정규화 측면에서 우세·인코딩 범위는 열세

남은 사이클: PDF (최종, 가장 복잡).
