# MDM 프로젝트 계획 문서

이 폴더는 MDM (Markdown+Media) 프로젝트의 상세한 구현 계획과 전략 문서들을 담고 있습니다.

## 📚 문서 목록

### 1. [roadmap.md](./roadmap.md) - 프로젝트 로드맵
기존 plan.md 파일로, MDM 프로젝트의 전체적인 로드맵과 단계별 마일스톤을 담고 있습니다.
- Phase 1: JavaScript MVP (2025 Q4 - 2026 Q2)
- Phase 2: Python 생태계 (2026 Q2 - Q3)
- Phase 3: Rust 고성능 코어 (2026 Q3 - Q4)
- Phase 4: 커뮤니티 및 Playground (진행중)

### 2. [implementation-guide.md](./implementation-guide.md) - 구현 가이드
MDM 프로젝트의 기술적 아키텍처와 구체적인 구현 방안을 설명합니다.
- 파서 구조 (Tokenizer → Parser → Transformer → Renderer)
- 각 언어별 구현 세부사항
- 프로젝트 구조 및 모듈 설계
- 성능 목표 및 보안 고려사항

### 3. [javascript-parser-plan.md](./javascript-parser-plan.md) - JavaScript 파서 상세 계획
JavaScript 파서의 구체적인 구현 계획과 코드 예제를 포함합니다.
- 프로젝트 구조 및 파일 조직
- Tokenizer, Parser, Renderer 구현 상세
- 이미지 프리셋 시스템
- API 설계 및 사용 예제
- 패키지 배포 계획

### 4. [testing-strategy.md](./testing-strategy.md) - 테스트 전략
크로스 언어 호환성을 보장하기 위한 포괄적인 테스트 전략입니다.
- 언어 중립적 스펙 테스트
- 테스트 케이스 카테고리 (기본, 프리셋, 엣지케이스, 호환성)
- 각 언어별 테스트 러너
- CI/CD 통합 및 호환성 매트릭스
- 성능 벤치마크

### 5. [todo-immediate-tasks.md](./todo-immediate-tasks.md) - 즉시 시작 가능한 작업
당장 시작할 수 있는 구체적인 작업들과 TODO 리스트입니다.
- JavaScript Parser MVP 초기 설정 명령어
- 첫 번째 구현 작업 목록
- 전체 TODO 리스트 (Phase별)
- 우선순위별 작업 분류
- 빠른 시작 스크립트

## 🚀 빠른 시작

MDM 프로젝트 개발을 시작하려면:

1. **계획 확인**: `roadmap.md`를 읽고 전체 프로젝트 방향을 이해하세요.
2. **구현 이해**: `implementation-guide.md`로 기술적 아키텍처를 파악하세요.
3. **작업 시작**: `todo-immediate-tasks.md`의 빠른 시작 스크립트를 실행하세요.

```bash
# JavaScript 파서 개발 시작
cd packages/parser-js
bash ../../plan/quick-start.sh  # (스크립트는 todo-immediate-tasks.md 참조)
```

## 📊 현재 상태

### Phase 1: JavaScript Parser (MVP)
- **상태**: 시작 준비 완료 🟢
- **목표**: 2025 Q4 - 2026 Q2
- **다음 단계**: Tokenizer 구현

### Phase 2: Python Implementation
- **상태**: 계획 수립 완료 🟡
- **목표**: 2026 Q2 - Q3
- **의존성**: JavaScript MVP 완성 후

### Phase 3: Rust Core
- **상태**: 계획 수립 완료 🟡
- **목표**: 2026 Q3 - Q4
- **의존성**: 크로스 언어 테스트 프레임워크

## 🎯 핵심 목표

1. **CommonMark 100% 호환성** - 기존 Markdown 문서와 완벽한 호환
2. **직관적인 `![[]]` 문법** - 배우기 쉽고 사용하기 편한 문법
3. **고성능** - 실시간 애플리케이션에서도 빠른 파싱
4. **크로스 플랫폼** - JavaScript, Python, Rust 모든 환경 지원

## 📝 기여 방법

1. 해당 문서를 참고하여 작업 선택
2. `todo-immediate-tasks.md`에서 미완료 작업 확인
3. 브랜치 생성 후 작업 시작
4. 테스트 작성 및 통과 확인
5. PR 제출

## 🔗 관련 링크

- [메인 README](../README.md)
- [시작 가이드 (한국어)](../start.ko.md)
- [시작 가이드 (English)](../start.md)

---

_이 문서들은 MDM 프로젝트의 성공적인 구현을 위한 상세한 가이드입니다. 질문이나 제안사항이 있다면 Issue를 생성해주세요._