//! HTML to Markdown converter.
//!
//! Uses regex-based tag matching for a lightweight conversion without heavy
//! external dependencies.

use std::io::{self, Read};
use std::path::Path;

use regex::Regex;
use lazy_static::lazy_static;

/// Parsed HTML document.
#[derive(Debug, Clone)]
pub struct HtmlDocument {
    pub markdown: String,
    pub title: Option<String>,
}

/// HTML parser.
pub struct HtmlParser {
    content: String,
}

impl HtmlParser {
    /// Open an HTML file from disk.
    pub fn open<P: AsRef<Path>>(path: P) -> io::Result<Self> {
        let mut data = Vec::new();
        std::fs::File::open(path.as_ref())?.read_to_end(&mut data)?;
        let content = decode_html_bytes(&data);
        Ok(Self { content })
    }

    /// Create a parser from raw bytes.
    pub fn from_bytes(data: Vec<u8>) -> io::Result<Self> {
        let content = decode_html_bytes(&data);
        Ok(Self { content })
    }

    /// Parse the HTML into an `HtmlDocument`.
    pub fn parse(&self) -> io::Result<HtmlDocument> {
        let title = extract_title(&self.content);
        let markdown = html_to_markdown(&self.content);
        Ok(HtmlDocument { markdown, title })
    }
}

impl HtmlDocument {
    /// Convenience: render to MDX with front-matter.
    pub fn to_mdx(&self, source_name: &str) -> String {
        let title_line = self
            .title
            .as_ref()
            .map(|t| format!("title: \"{}\"\n", t.replace('"', "\\\"")))
            .unwrap_or_default();
        format!(
            "---\nformat: html\nsource: \"{}\"\n{}---\n\n{}",
            source_name.replace('"', "\\\""),
            title_line,
            self.markdown,
        )
    }
}

// ---------------------------------------------------------------------------
// Core conversion
// ---------------------------------------------------------------------------

lazy_static! {
    // Tags whose content should be removed entirely.
    static ref RE_STRIP_SCRIPT: Regex = Regex::new(r"(?is)<script[\s>].*?</script>").unwrap();
    static ref RE_STRIP_STYLE: Regex = Regex::new(r"(?is)<style[\s>].*?</style>").unwrap();
    static ref RE_STRIP_NAV: Regex = Regex::new(r"(?is)<nav[\s>].*?</nav>").unwrap();
    static ref RE_STRIP_HEADER: Regex = Regex::new(r"(?is)<header[\s>].*?</header>").unwrap();
    static ref RE_STRIP_FOOTER: Regex = Regex::new(r"(?is)<footer[\s>].*?</footer>").unwrap();
    static ref RE_STRIP_NOSCRIPT: Regex = Regex::new(r"(?is)<noscript[\s>].*?</noscript>").unwrap();
    static ref RE_STRIP_SVG: Regex = Regex::new(r"(?is)<svg[\s>].*?</svg>").unwrap();
    static ref RE_STRIP_HEAD: Regex = Regex::new(r"(?is)<head[\s>].*?</head>").unwrap();

    // HTML comments.
    static ref RE_COMMENT: Regex = Regex::new(r"(?s)<!--.*?-->").unwrap();

    // Title tag.
    static ref RE_TITLE: Regex = Regex::new(r"(?is)<title[^>]*>(.*?)</title>").unwrap();

    // Headings.
    static ref RE_H1: Regex = Regex::new(r"(?is)<h1[^>]*>(.*?)</h1>").unwrap();
    static ref RE_H2: Regex = Regex::new(r"(?is)<h2[^>]*>(.*?)</h2>").unwrap();
    static ref RE_H3: Regex = Regex::new(r"(?is)<h3[^>]*>(.*?)</h3>").unwrap();
    static ref RE_H4: Regex = Regex::new(r"(?is)<h4[^>]*>(.*?)</h4>").unwrap();
    static ref RE_H5: Regex = Regex::new(r"(?is)<h5[^>]*>(.*?)</h5>").unwrap();
    static ref RE_H6: Regex = Regex::new(r"(?is)<h6[^>]*>(.*?)</h6>").unwrap();

    // Inline formatting.
    static ref RE_BOLD_B: Regex = Regex::new(r"(?is)<b[^>]*>(.*?)</b>").unwrap();
    static ref RE_BOLD_STRONG: Regex = Regex::new(r"(?is)<strong[^>]*>(.*?)</strong>").unwrap();
    static ref RE_ITALIC_I: Regex = Regex::new(r"(?is)<i[^>]*>(.*?)</i>").unwrap();
    static ref RE_ITALIC_EM: Regex = Regex::new(r"(?is)<em[^>]*>(.*?)</em>").unwrap();
    static ref RE_CODE_INLINE: Regex = Regex::new(r"(?is)<code[^>]*>(.*?)</code>").unwrap();

    // Links and images.
    static ref RE_LINK: Regex = Regex::new(r#"(?is)<a\s[^>]*href="([^"]*)"[^>]*>(.*?)</a>"#).unwrap();
    static ref RE_IMG: Regex = Regex::new(r#"(?is)<img\s[^>]*src="([^"]*)"[^>]*/?\s*>"#).unwrap();

    // Block elements.
    static ref RE_PRE: Regex = Regex::new(r"(?is)<pre[^>]*>(.*?)</pre>").unwrap();
    static ref RE_BLOCKQUOTE: Regex = Regex::new(r"(?is)<blockquote[^>]*>(.*?)</blockquote>").unwrap();
    static ref RE_P: Regex = Regex::new(r"(?is)<p[^>]*>(.*?)</p>").unwrap();
    static ref RE_BR: Regex = Regex::new(r"(?i)<br\s*/?>").unwrap();
    static ref RE_HR: Regex = Regex::new(r"(?i)<hr\s*/?>").unwrap();

    // Lists.
    static ref RE_UL: Regex = Regex::new(r"(?is)<ul[^>]*>(.*?)</ul>").unwrap();
    static ref RE_OL: Regex = Regex::new(r"(?is)<ol[^>]*>(.*?)</ol>").unwrap();
    static ref RE_LI: Regex = Regex::new(r"(?is)<li[^>]*>(.*?)</li>").unwrap();

    // Table.
    static ref RE_TABLE: Regex = Regex::new(r"(?is)<table[^>]*>(.*?)</table>").unwrap();
    static ref RE_TR: Regex = Regex::new(r"(?is)<tr[^>]*>(.*?)</tr>").unwrap();
    static ref RE_TH: Regex = Regex::new(r"(?is)<th[^>]*>(.*?)</th>").unwrap();
    static ref RE_TD: Regex = Regex::new(r"(?is)<td[^>]*>(.*?)</td>").unwrap();

    // Generic tag stripper (leftover tags).
    static ref RE_TAG: Regex = Regex::new(r"<[^>]+>").unwrap();

    // Collapse multiple blank lines.
    static ref RE_BLANK_LINES: Regex = Regex::new(r"\n{3,}").unwrap();

    // HTML entities.
    static ref RE_ENTITY_AMP: Regex = Regex::new(r"&amp;").unwrap();
    static ref RE_ENTITY_LT: Regex = Regex::new(r"&lt;").unwrap();
    static ref RE_ENTITY_GT: Regex = Regex::new(r"&gt;").unwrap();
    static ref RE_ENTITY_QUOT: Regex = Regex::new(r"&quot;").unwrap();
    static ref RE_ENTITY_APOS: Regex = Regex::new(r"&#39;|&apos;").unwrap();
    static ref RE_ENTITY_NBSP: Regex = Regex::new(r"&nbsp;").unwrap();
    static ref RE_ENTITY_NUM: Regex = Regex::new(r"&#(\d+);").unwrap();
}

fn extract_title(html: &str) -> Option<String> {
    RE_TITLE
        .captures(html)
        .map(|c| strip_tags(c.get(1).unwrap().as_str()).trim().to_string())
        .filter(|s| !s.is_empty())
}

fn html_to_markdown(html: &str) -> String {
    let mut s = html.to_string();

    // Remove stripped block elements (script, style, nav, etc.).
    s = RE_STRIP_SCRIPT.replace_all(&s, "").to_string();
    s = RE_STRIP_STYLE.replace_all(&s, "").to_string();
    s = RE_STRIP_NAV.replace_all(&s, "").to_string();
    s = RE_STRIP_HEADER.replace_all(&s, "").to_string();
    s = RE_STRIP_FOOTER.replace_all(&s, "").to_string();
    s = RE_STRIP_NOSCRIPT.replace_all(&s, "").to_string();
    s = RE_STRIP_SVG.replace_all(&s, "").to_string();
    s = RE_STRIP_HEAD.replace_all(&s, "").to_string();
    s = RE_COMMENT.replace_all(&s, "").to_string();

    // Pre blocks (must be handled before tag stripping).
    s = RE_PRE.replace_all(&s, |caps: &regex::Captures| {
        let code = decode_entities(&strip_tags(caps.get(1).unwrap().as_str()));
        format!("\n\n```\n{}\n```\n\n", code.trim())
    }).to_string();

    // Tables.
    s = RE_TABLE.replace_all(&s, |caps: &regex::Captures| {
        convert_table(caps.get(1).unwrap().as_str())
    }).to_string();

    // Headings.
    s = RE_H1.replace_all(&s, |c: &regex::Captures| format!("\n\n# {}\n\n", strip_tags(c.get(1).unwrap().as_str()).trim())).to_string();
    s = RE_H2.replace_all(&s, |c: &regex::Captures| format!("\n\n## {}\n\n", strip_tags(c.get(1).unwrap().as_str()).trim())).to_string();
    s = RE_H3.replace_all(&s, |c: &regex::Captures| format!("\n\n### {}\n\n", strip_tags(c.get(1).unwrap().as_str()).trim())).to_string();
    s = RE_H4.replace_all(&s, |c: &regex::Captures| format!("\n\n#### {}\n\n", strip_tags(c.get(1).unwrap().as_str()).trim())).to_string();
    s = RE_H5.replace_all(&s, |c: &regex::Captures| format!("\n\n##### {}\n\n", strip_tags(c.get(1).unwrap().as_str()).trim())).to_string();
    s = RE_H6.replace_all(&s, |c: &regex::Captures| format!("\n\n###### {}\n\n", strip_tags(c.get(1).unwrap().as_str()).trim())).to_string();

    // Blockquote.
    s = RE_BLOCKQUOTE.replace_all(&s, |c: &regex::Captures| {
        let inner = strip_tags(c.get(1).unwrap().as_str());
        let quoted: String = inner.lines().map(|l| format!("> {}", l.trim())).collect::<Vec<_>>().join("\n");
        format!("\n\n{}\n\n", quoted)
    }).to_string();

    // Lists.
    s = RE_UL.replace_all(&s, |c: &regex::Captures| {
        let items = extract_list_items(c.get(1).unwrap().as_str());
        let md: String = items.iter().map(|i| format!("- {}", i)).collect::<Vec<_>>().join("\n");
        format!("\n\n{}\n\n", md)
    }).to_string();

    s = RE_OL.replace_all(&s, |c: &regex::Captures| {
        let items = extract_list_items(c.get(1).unwrap().as_str());
        let md: String = items.iter().enumerate().map(|(i, t)| format!("{}. {}", i + 1, t)).collect::<Vec<_>>().join("\n");
        format!("\n\n{}\n\n", md)
    }).to_string();

    // Inline formatting (before tag strip).
    s = RE_LINK.replace_all(&s, |c: &regex::Captures| {
        let href = c.get(1).unwrap().as_str();
        let text = strip_tags(c.get(2).unwrap().as_str());
        format!("[{}]({})", text.trim(), href)
    }).to_string();

    s = RE_IMG.replace_all(&s, |c: &regex::Captures| {
        format!("![]({})", c.get(1).unwrap().as_str())
    }).to_string();

    s = RE_BOLD_STRONG.replace_all(&s, |c: &regex::Captures| {
        format!("**{}**", c.get(1).unwrap().as_str())
    }).to_string();
    s = RE_BOLD_B.replace_all(&s, |c: &regex::Captures| {
        format!("**{}**", c.get(1).unwrap().as_str())
    }).to_string();

    s = RE_ITALIC_EM.replace_all(&s, |c: &regex::Captures| {
        format!("*{}*", c.get(1).unwrap().as_str())
    }).to_string();
    s = RE_ITALIC_I.replace_all(&s, |c: &regex::Captures| {
        format!("*{}*", c.get(1).unwrap().as_str())
    }).to_string();

    s = RE_CODE_INLINE.replace_all(&s, |c: &regex::Captures| {
        format!("`{}`", c.get(1).unwrap().as_str())
    }).to_string();

    // Paragraphs.
    s = RE_P.replace_all(&s, |c: &regex::Captures| {
        format!("\n\n{}\n\n", c.get(1).unwrap().as_str().trim())
    }).to_string();

    // Line breaks and rules.
    s = RE_BR.replace_all(&s, "\n").to_string();
    s = RE_HR.replace_all(&s, "\n\n---\n\n").to_string();

    // Strip remaining tags.
    s = RE_TAG.replace_all(&s, "").to_string();

    // Decode entities.
    s = decode_entities(&s);

    // Collapse blank lines and trim.
    s = RE_BLANK_LINES.replace_all(&s, "\n\n").to_string();
    s.trim().to_string()
}

// ---------------------------------------------------------------------------
// Table conversion
// ---------------------------------------------------------------------------

fn convert_table(inner_html: &str) -> String {
    let mut rows: Vec<Vec<String>> = Vec::new();
    let mut has_header = false;

    for tr_cap in RE_TR.captures_iter(inner_html) {
        let tr_content = tr_cap.get(1).unwrap().as_str();
        let mut cells: Vec<String> = Vec::new();

        // Check for <th> first.
        let th_cells: Vec<String> = RE_TH
            .captures_iter(tr_content)
            .map(|c| strip_tags(c.get(1).unwrap().as_str()).trim().to_string())
            .collect();

        if !th_cells.is_empty() {
            has_header = true;
            cells = th_cells;
        } else {
            cells = RE_TD
                .captures_iter(tr_content)
                .map(|c| strip_tags(c.get(1).unwrap().as_str()).trim().to_string())
                .collect();
        }

        if !cells.is_empty() {
            rows.push(cells);
        }
    }

    if rows.is_empty() {
        return String::new();
    }

    // Build pipe table.
    let cols = rows.iter().map(|r| r.len()).max().unwrap_or(0);
    let mut out = String::from("\n\n");

    // Header row.
    let header = &rows[0];
    out.push('|');
    for i in 0..cols {
        let cell = header.get(i).map(|s| s.as_str()).unwrap_or("");
        out.push_str(&format!(" {} |", escape_pipe(cell)));
    }
    out.push('\n');

    // Separator.
    out.push('|');
    for _ in 0..cols {
        out.push_str(" --- |");
    }
    out.push('\n');

    // Data rows.
    let start = if has_header { 1 } else { 0 };
    // If no header detected, row 0 is already printed as header above, skip it.
    for row in &rows[1..] {
        out.push('|');
        for i in 0..cols {
            let cell = row.get(i).map(|s| s.as_str()).unwrap_or("");
            out.push_str(&format!(" {} |", escape_pipe(cell)));
        }
        out.push('\n');
    }

    out.push('\n');
    out
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn extract_list_items(list_inner: &str) -> Vec<String> {
    RE_LI
        .captures_iter(list_inner)
        .map(|c| strip_tags(c.get(1).unwrap().as_str()).trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

fn strip_tags(s: &str) -> String {
    RE_TAG.replace_all(s, "").to_string()
}

fn escape_pipe(s: &str) -> String {
    s.replace('|', "\\|")
}

fn decode_entities(s: &str) -> String {
    let mut out = s.to_string();
    out = RE_ENTITY_NBSP.replace_all(&out, " ").to_string();
    out = RE_ENTITY_AMP.replace_all(&out, "&").to_string();
    out = RE_ENTITY_LT.replace_all(&out, "<").to_string();
    out = RE_ENTITY_GT.replace_all(&out, ">").to_string();
    out = RE_ENTITY_QUOT.replace_all(&out, "\"").to_string();
    out = RE_ENTITY_APOS.replace_all(&out, "'").to_string();
    out = RE_ENTITY_NUM.replace_all(&out, |caps: &regex::Captures| {
        let n: u32 = caps[1].parse().unwrap_or(0);
        char::from_u32(n).map(|c| c.to_string()).unwrap_or_default()
    }).to_string();
    out
}

/// Detect encoding from BOM or meta charset, decode to String.
fn decode_html_bytes(data: &[u8]) -> String {
    // BOM detection.
    if data.len() >= 3 && data[0] == 0xEF && data[1] == 0xBB && data[2] == 0xBF {
        return String::from_utf8_lossy(&data[3..]).to_string();
    }

    // Try UTF-8 first.
    if let Ok(s) = std::str::from_utf8(data) {
        return s.to_string();
    }

    // Fallback: try EUC-KR.
    let (decoded, _, _) = encoding_rs::EUC_KR.decode(data);
    decoded.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_heading_conversion() {
        let html = "<h1>Title</h1><h2>Sub</h2>";
        let md = html_to_markdown(html);
        assert!(md.contains("# Title"));
        assert!(md.contains("## Sub"));
    }

    #[test]
    fn test_bold_italic() {
        let md = html_to_markdown("<p><strong>bold</strong> and <em>italic</em></p>");
        assert!(md.contains("**bold**"));
        assert!(md.contains("*italic*"));
    }

    #[test]
    fn test_link() {
        let md = html_to_markdown(r#"<a href="https://example.com">click</a>"#);
        assert!(md.contains("[click](https://example.com)"));
    }

    #[test]
    fn test_unordered_list() {
        let md = html_to_markdown("<ul><li>one</li><li>two</li></ul>");
        assert!(md.contains("- one"));
        assert!(md.contains("- two"));
    }

    #[test]
    fn test_strip_script_style() {
        let html = "<script>alert(1)</script><p>hello</p><style>.x{}</style>";
        let md = html_to_markdown(html);
        assert!(!md.contains("alert"));
        assert!(!md.contains(".x"));
        assert!(md.contains("hello"));
    }

    #[test]
    fn test_extract_title() {
        let html = "<html><head><title>My Page</title></head><body></body></html>";
        assert_eq!(extract_title(html), Some("My Page".to_string()));
    }

    #[test]
    fn test_table_conversion() {
        let html = "<table><tr><th>A</th><th>B</th></tr><tr><td>1</td><td>2</td></tr></table>";
        let md = html_to_markdown(html);
        assert!(md.contains("| A |"));
        assert!(md.contains("| --- |"));
        assert!(md.contains("| 1 |"));
    }

    #[test]
    fn test_entities() {
        let s = decode_entities("&amp; &lt; &gt; &quot; &nbsp;");
        // &nbsp; is decoded to a regular space, so result has trailing space.
        assert_eq!(s, "& < > \"  ");
    }
}
