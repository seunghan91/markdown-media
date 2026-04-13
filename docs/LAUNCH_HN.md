# Show HN: MDM -- Rust-powered document-to-AI converter (HWP/PDF/DOCX to Markdown + Media)

MDM converts documents into structured Markdown + Media bundles optimized for LLM/AI consumption. Unlike existing tools that extract text and discard media, MDM preserves every image, table, chart, and embeds them with type-specific references.

**What it does:**
- Converts HWP (Korean gov docs), PDF, DOCX into clean Markdown + extracted media assets
- Generates a manifest indexing every extracted asset with metadata
- 6 media-type prefixes: `@[[image]]` `~[[table]]` `&[[embed]]` `%[[video]]` `$[[equation]]` `^[[audio]]`
- Python package: `pip install mdm-core` with LangChain/LlamaIndex integration
- Benchmarks: 100% feature score vs Pandoc (DOCX), 93% vs Marker (PDF)
- 383-page PDF in 5.6 seconds (Rust + Rayon parallel processing)

**Why Markdown?**
Markdown uses 34-38% fewer tokens than JSON across all major LLMs. For RAG pipelines, this means lower cost and more content fits in context windows.

**The gap we fill:**
Every existing converter (MarkItDown, Docling, Unstructured, Marker) extracts text but has NO integrated media manifest. MDM is the first to provide:
- Content-addressable asset storage (hash-based dedup)
- Type-specific media references in the output
- Asset manifest with source position, metadata, and cross-references

**Tech:** Rust core (21K LOC), PyO3 Python bindings, 159 tests, 55 real-world test documents from Korean government.

GitHub: https://github.com/seunghan91/markdown-media
