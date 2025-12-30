# core/src/main.rs 빌드 에러 최소 수정 (임시 공지)

> 이 파일은 **작업 충돌 방지용 공지**입니다.
> `markdown-media/core/src/main.rs`에서 `FileStructure`에 없는 `version` 필드를 참조하여
> `cargo build --release`가 실패하는 문제가 확인되어, 빌드가 깨지지 않도록 **최소 수정만** 진행합니다.

## 수정 범위(잠금 대상)
- `/Users/seunghan/MDM/markdown-media/core/src/main.rs` (`show_hwp_info()`의 version 출력/JSON)

## 수정 원칙
- `FileStructure`에 필드를 추가하지 않고, **`HwpParser::extract_metadata()`로 version을 가져오기**
- 기능 확장/리팩터링은 하지 않음 (merge 충돌 최소화)

## 상태
- **상태**: 확인 완료 (현재 `cargo build --release` 성공) / 추가 수정 없음
- **작성일**: 2025-12-31

