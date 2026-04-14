# DOCX Findings: MDM × MarkItDown

**Date**: 2026-04-14
**Source matrix**: [docx-matrix.md](docx-matrix.md)

## 실측 결과

| 테스트 문서 | MDM | MarkItDown | 비고 |
|-----------|:---:|:---------:|-----|
| `test_comprehensive.docx` (24 피처) | ✅ 24/24 (100%) | ❌ **FileConversionException** | MarkItDown가 footnote 처리 KeyError로 변환 실패 |
| `test_korean_gov.docx` (8 피처) | ✅ 8/8 (100%) | ✅ 변환 성공 (피처 비교 없음) | 둘 다 변환 |
| `test_tables.docx` (7 피처) | ✅ 7/7 (100%) | ✅ 변환 성공 | 둘 다 변환 |
| `test_equations.docx` (신규) | ✅ 수식 4개 모두 LaTeX 보존 | ⚠️ 수식 4개 모두 **드롭** (빈 문단) | **MDM 우세** |

실측 환경:
- MDM: 현재 브랜치 (release build, 수식 지원 추가 후)
- MarkItDown: pip latest `markitdown[docx]` (mammoth + pre_process_docx 의존)

## 결정 테이블

| 후보 | 채용? | 이유 |
|-----|:---:|-----|
| **OMML → LaTeX 변환** | ✅ 채용 | MDM의 명백한 누락. MarkItDown은 구현되어 있으나 일부 문서에서 실패. MDM이 Rust로 재구현하여 안정성 + 기능 모두 확보 |
| markdownify 하이퍼링크 title 속성 | ❌ 기각 | 가치 낮음. DOCX는 mammoth가 title 속성을 거의 안 내보냄 |
| JavaScript URL 스트립 | ❌ 기각 | DOCX에서 js: URL은 존재하지 않음 (웹 문서에만 의미) |
| URL 퍼센트 이스케이프 | ❌ 기각 | 현재 MDM URL은 DOCX relationship 그대로 사용 — DOCX URL에는 특수문자 거의 없음 |
| 콘텐츠 컨트롤 체크박스 `[x]`/`[ ]` | ❌ 기각 | DOCX 체크박스(w:sdt/w:checkBox)는 현업에서 드묾, ROI 낮음 |
| 인접 리스트 병합 버그 (MarkItDown 단점) | N/A | MDM은 이미 옳게 처리 — 채용 불필요 |

## 구현 요약

### 신규 파일
- **`core/src/docx/math.rs`** (496 LOC, MIT attribution) — OMML → LaTeX 스트리밍 변환기
  - 스택 기반, quick_xml 이벤트 주도
  - 지원 구조: `m:r/m:t`, `m:sSub/m:sSup/m:sSubSup`, `m:f`(fraction), `m:rad`(radical + degree), `m:d`(delimiter + begChr/endChr), `m:nary`(sum/∫/∏/∐/∬/∭/∮/∧/∨/∩/∪), `m:func/m:fName`(function apply), `m:acc`(accent), `m:bar`(over/underline), `m:limLow/m:limUpp`, `m:groupChr`
  - `*Pr` 프로퍼티 프레임: `naryPr/dPr/accPr/barPr/groupChrPr`의 child `m:chr`/`m:begChr`/`m:endChr`/`m:pos` 값 캡처
  - LaTeX 이스케이프: `\`, `{`, `}`, `$`, `%`, `&`, `#`, `_`, `^`, `~`
  - 알 수 없는 태그는 자식 텍스트로 graceful degradation
  - 유닛 테스트 12개 포함
- **`tests/docx_benchmark/generate_equation_docx.py`** — OMML을 직접 XML 주입하는 테스트 문서 생성기

### 수정 파일
- **`core/src/docx/mod.rs`** — `pub mod math;` 추가
- **`core/src/docx/parser.rs`** — 메인 XML 이벤트 루프에 OMML 라우팅 4곳 추가:
  - `Event::Start` at `<m:oMath>` / `<m:oMathPara>`: OmmlBuilder 시작
  - `Event::Start/Empty/Text` (수식 영역 내부): OmmlBuilder로 포워딩
  - `Event::End` at 가장 바깥 `<m:oMath>` / `<m:oMathPara>`: finish() → `$...$` / `$$...$$` TextRun 삽입

## 변환 예시

입력 OMML (원본 DOCX 내):
```xml
<m:oMathPara>
  <m:oMath>
    <m:nary>
      <m:naryPr><m:chr m:val="∑"/></m:naryPr>
      <m:sub><m:r><m:t>i=1</m:t></m:r></m:sub>
      <m:sup><m:r><m:t>n</m:t></m:r></m:sup>
      <m:e><m:sSup>
        <m:e><m:r><m:t>i</m:t></m:r></m:e>
        <m:sup><m:r><m:t>2</m:t></m:r></m:sup>
      </m:sSup></m:e>
    </m:nary>
  </m:oMath>
</m:oMathPara>
```

MDM 출력:
```
$$\sum_{i=1}^{n} {i}^{2}$$
```

MarkItDown 출력:
```
(비어 있음 — 문단 자체가 사라짐)
```

## 회귀 검증

- **Library 유닛 테스트**: 201 → 213 passed (+12 신규 수식 테스트, 0 회귀)
- **DOCX 벤치마크 (vs Pandoc)**: 39/39 (100%) — 이전과 동일, 회귀 없음
- **성능**: 비수식 DOCX 변환 속도 변화 없음 (math_builder는 `Option::None`일 때 0 비용)

## ROI 결론

- **수식 보존율**: 0% → 100% (4/4 수식 완벽 변환)
- **구조 보존율 (종합)**: MDM은 MarkItDown이 실패하는 문서(footnote 포함)도 완벽 변환
- 스펙 목표 "1%+ 진보" 초과 달성

다음 사이클 제안: **PPTX** (OOXML 패밀리, DOCX 학습 재활용 가능). OMML 수식 핸들러는 PPTX에서도 바로 재사용 가능할 가능성이 높음(PowerPoint 슬라이드 내 수식).
