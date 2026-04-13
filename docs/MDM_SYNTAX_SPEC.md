# MDM 미디어 참조 문법 스펙 v2

## 설계 원칙

1. **`[[]]` 더블 브라켓이 MDM 표식**: 더블 브라켓 자체가 "이것은 MDM 미디어 참조"라는 표식(sigil). 일반 텍스트에서 `[[`가 나올 확률은 극히 낮음
2. **접두사 기호 = 타입 선언**: `[[` 앞의 기호 하나로 미디어 타입을 즉시 식별
3. **빈도 기반 배정**: RISC-V 인코딩처럼, 자주 쓰이는 타입에 입력하기 쉬운 기호 배정
4. **충돌 없음**: `기호 + [[` 2-token 패턴은 기존 마크다운 어떤 문법과도 겹치지 않음 (검증 완료)

### 왜 충돌이 불가능한가

MDM 파서는 반드시 **`기호 + [[`** 연속 패턴을 찾습니다. 기호 단독(`~`, `$`, `@` 등)은 MDM으로 인식하지 않습니다.

- `~물결표시~` → 일반 텍스트 (뒤에 `[[` 없음)
- `$100` → 일반 텍스트 (뒤에 `[[` 없음)
- `~[[table.svg]]` → MDM 표/차트 (`~` + `[[` 패턴 일치)

이 2-token 패턴(`기호[[`)은 기존 마크다운/CommonMark/GFM 스펙의 어떤 문법과도 겹치지 않습니다:

| 기존 문법 | 패턴 | MDM 패턴 | 충돌 여부 |
|-----------|------|----------|:---------:|
| `~~strike~~` | `~~` + 텍스트 + `~~` | `~[[` + 내용 + `]]` | 없음 (싱글 `~` vs 더블 `~~`) |
| `$math$` | `$` + 수식 + `$` | `$[[` + 내용 + `]]` (닫는 `$` 없음) | 없음 |
| `[^1]` 각주 | `[^` | `^[[` (시작 문자 순서 다름) | 없음 |
| `![img](url)` | `![` | `@[[` (완전히 다른 문자) | 없음 |
| `&amp;` HTML | `&` + 영문 + `;` | `&[[` (브라켓은 유효한 엔티티 아님) | 없음 |
| `# Heading` | `#` + 공백 | MDM에서 `#` 미사용 (표/차트는 `~`) | 없음 |

> Obsidian의 `![[]]`가 `!` + `[[]]` 조합으로 마크다운 이미지(`![]()`))와 충돌 가능성이 있는 반면, MDM의 `@[[]]`는 `@`가 마크다운에서 아무 의미 없으므로 완전 안전합니다.

## 문법 테이블

| 기호 | 키보드 | 타입 | 빈도 | 문법 | 연상 |
|:----:|:------:|------|:----:|------|------|
| `@` | Shift+2 | 이미지 | 76% | `@[[photo.jpg]]` | @=위치(at) |
| `~` | Shift+` | 표/차트 | 39% | `~[[table.svg]]` | ~=파형(wave/grid) |
| `&` | Shift+7 | 임베드 | 24% | `&[[youtube:id]]` | &=연결(link) |
| `%` | Shift+5 | 동영상 | 10% | `%[[video.mp4]]` | %=진행률 |
| `$` | Shift+4 | 수식 | 5% | `$[[E=mc^2]]` | $=LaTeX 관례 |
| `^` | Shift+6 | 오디오 | 2% | `^[[audio.mp3]]` | ^=음파(wave) |

빈도 산출: 전통 문서(HWP/PDF/DOCX) 40% + 현대 웹 문서(Notion/Obsidian) 60% 가중 평균

## 문법 상세

### 기본 형태

```
기호[[리소스]]
기호[[리소스 | 속성]]
기호[[리소스:프리셋]]
기호[[리소스:프리셋 | 속성]]
```

### 이미지 `@[[ ]]`

```markdown
@[[photo.jpg]]
@[[photo.jpg | w=800 center]]
@[[photo.jpg | w=800 caption="서울 야경" alt="서울 도심 야경 사진"]]
@[[logo:header]]
@[[logo:thumbnail | align=right]]
```

지원 확장자: `.jpg`, `.jpeg`, `.png`, `.gif`, `.webp`, `.svg`, `.avif`

속성:
- `w` / `width`: 너비 (px 또는 %)
- `h` / `height`: 높이
- `align`: `left`, `center`, `right`
- `caption`: 캡션 텍스트
- `alt`: 대체 텍스트
- `float`: `left`, `right`

프리셋:
- `thumb`: 150px
- `small`: 480px
- `medium`: 768px
- `large`: 1024px
- `full`: 100%

비율 프리셋:
- `square`: 1:1
- `wide`: 16:9
- `portrait`: 3:4
- `story`: 9:16

### 표/차트 `#[[ ]]`

```markdown
~[[table_01.svg]]
~[[table_01.svg | caption="인원 현황표"]]
~[[chart_budget.png | w=600 center]]
~[[flowchart.svg | w=100%]]
```

지원 확장자: `.svg`, `.png`, `.jpg` (시각적 표/차트를 이미지로 추출한 것)

표/차트는 본문의 GFM pipe table(`| a | b |`)과 다릅니다. `#[[]]`는 원본 문서에서 복잡한 표를 이미지로 추출했을 때 참조하는 용도입니다.

### 임베드 `&[[ ]]`

```markdown
&[[youtube:dQw4w9WgXcQ]]
&[[youtube:dQw4w9WgXcQ | w=800 autoplay]]
&[[figma:file_id/node_id]]
&[[google-maps:place_id]]
&[[tweet:1234567890]]
&[[codepen:user/pen_id]]
&[[spotify:track:6rqhFgbbKwnb9MLmUQDhG6]]
&[[https://example.com/embed | w=100% h=400]]
```

형식: `&[[provider:id]]` 또는 `&[[url]]`

지원 프로바이더:
- `youtube`: YouTube 동영상
- `figma`: Figma 디자인
- `google-maps`: Google 지도
- `tweet` / `x`: Twitter/X 게시물
- `codepen`: CodePen
- `spotify`: Spotify 트랙/플레이리스트
- `github-gist`: GitHub Gist
- URL 직접: 임의의 iframe 임베드

### 동영상 `%[[ ]]`

```markdown
%[[intro.mp4]]
%[[intro.mp4 | autoplay muted loop]]
%[[intro.mp4 | w=800 poster=thumb.jpg controls]]
%[[promo.webm | w=100% caption="제품 소개"]]
```

지원 확장자: `.mp4`, `.webm`, `.mov`, `.avi`, `.ogv`

속성:
- `autoplay`: 자동 재생
- `muted`: 음소거
- `loop`: 반복 재생
- `controls`: 재생 컨트롤 표시
- `poster`: 썸네일 이미지 경로
- `w`, `h`, `caption`

### 수식 `$[[ ]]`

```markdown
$[[E = mc^2]]
$[[\int_0^1 f(x)\,dx = F(1) - F(0)]]
$[[\begin{pmatrix} a & b \\ c & d \end{pmatrix}]]
$[[formula_01.tex]]
```

내용: LaTeX 수식 문법 또는 `.tex` 파일 참조

기존 마크다운의 `$...$` (인라인 수식)과 구분:
- `$x^2$` → 인라인 수식 (기존 LaTeX 관례)
- `$[[x^2]]` → MDM 수식 블록 (별도 렌더링, 번호 매기기 가능)

### 오디오 `^[[ ]]`

```markdown
^[[podcast.mp3]]
^[[podcast.mp3 | controls]]
^[[narration.wav | autoplay]]
^[[bgm.ogg | loop muted]]
```

지원 확장자: `.mp3`, `.wav`, `.ogg`, `.aac`, `.flac`, `.m4a`

속성:
- `controls`: 재생 컨트롤
- `autoplay`: 자동 재생
- `loop`: 반복
- `muted`: 음소거

## 사이드카 매니페스트 (.mdm)

MDM 번들에는 `.mdm` 매니페스트 파일이 포함됩니다. 이 파일은 리소스 정의, 프리셋, 메타데이터를 관리합니다.

### 구조

```yaml
version: "2.0"
media_root: ./assets

resources:
  hero-banner:
    type: image
    src: images/hero.jpg
    alt: "메인 배너"
    presets:
      mobile: { w: 375 }
      desktop: { w: 1200 }

  budget-table:
    type: table
    src: tables/budget_2026.svg
    caption: "2026년 예산 현황"

  intro-video:
    type: video
    src: videos/intro.mp4
    poster: videos/intro-thumb.jpg
    duration: "2:35"

  team-podcast:
    type: audio
    src: audio/team-talk-ep1.mp3
    duration: "45:23"

presets:
  thumb: { w: 150, h: 150 }
  hero: { w: "100%", h: 400 }
  article: { max-w: 768 }
```

### 프리셋 참조

매니페스트에 정의된 리소스는 이름으로 참조:

```markdown
@[[hero-banner:desktop]]
#[[budget-table]]
%[[intro-video | controls]]
^[[team-podcast | controls]]
```

매니페스트에 없는 파일은 직접 경로로 참조:

```markdown
@[[images/screenshot.png]]
%[[videos/demo.mp4]]
```

## 변환 출력 번들 구조

MDM 변환기가 HWP/PDF/DOCX를 변환하면 다음 구조를 생성합니다:

```
output/
├── index.md          # 본문 (MDM 참조 문법 포함)
├── manifest.mdm      # 리소스 매니페스트 (YAML)
└── assets/
    ├── images/
    │   ├── image_001.png
    │   ├── image_002.jpg
    │   └── table_001.svg   # 복잡한 표를 SVG로 추출
    ├── videos/             # (원본에 동영상 있는 경우)
    └── audio/              # (원본에 오디오 있는 경우)
```

변환된 `index.md` 예시:

```markdown
---
source: "2026년_채용공고.hwp"
format: hwp
---

# 2026년 행정안전부 청년인턴 채용 공고

@[[images/header_logo.png | w=200 center]]

## 1. 선발예정인원 (총 114명)

~[[tables/table_001.svg | caption="선발 인원표"]]

## 2. 지원 자격

지원자는 다음 요건을 충족해야 합니다...
```

## 충돌 해소

MDM 문법은 기존 마크다운/CommonMark와 100% 호환됩니다.

| 기존 문법 | MDM 문법 | 구분 방법 |
|-----------|----------|-----------|
| `# Title` (heading) | `~[[table.svg]]` | `#` 뒤에 공백 vs `[[` |
| `$x^2$` (LaTeX inline) | `$[[E=mc^2]]` | `$` 뒤에 텍스트 vs `[[` |
| `[^1]` (footnote) | `^[[audio.mp3]]` | `[^` vs `^[[` |
| `![alt](src)` (image) | `@[[image.jpg]]` | `![` vs `@[[` |
| `&amp;` (HTML entity) | `&[[youtube:id]]` | `&` 뒤에 영문 vs `[[` |

파서 구분 규칙: **`기호 + [[`** 패턴이면 MDM, 그 외는 기존 마크다운.

## 인덱스 맵핑 시스템

MDM 번들에서 본문의 참조(`@[[image_003]]`)와 실제 파일(`assets/images/image_003.png`)을 연결하는 체계입니다.

### 매니페스트 인덱스

`manifest.mdm`의 `resources` 섹션이 인덱스 역할을 합니다:

```yaml
version: "2.0"
media_root: ./assets
source: "2026년_채용공고.hwp"

resources:
  # 자동 생성된 인덱스 (변환기가 생성)
  image_001:
    type: image
    src: images/image_001.png
    page: 1
    position: { x: 72, y: 650 }
    original_size: { w: 400, h: 300 }

  image_002:
    type: image
    src: images/image_002.jpg
    page: 2
    position: { x: 100, y: 400 }

  table_001:
    type: table
    src: tables/table_001.svg
    page: 1
    rows: 12
    cols: 4
    caption: "선발 인원표"

  table_002:
    type: table
    src: tables/table_002.svg
    page: 3
    rows: 5
    cols: 3

  # 사용자 정의 (수동 추가)
  hero-banner:
    type: image
    src: images/hero.jpg
    alt: "메인 배너"
    presets:
      mobile: { w: 375 }
      desktop: { w: 1200 }
```

### 자동 번호 규칙

변환기가 원본 문서에서 미디어를 추출할 때 다음 규칙으로 번호를 매깁니다:

```
{type}_{순번:3자리}

이미지:  image_001, image_002, image_003, ...
표:      table_001, table_002, ...
차트:    chart_001, chart_002, ...
수식:    eq_001, eq_002, ...
```

순번은 문서 내 출현 순서 (페이지 순 → 위→아래 순 → 왼→오른 순).

### 본문에서의 참조

자동 생성된 본문 (`index.md`):

```markdown
## 1. 선발예정인원

@[[image_001 | w=200 align=right]]

총 114명을 선발하며, 세부 내역은 아래 표와 같습니다.

~[[table_001 | caption="선발 인원표"]]

## 2. 예산 현황

~[[chart_001 | w=600 center]]

예산 산출 공식:

$[[\sum_{i=1}^{n} C_i \times R_i = T]]
```

### 역참조 (Reverse Lookup)

매니페스트에서 특정 리소스가 본문 어디에서 사용되는지 추적:

```yaml
resources:
  table_001:
    type: table
    src: tables/table_001.svg
    # 역참조 (자동 생성)
    used_in:
      - { file: "index.md", line: 15 }
      - { file: "appendix.md", line: 42 }
```

### 오프라인 무결성 검증

```bash
# 매니페스트의 모든 리소스 파일이 실제로 존재하는지 확인
hwp2mdm validate output/

# 결과:
#   image_001: OK (assets/images/image_001.png, 45KB)
#   image_002: OK (assets/images/image_002.jpg, 120KB)
#   table_001: OK (assets/tables/table_001.svg, 8KB)
#   table_003: MISSING (assets/tables/table_003.svg)
#   image_005: ORPHAN (매니페스트에 있지만 본문에서 미참조)
```

## 빈도 기반 설계 근거

미디어 타입별 출현 빈도 (전통 문서 40% + 현대 웹 60% 가중):

```
이미지   76%  ████████████████████████████████████████
표/차트  39%  ████████████████████
임베드   24%  ████████████
동영상   10%  █████
수식      5%  ██
오디오    2%  █
```

키보드 배정 원칙:
- Shift+2 (`@`) ~ Shift+7 (`&`): 숫자행 왼쪽→오른쪽 순
- 빈도 높은 타입은 왼쪽(타이핑 쉬움)에, 낮은 타입은 오른쪽에
- 예외: `$`(수식)는 LaTeX 국제 관례를 존중하여 Shift+4에 고정
