# Phase 3 통합검수 수정 작업 기록 (Integration Audit)

> 이 문서는 “Phase 3까지 완료” 상태에서 통합검수 중 발견된 **경로/워크플로우 정합성 이슈**를 정리하고,
> 필요한 경우 최소 수정(Hotfix)을 적용했음을 기록하기 위한 파일입니다.

## 발견 이슈 요약
- `packages/parser-js`(루트)와 `markdown-media/packages/parser-js`(서브) 사이에 **패키징/빌드 설정 불일치**
  - CI/Release 워크플로우는 `packages/parser-js`를 참조
  - 패키징용 `rollup.config.js`, `src/presets.js`, exports 등이 루트에 없거나 불완전할 수 있음
- `tests/spec`, `tests/runners` 경로가 계획서와 실제 구현(`markdown-media/tests/...`) 사이에 불일치

## 조치 원칙
- 계획서 경로를 만족시키기 위해 루트 경로에 **래퍼/미러 파일**을 추가할 수 있음
- CI/Release에서 “실패”를 유발하는 부분은 최소 수정으로 정상 동작하도록 조정

## 상태
- **작성일**: 2025-12-31
- **상태**: 통합검수 진행 중

