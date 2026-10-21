//! The core and the service know only the ports and the `tracing` facade (spec 012,
//! REQ-012-2; ADR 0001; task 012-19): no crate of an output reaches them, even
//! transitively. The workspace-hack is skipped: it unifies features at build time and
//! exports nothing the code could use.

use std::collections::{BTreeSet, HashMap};
use std::process::Command;

use serde_json::Value;

/// Crates of the outputs and of the telemetry adapters.
const FORBIDDEN: [&str; 11] = [
    "kartenn-telemetry",
    "syslog-sender",
    "tracing-subscriber",
    "tracing-opentelemetry",
    "tracing-journald",
    "tracing-appender",
    "opentelemetry",
    "opentelemetry_sdk",
    "opentelemetry-otlp",
    "opentelemetry-appender-tracing",
    "tonic",
];

#[test]
fn the_core_and_the_service_reach_no_output() {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let output = Command::new(cargo)
        .args(["metadata", "--format-version", "1", "--offline"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("cargo metadata");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let metadata: Value = serde_json::from_slice(&output.stdout).unwrap();
    let names: HashMap<&str, &str> = metadata["packages"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| (p["id"].as_str().unwrap(), p["name"].as_str().unwrap()))
        .collect();
    // Normal (non-dev, non-build) dependencies of each package.
    let edges: HashMap<&str, Vec<&str>> = metadata["resolve"]["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|n| {
            let deps = n["deps"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|d| {
                    d["dep_kinds"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|k| k["kind"].is_null())
                })
                .map(|d| d["pkg"].as_str().unwrap())
                .collect();
            (n["id"].as_str().unwrap(), deps)
        })
        .collect();
    // The crates `root` reaches, by name, the workspace-hack skipped.
    let reach = |root: &str| -> BTreeSet<&str> {
        let start = names.iter().find(|(_, n)| **n == root).unwrap().0;
        let mut seen = BTreeSet::new();
        let mut todo = vec![*start];
        while let Some(id) = todo.pop() {
            if names[id] == "kartenn-workspace-hack" || !seen.insert(id) {
                continue;
            }
            todo.extend(edges.get(id).into_iter().flatten().copied());
        }
        seen.into_iter().map(|id| names[id]).collect()
    };
    for root in ["kartenn-core", "kartenn-service"] {
        let reached = reach(root);
        assert!(reached.len() > 1, "{root} has dependencies");
        for name in FORBIDDEN {
            assert!(
                !reached.contains(name),
                "{root} reaches {name}, an output crate (REQ-012-2)"
            );
        }
    }
    // The check does see a reachable output: the server composes them.
    assert!(reach("kartenn-server").contains("kartenn-telemetry"));
}
