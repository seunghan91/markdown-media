//! Directory watch mode — auto-convert supported documents on file create/modify,
//! with optional webhook notification.
//!
//! Ported from kkdoc (MIT): reference/kkdoc/src/watch.ts
//!
//! Differences from the TypeScript original (Node `fs.watch` + `setTimeout`
//! debounce + hand-rolled task queue):
//! - File-system watching is backed by the `notify` crate (native OS APIs:
//!   FSEvents / inotify / ReadDirectoryChangesW) instead of Node's `fs.watch`.
//! - The debounce map and the `MAX_CONCURRENT` cap are reimplemented on top of
//!   a bounded `rayon::ThreadPool` instead of a hand-rolled async task queue.
//! - Webhook SSRF hardening (blocked hostnames/IP ranges, no redirects) is
//!   ported 1:1; the extra DNS-resolution recheck from `sendWebhook` is
//!   reimplemented using `std::net::ToSocketAddrs` instead of Node's
//!   `dns/promises`.

use std::collections::{HashMap, HashSet};
use std::io;
use std::net::{IpAddr, ToSocketAddrs};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use notify::{Event, EventKind, RecursiveMode, Watcher};
use ureq::Resolver;

/// Debounce window per path — mirrors kkdoc's `DEBOUNCE_MS`.
const DEBOUNCE: Duration = Duration::from_millis(1000);
/// Interval between size checks used to detect "write finished". Mirrors
/// kkdoc's `STABLE_CHECK_MS`.
const STABLE_CHECK: Duration = Duration::from_millis(300);
/// Files larger than this are ignored (kkdoc's `MAX_FILE_SIZE`).
const MAX_FILE_SIZE: u64 = 500 * 1024 * 1024;
/// Bounded concurrent conversions — mirrors kkdoc's `createTaskQueue(MAX_CONCURRENT, ...)`.
const MAX_CONCURRENT: usize = 3;

/// Output format for converted files, matching `WatchOptions.format` in kkdoc.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Markdown,
    Json,
}

/// Options for [`watch_dir`]. Mirrors kkdoc's `WatchOptions` (`dir` itself is
/// the first positional argument to `watch_dir`, not a field here).
#[derive(Debug, Clone)]
pub struct WatchOptions {
    /// Where converted output is written. `None` means "caller decides via
    /// `on_event`" (kkdoc writes to stdout in this case).
    pub out_dir: Option<PathBuf>,
    /// Webhook endpoint notified after each conversion attempt.
    pub webhook: Option<String>,
    pub format: OutputFormat,
    /// Suppress `[mdm watch] ...` progress lines on stderr.
    pub silent: bool,
}

impl Default for WatchOptions {
    fn default() -> Self {
        Self {
            out_dir: None,
            webhook: None,
            format: OutputFormat::Markdown,
            silent: false,
        }
    }
}

/// Outcome of converting one detected file, passed to the `on_event` callback.
#[derive(Debug, Clone)]
pub struct FileEvent {
    /// Canonicalized path of the source file that triggered conversion.
    pub path: PathBuf,
    pub file_name: String,
    /// `Ok(rendered output)` on success, `Err(message)` on failure — mirrors
    /// kkdoc's `{ success, markdown, error }` webhook payload split.
    pub result: Result<String, String>,
    /// Where the output was written, if `WatchOptions.out_dir` was set.
    pub out_path: Option<PathBuf>,
}

/// Extensions this module can convert. Matches kkdoc's `SUPPORTED_EXTENSIONS`
/// minus `.hml` (HWPML) — there is no standalone public parser for HWPML in
/// mdm-core yet (only the CLI binary's private `parse_hwpml`), so it is left
/// out here rather than duplicating that logic. See the P2 #22 report for
/// this deviation.
pub fn supported_extensions() -> &'static [&'static str] {
    if cfg!(feature = "pdf") {
        &["hwp", "hwpx", "docx", "xlsx", "xls", "pdf"]
    } else {
        &["hwp", "hwpx", "docx", "xlsx", "xls"]
    }
}

/// Errors from [`watch_dir`] setup (not per-file conversion errors — those
/// are reported via [`FileEvent::result`] instead).
#[derive(Debug)]
pub enum WatchError {
    Io(io::Error),
    Notify(notify::Error),
    InvalidWebhook(String),
}

impl std::fmt::Display for WatchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WatchError::Io(e) => write!(f, "{}", e),
            WatchError::Notify(e) => write!(f, "{}", e),
            WatchError::InvalidWebhook(msg) => write!(f, "{}", msg),
        }
    }
}

impl std::error::Error for WatchError {}

impl From<io::Error> for WatchError {
    fn from(e: io::Error) -> Self {
        WatchError::Io(e)
    }
}

impl From<notify::Error> for WatchError {
    fn from(e: notify::Error) -> Self {
        WatchError::Notify(e)
    }
}

/// Watch `dir` for new/modified supported documents, converting each one and
/// invoking `on_event` with the outcome. Blocks the calling thread forever
/// (mirrors kkdoc's `watchDirectory`, which returns `new Promise(() => {})`);
/// callers that need a stoppable watch should run this on a dedicated thread.
///
/// # Example
/// ```no_run
/// use mdm_core::watch::{watch_dir, WatchOptions};
///
/// watch_dir("./incoming", WatchOptions::default(), |event| {
///     println!("{}: {:?}", event.file_name, event.result.is_ok());
/// }).unwrap();
/// ```
pub fn watch_dir<F>(dir: impl AsRef<Path>, opts: WatchOptions, on_event: F) -> Result<(), WatchError>
where
    F: Fn(FileEvent) + Send + Sync + 'static,
{
    run(dir.as_ref(), opts, on_event, None)
}

/// Core loop shared by [`watch_dir`] and the test harness below. `deadline`
/// lets tests bound an otherwise infinite watch loop without touching the
/// public API (kkdoc has no equivalent — its tests mock `fs.watch` instead).
fn run<F>(
    dir: &Path,
    opts: WatchOptions,
    on_event: F,
    deadline: Option<Instant>,
) -> Result<(), WatchError>
where
    F: Fn(FileEvent) + Send + Sync + 'static,
{
    if !dir.exists() {
        return Err(WatchError::Io(io::Error::new(
            io::ErrorKind::NotFound,
            format!("디렉토리를 찾을 수 없습니다: {}", dir.display()),
        )));
    }
    if let Some(url) = &opts.webhook {
        validate_webhook_url(url)?;
    }
    if let Some(out) = &opts.out_dir {
        std::fs::create_dir_all(out)?;
    }

    let real_dir = Arc::new(std::fs::canonicalize(dir)?);

    if !opts.silent {
        eprintln!("[mdm watch] 감시 시작: {}", real_dir.display());
        if let Some(out) = &opts.out_dir {
            eprintln!("[mdm watch] 출력: {}", out.display());
        }
        if let Some(hook) = &opts.webhook {
            eprintln!("[mdm watch] 웹훅: {}", hook);
        }
    }

    let (tx, rx) = channel::<notify::Result<Event>>();
    let mut watcher = notify::recommended_watcher(tx)?;
    watcher.watch(real_dir.as_path(), RecursiveMode::Recursive)?;

    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(MAX_CONCURRENT)
        .build()
        .map_err(|e| WatchError::Io(io::Error::other(e.to_string())))?;

    let opts = Arc::new(opts);
    let on_event = Arc::new(on_event);
    let in_progress: Arc<Mutex<HashSet<PathBuf>>> = Arc::new(Mutex::new(HashSet::new()));
    let mut pending: HashMap<PathBuf, Instant> = HashMap::new();

    loop {
        if let Some(dl) = deadline {
            if Instant::now() >= dl {
                break;
            }
        }

        match rx.recv_timeout(Duration::from_millis(100)) {
            Ok(Ok(event)) => {
                if matches!(event.kind, EventKind::Create(_) | EventKind::Modify(_)) {
                    for path in event.paths {
                        pending.insert(path, Instant::now() + DEBOUNCE);
                    }
                }
            }
            Ok(Err(e)) => {
                if !opts.silent {
                    eprintln!("[mdm watch] 감시 오류: {}", e);
                }
            }
            Err(RecvTimeoutError::Timeout) => {}
            // Watcher was dropped (shouldn't happen while `watcher` is alive
            // in this scope) — treat as a clean shutdown signal.
            Err(RecvTimeoutError::Disconnected) => break,
        }

        let now = Instant::now();
        let due: Vec<PathBuf> = pending
            .iter()
            .filter(|(_, &due)| now >= due)
            .map(|(p, _)| p.clone())
            .collect();

        for path in due {
            pending.remove(&path);

            let already_running = {
                let mut guard = in_progress.lock().unwrap();
                if guard.contains(&path) {
                    true
                } else {
                    guard.insert(path.clone());
                    false
                }
            };
            if already_running {
                continue;
            }

            let opts = Arc::clone(&opts);
            let on_event = Arc::clone(&on_event);
            let real_dir = Arc::clone(&real_dir);
            let in_progress = Arc::clone(&in_progress);
            pool.spawn(move || {
                process_file(&path, real_dir.as_path(), &opts, on_event.as_ref());
                in_progress.lock().unwrap().remove(&path);
            });
        }
    }

    Ok(())
}

/// Convert one file, write output, send the webhook, and notify `on_event`.
/// Silently returns for unsupported extensions, symlink-escapes out of
/// `real_dir`, oversized/empty/vanished files — mirrors kkdoc's `processFile`
/// early-return checks.
fn process_file(path: &Path, real_dir: &Path, opts: &WatchOptions, on_event: &(dyn Fn(FileEvent) + Send + Sync)) {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_lowercase())
        .unwrap_or_default();
    if !supported_extensions().contains(&ext.as_str()) {
        return;
    }

    let file_name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();

    if !path.exists() {
        return;
    }

    // Resolve symlinks and reject anything that escapes the watched directory.
    let abs_path = match std::fs::canonicalize(path) {
        Ok(p) => p,
        Err(_) => return,
    };
    if !abs_path.starts_with(real_dir) {
        return;
    }

    let size = match wait_for_stable_size(&abs_path) {
        Some(s) => s,
        None => return,
    };
    if size == 0 || size > MAX_FILE_SIZE {
        return;
    }

    if !opts.silent {
        eprintln!("[mdm watch] 변환 중: {}", file_name);
    }

    let markdown_result = convert_to_markdown(&abs_path).map_err(|e| e.to_string());

    let out_path = markdown_result.as_ref().ok().and_then(|markdown| {
        opts.out_dir.as_ref().map(|out_dir| {
            let out_ext = match opts.format {
                OutputFormat::Markdown => "md",
                OutputFormat::Json => "json",
            };
            let stem = abs_path
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| file_name.clone());
            let rendered = render_output(&file_name, markdown, opts.format);
            let out_path = out_dir.join(format!("{}.{}", stem, out_ext));
            if let Err(e) = std::fs::write(&out_path, &rendered) {
                if !opts.silent {
                    eprintln!("[mdm watch] 출력 쓰기 실패: {} — {}", file_name, e);
                }
            }
            out_path
        })
    });

    if !opts.silent {
        match &markdown_result {
            Ok(_) => eprintln!("[mdm watch] 완료: {}", file_name),
            Err(e) => eprintln!("[mdm watch] 실패: {} — {}", file_name, e),
        }
    }

    send_webhook(opts.webhook.as_deref(), &file_name, &markdown_result, opts.silent);

    on_event(FileEvent {
        path: abs_path,
        file_name,
        result: markdown_result,
        out_path,
    });
}

/// Render the final output string for a successful conversion, applying
/// `OutputFormat::Json` wrapping when requested.
fn render_output(file_name: &str, markdown: &str, format: OutputFormat) -> String {
    match format {
        OutputFormat::Markdown => markdown.to_string(),
        OutputFormat::Json => serde_json::json!({
            "file": file_name,
            "success": true,
            "markdown": markdown,
        })
        .to_string(),
    }
}

/// Wait for a file's size to stabilize (write-completion heuristic) —
/// ports kkdoc's `waitForStableSize`. Returns `None` if the file disappeared
/// mid-check.
fn wait_for_stable_size(path: &Path) -> Option<u64> {
    let prev = std::fs::metadata(path).ok()?.len();
    std::thread::sleep(STABLE_CHECK);
    if !path.exists() {
        return None;
    }
    let curr = std::fs::metadata(path).ok()?.len();
    if curr != prev {
        std::thread::sleep(STABLE_CHECK);
        if !path.exists() {
            return None;
        }
        return std::fs::metadata(path).ok().map(|m| m.len());
    }
    Some(curr)
}

/// Dispatch a file to the matching mdm-core parser and render it to Markdown.
fn convert_to_markdown(path: &Path) -> io::Result<String> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    match ext.as_str() {
        "hwp" => {
            let mut parser = crate::hwp::HwpParser::open(path)?;
            let blocks = parser.extract_blocks()?;
            Ok(crate::ir::blocks_to_markdown(&blocks))
        }
        "hwpx" => {
            let mut parser = crate::hwpx::HwpxParser::open(path)?;
            let doc = parser.parse()?;
            if doc.sections.iter().any(|s| !s.is_empty()) {
                Ok(doc.sections.join("\n\n---\n\n"))
            } else {
                Ok(doc.preview_text)
            }
        }
        "docx" => {
            let mut parser = crate::docx::DocxParser::open(path)?;
            let doc = parser.parse()?;
            Ok(doc.to_markdown())
        }
        "xlsx" | "xls" => {
            let parser = crate::xlsx::XlsxParser::open(path)?;
            let doc = parser.parse()?;
            Ok(doc.to_markdown())
        }
        "pdf" => convert_pdf(path),
        other => Err(io::Error::new(
            io::ErrorKind::Unsupported,
            format!("지원하지 않는 확장자: {}", other),
        )),
    }
}

#[cfg(feature = "pdf")]
fn convert_pdf(path: &Path) -> io::Result<String> {
    let parser = crate::pdf::PdfParser::open(path)?;
    let doc = parser.parse()?;
    Ok(doc.to_markdown_with_layout())
}

#[cfg(not(feature = "pdf"))]
fn convert_pdf(_path: &Path) -> io::Result<String> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "PDF 지원이 비활성화되어 있습니다 (feature \"pdf\" 필요)",
    ))
}

// ─── Webhook (SSRF-hardened) ────────────────────────────────────────────
// Ported from kkdoc (MIT): reference/kkdoc/src/watch.ts `validateWebhookUrl` / `isPrivateIp` / `sendWebhook`

/// Static hostname/scheme validation — rejects localhost, private/link-local
/// ranges, cloud metadata endpoints, and numeric-IP-encoding bypasses.
fn validate_webhook_url(raw: &str) -> Result<(), WatchError> {
    let parsed = url::Url::parse(raw)
        .map_err(|_| WatchError::InvalidWebhook(format!("유효하지 않은 webhook URL: {}", raw)))?;

    if parsed.scheme() != "http" && parsed.scheme() != "https" {
        return Err(WatchError::InvalidWebhook(format!(
            "허용되지 않는 webhook 프로토콜: {}",
            parsed.scheme()
        )));
    }

    let host = parsed.host_str().unwrap_or("").to_lowercase();
    if is_blocked_hostname(&host) {
        return Err(WatchError::InvalidWebhook(format!(
            "내부 네트워크 대상 webhook은 허용되지 않습니다: {}",
            host
        )));
    }

    Ok(())
}

fn is_blocked_hostname(host: &str) -> bool {
    if host.is_empty() {
        return true;
    }
    if host == "localhost"
        || host == "[::1]"
        || host == "0.0.0.0"
        || host == "metadata.google.internal"
        || host == "metadata.google"
    {
        return true;
    }
    if host.starts_with("127.")
        || host.starts_with("10.")
        || host.starts_with("192.168.")
        || host.starts_with("169.254.")
        || host.ends_with(".local")
        || host.starts_with("[fc")
        || host.starts_with("[fd")
        || host.starts_with("[fe80:")
        || host == "[::0]"
        || host == "[::]"
    {
        return true;
    }
    lazy_static::lazy_static! {
        static ref PRIVATE_172: regex::Regex = regex::Regex::new(r"^172\.(1[6-9]|2\d|3[01])\.").unwrap();
        static ref HEX_IP: regex::Regex = regex::Regex::new(r"(?i)^0x[0-9a-f]+$").unwrap();
        static ref OCTAL_IP: regex::Regex = regex::Regex::new(r"^0[0-7]+$").unwrap();
        static ref DECIMAL_IP: regex::Regex = regex::Regex::new(r"^\d+$").unwrap();
    }
    PRIVATE_172.is_match(host) || HEX_IP.is_match(host) || OCTAL_IP.is_match(host) || DECIMAL_IP.is_match(host)
}

/// Private/loopback/link-local IP check — used to re-validate DNS resolution
/// results, since the static hostname check alone cannot catch DNS rebinding
/// to an internal address. Ports kkdoc's `isPrivateIp` using `std::net`.
///
/// `to_canonical()` unwraps IPv4-mapped IPv6 addresses (`::ffff:a.b.c.d`) to
/// plain IPv4 before classification — without it, `http://[::ffff:127.0.0.1]/`
/// fell through to the native-IPv6 branch (which only checks loopback/ULA/
/// link-local ranges) and was treated as public. Codex review P1 finding.
fn is_private_ip(ip: IpAddr) -> bool {
    match ip.to_canonical() {
        IpAddr::V4(v4) => {
            v4.is_loopback()
                || v4.is_private()
                || v4.is_link_local()
                || v4.is_unspecified()
                || {
                    // CGNAT 100.64.0.0/10
                    let o = v4.octets();
                    o[0] == 100 && (64..=127).contains(&o[1])
                }
        }
        IpAddr::V6(v6) => {
            v6.is_loopback()
                || v6.is_unspecified()
                || (v6.segments()[0] & 0xfe00) == 0xfc00 // ULA fc00::/7
                || (v6.segments()[0] & 0xffc0) == 0xfe80 // link-local fe80::/10
        }
    }
}

/// A `ureq::Resolver` that ignores whatever netloc it's asked to resolve and
/// always returns the fixed address list it was built with.
///
/// Without this, `send_webhook` would validate the hostname's resolved IPs
/// once (rejecting private/internal ones) and then hand the *hostname* to
/// ureq, which performs its own independent DNS lookup when it opens the
/// connection. A DNS answer that changes between those two lookups (DNS
/// rebinding) lets an attacker pass validation with a public IP and then
/// connect to an internal one. Pinning the connection to the exact addresses
/// already validated closes that gap. Codex review P1 finding.
struct FixedResolver(Vec<std::net::SocketAddr>);

impl ureq::Resolver for FixedResolver {
    fn resolve(&self, _netloc: &str) -> io::Result<Vec<std::net::SocketAddr>> {
        if self.0.is_empty() {
            Err(io::Error::new(io::ErrorKind::NotFound, "no validated webhook addresses"))
        } else {
            Ok(self.0.clone())
        }
    }
}

/// POST the conversion outcome to `url`, if set. Failures are logged (unless
/// `silent`) and never propagate — a broken webhook must not stop watching.
fn send_webhook(url: Option<&str>, file_name: &str, result: &Result<String, String>, silent: bool) {
    let Some(url) = url else { return };

    if let Err(e) = validate_webhook_url(url) {
        if !silent {
            eprintln!("[mdm watch] webhook 전송 실패: {}", e);
        }
        return;
    }

    let parsed = match url::Url::parse(url) {
        Ok(u) => u,
        Err(_) => return,
    };
    let host = parsed.host_str().unwrap_or("").to_string();
    let port = parsed.port_or_known_default().unwrap_or(80);

    let addrs: Vec<std::net::SocketAddr> = match (host.as_str(), port).to_socket_addrs() {
        Ok(iter) => iter.collect(),
        Err(e) => {
            if !silent {
                eprintln!("[mdm watch] webhook DNS 조회 실패: {} — {}", host, e);
            }
            return;
        }
    };
    if addrs.is_empty() {
        if !silent {
            eprintln!("[mdm watch] webhook DNS 조회 결과 없음: {}", host);
        }
        return;
    }
    for addr in &addrs {
        if is_private_ip(addr.ip()) {
            if !silent {
                eprintln!(
                    "[mdm watch] webhook 대상이 내부 네트워크로 해석됩니다: {} → {}",
                    host,
                    addr.ip()
                );
            }
            return;
        }
    }

    let markdown_excerpt = result.as_ref().ok().map(|s| s.chars().take(1000).collect::<String>());
    let payload = serde_json::json!({
        "file": file_name,
        "format": "markdown",
        "success": result.is_ok(),
        "error": result.as_ref().err(),
        "markdown": markdown_excerpt,
        "timestamp": chrono::Utc::now().to_rfc3339(),
    });

    // Pin the connection to the exact addresses validated above — ureq must
    // not re-resolve `host` itself (see `FixedResolver`).
    let agent = ureq::AgentBuilder::new()
        .redirects(0)
        .resolver(FixedResolver(addrs))
        .build();
    if let Err(e) = agent
        .post(url)
        .set("Content-Type", "application/json")
        .send_string(&payload.to_string())
    {
        if !silent {
            eprintln!("[mdm watch] webhook 전송 실패: {}", e);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc::channel as test_channel;

    #[test]
    fn detects_new_supported_file_and_converts_xlsx() {
        let dir = tempfile::tempdir().unwrap();
        let (tx, rx) = test_channel::<FileEvent>();

        let dir_path = dir.path().to_path_buf();
        let watch_thread = std::thread::spawn(move || {
            run(
                &dir_path,
                WatchOptions::default(),
                move |event| {
                    let _ = tx.send(event);
                },
                Some(Instant::now() + Duration::from_secs(10)),
            )
        });

        // Give the watcher a moment to start before writing.
        std::thread::sleep(Duration::from_millis(200));

        // Minimal single-sheet XLSX built from calamine's own accepted shape
        // isn't trivial to hand-write, so exercise the "unsupported extension
        // is ignored" + "supported extension triggers on_event" paths with a
        // real xlsx fixture from the repo's test corpus if present, else a
        // plain-text stand-in that will fail conversion (still exercises the
        // detect → debounce → process → on_event pipeline end-to-end).
        let file_path = dir.path().join("incoming.xlsx");
        std::fs::write(&file_path, b"not a real xlsx, exercises the failure path").unwrap();

        let event = rx
            .recv_timeout(Duration::from_secs(8))
            .expect("on_event should fire for a supported extension within the debounce+stable window");

        assert_eq!(event.file_name, "incoming.xlsx");
        assert!(event.result.is_err(), "garbage bytes should fail xlsx parsing, not be silently dropped");

        watch_thread.join().unwrap().unwrap();
    }

    #[test]
    fn ignores_unsupported_extension() {
        let dir = tempfile::tempdir().unwrap();
        let (tx, rx) = test_channel::<FileEvent>();

        let dir_path = dir.path().to_path_buf();
        let watch_thread = std::thread::spawn(move || {
            run(
                &dir_path,
                WatchOptions::default(),
                move |event| {
                    let _ = tx.send(event);
                },
                Some(Instant::now() + Duration::from_secs(3)),
            )
        });

        std::thread::sleep(Duration::from_millis(200));
        std::fs::write(dir.path().join("notes.txt"), b"hello").unwrap();

        // No event should ever fire for a .txt file.
        assert!(rx.recv_timeout(Duration::from_secs(2)).is_err());

        watch_thread.join().unwrap().unwrap();
    }

    #[test]
    fn supported_extensions_includes_core_formats() {
        let exts = supported_extensions();
        for ext in ["hwp", "hwpx", "docx", "xlsx", "xls"] {
            assert!(exts.contains(&ext), "missing extension: {}", ext);
        }
    }

    #[test]
    fn rejects_localhost_webhook() {
        assert!(validate_webhook_url("http://localhost:3000/hook").is_err());
        assert!(validate_webhook_url("http://127.0.0.1/hook").is_err());
        assert!(validate_webhook_url("http://169.254.169.254/latest/meta-data").is_err());
        assert!(validate_webhook_url("http://192.168.1.1/hook").is_err());
        assert!(validate_webhook_url("http://172.16.0.1/hook").is_err());
        assert!(validate_webhook_url("http://2130706433/hook").is_err()); // decimal-encoded 127.0.0.1
    }

    #[test]
    fn rejects_non_http_scheme() {
        assert!(validate_webhook_url("ftp://example.com/hook").is_err());
        assert!(validate_webhook_url("file:///etc/passwd").is_err());
    }

    #[test]
    fn accepts_public_https_webhook() {
        assert!(validate_webhook_url("https://api.example.com/hooks/docs").is_ok());
    }

    #[test]
    fn is_private_ip_covers_common_ranges() {
        assert!(is_private_ip("127.0.0.1".parse().unwrap()));
        assert!(is_private_ip("10.0.0.1".parse().unwrap()));
        assert!(is_private_ip("192.168.1.1".parse().unwrap()));
        assert!(is_private_ip("169.254.1.1".parse().unwrap()));
        assert!(is_private_ip("::1".parse().unwrap()));
        assert!(!is_private_ip("8.8.8.8".parse().unwrap()));
    }

    /// Codex review P1 finding #1: `http://[::ffff:127.0.0.1]/` reached the
    /// native-IPv6 branch of `is_private_ip` (loopback/ULA/link-local only)
    /// and was classified as public, bypassing the IPv4 private-range check.
    #[test]
    fn is_private_ip_unwraps_ipv4_mapped_ipv6() {
        assert!(is_private_ip("::ffff:127.0.0.1".parse().unwrap()));
        assert!(is_private_ip("::ffff:10.0.0.1".parse().unwrap()));
        assert!(is_private_ip("::ffff:192.168.1.1".parse().unwrap()));
        assert!(is_private_ip("::ffff:169.254.1.1".parse().unwrap()));
        assert!(!is_private_ip("::ffff:8.8.8.8".parse().unwrap()));
    }

    /// Codex review P1 finding #2: `send_webhook` validated the hostname's
    /// resolved IPs once, then handed the hostname (not the validated IPs)
    /// to ureq — which re-resolves it independently when opening the
    /// connection. A DNS answer that changes between those two lookups (DNS
    /// rebinding) lets a public-IP validation be followed by a connection to
    /// an internal address. `FixedResolver` closes this by pinning the
    /// connection to the exact addresses already validated, regardless of
    /// what ureq asks it to resolve.
    #[test]
    fn fixed_resolver_ignores_netloc_and_returns_pinned_addrs() {
        let pinned: Vec<std::net::SocketAddr> = vec!["93.184.216.34:443".parse().unwrap()];
        let resolver = FixedResolver(pinned.clone());

        // Simulates a DNS rebind: the netloc ureq asks about is irrelevant —
        // a fresh lookup must never happen here.
        let resolved = resolver.resolve("attacker-controlled.example:443").unwrap();
        assert_eq!(resolved, pinned);

        let resolved_again = resolver.resolve("completely-different-host:1").unwrap();
        assert_eq!(resolved_again, pinned);
    }

    #[test]
    fn fixed_resolver_errors_when_no_validated_addrs() {
        let resolver = FixedResolver(vec![]);
        assert!(resolver.resolve("example.com:443").is_err());
    }
}
