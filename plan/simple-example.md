# MDM 시스템 간단한 예제

## 🎯 시나리오

블로그 프로젝트에서 미디어를 효율적으로 관리하는 예제입니다.

## 📁 프로젝트 구조

```
my-blog/
├── posts/
│   ├── 2024-01-15-welcome.md
│   ├── 2024-01-20-tutorial.md
│   └── 2024-01-21-review.md
├── assets/
│   ├── images/
│   │   ├── logo.png
│   │   ├── hero-bg.jpg
│   │   └── screenshots/
│   │       ├── app-main.png
│   │       └── app-settings.png
│   └── videos/
│       └── intro.mp4
└── blog.mdm           # 미디어 정의 파일
```

## 📄 blog.mdm (미디어 정의)

```yaml
version: 1.0
media_root: ./assets

# 전역 프리셋
presets:
  thumb: 
    width: 300
    height: 200
    object-fit: cover
  
  hero:
    width: 100%
    max-width: 1200
    height: 400
  
  inline:
    max-width: 768
    margin: "20px auto"

# 미디어 리소스
resources:
  # 브랜딩
  site-logo:
    type: image
    src: images/logo.png
    alt: "My Blog Logo"
    presets:
      header: { height: 50 }
      footer: { height: 30, opacity: 0.8 }
  
  # 히어로 이미지
  hero-welcome:
    type: image
    src: images/hero-bg.jpg
    alt: "환영 배너"
    loading: eager
    presets:
      mobile: { width: 100%, height: 200 }
      desktop: { width: 100%, height: 400 }
  
  # 스크린샷
  app-screenshot-main:
    type: image
    src: images/screenshots/app-main.png
    alt: "앱 메인 화면"
    presets:
      small: { width: 400 }
      large: { width: 800 }
      comparison: { width: 50% }
  
  app-screenshot-settings:
    type: image
    src: images/screenshots/app-settings.png
    alt: "앱 설정 화면"
    presets:
      small: { width: 400 }
      large: { width: 800 }
      comparison: { width: 50% }
  
  # 비디오
  intro-video:
    type: video
    src: videos/intro.mp4
    poster: images/intro-thumb.jpg
    duration: "1:30"
    presets:
      hero: { width: 100%, autoplay: true, muted: true, loop: true }
      inline: { width: 800, controls: true }
  
  # 외부 미디어
  youtube-demo:
    type: embed
    provider: youtube
    id: "abc123xyz"
    title: "제품 데모 영상"
```

## 📝 2024-01-15-welcome.md

```markdown
---
title: 블로그를 시작합니다!
date: 2024-01-15
---

# 블로그를 시작합니다!

![[hero-welcome:desktop]]

안녕하세요! 새로운 블로그에 오신 것을 환영합니다.

## 블로그 소개

![[site-logo:header | float=right margin="0 0 20px 20px"]]

이 블로그는 기술과 일상을 공유하는 공간입니다. 
주로 다음과 같은 내용을 다룰 예정입니다:

- 웹 개발 튜토리얼
- 프로젝트 리뷰
- 개발 도구 소개

## 첫 프로젝트 소개

최근에 만든 앱을 소개합니다:

![[intro-video:inline | caption="앱 소개 영상 (1분 30초)"]]

더 자세한 내용은 다음 포스트에서 다루겠습니다!
```

## 📝 2024-01-20-tutorial.md

```markdown
---
title: 앱 사용법 가이드
date: 2024-01-20
---

# 앱 사용법 가이드

이번 포스트에서는 앱의 주요 기능을 설명합니다.

## 메인 화면

![[app-screenshot-main:large | caption="앱의 메인 대시보드"]]

메인 화면에서는 다음과 같은 기능을 사용할 수 있습니다:

1. **대시보드** - 전체 현황 확인
2. **분석** - 상세 데이터 분석
3. **리포트** - 보고서 생성

## 설정 화면

![[app-screenshot-settings:small | float=right]]

설정에서는 다음을 커스터마이즈할 수 있습니다:

- 테마 변경 (라이트/다크)
- 언어 설정
- 알림 설정
- 데이터 백업

각 설정의 변경사항은 즉시 적용됩니다.

## 비교 화면

두 화면을 나란히 비교해보세요:

<div style="display: flex; gap: 20px;">
![[app-screenshot-main:comparison]]
![[app-screenshot-settings:comparison]]
</div>

## 동영상 튜토리얼

더 자세한 사용법은 아래 영상을 참고하세요:

![[youtube-demo | width=800 height=450]]
```

## 📝 2024-01-21-review.md

```markdown
---
title: 첫 프로젝트 회고
date: 2024-01-21
---

# 첫 프로젝트 회고

![[hero-welcome | opacity=0.7 height=300]]

## 프로젝트를 마치며

3개월간의 개발을 마치고 느낀 점을 정리해봅니다.

### 잘한 점

![[app-screenshot-main:thumb | float=left margin="0 20px 20px 0"]]

1. **사용자 중심 디자인**
   - 직관적인 UI
   - 빠른 반응 속도
   - 접근성 고려

2. **기술 스택 선택**
   - React + TypeScript
   - 타입 안정성 확보
   - 유지보수 용이

### 개선할 점

1. **성능 최적화**
   - 이미지 레이지 로딩
   - 코드 스플리팅
   - 캐싱 전략

2. **테스트 커버리지**
   - 현재 65%
   - 목표 80% 이상

## 마무리

![[intro-video:hero]]

앞으로도 꾸준히 개선해 나가겠습니다.
감사합니다! 🙏
```

## 🎨 렌더링 결과 예시

### MD 파일의 `![[hero-welcome:desktop]]`는:

1. **MDM 파일에서 찾기**
   - `hero-welcome` 리소스 확인
   - `desktop` 프리셋 적용

2. **HTML로 변환**
   ```html
   <img src="./assets/images/hero-bg.jpg" 
        alt="환영 배너"
        width="100%"
        height="400"
        loading="eager">
   ```

### MD 파일의 `![[app-screenshot-main:large | caption="설명"]]`는:

1. **MDM 설정 + 인라인 속성**
   ```html
   <figure>
     <img src="./assets/images/screenshots/app-main.png"
          alt="앱 메인 화면"
          width="800">
     <figcaption>설명</figcaption>
   </figure>
   ```

## 🚀 이 방식의 장점

1. **미디어 중앙 관리**
   - 모든 미디어 정보가 `blog.mdm`에 집중
   - 경로 변경 시 한 곳만 수정

2. **일관된 스타일**
   - 프리셋으로 통일된 크기
   - 반복 작업 최소화

3. **의미있는 이름**
   - `hero-welcome`처럼 용도가 명확
   - 파일명보다 이해하기 쉬움

4. **유연한 사용**
   - 기본 설정 사용 가능
   - 필요시 인라인으로 오버라이드

5. **마크다운 파일 깔끔**
   - 긴 경로나 복잡한 속성 없음
   - 가독성 향상