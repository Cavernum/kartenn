//! The shell served on the origin of the API (spec 007, REQ-007-1 to 3; task 007-3).

use std::path::{Path, PathBuf};
use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::http::{Request, Response, StatusCode, header};
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use http_body_util::BodyExt;
use kartenn_rest::health::Readiness;
use kartenn_rest::quota::{Quotas, Rates};
use kartenn_rest::transport::Settings;
use kartenn_rest::web::{Shell, Web, WebError};
use sha2::{Digest, Sha256};
use tower::ServiceExt;
use uuid::Uuid;

use crate::common;

struct Ready;

impl Readiness for Ready {
    fn ready(&self) -> std::pin::Pin<Box<dyn std::future::Future<Output = bool> + Send + '_>> {
        Box::pin(std::future::ready(true))
    }
}

const MAP: &str = r#"{"imports":{"@kartenn/sdk":"/assets/sdk-1a2b.js"}}"#;

fn index(map: &str) -> String {
    format!(
        "<!doctype html><html><head><script type=\"importmap\">{map}</script>\
         <script type=\"module\" src=\"/assets/index-3f2a.js\"></script></head>\
         <body><div id=\"app\"></div></body></html>"
    )
}

/// A build of the shell: `index.html`, two assets and a favicon.
fn shell() -> PathBuf {
    let dir = std::env::temp_dir().join(format!("kartenn-web-{}", Uuid::now_v7()));
    std::fs::create_dir_all(dir.join("assets")).unwrap();
    std::fs::write(dir.join("index.html"), index(MAP)).unwrap();
    std::fs::write(dir.join("assets/index-3f2a.js"), "export {};").unwrap();
    std::fs::write(dir.join("assets/index-3f2a.css"), "body{}").unwrap();
    std::fs::write(dir.join("favicon.ico"), [0_u8, 1, 2]).unwrap();
    std::fs::write(dir.join(".env"), "SECRET=1").unwrap();
    dir
}

fn app(root: Option<&Path>) -> Router {
    let web = root.map(|root| Arc::new(Web::new(Some(Shell::open(root).unwrap()), None)));
    kartenn_rest::router_with_web(
        Router::new(),
        Arc::new(Ready),
        Settings::default(),
        Arc::new(Quotas::new(Rates::default())),
        common::anyone(),
        web,
    )
}

async fn send(app: &Router, method: &str, uri: &str) -> Response<Body> {
    app.clone()
        .oneshot(
            Request::builder()
                .method(method)
                .uri(uri)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap()
}

async fn text(response: Response<Body>) -> String {
    String::from_utf8(
        response
            .into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes()
            .to_vec(),
    )
    .unwrap()
}

fn header_of<'a>(response: &'a Response<Body>, name: &header::HeaderName) -> &'a str {
    response
        .headers()
        .get(name)
        .map_or("", |v| v.to_str().unwrap())
}

fn digest(map: &str) -> String {
    format!(
        "'sha256-{}'",
        STANDARD.encode(Sha256::digest(map.as_bytes()))
    )
}

#[tokio::test]
async fn a_route_of_the_client_gets_index_html_with_the_headers_of_the_shell() {
    let root = shell();
    let app = app(Some(&root));
    let response = send(&app, "GET", &format!("/prefixes/{}", Uuid::now_v7())).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert!(header_of(&response, &header::CONTENT_TYPE).starts_with("text/html"));
    let csp = header_of(&response, &header::CONTENT_SECURITY_POLICY).to_owned();
    assert!(
        csp.contains(&format!("script-src 'self' {}", digest(MAP))),
        "{csp}"
    );
    for directive in [
        "default-src 'none'",
        "connect-src 'self'",
        "frame-ancestors 'none'",
        "frame-src 'none'",
        "base-uri 'none'",
    ] {
        assert!(csp.contains(directive), "{directive}: {csp}");
    }
    assert!(!csp.contains("unsafe-inline") && !csp.contains("unsafe-eval"));
    assert_eq!(
        header_of(&response, &header::X_CONTENT_TYPE_OPTIONS),
        "nosniff"
    );
    assert_eq!(header_of(&response, &header::CACHE_CONTROL), "no-cache");
    assert!(!header_of(&response, &header::ETAG).is_empty());
    assert_eq!(text(response).await, index(MAP));
    assert_eq!(send(&app, "GET", "/").await.status(), StatusCode::OK);
}

#[tokio::test]
async fn the_api_keeps_its_problems_and_a_missing_file_is_404() {
    let root = shell();
    let app = app(Some(&root));
    let response = app
        .clone()
        .oneshot(common::bearer(
            Request::get("/v1/inconnu").body(Body::empty()).unwrap(),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    assert_eq!(
        header_of(&response, &header::CONTENT_TYPE),
        "application/problem+json"
    );
    assert!(header_of(&response, &header::CONTENT_SECURITY_POLICY).is_empty());
    assert_eq!(
        send(&app, "GET", "/health/absent").await.status(),
        StatusCode::NOT_FOUND
    );
    let response = send(&app, "GET", "/assets/absent.js").await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    assert_eq!(
        header_of(&response, &header::X_CONTENT_TYPE_OPTIONS),
        "nosniff"
    );
}

#[tokio::test]
async fn nothing_outside_the_root_nor_hidden_is_served() {
    let root = shell();
    let outside = std::env::temp_dir().join(format!("kartenn-outside-{}.txt", Uuid::now_v7()));
    std::fs::write(&outside, "outside").unwrap();
    std::os::unix::fs::symlink(&outside, root.join("leak.txt")).unwrap();
    std::os::unix::fs::symlink(std::env::temp_dir(), root.join("tmp")).unwrap();
    let app = app(Some(&root));
    for path in [
        "/../etc/passwd",
        "/%2e%2e/etc/passwd",
        "/assets/%2e%2e/%2e%2e/etc/passwd.txt",
        "/.env",
        "/assets/.hidden.js",
        "/leak.txt",
        "/a%2f..%2f.env",
    ] {
        let response = send(&app, "GET", path).await;
        assert_eq!(response.status(), StatusCode::NOT_FOUND, "{path}");
        assert!(!text(response).await.contains("SECRET"), "{path}");
    }
    let name = outside.file_name().unwrap().to_str().unwrap();
    let response = send(&app, "GET", &format!("/tmp/{name}")).await;
    assert_eq!(
        response.status(),
        StatusCode::NOT_FOUND,
        "symbolic link out"
    );
}

/// The OCI image points `[web] root` at a link into the store (`/opt/kartenn/shell`,
/// spec 015): the root is canonicalized, its files are served.
#[tokio::test]
async fn a_root_that_is_a_link_is_served_through() {
    let real = shell();
    let link = std::env::temp_dir().join(format!("kartenn-web-link-{}", Uuid::now_v7()));
    std::os::unix::fs::symlink(&real, &link).unwrap();
    let app = app(Some(&link));
    assert_eq!(send(&app, "GET", "/").await.status(), StatusCode::OK);
    assert_eq!(
        send(&app, "GET", "/assets/index-3f2a.js").await.status(),
        StatusCode::OK
    );
}

#[tokio::test]
async fn only_get_and_head_are_served() {
    let root = shell();
    let app = app(Some(&root));
    let response = send(&app, "POST", "/prefixes").await;
    assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
    assert_eq!(header_of(&response, &header::ALLOW), "GET, HEAD");
    let response = send(&app, "HEAD", "/favicon.ico").await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(header_of(&response, &header::CONTENT_LENGTH), "3");
    assert_eq!(header_of(&response, &header::CONTENT_TYPE), "image/x-icon");
    assert!(text(response).await.is_empty());
}

#[tokio::test]
async fn hashed_assets_are_immutable_and_the_others_revalidated() {
    let root = shell();
    let app = app(Some(&root));
    let response = send(&app, "GET", "/assets/index-3f2a.js").await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        header_of(&response, &header::CACHE_CONTROL),
        "public, max-age=31536000, immutable"
    );
    assert_eq!(
        header_of(&response, &header::CONTENT_TYPE),
        "text/javascript; charset=utf-8"
    );
    let response = send(&app, "GET", "/").await;
    let tag = header_of(&response, &header::ETAG).to_owned();
    let again = app
        .clone()
        .oneshot(
            Request::get("/")
                .header(header::IF_NONE_MATCH, &tag)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(again.status(), StatusCode::NOT_MODIFIED);
    assert_eq!(header_of(&again, &header::ETAG), tag);
}

#[tokio::test]
async fn a_new_build_gives_the_digest_of_its_import_map_at_the_next_request() {
    let root = shell();
    let app = app(Some(&root));
    let before = send(&app, "GET", "/").await;
    assert!(header_of(&before, &header::CONTENT_SECURITY_POLICY).contains(&digest(MAP)));
    let map = r#"{"imports":{"@kartenn/sdk":"/assets/sdk-9z8y.js"}}"#;
    std::fs::write(
        root.join("index.html"),
        format!("{}<!-- new -->", index(map)),
    )
    .unwrap();
    let after = send(&app, "GET", "/assets/index-3f2a.css").await;
    let csp = header_of(&after, &header::CONTENT_SECURITY_POLICY);
    assert!(csp.contains(&digest(map)), "{csp}");
    assert!(!csp.contains(&digest(MAP)), "{csp}");
}

#[tokio::test]
async fn without_web_everything_outside_the_api_is_404_as_before() {
    let app = app(None);
    let response = send(&app, "GET", "/prefixes").await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    assert_eq!(
        header_of(&response, &header::CONTENT_TYPE),
        "application/problem+json"
    );
}

#[tokio::test]
async fn a_shell_of_sixty_files_loaded_without_login_is_not_throttled() {
    let root = shell();
    for i in 0..60 {
        std::fs::write(root.join(format!("assets/chunk-{i}.js")), "export {};").unwrap();
    }
    let app = app(Some(&root));
    for i in 0..60 {
        let response = send(&app, "GET", &format!("/assets/chunk-{i}.js")).await;
        assert_eq!(response.status(), StatusCode::OK, "file {i}");
    }
}

#[test]
fn a_root_without_index_or_import_map_is_refused() {
    let root = shell();
    std::fs::write(root.join("index.html"), "<html></html>").unwrap();
    assert!(matches!(Shell::open(&root), Err(WebError::NoImportMap(_))));
    std::fs::remove_file(root.join("index.html")).unwrap();
    assert!(matches!(Shell::open(&root), Err(WebError::Root(_))));
}
