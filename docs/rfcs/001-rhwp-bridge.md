# RFC 001 — MDM ↔ rhwp 브리지

| 항목 | 값 |
|---|---|
| Status | Draft |
| Authors | seunghan91 (MDM), @edwardkim (rhwp, invited) |
| Created | 2026-04-16 |
| Target | MDM v0.3 · rhwp v0.6 |
| Depends on | [rhwp PR #153](https://github.com/edwardkim/rhwp/pull/153), [rhwp PR #154](https://github.com/edwardkim/rhwp/pull/154) |

## TL;DR

MDM(추출 엔진 — HWP/HWPX → Markdown)과 rhwp(뷰어/에디터 — HWP 원본 렌더링 + 편집)를 **하나의 공식 통합층**으로 묶자. 역할은 분명하게 분리하되, 공유 Document 모델과 ID 매핑 프로토콜을 통해 사용자 관점에서는 "같은 문서에 대한 두 가지 뷰"가 자연스럽게 전환되도록 한다. 결과: rhwp는 LLM 친화 출력을 얻고, MDM은 양방향 편집·원본 충실 렌더링을 얻는다.

## 배경

### 두 프로젝트의 미션 차이

| 항목 | MDM | rhwp |
|---|---|---|
| 미션 | HWP 콘텐츠를 LLM이 소비 가능한 Markdown으로 일방향 추출 | HWP 원본을 한컴 대등 품질로 뷰/에디트 |
| 우선 가치 | 의미 정보 보존, 배치 처리, 다중 포맷 파이프 | 픽셀 충실도, 라운드트립, 편집 UX |
| 코어 언어 | Rust (21K LOC), WASM, Tauri 데스크톱 | Rust (326 .rs 파일), WASM, Chrome/Safari/VS Code |
| 테스트 수 | 242 (lib) + 신규 4 golden | 789 |
| 라이선스 | MIT | MIT |

### 지금까지의 상호작용

1. MDM의 `core/src/hwp/cfb_lenient.rs`는 rhwp `LenientCfbReader`에서 포팅됨 (MDM → rhwp 차용)
2. rhwp PR #153, #154: MDM에서 찾은 보안/파싱 이슈를 rhwp로 역기여
3. MDM은 rhwp가 이미 가진 기능(equation, pagination, ruby, emphasis_dot 렌더링 구조)을 단계적으로 학습 중

이 관계는 **자연스러운 상호보완**으로 성숙하고 있다. 그러나 현재는 코드 복붙 + PR 단위 핑퐁이다. 사용자가 두 프로젝트를 하나의 경험으로 느끼려면 **공식 계약**이 필요하다.

## 비목표

- MDM이 rhwp를 흡수하거나 반대가 되는 것. 두 프로젝트는 독립 유지.
- rhwp의 편집 UX를 MDM이 복제하는 것. 모든 편집은 rhwp가 담당.
- MDM이 HWP writer(serializer)를 구현하는 것. 라운드트립이 필요하면 rhwp로 위임.
- 같은 언어/플랫폼 선택을 강요하는 것. rhwp는 HWP 원본 충실도를, MDM은 Markdown 추출 품질을 각자 최적화할 수 있어야 한다.

## 제안

### 1. 역할 분담 (계약)

```
                    ┌───────────────────────┐
                    │    사용자 (UI/CLI)    │
                    └──────────┬────────────┘
                               │
            ┌──────────────────▼───────────────────┐
            │         Bridge Adapter               │
            │  (공유 Document 모델 + ID 매핑)      │
            └──────┬───────────────────────┬───────┘
                   │                       │
       ┌───────────▼──────────┐   ┌────────▼──────────────┐
       │ MDM Core             │   │ @rhwp/core (WASM)     │
       │ ─ HWP → Markdown     │   │ ─ HWP → Canvas/SVG    │
       │ ─ 미디어 번들 생성   │   │ ─ 편집 커맨드 적용    │
       │ ─ Legal diff/annex   │   │ ─ HWP serializer      │
       └──────────────────────┘   └───────────────────────┘
```

**역할 규칙**

| 도메인 | 담당 | 이유 |
|---|---|---|
| 파싱 (읽기) | **둘 다** 수행. 상호 검증 용도 | 서로 다른 파서가 같은 문서에 도달하면 QA 효과 ↑ |
| 의미 추출 (markdown/JSON) | **MDM** | LLM 소비 우선 최적화 |
| 픽셀 렌더링 (Canvas/SVG) | **rhwp** | 한컴 parity는 별도 고난이도 과제 |
| 편집 커맨드 | **rhwp** | 이미 30 Actions + Field API 보유 |
| HWP 저장 (직렬화) | **rhwp** | `raw_data` 보존 라운드트립 구조 있음 |
| 신구 대조 / 법령 diff | **MDM** (1차). 향후 rhwp 기여 | `legal/chunker.rs` 기존 |
| AI 파이프(LLM 연동) | **MDM** | 한 방향 미션에 정합 |

### 2. 공유 Document 모델

**목표**: 한쪽이 만든 블록을 다른 쪽이 식별할 수 있도록 **안정적인 ID**를 부여한다. 그래야 "마크다운의 이 문단을 편집" → "rhwp가 HWP의 해당 문단을 수정" 왕복이 가능하다.

```ts
// packages/rhwp-bridge/src/types.ts (제안)
export interface BridgeBlockId {
  // rhwp 파라그래프 경로: section/para 인덱스 체인
  rhwpPath: number[];     // e.g. [0, 3, 2] = sec 0 > para 3 > child 2
  // MDM IR 블록 ID
  mdmBlockId: string;     // ULID
  // SHA-256 of original bytes for collision detection
  contentHash: string;
}

export interface BridgeDocument {
  // 원본 HWP/HWPX (디스크 경로 or ArrayBuffer)
  source: DocumentSource;
  // MDM의 추출 결과
  markdown: string;
  // rhwp의 파싱 결과 (선택 — lazy load)
  rhwpDoc?: RhwpDocument;
  // 블록 수준 ID 매핑 테이블
  idMap: BridgeBlockId[];
  // 사이드카 (메모, 커스텀 속성)
  sidecar?: MdmSidecar;
}
```

**ID 할당 알고리즘**

1. MDM이 HWP를 파싱할 때 각 IR 블록에 ULID 부여 + rhwp 경로 기록
2. rhwp가 해당 파일을 파싱할 때 같은 경로 규칙 사용 (`section.paragraph.run[.span]`)
3. 브리지가 두 결과를 매치해 `idMap` 생성
4. 내용 해시로 충돌 검증 (예: 표 셀이 위치만 같고 텍스트가 달라졌다면 → 재매핑)

### 3. 편집 위임 프로토콜

**시나리오**: 사용자가 MDM 뷰어 소스 창에서 특정 문단을 수정 → HWP로 저장하고 싶다.

```
MDM 뷰어                    Bridge                     rhwp WASM
  │                           │                           │
  │─ 수정된 문단 (blockId, new text) ──▶                   │
  │                           │── edit_paragraph(         │
  │                           │     path: [...],          │
  │                           │     text: "..."           │
  │                           │   ) ────────────────────▶ │
  │                           │                           │── 적용 + 재직렬화
  │                           │ ◀──── 수정된 HWP bytes ──│
  │ ◀── 저장 다이얼로그 ─────│                           │
```

**호출 규약** (bridge → rhwp):

```ts
// rhwp가 이미 가진 hwpctl API 기반
interface RhwpEditor {
  loadFile(data: ArrayBuffer): Promise<LoadResult>;
  editParagraph(path: number[], newText: string): Promise<void>;
  insertParagraph(path: number[], text: string): Promise<void>;
  deleteParagraph(path: number[]): Promise<void>;
  serialize(): Promise<ArrayBuffer>;
}
```

**중요**: 편집 명령은 **rhwp의 원본 모델**을 수정한다. MDM은 Markdown 편집본을 "의도"로 표현할 뿐이고, 실제 HWP 필드 업데이트는 rhwp가 원본의 스타일·폰트·위치를 보존하며 수행한다. 이게 MDM이 HWP writer를 직접 구현하지 않아도 되는 이유다.

### 4. 라운드트립 검증

브리지 CI에 다음 테스트를 붙인다:

```
for file in corpus/*.hwp{,x}:
    bytes_orig = read(file)
    doc        = rhwp.parse(bytes_orig)
    bytes_out  = rhwp.serialize(doc)
    assert bytes_orig == bytes_out       # bit-exact roundtrip
    md_orig    = mdm.extract(bytes_orig)
    md_out     = mdm.extract(bytes_out)
    assert md_orig == md_out             # extraction invariant
```

`rhwp`의 `raw_data` 보존 필드가 이 테스트를 가능하게 한다. 실패 시 두 프로젝트 중 어느 쪽 파서가 틀렸는지 즉시 드러남 (상호 QA 효과).

### 5. 충실도 뷰 / 브리지 UI

MDM 뷰어에 토글 추가:

| 모드 | 렌더러 | 용도 |
|---|---|---|
| Markdown | MDM | 기본. LLM 친화 |
| Side-by-side | MDM (좌) + rhwp Canvas (우) | 추출 품질 검증 |
| 원본 충실 | rhwp Canvas/SVG | "한컴처럼" 보기 |

이 토글은 **개발자·QA 도구**로서도 가치가 크다. "내 파서가 놓친 게 뭔가?"를 시각적으로 즉시 확인할 수 있다.

## 배포·버전 관리

### npm 패키지 토폴로지 (제안)

```
@mdm/core            — MDM Rust-WASM 래퍼 (기존)
@rhwp/core           — rhwp Rust-WASM 래퍼 (기존)
@rhwp/editor         — rhwp 에디터 UI (기존)
@mdm/rhwp-bridge     — 신규. 공유 Document 모델 + ID 매핑 + 편집 위임 어댑터
                        peer: @mdm/core ^0.3, @rhwp/core ^0.6
```

### 버전 호환성

- `@mdm/rhwp-bridge`는 두 peer 중 하나라도 breaking change를 내면 major bump
- CI에서 양쪽 최신 + 이전 안정판 조합 매트릭스 테스트
- Breaking change는 공동 RFC로 사전 합의

## 대안 검토

### A. 브리지 없이 그대로 두기

**비용**: 현재와 같이 PR 기반 역기여를 지속. 사용자는 여전히 "MDM 쓰다가 편집하려면 한컴 켜야" 한다.
**이득**: 제로 엔지니어링 비용.
**평가**: 두 프로젝트가 독립적으로 가치 있으므로 안전한 선택이지만, 상호 QA 효과와 양방향 UX를 포기.

### B. 완전 통합 (한 프로젝트가 다른 프로젝트 흡수)

**비용**: 조직/철학 충돌. 각자의 미션 최적화 포기.
**이득**: 하나의 배포 단위.
**평가**: 권장하지 않음. 두 프로젝트의 미션이 다르고 각자 이미 성숙.

### C. 본 RFC (브리지 패키지)

**비용**: 중간 수준. ID 매핑 + 라운드트립 CI + 편집 위임 프로토콜 구현.
**이득**: 두 프로젝트 모두 독립성 유지하면서 공동 UX 창출. 상호 QA로 파서 품질 지속 향상.
**평가**: **권장.**

## 단계적 로드맵

| 단계 | 범위 | 예상 기간 |
|---|---|---|
| 0 | 이 RFC 토론 + 합의 | 2주 |
| 1 | `@mdm/rhwp-bridge` 패키지 스캐폴드, 공유 타입 정의, ID 매핑 알고리즘 | 2주 |
| 2 | MDM 뷰어에 "충실도 뷰" 토글 (rhwp Canvas 위임) | 2주 |
| 3 | 편집 위임 프로토콜 — Markdown 편집 → HWP 저장 | 3-4주 |
| 4 | 라운드트립 CI + 상호 QA 레포트 | 2주 |
| 5 | 공동 배포: 두 프로젝트 릴리즈 노트 연계, 문서 링크 | 1주 |

**총**: 약 12-14주 (주요 기여자 1-2명 기준).

## 열린 질문

- [ ] `BridgeBlockId.rhwpPath`의 정확한 표현은? rhwp의 기존 `ParagraphId` 내부 표기를 따를지, 브리지가 독자 정의할지.
- [ ] 편집 위임 시 rhwp의 스타일 상속 규칙(현재 문단의 char shape)을 어떻게 Markdown 편집자에게 노출할지.
- [ ] MDM의 IR → rhwp Document 변환 방향은 필요한가? (역방향 매핑)
- [ ] 브리지 CI의 corpus — 저작권·개인정보 이슈 없는 공개 샘플 세트는 누가 관리?
- [ ] 버전 호환성 breakage를 미리 잡기 위한 contract test 프레임워크.

## 의사결정 기록

- **2026-04-16**: MDM 측 초안 작성 (seunghan91). 아직 edwardkim과 공식 토론 전.

## 다음 행동

1. **사용자 승인**: 이 RFC를 rhwp 쪽에 공유해도 되는지 확인.
2. **rhwp Discussion 오픈**: `edwardkim/rhwp/discussions` 에 요약 + 본 문서 링크.
3. **PoC**: `packages/rhwp-bridge/` 디렉토리에 타입·인터페이스만 먼저 구현 (2주).
4. **조인트 콜**: 범위·우선순위 합의 후 단계 1 시작.

---

> 이 RFC는 MDM 저장소의 `docs/rfcs/` 아래에 Draft 상태로 보관됩니다. 상태 변경(Draft → Proposed → Accepted → Implemented/Rejected)은 commit 메시지로 기록합니다.
