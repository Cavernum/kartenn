//! One version for the release (spec 015, REQ-015-12): the workspace, every crate of
//! kartenn, the web packages and the changelog carry it. `syslog-sender` is a library of
//! its own (012/L24) and `kartenn-workspace-hack` is managed by hakari: both keep theirs.

use std::path::{Path, PathBuf};

/// The root of the repository.
fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[test]
fn every_crate_of_kartenn_takes_the_workspace_version() {
    let mut crates = Vec::new();
    for entry in std::fs::read_dir(root().join("crates")).expect("the crates") {
        let dir = entry.expect("a crate").path();
        let name = dir.file_name().unwrap().to_string_lossy().into_owned();
        if !name.starts_with("kartenn-") || name == "kartenn-workspace-hack" {
            continue;
        }
        let manifest = std::fs::read_to_string(dir.join("Cargo.toml")).expect("its manifest");
        assert!(
            manifest
                .lines()
                .any(|l| l.trim() == "version.workspace = true"),
            "{name}: `version.workspace = true` expected"
        );
        crates.push(name);
    }
    assert!(crates.len() > 10, "{crates:?}");
}

#[test]
fn the_web_packages_carry_the_workspace_version() {
    for package in ["sdk", "shell", "e2e"] {
        let path = root().join("web").join(package).join("package.json");
        let json: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).expect("package.json"))
                .expect("JSON");
        assert_eq!(json["version"], VERSION, "{}", path.display());
    }
}

#[test]
fn the_changelog_has_an_entry_for_the_version() {
    let changelog = std::fs::read_to_string(root().join("CHANGELOG.md")).expect("CHANGELOG.md");
    let heading = format!("## [{VERSION}]");
    assert!(
        changelog.lines().any(|l| l.starts_with(&heading)),
        "CHANGELOG.md: no `{heading}` entry"
    );
}
