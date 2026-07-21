//! URL / web page extraction module.
//!
//! Fetches a URL, extracts the main content as Markdown.
//! Uses `ureq` for HTTP and `htmd` for HTML→Markdown conversion.
//!
//! Feature-gated behind `url-fetch` (see `core/Cargo.toml`).

use std::io;

#[derive(Debug, Clone)]
pub struct UrlDocument {
    pub url: String,
    pub title: Option<String>,
    pub markdown: String,
}

#[cfg(feature = "url-fetch")]
pub fn fetch_url(url: &str) -> io::Result<UrlDocument> {
    let response = ureq::get(url)
        .set("User-Agent", "mdm-url-fetcher/1.0")
        .call()
        .map_err(|e| io::Error::other(e.to_string()))?;

    if response.status() >= 400 {
        return Err(io::Error::other(
            format!("HTTP {}", response.status()),
        ));
    }

    let html = response
        .into_string()
        .map_err(|e| io::Error::other(e.to_string()))?;

    let title = extract_title(&html);
    let markdown = htmd::convert(&html)
        .map_err(|e| io::Error::other(e.to_string()))?;

    Ok(UrlDocument {
        url: url.to_string(),
        title,
        markdown,
    })
}

#[cfg(feature = "url-fetch")]
pub fn fetch_urls(urls: &[&str]) -> Vec<io::Result<UrlDocument>> {
    urls.iter().map(|url| fetch_url(url)).collect()
}

#[cfg(not(feature = "url-fetch"))]
pub fn fetch_url(_url: &str) -> io::Result<UrlDocument> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "URL fetching disabled. Build with `--features url-fetch`.",
    ))
}

#[cfg(not(feature = "url-fetch"))]
pub fn fetch_urls(_urls: &[&str]) -> Vec<io::Result<UrlDocument>> {
    vec![Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "URL fetching disabled.",
    ))]
}

fn extract_title(html: &str) -> Option<String> {
    let lower = html.to_lowercase();
    let start = lower.find("<title")?;
    let close = lower[start..].find('>')?;
    let end = lower[start + close + 1..].find("</title>")?;
    let title = &html[start + close + 1..start + close + 1 + end];
    let title = title.trim();
    if title.is_empty() { None } else { Some(title.to_string()) }
}

impl UrlDocument {
    pub fn to_mdx(&self) -> String {
        format!(
            "---\nformat: url\nsource: \"{}\"\ntitle: \"{}\"\n---\n\n{}",
            self.url.replace('"', "\\\""),
            self.title.as_deref().unwrap_or("").replace('"', "\\\""),
            self.markdown,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_title() {
        let html = "<html><head><title>Test Page</title></head><body></body></html>";
        assert_eq!(extract_title(html), Some("Test Page".to_string()));
    }

    #[test]
    fn test_extract_title_none() {
        assert_eq!(extract_title("no title here"), None);
    }
}
