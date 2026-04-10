# MDM 품질 벤치마크 테스트셋

## Baseline (2026-04-10)

### 전수 비교 결과: MDM Before vs unhwp

| 지표 | MDM (Before) | unhwp | 갭 |
|------|:---:|:---:|:---:|
| 변환 성공 | 36/36 | 35/36 | MDM +1 (password-12345 암호화 해독) |
| Table rows | 3 | 3 | 동일 |
| Headings | 2 | 7 | unhwp +5 |
| Images | 0 | 5 | unhwp +5 |
| Bold/Italic | 0 | 있음 | unhwp 우세 |

### 기능별 테스트셋 분류

#### 1. 텍스트 서식 (Bold/Italic/Underline)
- `charshape.hwp` — Bold, Italic, 밑줄, 취소선, 폰트 크기/이름
- `charstyle.hwp` — 글자 스타일 적용
- `underline-styles.hwp` — 다양한 밑줄 스타일

**체크 기준**: `**text**`, `*text*` 패턴 존재 여부

#### 2. 표 (Table)
- `table.hwp` — 기본 표
- `table-caption.hwp` — 표 + 캡션
- `table-position.hwp` — 표 위치/정렬
- `sample-5017.hwp` — 복합 표 (2x2 + 캡션)

**체크 기준**: `|...|...|` 패턴, 셀 내용 정확도

#### 3. 제목/구조 (Heading)
- `sample-5017.hwp` — 문서 제목 + 본문
- `multicolumns-layout.hwp` — 다단 + 제목 구조
- `multicolumns-in-common-controls.hwp` — 복합 제목

**체크 기준**: `^# ` 패턴, outline level 감지

#### 4. 이미지 (Image)
- `sample-5017-pics.hwp` — 텍스트 + 이미지 혼합 (12 resources)
- `sample-5017.hwp` — 이미지 참조 (3 resources)
- `shapecomponent-rect-fill.hwp` — 도형 + 이미지
- `shapecontainer-2.hwp` — 복합 도형
- `shapepict-scaled.hwp` — 크기 조절된 이미지

**체크 기준**: `![alt](path)` 패턴, 이미지 파일 추출

#### 5. 각주/미주 (Footnote)
- `footnote-endnote.hwp` — 각주 + 미주

**체크 기준**: `[^n]` 또는 각주 텍스트 보존

#### 6. 목록 (List)
- `lists.hwp` — 순서 있는/없는 목록
- `lists-bullet.hwp` — 불릿 목록

**체크 기준**: `- ` 또는 `1. ` 패턴 정확도

#### 7. 레이아웃 (Layout)
- `multicolumns.hwp` — 다단 레이아웃
- `multicolumns-widths.hwp` — 다단 폭 지정
- `multicolumns-layout.hwp` — 다단 + 레이아웃
- `pagedefs.hwp` — 페이지 정의
- `linespacing.hwp` — 줄간격
- `paragraph-split-page.hwp` — 페이지 분할
- `parashape.hwp` — 문단 모양
- `aligns.hwp` — 정렬

#### 8. 머리말/꼬리말 (Header/Footer)
- `headerfooter.hwp` — 머리말/꼬리말

#### 9. 암호화 (Encryption)
- `password-12345.hwp` — 암호: 12345

**체크 기준**: 변환 성공 여부 (MDM 독보적 강점)

#### 10. HWPX 포맷
- `2026년 제1기 행정안전부 청년인턴 채용 공고(최종).hwpx` — 실제 공문서

**체크 기준**: 표 렌더링, Bold 감지, 텍스트 구조

#### 11. 기타/도형
- `textbox.hwp` — 텍스트 상자
- `shaperect.hwp` — 사각형 도형
- `shapeline.hwp` — 선 도형
- `facename.hwp` / `facename2.hwp` — 폰트 정보
- `borderfill.hwp` — 테두리/채우기
- `tabdef.hwp` — 탭 정의
- `matrix.hwp` — 매트릭스 구조

---

## 개선 목표 (After 달성 기준)

| 체크 항목 | Before | Target |
|-----------|:------:|:------:|
| Bold 감지 (**text**) | ❌ | ✅ |
| Italic 감지 (*text*) | ❌ | ✅ |
| Heading 감지 (# text) | ❌ (2개만) | ✅ (7개+) |
| Image 참조 (![](path)) | ❌ | ✅ |
| Table header Bold (HWPX) | ❌ | ✅ |
| 암호화 문서 해독 | ✅ | ✅ (유지) |
| 전체 변환 성공률 | 36/36 | 36/36 (유지) |

## 실행 커맨드

```bash
# Before/After 비교 실행
bash tests/benchmark/compare.sh mdm_before unhwp_baseline
bash tests/benchmark/compare.sh mdm_after unhwp_baseline
```
