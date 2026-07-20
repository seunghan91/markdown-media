# MDM Desktop — Windows 빌드 가이드

> WSL/가상화 불필요. PowerShell만으로 빌드 가능.

## 1. 필수 도구 설치 (PowerShell 관리자)

```powershell
# Rust
winget install Rustlang.Rustup

# Node.js LTS
winget install OpenJS.NodeJS.LTS

# Git (없으면)
winget install Git.Git

# Visual Studio Build Tools — C++ 링커 필수
winget install Microsoft.VisualStudio.2022.BuildTools --override "--add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"

# WebView2 (Windows 11은 기본 포함, 10은 필요)
winget install Microsoft.EdgeWebView2Runtime
```

설치 후 **터미널 재시작** (PATH 반영)

## 2. 클론 & 의존성

```powershell
git clone https://github.com/seunghan91/markdown-media.git
cd markdown-media\desktop
npm install
```

## 3. 빌드

```powershell
npx tauri build
```

## 4. 결과물

```
src-tauri\target\release\bundle\
├── msi\MDM Desktop_0.1.0_x64_en-US.msi    ← 설치 프로그램
└── nsis\MDM Desktop_0.1.0_x64-setup.exe    ← 인스톨러
```

`.msi` 또는 `.exe` 더블클릭으로 설치 완료.

## 트러블슈팅

| 증상 | 해결 |
|------|------|
| `link.exe not found` | VS Build Tools C++ 워크로드 설치 확인 |
| `WebView2 not found` | Edge WebView2 Runtime 설치 |
| `cargo not found` | 터미널 재시작 (PATH 반영) |
| winget 안 됨 (회사 정책) | 아래 수동 설치 링크 참고 |

## winget 차단된 경우 수동 설치

| 도구 | 다운로드 |
|------|----------|
| Rust | https://rustup.rs → `rustup-init.exe` |
| Node.js | https://nodejs.org → LTS 설치 |
| Git | https://git-scm.com/download/win |
| VS Build Tools | https://visualstudio.microsoft.com/visual-cpp-build-tools/ |
| WebView2 | https://developer.microsoft.com/en-us/microsoft-edge/webview2/ |

## 참고

- WSL/Hyper-V/가상화 일절 불필요 — 전부 Windows 네이티브 도구
- Tauri 2.0 = WebView2 기반, 번들 크기 ~10MB (Electron 대비 1/10)
- 크로스 컴파일(macOS→Windows) 미지원, 각 OS에서 직접 빌드
