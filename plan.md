# MDM Project Plan & Roadmap

This document outlines the development plan and strategic roadmap for the MDM project. Our goal is to build a robust, multi-language ecosystem for rich media in Markdown.

## Guiding Principles

- **Compatibility First:** Maintain 100% compatibility with CommonMark.
- **Intuitive Syntax:** The `![[]]` syntax should be easy to learn and use.
- **Performance:** Deliver a fast and efficient parsing experience, especially for real-time applications.
- **Open & Collaborative:** Build a welcoming community for contributors.

## Phase 1: Core Engine & Web Foundation (2025 Q4 - 2026 Q2)

This phase focuses on establishing the core functionality and the primary JavaScript implementation.

> **MVP Focus:** While the overall goal of this phase is to support all media, the initial Minimum Viable Product (MVP) will prioritize delivering a robust and feature-rich experience for **images**. This includes support for multiple formats and convenient sizing presets. Video and audio support will follow as the phase progresses.

- **Milestone 1.1: Core Image Parsing (JS)** - `v0.1.0` (ETA: 2025-Q4)
    - [ ] Implement the basic block/inline parsing of `![[]]` syntax.
    - [ ] Support essential image attributes: `{width}`, `{height}`, `{align}`, `{alt}`, `{caption}`.
    - [ ] **Supported Formats (Initial):** Implement support for `jpg`, `jpeg`, `png`, `gif`.
    - [ ] Establish initial test suite with basic validation cases.

- **Milestone 1.2: Enhanced Image Features & Sidecar (JS)** - `v0.2.0` (ETA: 2026-Q1)
    - [ ] **Modern Formats:** Add support for modern image formats like `webp` and `svg`.
    - [ ] **Image Presets:** Introduce size and ratio presets for easy image styling (e.g., `{size=medium}`, `{ratio=widescreen}`).
    - [ ] Implement parsing of `media_root` from `.mdm` sidecar files to manage media paths.
    - [ ] Expand test suite to cover all new formats, presets, and sidecar functionality.

- **Milestone 1.3: Full Multimedia Support & Stable Release (JS)** - `v0.5.0` (ETA: 2026-Q2)
    - [ ] Add support for `<video>` and `<audio>` tag generation.
    - [ ] Support video/audio attributes: `{controls}`, `{autoplay}`, `{loop}`, `{muted}`.
    - [ ] Stabilize the JavaScript parser API for all media types.
    - [ ] Create comprehensive official documentation in the `docs/` directory.
    - [ ] Set up GitHub Pages for automated documentation deployment.
    - [ ] Publish the official `v0.5.0` package to NPM.

## Phase 2: Python Ecosystem & Cross-Language Testing (2026 Q2 - 2026 Q3)

This phase expands MDM to the Python ecosystem and establishes a shared testing framework.

- **Milestone 2.1: Python Parser Porting** - `v0.1.0` (ETA: 2026-Q2)
    - [ ] Port the complete JavaScript `v0.2.0` feature set to a Python package (`parser-py`).
    - [ ] Ensure feature parity between the JS and Python implementations.
    - [ ] Publish the initial package to PyPI.

- **Milestone 2.2: Cross-Language Spec Tests** (ETA: 2026-Q2)
    - [ ] Develop a language-agnostic test suite in the `tests/` directory.
    - [ ] These tests will consist of `.md` input files and expected `.html` output files.
    - [ ] Create scripts in `tools/` to run the spec tests against both JS and Python parsers.

- **Milestone 2.3: Python Ecosystem Integration** - `v0.2.0` (ETA: 2026-Q3)
    - [ ] Develop prototype plugins or extensions for key Python libraries like Jupyter, MkDocs, or Pelican.
    - [ ] Gather feedback from the Python community.

## Phase 3: Performance Core with Rust (2026 Q3 - 2026 Q4)

This phase focuses on building a high-performance core in Rust to power all other implementations.

- **Milestone 3.1: Core Logic in Rust** - `v0.1.0` (ETA: 2026-Q3)
    - [ ] Implement the core parsing logic in Rust (`parser-rs`), validated by the cross-language spec tests.
    - [ ] Focus on performance and memory safety.
    - [ ] Publish the initial crate to Crates.io.

- **Milestone 3.2: WASM & FFI Bindings** - `v0.2.0` (ETA: 2026-Q4)
    - [ ] Compile the Rust core to WebAssembly (WASM) to supercharge the JavaScript parser.
    - [ ] Create Foreign Function Interface (FFI) bindings to generate native Python modules.
    - [ ] Begin replacing the pure JS and Python parser logic with high-performance Rust-backed modules.

## Phase 4: Community & Playground (Ongoing)

This phase is continuous and focuses on improving the developer and user experience.

- **Milestone 4.1: Playground & Live Demo**
    - [ ] Develop an interactive playground in the `playground/` directory using a framework like Astro or Svelte.
    - [ ] Allow users to test the `![[]]` syntax in real-time and see the generated HTML.
    - [ ] Provide easy-to-use bug reporting directly from the playground.

- **Milestone 4.2: Contribution & Governance**
    - [ ] Refine `CONTRIBUTING.md` with detailed guides for each language.
    - [ ] Establish a clear governance model for the project.
    - [ ] Actively engage with the community through GitHub Discussions and other channels.
