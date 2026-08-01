# MDM 미디어 참조 문법 스펙 v3 (실물 기준)

## 개정 이력

- **v2 (폐기)**: 기호+`[[]]` 체계(`@`/`~`/`&`/`%`/`$`/`^`) 설계안. 문서만 존재했고 파서·CLI·뷰어 어디에도 구현되지 않았다.
- **v3 (현재)**: 실제 CLI(core/src/main.rs, `hwp2mdm`)가 생산하는 산출물과 뷰어(`packages/viewer-js`)가 실제로 파싱하는 문법을 기준으로 다시 썼다. 문법 단일화(로드맵 C5) 결정에 따라 v2 설계는 폐기했다.

## 정본 결정

1. **본문 이미지 참조 정본 = 표준 CommonMark** `![alt](path)`. HWP/HWPX/DOCX/PDF 4개 포맷 파서가 전부 이 문법으로 수렴한다.
2. **CLI 변환 산출물의 이미지 경로 = `assets/images/{content-hash12}.{ext}`**. SHA-256 콘텐츠 해시 앞 12자 기반. 동일 바이트의 이미지는 본문에 몇 번 참조되든 같은 경로를 재사용하고, 디스크엔 파일 하나만 저장된다(dedup).
3. **`.mdm` 사이드카 매니페스트 정본 = ManifestV2 JSON** (`version: "2.0"`, `assets[]` 배열). CLI가 항상 이 형식으로 생성한다.
4. 레거시 브래킷 문법 `![[name:preset | attrs]]`(Obsidian 스타일)은 **뷰어의 "수동 저작 확장"**으로 존치한다 — 사람이 직접 마크다운을 쓰면서 프리셋·속성을 쓰고 싶을 때. **CLI는 이 문법을 생성하지 않는다.**
5. v2가 제안했던 기호+`[[]]` 체계는 전부 폐기했다. 실물 코드 어디에도 없었다.

## 1. CLI 변환 산출물 (정본)

### 1.1 본문 이미지 참조

```markdown
![인원 현황표](assets/images/a1b2c3d4e5f6.png)
```

- **alt 텍스트**: 파서가 원본 문서에서 읽은 실제 alt/캡션 텍스트. 없으면 원본 파일명으로 대체한다(빈 alt를 emit하지 않는다).
- **경로**: 콘텐츠 SHA-256 해시 앞 12자 + 확장자, `assets/images/` 하위 상대경로. 소문자 확장자로 정규화된다.
- **dedup**: 동일 바이트를 가진 이미지가 본문에서 여러 곳 참조돼도 디스크엔 파일 하나만 존재하고, 두 참조 모두 같은 해시 경로를 가리킨다.

### 1.2 포맷별 배선 방식

각 파서는 원본 문서 구조에서 이미지 참조를 읽어내는 방식이 다르지만, 최종적으로 위 1.1의 표준 문법 하나로 수렴한다.

| 포맷 | 파서 | 본문 참조 배선 |
|---|---|---|
| HWP | `core/src/hwp/` | 레코드의 `bin_id`(십진)를 `BIN{:04X}` BinData 스트림명(16진)으로 역매핑한 뒤 CLI 레이어(main.rs)에서 실제 경로로 rewrite |
| HWPX | `core/src/hwpx/` | 파서가 본문에 `[이미지: id]` 마커를 emit → CLI 레이어에서 매니페스트 해시 경로로 치환 (표 셀 내부 포함) |
| DOCX | `core/src/docx/` | `w:drawing`(`wp:inline`/`wp:anchor` 둘 다) 파싱 시점에 실제 alt+원본 파일명으로 `![alt](filename)`을 본문에 직접 emit → CLI 레이어가 경로 부분만 해시 경로로 치환 |
| PDF | `core/src/pdf/` | 레이아웃 요소의 `ref_id`로 `![id](id)`를 본문에 직접 emit → CLI 레이어가 경로 부분만 해시 경로로 치환 |

본문에 실제로 쓰였는지 확인되지 않은(추출은 됐지만 참조되지 않은) 이미지는 문서 말미에 `## 이미지` 섹션으로 목록화해 소실을 막는다(HWPX/DOCX).

### 1.3 `.mdm` 사이드카 매니페스트 (ManifestV2 JSON)

```json
{
  "version": "2.0",
  "source": {
    "filename": "보도자료.hwpx",
    "format": "hwpx",
    "size_bytes": 114990,
    "hash": "c756716a...",
    "title": null,
    "author": null,
    "pages": null
  },
  "assets": [
    {
      "id": "image_001",
      "media_type": "image",
      "src": "assets/images/a1b2c3d4e5f6.png",
      "content_hash": "a1b2c3d4e5f6...",
      "original_name": null,
      "metadata": {
        "page": null,
        "width": 800,
        "height": 600,
        "format": "png",
        "caption": null,
        "alt_text": "인원 현황표"
      }
    }
  ],
  "stats": {
    "total_assets": 1,
    "images": 1,
    "tables": 0,
    "charts": 0,
    "equations": 0,
    "markdown_lines": 35,
    "markdown_chars": 4133,
    "conversion_ms": 0
  }
}
```

- `media_type`은 whitelist 없이 pass-through 한다 — 뷰어 로더가 아직 모르는 새 타입(예: 향후 `shape`)이 와도 로더 코드 수정 없이 통과시킨다.
- `assets[].id`의 자동 번호 규칙: `{type}_{순번:3자리}` — `image_001`, `table_001`, `chart_001`, `eq_001`(수식), 그 외(video/audio/embed)는 `asset_001`. 순번은 문서 내 등장 순.
- 뷰어 로더(`packages/viewer-js/src/mdm-loader.js`)는 `.mdm` 파일을 JSON으로 먼저 시도하고, `version: "2.0"` + `assets` 배열이면 내부 `resources` 맵으로 변환해 사용한다. JSON 파싱이 실패할 때만(순수 YAML일 때) §3의 레거시 형식으로 폴백한다.

## 2. 수동 저작 확장 — `![[name:preset | attrs]]` (뷰어 전용, CLI 미생성)

사람이 직접 마크다운을 쓸 때, `.mdm`에 미리 정의해 둔 리소스를 프리셋·속성과 함께 참조하고 싶으면 이 문법을 쓸 수 있다. **CLI 변환기는 이 문법을 생성하지 않는다** — 순수 뷰어(`packages/viewer-js`, `tokenizer.js`/`renderer.js`) 기능이며, §1의 표준 문법과 동시에 한 문서 안에서 섞어 써도 된다.

```markdown
![[hero-banner]]
![[hero-banner:desktop]]
![[photo.jpg | width=800 align=center caption="서울 야경" alt="서울 도심 야경 사진"]]
![[intro-video:inline | controls]]
```

### 형태

```
![[리소스]]
![[리소스 | 속성]]
![[리소스:프리셋]]
![[리소스:프리셋 | 속성]]
```

`리소스`가 `.mdm`의 `resources`(또는 ManifestV2 변환 후 내부 `resources`)에 있으면 그 정의를 쓰고, 없으면 파일 경로로 간주해 확장자로 타입을 추론한다(`renderDirectFile`).

### 지원 타입 (resource.type)

| type | 확장자 예 | 렌더 |
|---|---|---|
| `image` | jpg/jpeg/png/gif/webp/svg | `<img>`, `caption` 있으면 `<figure>`로 감쌈 |
| `video` | mp4/webm/ogv | `<video>` |
| `audio` | mp3/wav/ogg | `<audio>` |
| `embed` | — | `provider`(youtube/vimeo)에 따라 `<iframe>` |

### 속성

`width`/`height`(`w`/`h` 축약 없음, 그대로 `width=`/`height=`), `align`(left/center/right), `caption`, `alt`, `loading`, `controls`/`autoplay`/`muted`/`loop`(video/audio), `max-width`/`object-fit`/`opacity`/`float`(스타일).

## 3. 레거시 YAML 매니페스트 (수동 저작 확장 전용)

§2의 브래킷 문법용 사이드카는 YAML로도 작성할 수 있다 — CLI는 이 형식을 생성하지 않지만, 뷰어 로더가 하위 호환으로 계속 읽는다.

```yaml
version: "1.0"
media_root: ./assets

resources:
  hero-banner:
    type: image
    src: images/hero.jpg
    alt: "메인 배너"
    presets:
      mobile: { width: 375 }
      desktop: { width: 1200 }

presets:
  thumb: { width: 150, height: 150 }
```

## 4. 오프라인 무결성 검증

```bash
python3 scripts/verify_bundle.py --strict output/document.mdx
```

- `.mdx`의 모든 `![...](...)` 참조가 실제 파일로 해석되는지
- `[이미지:`/`@[[`/`![[` 같은 레거시 마커가 남아있지 않은지(`--strict`에서 실패 처리)
- `.mdm`의 asset 개수와 디스크 파일 개수가 일치하는지
- 모든 asset 파일 확장자가 소문자인지

## 5. 폐기된 설계 (v2, 미구현 — 참고용)

과거 이 문서가 제안했던 기호+`[[]]` 체계는 다음과 같았다: `@[[image.jpg]]`(이미지), `~[[table.svg]]`(표/차트), `&[[youtube:id]]`(임베드), `%[[video.mp4]]`(동영상), `$[[E=mc^2]]`(수식), `^[[audio.mp3]]`(오디오). 매니페스트도 `.mdm`을 YAML `resources` 맵으로 가정했다.

실제로는 어떤 파서·CLI·뷰어에도 구현되지 않았고, PDF 변환 경로가 한때 이미지 한정으로 `@[[id]]`를 부분 구현했었으나(§1 표준 문법으로 rewrite되기 전 중간 단계) 문법 단일화 과정에서 제거했다. 이 섹션은 과거 설계 의도를 기록하기 위해서만 남긴다 — 새 코드에서 참조하지 말 것.
