MDM: The Future of Multimedia Storytelling in Markdown
MDM (Markdown+Media) is a superset of Markdown designed to seamlessly embed and control local multimedia content like images, videos, and audio with an intuitive syntax. It aims to solve the problem of broken media links and the lack of rich media control in standard Markdown, making it perfect for personal knowledge management (PKM), technical documentation, and digital content creation.

Note: This project is in the specification and initial development phase. The NPM/PyPI/Crates badges are placeholders for our future releases.

🤔 The Problem
Standard Markdown is great for text, but it falls short with local media:

Fragile Paths: Moving your .md files often breaks image links (../images/pic.png).

No Control: You can't specify a video's width, make it autoplay, or loop it without resorting to raw HTML.

Limited Media Types: Embedding audio files or creating image galleries is cumbersome and non-standard.

💡 The Solution: ![[]] Syntax
MDM introduces a single, powerful syntax: ![[]]. It's an intuitive extension of the familiar Markdown image syntax and the "wikilink" style used in tools like Obsidian.

🖼️ Images
Go beyond simple image display. Create centered, captioned, or stylized images.

Markdown

// Simple image embed
![[profile.jpg]]

// Image with attributes (alignment, width, alt text, caption)
![[brand-logo.png]{width=250px align=center alt="MDM Project Logo" caption="The official MDM logo"}]]
🎬 Video & 🔉 Audio
Embed and control video and audio files natively within your Markdown.

Markdown

// Video with controls, specified size, and no autoplay
![[product-demo.mp4]{width=720px controls=true autoplay=false}]]

// A looping, muted background video
![[background-loop.mp4]{loop=true muted=true}]]

// Embed an audio file with player controls
![[podcast-episode-1.mp3]{controls=true}]]
⚙️ The .mdm Sidecar File: How It Works
To maintain 100% compatibility with standard Markdown, MDM uses an optional .mdm sidecar file. This approach separates media configuration from content, maximizing flexibility and portability.

1. File Naming Conventions
Default (1:1 Match): By default, an .md file is paired with an .mdm file of the exact same name located in the same directory. This is the primary and most intuitive method.

MyNote.md ↔ MyNote.mdm

Shared (N:1 Match): Multiple .md files can share a single .mdm configuration. To enable this, an .md file must explicitly declare the path to its source .mdm file in its YAML Front Matter.

YAML

---
title: "Chapter 1: Introduction"
# This document uses the project's shared media configuration.
mdm_source: "../_shared_media.mdm" 
---
2. media_root Path Resolution
Base Directory: When a relative path is specified for media_root, it is always resolved relative to the location of the .mdm file itself. This ensures that media links remain intact even if the entire project folder is moved or renamed.

Example: A media_root: "./assets" declaration points to an assets folder located in the same directory as the .mdm file.

🚀 Technology Roadmap: Why & Milestones
MDM adopts a multi-language strategy to ensure both universal accessibility and high performance.

1. JavaScript (NPM) - The Web Ecosystem Hub
Why: To achieve maximum accessibility and ease of integration for web developers. This will be the reference implementation, perfect for static site generators, web-based editors (like VS Code extensions), and any frontend project.

Milestones:

v0.1.0 (Core Parsing Engine) - ETA 2025-Q4: Basic block/inline parsing of ![[]] syntax with image attributes ({width}, {align}).

v0.2.0 (Multimedia & Sidecar) - ETA 2026-Q1: Support for <video>/<audio> tags and parsing of media_root from .mdm sidecar files.

v0.5.0 (Stable Release) - ETA 2026-Q2: API stabilization, comprehensive bug fixes, and complete official documentation.

2. Python (PyPI) - The Data & Tooling Backbone
Why: To integrate MDM into the vast Python ecosystem, including data science (Jupyter), machine learning documentation, backend frameworks, and popular static site generators like MkDocs.

Milestones:

v0.1.0 (Initial Porting) - ETA 2026-Q2: A complete port of the JavaScript v0.2.0 feature set to Python.

v0.2.0 (Ecosystem Integration) - ETA 2026-Q3: Develop prototype plugins or extensions for key libraries like Jupyter and MkDocs.

3. Rust - The Performance Core
Why: To deliver blazing-fast performance for demanding applications like real-time editor rendering and large-scale document processing. Ultimately, this Rust core will be compiled to WASM (for web) and native modules (for Python/Node.js) to power all other implementations.

Milestones:

v0.1.0 (Core Logic in Rust) - ETA 2026-Q3: Implement the core parsing logic, validated by the JS/Python versions, in Rust.

v0.2.0 (WASM & FFI) - ETA 2026-Q4: Begin work on compiling the core to WASM to supercharge the JS parser and using FFI (Foreign Function Interface) to generate native Python modules.

🤝 How to Contribute
MDM is an open-source project, and we welcome contributions of all kinds! Please see our CONTRIBUTING.md file for detailed guidelines on how to get involved.

📜 License
The MDM specification and documentation are licensed under Creative Commons BY-SA 4.0.
All source code is licensed under the MIT License.

CONTRIBUTING.md (Draft)
Contributing to MDM
Thank you for your interest in contributing to the MDM project! We are excited to build a vibrant community. All forms of contribution are welcome, from bug reports and feature requests to code and documentation.

Code of Conduct
This project and everyone participating in it is governed by our Code of Conduct. By participating, you are expected to uphold this code.

How to Report Bugs or Suggest Features
Bug Reports: Please use the "Bug Report" template on GitHub Issues. Provide a clear, detailed description and steps to reproduce the issue.

Feature Requests: Use the "Feature Request" template on GitHub Issues to outline the motivation and specific use cases for your suggestion.

Code Contribution Guide
1. Setting Up Your Environment
(Instructions for each language environment will be added here.)

2. Code Style Guides
To maintain a consistent codebase, we use standard formatters and linters for each language. Please ensure your code adheres to these styles before submitting a pull request.

JavaScript (ESLint):

Please follow the rules in our root .eslintrc.json file. You can check your code by running npm run lint.

View ESLint Config

Python (Black & Flake8):

All Python code must be formatted using Black.

Learn more about Black

Rust (Rustfmt):

We use the standard Rust formatter, rustfmt. You can format your code by running cargo fmt.

Read the Rustfmt documentation

3. Pull Request Process
Fork the repository and create your feature branch from main (git checkout -b feature/AmazingFeature).

Make your changes and ensure your code is properly formatted.

Add tests that cover your changes.

Commit your changes with a clear commit message (git commit -m 'feat: Add some AmazingFeature').

Push to your forked repository (git push origin feature/AmazingFeature).

Open a pull request to the main repository and provide a detailed description of your changes.