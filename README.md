# MDM: The Future of Multimedia Storytelling in Markdown

MDM (Markdown+Media) is a superset of Markdown designed to seamlessly embed and control local multimedia content like images, videos, and audio with an intuitive syntax. It aims to solve the problem of broken media links and the lack of rich media control in standard Markdown, making it perfect for personal knowledge management (PKM), technical documentation, and digital content creation.

**Note:** This project is in the specification and initial development phase. The NPM/PyPI/Crates badges are placeholders for our future releases.

## 🤔 The Problem

Standard Markdown is great for text, but it falls short with local media:

- **Fragile Paths:** Moving your `.md` files often breaks image links (`../images/pic.png`).
- **No Control:** You can't specify a video's width, make it autoplay, or loop it without resorting to raw HTML.
- **Limited Media Types:** Embedding audio files or creating image galleries is cumbersome and non-standard.

## 💡 The Solution: `![[]]` Syntax

MDM introduces a single, powerful syntax: `![[]]`. It's an intuitive extension of the familiar Markdown image syntax and the "wikilink" style used in tools like Obsidian.

### 🖼️ Images

Go beyond simple image display. Create centered, captioned, or stylized images.

```markdown
// Simple image embed
![[profile.jpg]]

// Image with attributes (alignment, width, alt text, caption)
![[brand-logo.png]{width=250px align=center alt="MDM Project Logo" caption="The official MDM logo"}]]
```

### 🎬 Video & 🔉 Audio

Embed and control video and audio files natively within your Markdown.

```markdown
// Video with controls, specified size, and no autoplay
![[product-demo.mp4]{width=720px controls=true autoplay=false}]]

// A looping, muted background video
![[background-loop.mp4]{loop=true muted=true}]]

// Embed an audio file with player controls
![[podcast-episode-1.mp3]{controls=true}]]
```

## 🚀 Technology Roadmap

MDM adopts a multi-language strategy to ensure both universal accessibility and high performance.

1.  **JavaScript (NPM):** The reference implementation for the web ecosystem.
2.  **Python (PyPI):** For integration into the data science and tooling ecosystem.
3.  **Rust:** The performance core, to be compiled to WASM and native modules.

## 🤝 How to Contribute

MDM is an open-source project, and we welcome contributions of all kinds! Please see our `CONTRIBUTING.md` file for detailed guidelines on how to get involved.

## 📜 License

The MDM specification and documentation are licensed under Creative Commons BY-SA 4.0.
All source code is licensed under the MIT License.
