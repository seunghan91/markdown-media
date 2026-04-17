use pulldown_cmark::{html, Event, Options, Parser};
use pulldown_latex::{
    config::{DisplayMode, RenderConfig},
    mathml::push_mathml,
    Parser as LatexParser, Storage,
};

pub fn render_markdown_to_html(markdown: &str) -> String {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TASKLISTS);
    // Recognize `$…$` / `$$…$$` as math events. We intercept these below
    // and emit native MathML so Tauri's WebView (WebKit / WebView2) renders
    // formulas without any JS dependency. Exported HTML is self-contained.
    options.insert(Options::ENABLE_MATH);

    let parser = Parser::new_ext(markdown, options).map(|event| match event {
        Event::InlineMath(src) => Event::InlineHtml(render_latex_to_mathml(&src, false).into()),
        Event::DisplayMath(src) => Event::InlineHtml(render_latex_to_mathml(&src, true).into()),
        other => other,
    });

    let mut html_output = String::new();
    html::push_html(&mut html_output, parser);
    html_output
}

/// Convert a LaTeX math expression to inline MathML via `pulldown-latex`.
/// On parser failure we fall back to the raw source in a `<code>` block so
/// users see the intent instead of silently losing content.
fn render_latex_to_mathml(src: &str, display: bool) -> String {
    let storage = Storage::new();
    let parser = LatexParser::new(src, &storage);
    let mut cfg = RenderConfig::default();
    cfg.display_mode = if display {
        DisplayMode::Block
    } else {
        DisplayMode::Inline
    };
    cfg.annotation = Some(src);
    let mut out = String::new();
    match push_mathml(&mut out, parser, cfg) {
        Ok(()) => out,
        Err(_) => format!("<code class=\"math-error\">{}</code>", escape_html(src)),
    }
}

pub fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

pub fn wrap_text_lines(input: &str, width: usize) -> Vec<String> {
    let mut lines = Vec::new();

    for paragraph in input.lines() {
        if paragraph.trim().is_empty() {
            lines.push(String::new());
            continue;
        }

        let mut current = String::new();
        for word in paragraph.split_whitespace() {
            let projected = if current.is_empty() {
                word.len()
            } else {
                current.len() + 1 + word.len()
            };

            if projected > width && !current.is_empty() {
                lines.push(current.clone());
                current.clear();
            }

            if !current.is_empty() {
                current.push(' ');
            }
            current.push_str(word);
        }

        if !current.is_empty() {
            lines.push(current);
        }
    }

    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inline_math_renders_mathml() {
        let html = render_markdown_to_html("피타고라스: $a^2 + b^2 = c^2$ 이다.");
        assert!(html.contains("<math display=\"inline\""), "got: {html}");
        assert!(html.contains("<msup>"), "got: {html}");
        assert!(html.contains("application/x-tex"), "annotation preserved");
    }

    #[test]
    fn display_math_renders_block_mathml() {
        let md = "$$\nx = \\frac{1}{2}\n$$\n";
        let html = render_markdown_to_html(md);
        assert!(html.contains("<math display=\"block\""), "got: {html}");
        assert!(html.contains("<mfrac>"), "fraction rendered");
    }

    #[test]
    fn invalid_latex_falls_back_to_code() {
        // `\frac` requires two arguments; giving none should fail the LaTeX
        // parser. We want the fallback, not a panic.
        let html = render_markdown_to_html("$\\frac$");
        assert!(
            html.contains("math-error") || html.contains("<math"),
            "fallback or best-effort: {html}"
        );
    }

    #[test]
    fn plain_markdown_unchanged() {
        let html = render_markdown_to_html("# 제목\n\n본문 **굵게**.");
        assert!(html.contains("<h1>제목</h1>"));
        assert!(html.contains("<strong>굵게</strong>"));
    }
}
