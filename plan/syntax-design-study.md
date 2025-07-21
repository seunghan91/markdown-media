# MDM 문법 디자인 연구

## 🎯 목적

마크다운 파일(.md)의 가장 큰 약점인 미디어(이미지, 동영상, 오디오) 처리를 개선하여 MD 파일을 더 효율적으로 사용할 수 있도록 하는 것입니다.

## 📌 현재 마크다운의 미디어 처리 문제점

### 1. 이미지 처리의 한계
```markdown
![alt text](image.jpg)
```
- ❌ 크기 조절 불가
- ❌ 정렬 제어 불가
- ❌ 캡션 추가 복잡
- ❌ 반응형 처리 불가
- ❌ 레이지 로딩 미지원

### 2. 동영상 지원 없음
- 표준 마크다운에는 동영상 문법 자체가 없음
- HTML `<video>` 태그 직접 사용해야 함
- 컨트롤, 자동재생, 반복 등 옵션 설정 복잡

### 3. 오디오 지원 없음
- 동영상과 마찬가지로 표준 문법 없음
- 팟캐스트, 음성 메모 등 삽입 어려움

## 🔍 제안된 `![[]]` 문법 분석

### 현재 제안
```markdown
![[filename.ext]{attributes}]]
```

### 장점
- ✅ Obsidian 사용자에게 친숙 (`![[]]` 기본 형태)
- ✅ 파일명이 먼저 와서 가독성 좋음
- ✅ 속성은 선택적 (기본 사용 간단)
- ✅ 확장 가능한 구조

### 잠재적 문제점
- ⚠️ `]]` 닫기가 두 번 반복되어 혼동 가능
- ⚠️ 속성 블록 `{}` 위치가 애매함
- ⚠️ 중첩된 대괄호로 파싱 복잡도 증가

## 🎨 대안 문법 검토

### 대안 1: 속성을 안쪽으로
```markdown
![[filename.ext {attributes}]]
```
- ✅ 닫는 괄호 하나로 명확
- ✅ 모든 내용이 `![[]]` 안에 포함
- ❌ 공백 처리 주의 필요

### 대안 2: 파이프 구분자
```markdown
![[filename.ext | width=500 align=center]]
```
- ✅ Obsidian의 별칭 문법과 일관성
- ✅ 구분이 명확
- ✅ 파싱 단순

### 대안 3: 콜론 구분자
```markdown
![[filename.ext : width=500 align=center]]
```
- ✅ 시각적으로 구분 명확
- ✅ URL과 유사한 느낌
- ❌ 콜론이 파일명에 있을 수 있음

### 대안 4: 이중 대괄호 분리
```markdown
![[filename.ext]][[width=500 align=center]]
```
- ✅ 각 부분이 독립적
- ✅ 선택적 속성 추가 명확
- ❌ 타이핑 양 증가

## 📋 미디어 타입별 문법 설계

### 1. 이미지
```markdown
# 기본
![[photo.jpg]]

# 크기 지정
![[photo.jpg | width=500]]
![[photo.jpg | width=50%]]
![[photo.jpg | size=medium]]  # 프리셋

# 정렬
![[photo.jpg | align=center]]
![[photo.jpg | float=right]]

# 캡션
![[photo.jpg | caption="설명 텍스트"]]

# 반응형
![[photo.jpg | responsive sizes="(max-width: 600px) 100vw, 50vw"]]

# 레이지 로딩
![[photo.jpg | loading=lazy]]

# 복합 속성
![[photo.jpg | width=800 align=center caption="메인 이미지" loading=lazy]]
```

### 2. 동영상
```markdown
# 기본
![[video.mp4]]

# 크기와 컨트롤
![[video.mp4 | width=640 controls]]

# 자동재생 (음소거)
![[video.mp4 | autoplay muted]]

# 반복 재생
![[video.mp4 | loop]]

# 포스터 이미지
![[video.mp4 | poster=thumbnail.jpg]]

# YouTube/Vimeo 임베드
![[youtube:dQw4w9WgXcQ | width=560 height=315]]
![[vimeo:123456789 | width=640]]
```

### 3. 오디오
```markdown
# 기본
![[audio.mp3]]

# 컨트롤과 자동재생
![[audio.mp3 | controls autoplay]]

# 반복
![[audio.mp3 | loop]]

# 볼륨 조절
![[audio.mp3 | volume=0.5]]
```

### 4. 기타 미디어
```markdown
# PDF
![[document.pdf | width=100% height=600]]

# iframe (외부 콘텐츠)
![[iframe:https://example.com | width=100% height=400]]

# 3D 모델
![[model.glb | width=500 height=500 controls]]
```

## 🔧 속성 문법 상세

### 속성 형식
```
key=value     # 기본
key="value"   # 공백 포함 시
key='value'   # 작은따옴표도 가능
key           # 불린 속성 (controls, autoplay 등)
```

### 단위 지원
- **픽셀**: `width=500` 또는 `width=500px`
- **퍼센트**: `width=50%`
- **뷰포트**: `width=50vw`
- **프리셋**: `size=medium`, `ratio=16:9`

## 🎯 프리셋 시스템 재설계

### 이미지 프리셋
```markdown
# 크기 프리셋
![[image.jpg | preset=thumbnail]]   # 150x150
![[image.jpg | preset=card]]         # 300x200
![[image.jpg | preset=hero]]         # 100%x400
![[image.jpg | preset=article]]      # 768px 최대폭

# 스타일 프리셋
![[image.jpg | style=rounded]]      # 둥근 모서리
![[image.jpg | style=circle]]       # 원형
![[image.jpg | style=shadow]]       # 그림자
```

### 동영상 프리셋
```markdown
![[video.mp4 | preset=background]]   # 배경 비디오 (muted autoplay loop)
![[video.mp4 | preset=hero]]         # 히어로 비디오
![[video.mp4 | preset=inline]]       # 인라인 플레이어
```

## 🌐 경로 처리

### 상대 경로
```markdown
![[./images/photo.jpg]]
![[../assets/video.mp4]]
```

### 절대 경로 (media_root 기준)
```markdown
# .mdm 파일에 media_root 정의 시
![[/photos/vacation.jpg]]  # {media_root}/photos/vacation.jpg
```

### 원격 URL
```markdown
![[https://example.com/image.jpg | cache=true]]
```

## 💡 최종 문법 제안

### 추천: 파이프 구분자 방식
```markdown
![[filename | attributes]]
```

**이유:**
1. **일관성**: Obsidian 별칭 문법과 유사
2. **가독성**: 파일명과 속성 명확히 구분
3. **확장성**: 속성 추가 용이
4. **파싱**: 구현 단순하고 명확
5. **타이핑**: 직관적이고 빠름

### 문법 예제
```markdown
# 이미지
![[photo.jpg]]
![[photo.jpg | width=500 align=center]]
![[photo.jpg | preset=hero caption="메인 비주얼"]]

# 동영상  
![[intro.mp4 | autoplay muted loop]]
![[tutorial.mp4 | width=800 controls poster=thumb.jpg]]

# 오디오
![[podcast.mp3 | controls]]
![[bgm.mp3 | autoplay loop volume=0.3]]

# 외부 미디어
![[youtube:videoId | width=640 height=360]]
![[vimeo:videoId | responsive]]
```

## 🚀 구현 우선순위

### Phase 1: 핵심 이미지 기능
1. 기본 이미지 삽입
2. 크기 조절 (width, height)
3. 정렬 (align, float)
4. 캡션
5. 대체 텍스트

### Phase 2: 고급 이미지 기능
1. 프리셋 시스템
2. 반응형 이미지
3. 레이지 로딩
4. 이미지 최적화

### Phase 3: 동영상/오디오
1. 기본 동영상 삽입
2. 컨트롤 옵션
3. 오디오 지원
4. 스트리밍 미디어

### Phase 4: 확장 기능
1. 외부 미디어 임베드
2. PDF 뷰어
3. 3D 모델
4. 갤러리 뷰

## 📝 결론

`![[filename | attributes]]` 문법은:
- ✅ 마크다운의 간결함 유지
- ✅ 강력한 미디어 제어 제공
- ✅ 학습과 사용이 쉬움
- ✅ 기존 도구와의 호환 가능성

이 문법으로 마크다운 파일에서 미디어를 효율적으로 다룰 수 있게 됩니다.