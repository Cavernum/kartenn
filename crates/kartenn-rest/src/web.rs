//! The web shell and its plugins served on the origin of the API (spec 007, REQ-007-1 to
//! 3; W2, W17; design « Service du shell »): the files of `[web] root`, `index.html` for
//! the routes of the client, and the headers of every response outside `/v1` and
//! `/health` (CSP with the digest of the import map, `nosniff`, cache).

use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;

use arc_swap::ArcSwap;
use axum::body::Body;
use axum::extract::{Request, State};
use axum::http::{HeaderMap, HeaderValue, Method, StatusCode, header};
use axum::response::{IntoResponse, Response};
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use sha2::{Digest, Sha256};

use crate::problem::{Problem, ProblemKind};

/// Why `[web]` is refused at the start.
#[derive(Debug, thiserror::Error)]
pub enum WebError {
    /// `root` cannot be read or has no `index.html`.
    #[error("web.root: {0}")]
    Root(String),
    /// `index.html` has no import map.
    #[error("web.root: {0}/index.html has no <script type=\"importmap\">")]
    NoImportMap(String),
}

/// What kartenn serves outside `/v1` and `/health`.
#[derive(Debug)]
pub struct Web {
    shell: Option<Shell>,
}

impl Web {
    /// The shell of `root`, if any.
    #[must_use]
    pub const fn new(shell: Option<Shell>) -> Self {
        Self { shell }
    }
}

/// The build of the shell under `[web] root`.
#[derive(Debug)]
pub struct Shell {
    /// The canonical root: every file served lies under it.
    root: PathBuf,
    index: ArcSwap<Index>,
}

/// `index.html` as last read: read again when its size or date change.
#[derive(Debug)]
struct Index {
    len: u64,
    modified: Option<SystemTime>,
    body: Vec<u8>,
    etag: String,
    /// `'sha256-…'` of the import map, for `script-src`.
    import_map: String,
}

impl Shell {
    /// Opens the build under `root`.
    ///
    /// # Errors
    ///
    /// `root` unreadable, without `index.html`, or `index.html` without import map.
    pub fn open(root: &Path) -> Result<Self, WebError> {
        let shown = root.display().to_string();
        let canonical = root
            .canonicalize()
            .map_err(|e| WebError::Root(format!("{shown}: {e}")))?;
        let index = read_index(&canonical.join("index.html"))
            .map_err(|e| WebError::Root(format!("{shown}/index.html: {e}")))?
            .ok_or(WebError::NoImportMap(shown))?;
        Ok(Self {
            root: canonical,
            index: ArcSwap::from_pointee(index),
        })
    }

    /// `index.html`, read again if it changed since; a new build without import map or
    /// an unreadable one leaves the last good one in place.
    fn index(&self) -> Arc<Index> {
        let current = self.index.load_full();
        let path = self.root.join("index.html");
        let Ok(meta) = std::fs::metadata(&path) else {
            return current;
        };
        if meta.len() == current.len && meta.modified().ok() == current.modified {
            return current;
        }
        match read_index(&path) {
            Ok(Some(index)) => {
                let index = Arc::new(index);
                self.index.store(index.clone());
                index
            }
            Ok(None) => {
                tracing::warn!(name: "web.shell.no_import_map", "index.html without import map, the previous one stays");
                current
            }
            Err(e) => {
                tracing::warn!(name: "web.shell.unreadable", error = %e, "index.html unreadable, the previous one stays");
                current
            }
        }
    }

    /// The regular file of the shell at these decoded segments, if it lies under `root`
    /// (a symbolic link that leaves it is refused).
    fn file(&self, segments: &[String]) -> Option<PathBuf> {
        let path = segments
            .iter()
            .fold(self.root.clone(), |path, segment| path.join(segment));
        let canonical = path.canonicalize().ok()?;
        (canonical.starts_with(&self.root) && canonical.is_file()).then_some(canonical)
    }
}

/// Reads `index.html` and digests its import map; `None` without one.
fn read_index(path: &Path) -> std::io::Result<Option<Index>> {
    let meta = std::fs::metadata(path)?;
    let body = std::fs::read(path)?;
    let Some(map) = import_map(&String::from_utf8_lossy(&body)) else {
        return Ok(None);
    };
    Ok(Some(Index {
        len: meta.len(),
        modified: meta.modified().ok(),
        etag: etag(&body),
        import_map: format!(
            "'sha256-{}'",
            STANDARD.encode(Sha256::digest(map.as_bytes()))
        ),
        body,
    }))
}

/// The text of the first `<script type="importmap">`, as the browser hashes it.
fn import_map(html: &str) -> Option<String> {
    let lower = html.to_ascii_lowercase();
    let mut from = 0;
    while let Some(start) = lower[from..].find("<script").map(|i| i + from) {
        let end = lower[start..].find('>').map(|i| i + start)?;
        let tag: String = lower[start..end]
            .chars()
            .filter(|c| !matches!(c, '"' | '\''))
            .collect();
        if tag.split_ascii_whitespace().any(|a| a == "type=importmap") {
            let close = lower[end..].find("</script").map(|i| i + end)?;
            return Some(html[end + 1..close].to_owned());
        }
        from = end;
    }
    None
}

/// A strong validator: the SHA-256 of the content.
fn etag(body: &[u8]) -> String {
    format!("\"{}\"", STANDARD.encode(Sha256::digest(body)))
}

/// The policy of every response outside `/v1` and `/health` (design « Service du
/// shell »). No `form-action`: the logout is a form followed by a redirection to the
/// issuer.
fn csp(import_map: Option<&str>) -> String {
    let script = import_map.map_or_else(|| "'self'".to_owned(), |hash| format!("'self' {hash}"));
    format!(
        "default-src 'none'; script-src {script}; style-src 'self'; img-src 'self' data:; \
         font-src 'self'; connect-src 'self'; manifest-src 'self'; base-uri 'none'; \
         frame-src 'none'; frame-ancestors 'none'"
    )
}

/// The media type by extension: a closed table (design « Service du shell »).
fn media_type(path: &str) -> &'static str {
    match path
        .rsplit_once('.')
        .map(|(_, e)| e.to_ascii_lowercase())
        .as_deref()
    {
        Some("html") => "text/html; charset=utf-8",
        Some("js" | "mjs") => "text/javascript; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("json") => "application/json",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("ico") => "image/x-icon",
        Some("woff2") => "font/woff2",
        Some("txt") => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}

/// The decoded segments of a path, or `None` if one is refused: empty but the last,
/// starting with `.` (hidden files, `.` and `..`), or holding `/`, `\` or NUL once
/// decoded.
fn segments(path: &str) -> Option<Vec<String>> {
    let rest = path.strip_prefix('/')?;
    if rest.is_empty() {
        return Some(Vec::new());
    }
    let rest = rest.strip_suffix('/').unwrap_or(rest);
    rest.split('/')
        .map(|raw| {
            let segment = percent_encoding::percent_decode_str(raw)
                .decode_utf8()
                .ok()?
                .into_owned();
            let refused = segment.is_empty()
                || segment.starts_with('.')
                || segment.contains(['/', '\\', '\0'])
                || Path::new(&segment)
                    .components()
                    .any(|c| !matches!(c, Component::Normal(_)));
            (!refused).then_some(segment)
        })
        .collect()
}

fn under(path: &str, prefix: &str) -> bool {
    path == prefix
        || path
            .strip_prefix(prefix)
            .is_some_and(|r| r.starts_with('/'))
}

/// The fallback of the router when `[web]` is configured: the API keeps its 404 problem,
/// everything else is a file of the shell, `index.html` for a path without extension, or
/// 404.
pub async fn serve(State(web): State<Arc<Web>>, request: Request) -> Response {
    let path = request.uri().path().to_owned();
    if under(&path, "/v1") || under(&path, "/health") {
        return Problem::new(ProblemKind::NotFound).into_response();
    }
    let index = web.shell.as_ref().map(Shell::index);
    let mut response = match request.method() {
        &Method::GET | &Method::HEAD => answer(&web, index.as_deref(), &path, request.headers()),
        _ => (
            StatusCode::METHOD_NOT_ALLOWED,
            [(header::ALLOW, HeaderValue::from_static("GET, HEAD"))],
        )
            .into_response(),
    };
    if request.method() == Method::HEAD {
        *response.body_mut() = Body::empty();
    }
    secure(response.headers_mut(), index.as_deref());
    response
}

/// The headers of every response outside `/v1` and `/health` (REQ-007-2).
fn secure(headers: &mut HeaderMap, index: Option<&Index>) {
    if let Ok(value) = HeaderValue::from_str(&csp(index.map(|i| i.import_map.as_str()))) {
        headers.insert(header::CONTENT_SECURITY_POLICY, value);
    }
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    headers.insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("same-origin"),
    );
}

fn answer(web: &Web, index: Option<&Index>, path: &str, headers: &HeaderMap) -> Response {
    let (Some(shell), Some(index), Some(segments)) = (&web.shell, index, segments(path)) else {
        return not_found();
    };
    if let Some(file) = shell.file(&segments) {
        let Ok(body) = std::fs::read(&file) else {
            return not_found();
        };
        let name = file.to_string_lossy();
        if segments.first().is_some_and(|s| s == "assets") {
            return file_response(
                body,
                media_type(&name),
                "public, max-age=31536000, immutable",
                None,
            );
        }
        let tag = etag(&body);
        return revalidated(body, media_type(&name), &tag, headers);
    }
    let extension = segments.last().is_some_and(|s| s.contains('.'));
    if extension {
        return not_found();
    }
    revalidated(
        index.body.clone(),
        "text/html; charset=utf-8",
        &index.etag,
        headers,
    )
}

/// `no-cache` with a strong `ETag`; 304 when `If-None-Match` names it.
fn revalidated(body: Vec<u8>, media: &str, tag: &str, headers: &HeaderMap) -> Response {
    let matches = headers
        .get(header::IF_NONE_MATCH)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.split(',').any(|t| t.trim() == tag || t.trim() == "*"));
    if matches {
        let mut response = StatusCode::NOT_MODIFIED.into_response();
        set(&mut response, header::CACHE_CONTROL, "no-cache");
        set(&mut response, header::ETAG, tag);
        return response;
    }
    file_response(body, media, "no-cache", Some(tag))
}

fn file_response(body: Vec<u8>, media: &str, cache: &str, tag: Option<&str>) -> Response {
    let len = body.len();
    let mut response = Response::new(Body::from(body));
    set(&mut response, header::CONTENT_TYPE, media);
    set(&mut response, header::CACHE_CONTROL, cache);
    set(&mut response, header::CONTENT_LENGTH, &len.to_string());
    if let Some(tag) = tag {
        set(&mut response, header::ETAG, tag);
    }
    response
}

fn set(response: &mut Response, name: header::HeaderName, value: &str) {
    if let Ok(value) = HeaderValue::from_str(value) {
        response.headers_mut().insert(name, value);
    }
}

fn not_found() -> Response {
    (
        StatusCode::NOT_FOUND,
        [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
        "not found",
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_import_map_is_the_text_of_its_script() {
        let html = "<html><head><script type=\"module\" src=\"/a.js\"></script>\
                    <SCRIPT type='importmap'>{\"imports\":{}}</SCRIPT></head></html>";
        assert_eq!(import_map(html).as_deref(), Some("{\"imports\":{}}"));
        assert_eq!(import_map("<script>x</script>"), None);
    }

    #[test]
    fn segments_refuse_what_leaves_the_root() {
        assert_eq!(segments("/"), Some(vec![]));
        assert_eq!(
            segments("/prefixes/abc"),
            Some(vec!["prefixes".into(), "abc".into()])
        );
        for refused in [
            "/../etc/passwd",
            "/%2e%2e/etc",
            "/.env",
            "/a//b",
            "/a%2fb",
            "/a%5cb",
            "/a%00",
        ] {
            assert_eq!(segments(refused), None, "{refused}");
        }
    }
}
