# Phase 2-3 병렬 작업 진행 중 (다른 팀 공지)

> 이 파일은 **작업 충돌 방지용 공지**입니다.
> 아래 파일들은 현재 **병렬 작업 팀**에서 작업 중입니다.

## 작업 범위 (잠금 대상)

### Phase 2.2 - Sidecar 파일 완전 구현
- `/Users/seunghan/MDM/markdown-media/packages/parser-js/src/presets.js` (새 파일)

### Phase 3.1 - npm 패키지 준비
- `/Users/seunghan/MDM/markdown-media/packages/parser-js/package.json` (업데이트)
- `/Users/seunghan/MDM/markdown-media/packages/parser-js/rollup.config.js` (새 파일)

### Phase 3.2 - PyPI 패키지 준비
- `/Users/seunghan/MDM/markdown-media/packages/parser-py/pyproject.toml` (업데이트)

### Phase 3.6 - Docker 컨테이너
- `/Users/seunghan/MDM/markdown-media/Dockerfile` (업데이트)

### Phase 3.7 - 통합 테스트 스펙
- `/Users/seunghan/MDM/markdown-media/tests/spec/` (새 디렉토리)

## 이미 다른 팀에서 작업 중인 것들 (건드리지 않음)
- ⚠️ Phase 3.4 CI 워크플로우 (ci.yml) - **C팀 작업 중**
- ⚠️ Phase 3.5 릴리스 워크플로우 (release.yml) - **C팀 작업 중**
- ⚠️ Phase 1.5 테이블 SVG 렌더러 - **다른 팀 작업 중**

## 상태
- **상태**: ✅ 완료
- **작성일**: 2025-12-31
- **완료일**: 2025-12-31

## 생성된 파일 목록

### Phase 2.2
- ✅ `/markdown-media/packages/parser-js/src/presets.js` (340줄)

### Phase 3.1
- ✅ `/markdown-media/packages/parser-js/rollup.config.js` (147줄)
- ✅ `/markdown-media/packages/parser-js/package.json` (업데이트됨)

### Phase 3.2
- ✅ `/markdown-media/packages/parser-py/pyproject.toml` (업데이트됨 - dev/docs deps 추가)

### Phase 3.6
- ✅ `/markdown-media/Dockerfile` (멀티스테이지 빌드, 153줄)

### Phase 3.7
- ✅ `/markdown-media/tests/spec/README.md`
- ✅ `/markdown-media/tests/spec/basic/*.md`, `*.expected.json` (3개 테스트)
- ✅ `/markdown-media/tests/spec/presets/*.md`, `*.expected.json` (1개 테스트)
- ✅ `/markdown-media/tests/spec/sidecar/*.md`, `*.mdm`, `*.expected.json` (1개 테스트)
- ✅ `/markdown-media/tests/runners/run-js.js` (273줄)
- ✅ `/markdown-media/tests/runners/run-py.py` (219줄)

