//! The plugins of the shell: directory, manifest, digests, hot reload (spec 007, REQ-007-4
//! to 8, 11; task 007-4).

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::body::Body;
use axum::http::{Request, Response, StatusCode, header};
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use http_body_util::BodyExt;
use kartenn_rest::health::Readiness;
use kartenn_rest::quota::{Quotas, Rates};
use kartenn_rest::transport::Settings;
use kartenn_rest::web::plugins::{MAX_FILES, Published};
use kartenn_rest::web::{Plugins, Web};
use serde_json::{Value, json};
use tower::ServiceExt;
use uuid::Uuid;

use crate::common;

struct Ready;

impl Readiness for Ready {
    fn ready(&self) -> std::pin::Pin<Box<dyn std::future::Future<Output = bool> + Send + '_>> {
        Box::pin(std::future::ready(true))
    }
}

fn dir() -> PathBuf {
    let dir = std::env::temp_dir().join(format!("kartenn-plugins-{}", Uuid::now_v7()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn manifest(name: &str, version: &str) -> Value {
    json!({
        "name": name,
        "version": version,
        "sdk": "^1.0",
        "entry": "index.js",
        "module": null,
        "catalogs": { "fr": "i18n/fr.json", "en": "i18n/en.json" },
        "extensions": [
            { "point": "view", "id": "overview", "element": format!("{name}-overview"), "nav": "nav.overview" },
            { "point": "tab", "id": "notes", "element": format!("{name}-notes"),
              "object_types": ["prefix"], "label": "tab.notes", "permission": "core.prefix.read" },
            { "point": "column", "id": "flag", "element": format!("{name}-flag"),
              "object_types": ["address"], "label": "column.flag" }
        ]
    })
}

/// Deposits a version: manifest, entry, a chunk, a style sheet, two catalogues, an image.
fn deposit(root: &Path, manifest: &Value) -> PathBuf {
    let name = manifest["name"].as_str().unwrap();
    let version = manifest["version"].as_str().unwrap();
    let dir = root.join(name).join(version);
    std::fs::create_dir_all(dir.join("i18n")).unwrap();
    std::fs::write(dir.join("manifest.json"), manifest.to_string()).unwrap();
    std::fs::write(
        dir.join("index.js"),
        format!("export const v = '{version}';"),
    )
    .unwrap();
    std::fs::write(dir.join("chunk-3f2a.js"), "export {};").unwrap();
    std::fs::write(dir.join("styles.css"), ":host{}").unwrap();
    std::fs::write(dir.join("logo.png"), [137_u8, 80, 78, 71]).unwrap();
    let catalog = json!({ "nav": { "overview": "Vue" }, "tab.notes": "Notes", "column": { "flag": "Drapeau" } });
    std::fs::write(dir.join("i18n/fr.json"), catalog.to_string()).unwrap();
    std::fs::write(dir.join("i18n/en.json"), catalog.to_string()).unwrap();
    dir
}

fn modules() -> BTreeSet<String> {
    BTreeSet::from(["objects".to_owned()])
}

fn published(plugins: &Plugins) -> Vec<(String, String)> {
    plugins
        .published()
        .into_iter()
        .map(|p| (p.name, p.version))
        .collect()
}

fn app(plugins: Arc<Plugins>) -> Router {
    kartenn_rest::router_with_web(
        Router::new(),
        Arc::new(Ready),
        Settings::default(),
        Arc::new(Quotas::new(Rates::default())),
        common::anyone(),
        Some(Arc::new(Web::new(None, Some(plugins)))),
    )
}

async fn get(app: &Router, uri: &str) -> Response<Body> {
    app.clone()
        .oneshot(Request::get(uri).body(Body::empty()).unwrap())
        .await
        .unwrap()
}

#[test]
fn the_highest_valid_version_is_published_and_the_others_served() {
    let root = dir();
    deposit(&root, &manifest("example", "1.0.0"));
    deposit(&root, &manifest("example", "1.1.0"));
    deposit(&root, &manifest("example", "1.10.0-beta.1"));
    let plugins = Plugins::open(&root, modules()).unwrap();
    assert_eq!(
        published(&plugins),
        [("example".to_owned(), "1.10.0-beta.1".to_owned())]
    );
    std::fs::remove_dir_all(root.join("example/1.10.0-beta.1")).unwrap();
    plugins.reload();
    let p: Published = plugins.published().pop().unwrap();
    assert_eq!(p.version, "1.1.0");
    assert_eq!(p.base, "/plugins/example/1.1.0/");
    assert_eq!(p.entry, "/plugins/example/1.1.0/index.js");
    assert_eq!(p.sdk, "^1.0");
    assert_eq!(
        p.integrity.keys().map(String::as_str).collect::<Vec<_>>(),
        [
            "chunk-3f2a.js",
            "i18n/en.json",
            "i18n/fr.json",
            "index.js",
            "manifest.json",
            "styles.css"
        ],
        "modules, style sheets and catalogues; not the images"
    );
    assert_eq!(p.extensions.len(), 3);
}

#[tokio::test]
async fn files_are_served_with_their_digest_and_nothing_else() {
    let root = dir();
    deposit(&root, &manifest("example", "1.0.0"));
    deposit(&root, &manifest("example", "1.1.0"));
    std::fs::write(root.join("example/secret.txt"), "outside a version").unwrap();
    let plugins = Arc::new(Plugins::open(&root, modules()).unwrap());
    let app = app(plugins.clone());
    let old = get(&app, "/plugins/example/1.0.0/index.js").await;
    assert_eq!(old.status(), StatusCode::OK);
    assert_eq!(
        old.headers()[header::CONTENT_TYPE],
        "text/javascript; charset=utf-8"
    );
    assert_eq!(old.headers()[header::CACHE_CONTROL], "no-cache");
    let sri = &plugins.published()[0].integrity["index.js"];
    let tag = old.headers()[header::ETAG].to_str().unwrap().to_owned();
    assert!(tag.starts_with("\"sha384-"));
    let body = old.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(&body[..], b"export const v = '1.0.0';");
    let new = get(&app, "/plugins/example/1.1.0/index.js").await;
    assert_eq!(
        new.headers()[header::ETAG].to_str().unwrap(),
        format!("\"{sri}\"")
    );
    let again = app
        .clone()
        .oneshot(
            Request::get("/plugins/example/1.0.0/index.js")
                .header(header::IF_NONE_MATCH, &tag)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(again.status(), StatusCode::NOT_MODIFIED);
    for path in [
        "/plugins/example/secret.txt",
        "/plugins/example/1.1.0/../secret.txt",
        "/plugins/example/1.1.0/%2e%2e/secret.txt",
        "/plugins/example/1.1.0/absent.js",
        "/plugins/example/2.0.0/index.js",
        "/plugins/absent",
    ] {
        assert_eq!(
            get(&app, path).await.status(),
            StatusCode::NOT_FOUND,
            "{path}"
        );
    }
}

#[test]
fn the_digest_is_that_of_sri() {
    let root = dir();
    let version = deposit(&root, &manifest("example", "1.0.0"));
    std::fs::write(version.join("index.js"), "abc").unwrap();
    let plugins = Plugins::open(&root, modules()).unwrap();
    // SHA-384 of "abc" (FIPS 180-2, appendix D.1), as `openssl dgst -sha384 -binary |
    // base64` prints it.
    let hex = "cb00753f45a35e8bb5a03d699ac65007272c32ab0eded1631a8b605a43ff5bed\
               8086072ba1e7cc2358baeca134c825a7";
    let bytes: Vec<u8> = (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
        .collect();
    assert_eq!(
        plugins.published()[0].integrity["index.js"],
        format!("sha384-{}", STANDARD.encode(bytes))
    );
}

/// A change that makes a manifest faulty.
type Fault = Box<dyn Fn(&mut Value)>;

#[test]
fn each_faulty_manifest_is_rejected_and_the_others_published() {
    let root = dir();
    deposit(&root, &manifest("example", "1.0.0"));
    let faults: Vec<(&str, Fault)> = vec![
        ("name", Box::new(|m| m["name"] = json!("autre"))),
        ("version", Box::new(|m| m["version"] = json!("1.0.1"))),
        ("sdk", Box::new(|m| m["sdk"] = json!(">=1"))),
        ("entry", Box::new(|m| m["entry"] = json!("absent.js"))),
        (
            "point",
            Box::new(|m| m["extensions"][0]["point"] = json!("slot")),
        ),
        (
            "element",
            Box::new(|m| m["extensions"][1]["element"] = json!("autre-onglet")),
        ),
        (
            "catalog",
            Box::new(|m| m["catalogs"]["de"] = json!("i18n/de.json")),
        ),
        (
            "english",
            Box::new(|m| {
                m["catalogs"].as_object_mut().unwrap().remove("en");
            }),
        ),
        (
            "label",
            Box::new(|m| m["extensions"][1]["label"] = json!("tab.absent")),
        ),
        (
            "types",
            Box::new(|m| m["extensions"][1]["object_types"] = json!([])),
        ),
        (
            "id",
            Box::new(|m| m["extensions"][2]["id"] = json!("notes")),
        ),
        ("module", Box::new(|m| m["module"] = json!("vlan"))),
    ];
    for (i, (_, fault)) in faults.iter().enumerate() {
        let name = format!("faulty{i}");
        let mut m = manifest(&name, "1.0.0");
        let dir = deposit(&root, &m);
        fault(&mut m);
        std::fs::write(dir.join("manifest.json"), m.to_string()).unwrap();
    }
    let plugins = Plugins::open(&root, modules()).unwrap();
    assert_eq!(
        published(&plugins),
        [("example".to_owned(), "1.0.0".to_owned())],
        "{:?}",
        faults.iter().map(|(n, _)| *n).collect::<Vec<_>>()
    );
}

#[test]
fn a_plugin_of_a_module_of_the_build_is_published() {
    let root = dir();
    let mut m = manifest("vlan", "1.0.0");
    m["module"] = json!("vlan");
    deposit(&root, &m);
    let without = Plugins::open(&root, modules()).unwrap();
    assert!(published(&without).is_empty());
    let with = Plugins::open(&root, BTreeSet::from(["vlan".to_owned()])).unwrap();
    assert_eq!(published(&with), [("vlan".to_owned(), "1.0.0".to_owned())]);
}

#[test]
fn limits_of_size_number_and_links() {
    let root = dir();
    let many = deposit(&root, &manifest("many", "1.0.0"));
    for i in 0..MAX_FILES {
        std::fs::write(many.join(format!("f{i}.txt")), "").unwrap();
    }
    let big = deposit(&root, &manifest("big", "1.0.0"));
    std::fs::write(big.join("big.bin"), vec![0_u8; 21 * 1024 * 1024]).unwrap();
    let linked = deposit(&root, &manifest("linked", "1.0.0"));
    std::os::unix::fs::symlink("/etc/hostname", linked.join("leak.txt")).unwrap();
    deposit(&root, &manifest("example", "1.0.0"));
    let plugins = Plugins::open(&root, modules()).unwrap();
    assert_eq!(
        published(&plugins),
        [("example".to_owned(), "1.0.0".to_owned())]
    );
}

/// The OCI image points `plugins_dir` at a link into the store (`/opt/kartenn/plugins`,
/// spec 015): the directory itself may be a link, its content may not.
#[tokio::test]
async fn a_plugins_dir_that_is_a_link_is_read_through() {
    let real = dir();
    deposit(&real, &manifest("example", "1.0.0"));
    let link = std::env::temp_dir().join(format!("kartenn-plugins-link-{}", Uuid::now_v7()));
    std::os::unix::fs::symlink(&real, &link).unwrap();
    let plugins = Arc::new(Plugins::open(&link, modules()).unwrap());
    assert_eq!(
        published(&plugins),
        [("example".to_owned(), "1.0.0".to_owned())]
    );
    let app = app(plugins);
    assert_eq!(
        get(&app, "/plugins/example/1.0.0/index.js").await.status(),
        StatusCode::OK
    );
}

#[tokio::test]
async fn a_deposit_and_a_removal_appear_at_the_next_reading() {
    let root = dir();
    let plugins = Arc::new(Plugins::open(&root, modules()).unwrap());
    let watcher = plugins.watch(Duration::from_millis(100));
    deposit(&root, &manifest("example", "1.2.0"));
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    while published(&plugins).is_empty() {
        assert!(tokio::time::Instant::now() < deadline, "never read again");
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    std::fs::remove_dir_all(root.join("example")).unwrap();
    while !published(&plugins).is_empty() {
        assert!(tokio::time::Instant::now() < deadline, "never removed");
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    watcher.abort();
    assert_eq!(
        kartenn_rest::web::plugins::RELOAD_EVERY,
        Duration::from_secs(10)
    );
}

#[test]
fn a_rejection_is_logged_once_per_state_and_a_load_once() {
    use tracing_subscriber::layer::SubscriberExt;
    let (output, mut logs) = kartenn_telemetry::capture::Output::new("test", "debug", 64).unwrap();
    let subscriber = tracing_subscriber::registry()
        .with(kartenn_telemetry::capture::CaptureLayer::new(vec![output]));
    let _guard = tracing::subscriber::set_default(subscriber);
    let root = dir();
    deposit(&root, &manifest("example", "1.0.0"));
    let mut m = manifest("faulty", "1.0.0");
    let faulty = deposit(&root, &m);
    m["sdk"] = json!("*");
    std::fs::write(faulty.join("manifest.json"), m.to_string()).unwrap();
    let plugins = Plugins::open(&root, modules()).unwrap();
    plugins.reload();
    let mut names = Vec::new();
    while let Ok(record) = logs.try_recv() {
        if record.name.starts_with("web.plugin") {
            names.push((record.name, record.fields.get("plugin").cloned()));
        }
    }
    assert_eq!(
        names,
        [
            ("web.plugin.rejected", Some("faulty".to_owned())),
            ("web.plugin.loaded", Some("example".to_owned())),
        ]
    );
}

#[test]
fn a_file_rewritten_in_place_gets_its_new_digest() {
    let root = dir();
    let version = deposit(&root, &manifest("example", "1.0.0"));
    let plugins = Plugins::open(&root, modules()).unwrap();
    let before = plugins.published()[0].integrity["index.js"].clone();
    std::fs::write(
        version.join("index.js"),
        "export const v = 'other, longer';",
    )
    .unwrap();
    plugins.reload();
    assert_ne!(plugins.published()[0].integrity["index.js"], before);
}

/// The plugin of the VLAN module as written in web/plugins/vlan (spec 008, REQ-008-24): its
/// manifest and catalogues hold, and it is published only by a build with the module.
#[test]
fn the_vlan_plugin_is_published_only_with_its_module() {
    use tracing_subscriber::layer::SubscriberExt;
    // The reason of a rejection, in the message of a failure.
    let (output, mut logs) = kartenn_telemetry::capture::Output::new("test", "debug", 64).unwrap();
    let subscriber = tracing_subscriber::registry()
        .with(kartenn_telemetry::capture::CaptureLayer::new(vec![output]));
    let _guard = tracing::subscriber::set_default(subscriber);
    let mut rejections = || {
        let mut found = Vec::new();
        while let Ok(record) = logs.try_recv() {
            if record.name == "web.plugin.rejected" {
                found.push(format!("{:?}", record.fields));
            }
        }
        found
    };
    let public = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../web/plugins/vlan/public");
    let manifest: Value =
        serde_json::from_slice(&std::fs::read(public.join("manifest.json")).unwrap()).unwrap();
    let with = |modules: &[&str]| {
        let root = dir();
        let version = root
            .join("vlan")
            .join(manifest["version"].as_str().unwrap());
        std::fs::create_dir_all(version.join("i18n")).unwrap();
        for file in [
            "manifest.json",
            "styles.css",
            "i18n/fr.json",
            "i18n/en.json",
        ] {
            std::fs::copy(public.join(file), version.join(file)).unwrap();
        }
        std::fs::write(version.join("index.js"), "export {};").unwrap();
        let modules = modules.iter().map(|&m| m.to_owned()).collect();
        published(&Plugins::open(&root, modules).unwrap())
    };
    assert_eq!(
        with(&["objects", "vlan"]),
        [("vlan".to_owned(), "1.0.0".to_owned())],
        "{:?}",
        rejections()
    );
    assert!(with(&["objects"]).is_empty());
    assert!(
        rejections()
            .iter()
            .any(|r| r.contains("module vlan absent from the build"))
    );
}
