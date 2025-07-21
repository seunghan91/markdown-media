# MDM 파일 구조 및 참조 시스템

## 🎯 핵심 개념

MDM 시스템은 두 가지 파일로 구성됩니다:
1. **`.mdm` 파일** - 미디어 리소스 정의 및 관리
2. **`.md` 파일** - MDM에 정의된 리소스를 참조

## 📁 MDM 파일 구조

### 기본 구조 (YAML 형식)
```yaml
# project.mdm
version: 1.0
media_root: ./assets

# 미디어 리소스 정의
resources:
  # 이미지
  logo:
    type: image
    src: images/company-logo.png
    alt: "회사 로고"
    presets:
      small: { width: 100 }
      medium: { width: 300 }
      large: { width: 600 }
  
  hero-banner:
    type: image
    src: images/hero-bg.jpg
    alt: "메인 배너 이미지"
    responsive: true
    sizes: "(max-width: 640px) 100vw, 1200px"
  
  profile-photo:
    type: image
    src: images/profile.jpg
    alt: "프로필 사진"
    styles:
      rounded: { border-radius: 50% }
      square: { border-radius: 8px }
  
  # 동영상
  intro-video:
    type: video
    src: videos/introduction.mp4
    poster: videos/intro-thumb.jpg
    duration: "2:35"
    captions: 
      ko: videos/intro-ko.vtt
      en: videos/intro-en.vtt
  
  demo-screencast:
    type: video
    src: videos/product-demo.mp4
    controls: true
    presets:
      inline: { width: 800, controls: true }
      background: { autoplay: true, muted: true, loop: true }
  
  # 오디오
  podcast-ep1:
    type: audio
    src: audio/episode-001.mp3
    title: "Episode 1: Getting Started"
    duration: "45:23"
  
  # 외부 미디어
  youtube-tutorial:
    type: embed
    provider: youtube
    id: "dQw4w9WgXcQ"
    title: "Tutorial Video"

# 전역 프리셋 정의
presets:
  thumbnail: 
    width: 150
    height: 150
    object-fit: cover
  
  hero:
    width: 100%
    height: 400
    object-fit: cover
  
  article:
    max-width: 768
    margin: "0 auto"
```

### JSON 형식 대안
```json
{
  "version": "1.0",
  "media_root": "./assets",
  "resources": {
    "logo": {
      "type": "image",
      "src": "images/company-logo.png",
      "alt": "회사 로고",
      "presets": {
        "small": { "width": 100 },
        "medium": { "width": 300 },
        "large": { "width": 600 }
      }
    }
  }
}
```

## 📝 MD 파일에서의 참조 문법

### 기본 참조
```markdown
# 기본 형태 - MDM에 정의된 이름으로 참조
![[logo]]
![[hero-banner]]
![[intro-video]]
```

### 프리셋 적용
```markdown
# 미리 정의된 프리셋 사용
![[logo:small]]
![[logo:medium]]
![[demo-screencast:background]]
```

### 인라인 속성 오버라이드
```markdown
# 프리셋 + 추가 속성
![[logo:small | align=center]]
![[hero-banner | width=800 caption="메인 이미지"]]

# 프리셋 없이 속성만
![[profile-photo | width=200 style=rounded]]
```

### 캡션 추가
```markdown
# 짧은 캡션
![[hero-banner | caption="2024년 신제품"]]

# 긴 캡션 (멀티라인)
![[intro-video | caption="""
제품 소개 영상입니다.
주요 기능을 확인하세요.
"""]]
```

## 🔍 참조 해석 규칙

### 1. 이름 우선 해석
```markdown
![[resource-name:preset | additional-attributes]]
```

순서:
1. `resource-name` - MDM에 정의된 리소스 이름
2. `:preset` - (선택) 해당 리소스의 프리셋
3. `| attributes` - (선택) 추가 속성

### 2. 폴백 메커니즘
```markdown
![[unknown-resource]]
```
- MDM에 정의되지 않은 경우 → 파일명으로 간주
- 상대 경로로 이미지 찾기 시도

### 3. 네임스페이스 지원
```markdown
# 다른 MDM 파일 참조
![[shared:logo]]
![[components:button-icon]]
```

## 💡 고급 기능

### 1. 조건부 미디어
```yaml
# MDM 파일
resources:
  logo-adaptive:
    type: image
    variants:
      light: images/logo-light.svg
      dark: images/logo-dark.svg
    responsive:
      mobile: images/logo-mobile.svg
      desktop: images/logo-desktop.svg
```

```markdown
# MD 파일에서 사용
![[logo-adaptive]]  # 자동으로 적절한 버전 선택
```

### 2. 미디어 그룹
```yaml
# MDM 파일
groups:
  gallery-photos:
    - photo-1
    - photo-2
    - photo-3
    - photo-4
```

```markdown
# MD 파일에서 갤러리로 표시
![[gallery:gallery-photos | columns=2]]
```

### 3. 동적 속성
```yaml
# MDM 파일
resources:
  chart:
    type: image
    src: "charts/{{date}}-report.png"  # 동적 경로
    cache: false
```

## 📋 실제 사용 예제

### 프로젝트 구조
```
my-project/
├── docs/
│   ├── README.md
│   ├── guide.md
│   └── tutorial.md
├── assets/
│   ├── images/
│   ├── videos/
│   └── audio/
└── media.mdm        # 미디어 정의 파일
```

### media.mdm
```yaml
version: 1.0
media_root: ./assets

resources:
  # 공통 리소스
  app-logo:
    type: image
    src: images/logo.svg
    alt: "MyApp Logo"
    presets:
      header: { height: 40 }
      footer: { height: 30, opacity: 0.7 }
  
  # 스크린샷
  screenshot-dashboard:
    type: image
    src: images/screenshots/dashboard.png
    alt: "대시보드 화면"
    presets:
      thumbnail: { width: 300 }
      full: { width: "100%", max-width: 1200 }
  
  # 튜토리얼 비디오
  tutorial-getting-started:
    type: video
    src: videos/tutorials/getting-started.mp4
    poster: videos/tutorials/getting-started-thumb.jpg
    chapters:
      - { time: "00:00", title: "소개" }
      - { time: "02:15", title: "설치" }
      - { time: "05:30", title: "첫 프로젝트" }
```

### README.md
```markdown
# MyApp

![[app-logo:header | align=center]]

MyApp에 오신 것을 환영합니다!

## 주요 기능

![[screenshot-dashboard:thumbnail | float=right]]

- 실시간 데이터 분석
- 직관적인 대시보드
- 다양한 차트 지원

## 시작하기

다음 비디오를 통해 빠르게 시작해보세요:

![[tutorial-getting-started | controls=true width=800]]

## 갤러리

![[screenshot-dashboard:full | caption="메인 대시보드"]]
![[screenshot-analytics:full | caption="분석 화면"]]
![[screenshot-settings:full | caption="설정 화면"]]
```

## 🚀 장점

1. **중앙 집중식 미디어 관리**
   - 모든 미디어 리소스를 한 곳에서 관리
   - 일관된 메타데이터 유지
   - 버전 관리 용이

2. **재사용성**
   - 한 번 정의하고 여러 곳에서 사용
   - 프리셋으로 일관된 스타일 유지
   - 업데이트 시 모든 참조 자동 반영

3. **유연성**
   - 기본 설정 + 인라인 오버라이드
   - 다양한 미디어 타입 지원
   - 조건부/동적 미디어 처리

4. **가독성**
   - MD 파일이 깔끔하게 유지됨
   - 의미있는 이름으로 참조
   - 미디어 설정이 문서와 분리

## 📝 마이그레이션 전략

### 기존 마크다운에서 MDM으로
```bash
# 1. 미디어 파일 스캔
mdm scan --input ./docs --output media.mdm

# 2. 자동 변환
mdm convert --input ./docs --mdm media.mdm

# 3. 검증
mdm validate --mdm media.mdm --docs ./docs
```

## 🔧 도구 지원

### VS Code Extension
- MDM 파일 문법 하이라이팅
- 자동 완성 (리소스 이름, 프리셋)
- 미리보기
- 참조 점프 (Ctrl+Click)

### CLI 도구
```bash
# MDM 파일 검증
mdm validate media.mdm

# 미사용 리소스 찾기
mdm cleanup media.mdm --docs ./docs

# 미디어 최적화
mdm optimize media.mdm --output ./dist
```