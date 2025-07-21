# MDM: The Future of Multimedia Storytelling in Markdown

MDM (Markdown+Media) is a superset of Markdown designed to seamlessly embed and control local multimedia content like images, videos, and audio with an intuitive syntax. It aims to solve the problem of broken media links and the lack of rich media control in standard Markdown, making it perfect for personal knowledge management (PKM), technical documentation, and digital content creation.

**Note:** This project is in the specification and initial development phase. The NPM/PyPI/Crates badges are placeholders for our future releases.

## 🤔 The Problem

Standard Markdown is great for text, but it falls short with local media:

- **Fragile Paths:** Moving your `.md` files often breaks image links (`../images/pic.png`).
- **No Control:** You can't specify a video's width, make it autoplay, or loop it without resorting to raw HTML.
- **Limited Media Types:** Embedding audio files or creating image galleries is cumbersome and non-standard.

## 💡 The Solution: `![[]]` Syntax for Advanced Images

MDM introduces a single, powerful syntax: `![[]]`. For our MVP, we are focusing on creating a best-in-class experience for image handling.

### 🖼️ Advanced Image Control

Go beyond simple image display. Create centered, captioned, or precisely-sized images with ease.

```markdown
// Simple image embed
![[profile.jpg]]

// Image with attributes (alignment, width, alt text, caption)
![[brand-logo.png]{width=250px align=center alt="MDM Project Logo" caption="The official MDM logo"}]]
```

### ✨ Image Presets (Size & Ratio)

To make responsive design intuitive, MDM includes built-in presets for common sizes and aspect ratios.

```markdown
// Use a preset for a thumbnail
![[photo.jpg]{size=thumb}]]

// Use a preset for a widescreen 16:9 ratio
![[landscape.jpg]{ratio=widescreen}]]
```

| Category | Preset Name | Representative Value |
| :--- | :--- | :--- |
| **Size** | `thumb`, `small`, `medium`, `large` | `150px`, `480px`, `768px`, `1024px` (width) |
| **Ratio** | `square`, `standard`, `widescreen`, `portrait`, `story` | `1:1`, `4:3`, `16:9`, `3:4`, `9:16` |

### 📁 Supported Image Formats

Our goal is to support a wide range of image formats. The MVP will prioritize:
- **Standard:** `jpg`, `jpeg`, `png`, `gif`
- **Modern:** `webp`, `svg`

## 🚀 MVP Roadmap: JavaScript First

Our immediate goal is to deliver a stable JavaScript parser as the foundation of the MDM ecosystem. Future phases will include Python and Rust implementations.

## 🤝 How to Contribute

MDM is an open-source project, and we welcome contributions of all kinds! Please see our `plan.md` for the detailed roadmap and `CONTRIBUTING.md` for guidelines on how to get involved.

## 📜 License

The MDM specification and documentation are licensed under Creative Commons BY-SA 4.0.
All source code is licensed under the MIT License.
