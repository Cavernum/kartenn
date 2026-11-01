//! The measures a change touches (ADR 0041, decision 3; task 028-4).
//!
//! [`PATHS`] associates each measure of the catalogues with the paths of the repository
//! it exercises: the file of `kartenn-bench` that times it, and the files of the workspace
//! crates that do the work it times (a directory ends with `/`). Only the files proper to
//! a few measures are listed; the plumbing every write or read goes through (the
//! pipeline, the writing of a changeset, the models, the modules) is not, nor are the
//! migrations, which every preparation runs.
//!
//! A changed path ([`touched`]):
//! - listed for some measures: those measures;
//! - a source of the preparation of a bench ([`template::sources`]): that whole bench,
//!   whose template it makes again;
//! - any other code (`src/`, `migrations/`, `Cargo.toml`, `build.rs`) of a crate a bench
//!   exercises ([`exercised`]): every bench that exercises it, whole;
//! - `Cargo.lock` or the manifest of the workspace: every bench, whole;
//! - under `web/v2/`, `web/` or `clients/python/`: the bench script outside
//!   `kartenn-bench` it concerns, proposed, not run ([`PROPOSED`]);
//! - anything else (documentation, specs, tests, crates no bench exercises): nothing.
//!
//! The crates a bench exercises are those its preparation goes through
//! ([`template::crates`], the list of its fingerprint) and those of the paths of its
//! measures, with their workspace dependencies ([`template::crate_hashes`]), and
//! `kartenn-bench` itself. The tests check that every measure has its paths, that every
//! path exists, and that the paths and the crates of a bench agree with the crates its
//! code names.

use std::collections::BTreeSet;
use std::fmt::Write as _;

use crate::measures::{BENCHES, Bench};
use crate::template;

/// This crate.
const BENCH_CRATE: &str = "kartenn-bench";

/// The files of `kartenn-bench` that time nothing a change of them could slow down: the
/// selection of the measures, the history, this table.
const UNMEASURED: &[&str] = &[
    "crates/kartenn-bench/src/affected.rs",
    "crates/kartenn-bench/src/history.rs",
    "crates/kartenn-bench/src/measures.rs",
];

/// The files of the workspace whose change concerns every bench: the versions of the
/// dependencies, the profiles.
const WORKSPACE: &[&str] = &["Cargo.lock", "Cargo.toml"];

/// The bench scripts outside `kartenn-bench` (Playwright, Python): the start of the paths
/// that concern them, and the script. The first match wins.
pub const PROPOSED: &[(&str, &str)] = &[
    ("web/v2/", "bench-ui"),
    ("web/", "bench-web"),
    ("clients/python/", "bench-cli"),
];

/// The layers of the router every request goes through: the measures that send requests
/// to it list them.
const ROUTER: &[&str] = &[
    "crates/kartenn-rest/src/authn.rs",
    "crates/kartenn-rest/src/quota.rs",
    "crates/kartenn-rest/src/transport.rs",
    "crates/kartenn-auth/src/tokens.rs",
];
/// What the server started by `bench-http` adds.
const SERVED: &[&str] = &["crates/kartenn-server/src/http.rs"];
/// The writing of the origins and the resolution of the properties.
const PROPS_WRITE: &[&str] = &[
    "crates/kartenn-bench/src/props.rs",
    "crates/kartenn-store-pg/src/origins.rs",
    "crates/kartenn-store-pg/src/properties.rs",
    "crates/kartenn-service/src/resolution.rs",
];
/// A unitary write of the core.
const CORE_WRITE: &[&str] = &[
    "crates/kartenn-bench/src/core_budgets.rs",
    "crates/kartenn-store-pg/src/tree.rs",
    "crates/kartenn-core/src/addr.rs",
    "crates/kartenn-core/src/objects.rs",
    "crates/kartenn-service/src/operations.rs",
    "crates/kartenn-service/src/authz.rs",
];
const VLAN: &[&str] = &[
    "crates/kartenn-bench/src/vlan_budgets.rs",
    "crates/kartenn-store-pg/src/vlan.rs",
    "crates/kartenn-core/src/vlan.rs",
];
/// The reads of the machines.
const MACHINES_READ: &[&str] = &[
    "crates/kartenn-bench/src/machine_budgets.rs",
    "crates/kartenn-bench/src/vlan_budgets.rs",
    "crates/kartenn-machines/src/rest.rs",
    "crates/kartenn-machines/src/read.rs",
    "crates/kartenn-machines/src/pg.rs",
];
/// A page through the router.
const PAGE: &[&str] = &[
    "crates/kartenn-rest/src/filter.rs",
    "crates/kartenn-rest/src/cursor.rs",
];
/// The writing of a machine.
const MACHINES_WRITE: &[&str] = &[
    "crates/kartenn-bench/src/machine_budgets.rs",
    "crates/kartenn-machines/src/handler.rs",
    "crates/kartenn-machines/src/pg.rs",
    "crates/kartenn-machines/src/hosting.rs",
];
const IMPORT: &[&str] = &[
    "crates/kartenn-service/src/import.rs",
    "crates/kartenn-store-pg/src/imports.rs",
];
/// A read under access control.
const RBAC: &[&str] = &[
    "crates/kartenn-bench/src/rbac.rs",
    "crates/kartenn-store-pg/src/rbac.rs",
];
const HTTP_READ: &[&str] = &[
    "crates/kartenn-bench/src/http.rs",
    "crates/kartenn-rest/src/objects.rs",
    "crates/kartenn-rest/src/dated.rs",
    "crates/kartenn-service/src/reads.rs",
    "crates/kartenn-store-pg/src/read.rs",
    "crates/kartenn-store-pg/src/rbac.rs",
    "crates/kartenn-store-pg/src/audit_journal.rs",
];
const HTTP_PAGE: &[&str] = &[
    "crates/kartenn-bench/src/http.rs",
    "crates/kartenn-rest/src/objects.rs",
    "crates/kartenn-store-pg/src/filter.rs",
    "crates/kartenn-store-pg/src/rbac.rs",
];
const HTTP_SUBTREE: &[&str] = &[
    "crates/kartenn-bench/src/http.rs",
    "crates/kartenn-rest/src/tree.rs",
    "crates/kartenn-store-pg/src/tree.rs",
    "crates/kartenn-store-pg/src/rbac.rs",
];
const HTTP_WRITE: &[&str] = &[
    "crates/kartenn-bench/src/http.rs",
    "crates/kartenn-rest/src/changesets.rs",
    "crates/kartenn-rest/src/resources.rs",
    "crates/kartenn-rest/src/simulation.rs",
    "crates/kartenn-rest/src/csrf.rs",
];
/// The verdict of a pair of the policy, and its snapshot.
const VERDICT: &[&str] = &[
    "crates/kartenn-bench/src/policy_budgets.rs",
    "crates/kartenn-bench/src/vlan_budgets.rs",
    "crates/kartenn-policy/src/verdict.rs",
    "crates/kartenn-policy/src/evaluate.rs",
    "crates/kartenn-policy/src/rest.rs",
    "crates/kartenn-policy/src/cache.rs",
    "crates/kartenn-policy/src/snapshot.rs",
    "crates/kartenn-policy/src/load.rs",
    "crates/kartenn-policy/src/read.rs",
    "crates/kartenn-machines/src/read.rs",
];
/// The inference of the policy.
const INFERENCE: &[&str] = &[
    "crates/kartenn-bench/src/policy_budgets.rs",
    "crates/kartenn-policy/src/infer.rs",
    "crates/kartenn-policy/src/inference.rs",
];
/// The evaluation of the flows on the policy.
const EVALUATION: &[&str] = &[
    "crates/kartenn-bench/src/flow_budgets.rs",
    "crates/kartenn-flow/src/evaluation.rs",
    "crates/kartenn-policy/src/evaluate.rs",
    "crates/kartenn-policy/src/read.rs",
    "crates/kartenn-policy/src/snapshot.rs",
];
/// The re-evaluation of the flows after a write of the policy.
const REEVALUATION: &[&str] = &[
    "crates/kartenn-flow/src/reevaluation.rs",
    "crates/kartenn-policy/src/handler.rs",
];
/// A page of the flows.
const FLOWS_PAGE: &[&str] = &[
    "crates/kartenn-bench/src/flow_budgets.rs",
    "crates/kartenn-bench/src/vlan_budgets.rs",
    "crates/kartenn-flow/src/rest.rs",
    "crates/kartenn-flow/src/read.rs",
];
/// The event stream.
const EVENTS: &[&str] = &[
    "crates/kartenn-bench/src/events_budgets.rs",
    "crates/kartenn-rest/src/events/",
    "crates/kartenn-service/src/events.rs",
    "crates/kartenn-store-pg/src/events.rs",
    "crates/kartenn-store-pg/src/outbox.rs",
];
/// The server started under each configuration of the telemetry, and its outputs.
const TELEMETRY: &[&str] = &[
    "crates/kartenn-bench/src/telemetry.rs",
    "crates/kartenn-server/src/app.rs",
    "crates/kartenn-server/src/config.rs",
    "crates/kartenn-telemetry/src/",
    "crates/syslog-sender/src/",
];
/// The audit of the reads.
const READ_AUDIT: &[&str] = &[
    "crates/kartenn-service/src/reads.rs",
    "crates/kartenn-service/src/audit.rs",
    "crates/kartenn-store-pg/src/audit_journal.rs",
];

/// Each measure of the catalogues and the paths it exercises, beyond the plumbing and the
/// preparation of its bench.
pub static PATHS: &[(&str, &[&[&str]])] = &[
    ("props.origins-every-root", &[PROPS_WRITE]),
    (
        "props.explained-resolution",
        &[&[
            "crates/kartenn-bench/src/props.rs",
            "crates/kartenn-store-pg/src/explain.rs",
            "crates/kartenn-store-pg/src/property_read.rs",
            "crates/kartenn-core/src/inheritance.rs",
            "crates/kartenn-core/src/properties.rs",
        ]],
    ),
    (
        "props.grouped-resolution",
        &[&[
            "crates/kartenn-bench/src/props.rs",
            "crates/kartenn-store-pg/src/explain.rs",
            "crates/kartenn-store-pg/src/property_read.rs",
        ]],
    ),
    (
        "props.filtered-pages",
        &[&[
            "crates/kartenn-bench/src/props.rs",
            "crates/kartenn-store-pg/src/filter.rs",
            "crates/kartenn-store-pg/src/property_read.rs",
        ]],
    ),
    (
        "props.changeset-10k",
        &[PROPS_WRITE, &["crates/kartenn-service/src/cascade.rs"]],
    ),
    (
        "props.value-every-root",
        &[PROPS_WRITE, &["crates/kartenn-service/src/cascade.rs"]],
    ),
    (
        "props.origins-3-domains",
        &[PROPS_WRITE, &["crates/kartenn-service/src/cascade.rs"]],
    ),
    (
        "core.read-object",
        &[&[
            "crates/kartenn-bench/src/core_budgets.rs",
            "crates/kartenn-store-pg/src/read.rs",
            "crates/kartenn-store-pg/src/temporal.rs",
            "crates/kartenn-store-pg/src/property_read.rs",
        ]],
    ),
    // An `EXPLAIN` of the history: its indexes are in the migrations.
    (
        "core.history-index",
        &[&["crates/kartenn-bench/src/core_budgets.rs"]],
    ),
    (
        "core.filtered-page",
        &[&[
            "crates/kartenn-bench/src/core_budgets.rs",
            "crates/kartenn-store-pg/src/filter.rs",
            "crates/kartenn-store-pg/src/list.rs",
            "crates/kartenn-store-pg/src/property_read.rs",
        ]],
    ),
    (
        "core.subtree",
        &[&[
            "crates/kartenn-bench/src/core_budgets.rs",
            "crates/kartenn-store-pg/src/tree.rs",
            "crates/kartenn-core/src/occupancy.rs",
        ]],
    ),
    (
        "core.roots-page",
        &[&[
            "crates/kartenn-bench/src/core_budgets.rs",
            "crates/kartenn-store-pg/src/tree.rs",
            "crates/kartenn-store-pg/src/rbac.rs",
            "crates/kartenn-store-pg/src/list.rs",
        ]],
    ),
    (
        "core.wide-children",
        &[&[
            "crates/kartenn-bench/src/core_budgets.rs",
            "crates/kartenn-store-pg/src/tree.rs",
            "crates/kartenn-store-pg/src/rbac.rs",
            "crates/kartenn-store-pg/src/list.rs",
            "crates/kartenn-store-pg/src/temporal.rs",
        ]],
    ),
    (
        "core.subtree-past",
        &[&[
            "crates/kartenn-bench/src/core_budgets.rs",
            "crates/kartenn-store-pg/src/tree.rs",
            "crates/kartenn-store-pg/src/temporal.rs",
            "crates/kartenn-core/src/occupancy.rs",
        ]],
    ),
    ("core.unitary-changeset", &[CORE_WRITE]),
    ("core.sustained", &[CORE_WRITE]),
    (
        "core.import",
        &[&["crates/kartenn-bench/src/core_budgets.rs"], IMPORT],
    ),
    (
        "core.vlan-read",
        &[
            VLAN,
            ROUTER,
            &[
                "crates/kartenn-rest/src/vlan.rs",
                "crates/kartenn-service/src/vlan.rs",
            ],
        ],
    ),
    (
        "core.vlan-filtered-page",
        &[
            VLAN,
            ROUTER,
            PAGE,
            &[
                "crates/kartenn-rest/src/vlan.rs",
                "crates/kartenn-store-pg/src/list.rs",
            ],
        ],
    ),
    (
        "core.vlan-occupancy",
        &[VLAN, ROUTER, &["crates/kartenn-rest/src/vlan.rs"]],
    ),
    (
        "core.vlan-create",
        &[VLAN, &["crates/kartenn-service/src/vlan.rs"]],
    ),
    ("core.machines-read", &[MACHINES_READ, ROUTER]),
    ("core.machines-owner-page", &[MACHINES_READ, ROUTER, PAGE]),
    ("core.machines-site-page", &[MACHINES_READ, ROUTER, PAGE]),
    ("core.machines-network", &[MACHINES_READ, ROUTER, PAGE]),
    (
        "core.machines-create",
        &[
            MACHINES_WRITE,
            &[
                "crates/kartenn-machines/src/rules.rs",
                "crates/kartenn-machines/src/name.rs",
            ],
        ],
    ),
    (
        "core.machines-suggestions",
        &[
            MACHINES_READ,
            ROUTER,
            &[
                "crates/kartenn-rest/src/objects.rs",
                "crates/kartenn-rest/src/vlan.rs",
                "crates/kartenn-store-pg/src/list.rs",
            ],
        ],
    ),
    (
        "core.machines-import",
        &[
            MACHINES_WRITE,
            IMPORT,
            &["crates/kartenn-machines/src/mac.rs"],
        ],
    ),
    (
        "core.stats",
        &[
            ROUTER,
            &[
                "crates/kartenn-bench/src/stats_budgets.rs",
                "crates/kartenn-rest/src/stats.rs",
                "crates/kartenn-machines/src/stats.rs",
                "crates/kartenn-store-pg/src/anomalies.rs",
                "crates/kartenn-store-pg/src/changesets.rs",
                "crates/kartenn-store-pg/src/outbox.rs",
                "crates/kartenn-store-pg/src/modules.rs",
                "crates/kartenn-service/src/review.rs",
            ],
        ],
    ),
    (
        "rbac.read-object",
        &[
            RBAC,
            &[
                "crates/kartenn-store-pg/src/read.rs",
                "crates/kartenn-service/src/authz.rs",
            ],
        ],
    ),
    (
        "rbac.read-past",
        &[
            RBAC,
            &[
                "crates/kartenn-store-pg/src/read.rs",
                "crates/kartenn-store-pg/src/temporal.rs",
                "crates/kartenn-service/src/authz.rs",
            ],
        ],
    ),
    (
        "rbac.filtered-page",
        &[RBAC, &["crates/kartenn-store-pg/src/filter.rs"]],
    ),
    (
        "rbac.subtree",
        &[RBAC, &["crates/kartenn-store-pg/src/tree.rs"]],
    ),
    (
        "rbac.unitary-changeset",
        &[
            RBAC,
            &[
                "crates/kartenn-service/src/authz.rs",
                "crates/kartenn-service/src/rbac.rs",
            ],
        ],
    ),
    (
        "rbac.resolution",
        &[RBAC, &["crates/kartenn-service/src/authz.rs"]],
    ),
    ("http.read-object", &[HTTP_READ, ROUTER, SERVED]),
    (
        "http.read-past",
        &[
            HTTP_READ,
            ROUTER,
            SERVED,
            &["crates/kartenn-store-pg/src/temporal.rs"],
        ],
    ),
    ("http.filtered-page", &[HTTP_PAGE, PAGE, ROUTER, SERVED]),
    ("http.subtree", &[HTTP_SUBTREE, ROUTER, SERVED]),
    ("http.unitary-changeset", &[HTTP_WRITE, ROUTER, SERVED]),
    (
        "http.labels",
        &[
            ROUTER,
            SERVED,
            &[
                "crates/kartenn-bench/src/http.rs",
                "crates/kartenn-rest/src/labels.rs",
                "crates/kartenn-store-pg/src/labels_read.rs",
                "crates/kartenn-core/src/labels.rs",
                "crates/kartenn-core/src/code_labels.rs",
            ],
        ],
    ),
    // The reads and the writes above, together.
    (
        "http.load",
        &[
            HTTP_READ,
            HTTP_PAGE,
            PAGE,
            HTTP_SUBTREE,
            HTTP_WRITE,
            ROUTER,
            SERVED,
        ],
    ),
    (
        "policy.snapshot-cold",
        &[&[
            "crates/kartenn-bench/src/policy_budgets.rs",
            "crates/kartenn-policy/src/cache.rs",
            "crates/kartenn-policy/src/read.rs",
            "crates/kartenn-policy/src/load.rs",
            "crates/kartenn-policy/src/snapshot.rs",
        ]],
    ),
    ("policy.verdict-pair", &[VERDICT, ROUTER]),
    ("policy.interface-pair", &[VERDICT, ROUTER]),
    ("policy.raw-port", &[VERDICT, ROUTER]),
    (
        "policy.rules-differential",
        &[&[
            "crates/kartenn-bench/src/policy_budgets.rs",
            "crates/kartenn-policy/src/handler.rs",
            "crates/kartenn-policy/src/checks.rs",
            "crates/kartenn-policy/src/coherence.rs",
            "crates/kartenn-policy/src/diff.rs",
            "crates/kartenn-policy/src/contribution.rs",
            "crates/kartenn-policy/src/rules.rs",
            "crates/kartenn-policy/src/evaluate.rs",
        ]],
    ),
    (
        "policy.audit-report",
        &[&[
            "crates/kartenn-bench/src/policy_budgets.rs",
            "crates/kartenn-policy/src/report.rs",
        ]],
    ),
    (
        "policy.inference-suggestion",
        &[
            INFERENCE,
            ROUTER,
            &["crates/kartenn-service/src/expected.rs"],
        ],
    ),
    (
        "policy.inference-control",
        &[
            INFERENCE,
            &[
                "crates/kartenn-policy/src/mismatch.rs",
                "crates/kartenn-service/src/expected.rs",
            ],
        ],
    ),
    (
        "policy.inference-preview",
        &[
            INFERENCE,
            &[
                "crates/kartenn-policy/src/preview.rs",
                "crates/kartenn-policy/src/handler.rs",
            ],
        ],
    ),
    (
        "policy.inference-check",
        &[INFERENCE, &["crates/kartenn-policy/src/mismatch.rs"]],
    ),
    (
        "flow.request-interfaces",
        &[
            EVALUATION,
            &[
                "crates/kartenn-flow/src/handler.rs",
                "crates/kartenn-flow/src/state.rs",
                "crates/kartenn-flow/src/approval.rs",
                "crates/kartenn-machines/src/read.rs",
            ],
        ],
    ),
    (
        "flow.reclassify",
        &[
            EVALUATION,
            &[
                "crates/kartenn-flow/src/reevaluation.rs",
                "crates/kartenn-flow/src/handler.rs",
            ],
        ],
    ),
    (
        "flow.rule-written",
        &[
            EVALUATION,
            REEVALUATION,
            &["crates/kartenn-policy/src/diff.rs"],
        ],
    ),
    ("flow.policy-written", &[EVALUATION, REEVALUATION]),
    ("flow.flows-page", &[FLOWS_PAGE, ROUTER]),
    ("flow.nonconformities-page", &[FLOWS_PAGE, ROUTER]),
    ("flow.endpoints-flows-page", &[FLOWS_PAGE, ROUTER]),
    ("flow.endpoints-nonconformities-page", &[FLOWS_PAGE, ROUTER]),
    ("events.reader-overhead", &[EVENTS]),
    ("events.latency", &[EVENTS]),
    ("events.memory", &[EVENTS]),
    ("events.storm", &[EVENTS]),
    (
        "events.visibility",
        &[&[
            "crates/kartenn-bench/src/events_budgets.rs",
            "crates/kartenn-store-pg/src/events.rs",
            "crates/kartenn-store-pg/src/rbac.rs",
            "crates/kartenn-rest/src/events/hub.rs",
        ]],
    ),
    (
        "telemetry.read-object",
        &[
            TELEMETRY,
            READ_AUDIT,
            &["crates/kartenn-store-pg/src/read.rs"],
        ],
    ),
    (
        "telemetry.filtered-page",
        &[
            TELEMETRY,
            READ_AUDIT,
            &["crates/kartenn-store-pg/src/filter.rs"],
        ],
    ),
    (
        "telemetry.subtree",
        &[TELEMETRY, &["crates/kartenn-store-pg/src/tree.rs"]],
    ),
    (
        "telemetry.unitary-changeset",
        &[TELEMETRY, &["crates/kartenn-service/src/audit.rs"]],
    ),
    (
        "telemetry.sustained",
        &[
            TELEMETRY,
            &[
                "crates/kartenn-store-pg/src/audit_relay.rs",
                "crates/kartenn-store-pg/src/audit_seal.rs",
                "crates/kartenn-store-pg/src/audit_journal.rs",
            ],
        ],
    ),
    (
        "telemetry.audit-volume",
        &[
            READ_AUDIT,
            &[
                "crates/kartenn-bench/src/telemetry.rs",
                "crates/kartenn-store-pg/src/audit_seal.rs",
                "crates/kartenn-store-pg/src/audit.rs",
                "crates/kartenn-core/src/audit/",
            ],
        ],
    ),
];

/// The paths of `measure`, in the order of the table, without repetition; none for a
/// name the table does not have.
pub fn paths(measure: &str) -> Vec<&'static str> {
    let mut out: Vec<&'static str> = Vec::new();
    for (_, groups) in PATHS.iter().filter(|(m, _)| *m == measure) {
        for path in groups.iter().flat_map(|g| g.iter().copied()) {
            if !out.contains(&path) {
                out.push(path);
            }
        }
    }
    out
}

/// Whether the entry of the table `listed` (a file, or a directory ending with `/`)
/// covers the path `path`.
fn covers(listed: &str, path: &str) -> bool {
    if listed.ends_with('/') {
        path.starts_with(listed)
    } else {
        path == listed
    }
}

/// The crate of a path under `crates/`, and the rest of the path in it.
fn crate_of(path: &str) -> Option<(&str, &str)> {
    path.strip_prefix("crates/")?.split_once('/')
}

/// Whether `rest`, a path in a crate, is what its library is built from: what the
/// fingerprint of the templates covers (`build.rs` of this crate), and its build script.
fn is_code(rest: &str) -> bool {
    rest == "Cargo.toml"
        || rest == "build.rs"
        || rest.starts_with("src/")
        || rest.starts_with("migrations/")
}

/// The crates `bench` exercises, sorted: those of its preparation and of the paths of its
/// measures, with their workspace dependencies, and `kartenn-bench`.
///
/// # Panics
///
/// On a bench that has no preparation listed in [`template`], or a path of the table in
/// a crate `kartenn-bench` does not depend on (tested).
pub fn exercised(bench: &Bench) -> Vec<&'static str> {
    let mut direct: Vec<&'static str> = template::crates(bench.name).to_vec();
    for measure in bench.measures {
        for path in paths(measure.name) {
            if let Some((name, _)) = crate_of(path)
                && name != BENCH_CRATE
            {
                direct.push(name);
            }
        }
    }
    let mut out: Vec<&'static str> = template::crate_hashes(&direct)
        .into_iter()
        .map(|(name, _)| name)
        .collect();
    out.push(BENCH_CRATE);
    out.sort_unstable();
    out
}

/// Why a path calls for what it does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reason {
    /// Listed for these measures.
    Measures(Vec<&'static str>),
    /// A source of the preparation of these benches, and listed for these measures of
    /// others.
    Preparation(Vec<&'static str>, Vec<&'static str>),
    /// Code of a crate that no measure lists, exercised by these benches.
    Uncovered(String, Vec<&'static str>),
    /// A file of the workspace that concerns every bench.
    Workspace,
    /// A bench script outside `kartenn-bench`.
    Proposed(&'static str),
    /// Nothing a bench measures.
    Nothing,
}

/// What a change of `path` (relative to the root of the repository) calls for.
pub fn touched(path: &str) -> Reason {
    let path = path.trim().trim_start_matches("./");
    if WORKSPACE.contains(&path) {
        return Reason::Workspace;
    }
    if let Some((_, script)) = PROPOSED.iter().find(|(start, _)| path.starts_with(start)) {
        return Reason::Proposed(script);
    }
    let Some((name, rest)) = crate_of(path) else {
        return Reason::Nothing;
    };
    if !is_code(rest) || UNMEASURED.contains(&path) {
        return Reason::Nothing;
    }
    let measures: Vec<&'static str> = BENCHES
        .iter()
        .flat_map(|b| b.measures.iter())
        .map(|m| m.name)
        .filter(|m| paths(m).iter().any(|listed| covers(listed, path)))
        .collect();
    let prepared: Vec<&'static str> = match rest.strip_prefix("src/") {
        Some(file) if name == BENCH_CRATE => BENCHES
            .iter()
            .filter(|b| template::sources(b.name).iter().any(|(f, _)| *f == file))
            .map(|b| b.name)
            .collect(),
        _ => Vec::new(),
    };
    if !prepared.is_empty() {
        return Reason::Preparation(prepared, measures);
    }
    if !measures.is_empty() {
        return Reason::Measures(measures);
    }
    let benches: Vec<&'static str> = BENCHES
        .iter()
        .filter(|b| exercised(b).contains(&name))
        .map(|b| b.name)
        .collect();
    if benches.is_empty() {
        Reason::Nothing
    } else {
        Reason::Uncovered(name.to_owned(), benches)
    }
}

/// What a bench runs for a change.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Run {
    /// Every measure.
    Whole,
    /// These measures, in the order of the catalogue; their prerequisites come with them
    /// (`--only`).
    Only(Vec<&'static str>),
}

/// What a change calls for: the benches of `kartenn-bench` to run, in the order of the
/// catalogues, the scripts to propose, and one line per path that calls for something.
#[derive(Debug, Default)]
pub struct Affected {
    pub runs: Vec<(&'static str, Run)>,
    pub proposed: Vec<&'static str>,
    pub explained: String,
    /// The paths that call for nothing.
    pub unconcerned: usize,
}

/// What the change of `paths` calls for.
pub fn affected<'a>(paths: impl IntoIterator<Item = &'a str>) -> Affected {
    let mut whole: BTreeSet<&'static str> = BTreeSet::new();
    let mut measures: BTreeSet<&'static str> = BTreeSet::new();
    let mut out = Affected::default();
    let mut seen = BTreeSet::new();
    for path in paths {
        let path = path.trim().trim_start_matches("./");
        if path.is_empty() || !seen.insert(path.to_owned()) {
            continue;
        }
        let line = match touched(path) {
            Reason::Measures(m) => {
                let line = format!("mesures : {}", m.join(", "));
                measures.extend(m);
                line
            }
            Reason::Preparation(b, m) => {
                let mut line = format!("bancs entiers : {} (préparation)", b.join(", "));
                if !m.is_empty() {
                    let _ = write!(line, " ; mesures : {}", m.join(", "));
                }
                whole.extend(b);
                measures.extend(m);
                line
            }
            Reason::Uncovered(name, b) => {
                let line = format!(
                    "bancs entiers : {} (code de {name} qu'aucune mesure ne liste)",
                    b.join(", ")
                );
                whole.extend(b);
                line
            }
            Reason::Workspace => {
                whole.extend(BENCHES.iter().map(|b| b.name));
                "bancs entiers : tous (dépendances ou profils du workspace)".to_owned()
            }
            Reason::Proposed(script) => {
                if !out.proposed.contains(&script) {
                    out.proposed.push(script);
                }
                format!("proposé : {script} (hors kartenn-bench, non lancé)")
            }
            Reason::Nothing => {
                out.unconcerned += 1;
                continue;
            }
        };
        let _ = writeln!(out.explained, "{path} → {line}");
    }
    for bench in BENCHES {
        if whole.contains(bench.name) {
            out.runs.push((bench.name, Run::Whole));
            continue;
        }
        let only: Vec<&'static str> = bench
            .measures
            .iter()
            .map(|m| m.name)
            .filter(|m| measures.contains(m))
            .collect();
        if !only.is_empty() {
            out.runs.push((bench.name, Run::Only(only)));
        }
    }
    out
}

/// The script of the bench `name` (`bench-properties` for `props`).
pub fn script(name: &str) -> String {
    match name {
        "props" => "bench-properties".to_owned(),
        other => format!("bench-{other}"),
    }
}

impl Affected {
    /// One line per bench to run, for the script `bench-affected`: its script, then the
    /// measures for `--only` when not all of them run.
    pub fn plan(&self) -> String {
        let mut out = String::new();
        for (bench, run) in &self.runs {
            match run {
                Run::Whole => {
                    let _ = writeln!(out, "{}", script(bench));
                }
                Run::Only(m) => {
                    let _ = writeln!(out, "{} {}", script(bench), m.join(","));
                }
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;
    use crate::template::named_crates;

    fn root() -> &'static Path {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .expect("the root of the repository")
    }

    #[test]
    fn every_measure_has_its_paths_and_its_file() {
        let names: Vec<&str> = BENCHES
            .iter()
            .flat_map(|b| b.measures.iter())
            .map(|m| m.name)
            .collect();
        for (measure, _) in PATHS {
            assert!(
                names.contains(measure),
                "{measure}: no measure of the catalogues"
            );
            assert_eq!(
                PATHS.iter().filter(|(m, _)| m == measure).count(),
                1,
                "{measure}: twice in the table"
            );
        }
        for name in names {
            let paths = paths(name);
            assert!(
                paths
                    .iter()
                    .any(|p| p.starts_with("crates/kartenn-bench/src/")),
                "{name}: no file of kartenn-bench times it"
            );
            // A measure of SQL alone: its code is in the migrations, which are not listed.
            assert!(
                name == "core.history-index"
                    || paths
                        .iter()
                        .any(|p| !p.starts_with("crates/kartenn-bench/")),
                "{name}: no path of the code it measures"
            );
        }
    }

    #[test]
    fn every_path_is_code_of_the_repository() {
        for (measure, _) in PATHS {
            for path in paths(measure) {
                let (_, rest) =
                    crate_of(path).unwrap_or_else(|| panic!("{measure}: {path} is in no crate"));
                assert!(is_code(rest), "{measure}: {path} is no code");
                assert!(
                    !rest.starts_with("migrations/"),
                    "{measure}: {path}: a migration changes every template, it is not listed"
                );
                assert!(
                    !UNMEASURED.contains(&path),
                    "{measure}: {path} measures nothing"
                );
                let on_disk = root().join(path);
                if path.ends_with('/') {
                    assert!(on_disk.is_dir(), "{measure}: no directory {path}");
                } else {
                    assert!(on_disk.is_file(), "{measure}: no file {path}");
                }
            }
        }
    }

    /// The files of `kartenn-bench` that time the measures of `bench`, and their text.
    fn bench_files(bench: &Bench) -> Vec<(&'static str, String)> {
        let mut files: Vec<&'static str> = bench
            .measures
            .iter()
            .flat_map(|m| paths(m.name))
            .filter(|p| p.starts_with("crates/kartenn-bench/"))
            .collect();
        files.sort_unstable();
        files.dedup();
        files
            .into_iter()
            .map(|f| (f, std::fs::read_to_string(root().join(f)).unwrap()))
            .collect()
    }

    /// The paths and the crates of a bench agree with the crates its code names: every
    /// crate the files timing its measures name is exercised, and every path of its
    /// measures is in a crate its preparation or those files go through.
    #[test]
    fn the_paths_of_a_bench_agree_with_the_crates_its_code_names() {
        for bench in BENCHES {
            let exercised = exercised(bench);
            let mut named: Vec<String> = template::crates(bench.name)
                .iter()
                .map(|c| (*c).to_owned())
                .collect();
            for (file, text) in bench_files(bench) {
                for name in named_crates(&text) {
                    assert!(
                        exercised.contains(&name.as_str()),
                        "{}: {file} names {name}, which the bench does not exercise",
                        bench.name
                    );
                    named.push(name);
                }
            }
            let named: Vec<&str> = named.iter().map(String::as_str).collect();
            let reached: Vec<&str> = template::crate_hashes(&named)
                .into_iter()
                .map(|(name, _)| name)
                .collect();
            for measure in bench.measures {
                for path in paths(measure.name) {
                    let (name, _) = crate_of(path).unwrap();
                    assert!(
                        name == BENCH_CRATE || reached.contains(&name),
                        "{}: {path} is in {name}, which the code of {} does not reach",
                        measure.name,
                        bench.name
                    );
                }
            }
        }
    }

    /// Every crate a bench exercises has paths in the table, but the crate that only
    /// unifies the features of the workspace, which has no code of its own.
    #[test]
    fn every_crate_a_bench_exercises_is_covered() {
        let listed: BTreeSet<&str> = PATHS
            .iter()
            .flat_map(|(m, _)| paths(m))
            .filter_map(|p| crate_of(p).map(|(name, _)| name))
            .collect();
        for bench in BENCHES {
            for name in exercised(bench) {
                assert!(
                    name == "kartenn-workspace-hack" || listed.contains(name),
                    "{}: exercises {name}, which no measure lists",
                    bench.name
                );
            }
        }
        let core = exercised(crate::measures::bench("core").unwrap());
        assert!(
            core.contains(&"kartenn-machines"),
            "by its measures: {core:?}"
        );
        assert!(core.contains(&"kartenn-bench"), "{core:?}");
    }

    #[test]
    fn a_file_of_kartenn_flow_runs_its_measures() {
        let a = affected(["crates/kartenn-flow/src/read.rs"]);
        assert_eq!(
            a.runs,
            [(
                "flow",
                Run::Only(vec![
                    "flow.flows-page",
                    "flow.nonconformities-page",
                    "flow.endpoints-flows-page",
                    "flow.endpoints-nonconformities-page",
                ])
            )]
        );
        assert_eq!(
            a.plan(),
            "bench-flow flow.flows-page,flow.nonconformities-page,\
             flow.endpoints-flows-page,flow.endpoints-nonconformities-page\n"
        );
        assert!(a.proposed.is_empty());
    }

    #[test]
    fn an_uncovered_file_runs_the_benches_of_its_crate() {
        // The model of the flows: no measure lists it. The benches whose crates reach
        // kartenn-flow: those of the modules, and those that go through kartenn-server,
        // which builds every module in.
        let flow = [
            ("http", Run::Whole),
            ("policy", Run::Whole),
            ("flow", Run::Whole),
            ("telemetry", Run::Whole),
        ];
        let a = affected(["crates/kartenn-flow/src/model.rs"]);
        assert_eq!(a.runs, flow, "{}", a.explained);
        assert_eq!(
            a.explained,
            "crates/kartenn-flow/src/model.rs → bancs entiers : http, policy, flow, telemetry \
             (code de kartenn-flow qu'aucune mesure ne liste)\n"
        );
        // A migration of the core: every bench goes through the store.
        let a = affected(["crates/kartenn-store-pg/migrations/0999_probe.sql"]);
        let every: Vec<(&str, Run)> = BENCHES.iter().map(|b| (b.name, Run::Whole)).collect();
        assert_eq!(a.runs, every);
        // A migration of a module: the benches that reach it.
        let a = affected(["crates/kartenn-flow/migrations/0999_probe.sql"]);
        assert_eq!(a.runs, flow);
    }

    #[test]
    fn a_whole_bench_takes_over_its_measures() {
        let a = affected([
            "crates/kartenn-flow/src/read.rs",
            "crates/kartenn-flow/src/model.rs",
            "crates/kartenn-store-pg/src/explain.rs",
        ]);
        assert_eq!(
            a.runs,
            [
                (
                    "props",
                    Run::Only(vec![
                        "props.explained-resolution",
                        "props.grouped-resolution"
                    ])
                ),
                ("http", Run::Whole),
                ("policy", Run::Whole),
                ("flow", Run::Whole),
                ("telemetry", Run::Whole),
            ]
        );
        assert_eq!(
            a.plan(),
            "bench-properties props.explained-resolution,props.grouped-resolution\n\
             bench-http\nbench-policy\nbench-flow\nbench-telemetry\n"
        );
    }

    #[test]
    fn the_files_of_kartenn_bench() {
        // A source of a preparation: the whole bench; and the measures it times elsewhere.
        assert_eq!(
            touched("crates/kartenn-bench/src/vlan_budgets.rs"),
            Reason::Preparation(
                vec!["policy", "flow"],
                vec![
                    "core.vlan-read",
                    "core.vlan-filtered-page",
                    "core.vlan-occupancy",
                    "core.vlan-create",
                    "core.machines-read",
                    "core.machines-owner-page",
                    "core.machines-site-page",
                    "core.machines-network",
                    "core.machines-suggestions",
                    "policy.verdict-pair",
                    "policy.interface-pair",
                    "policy.raw-port",
                    "flow.flows-page",
                    "flow.nonconformities-page",
                    "flow.endpoints-flows-page",
                    "flow.endpoints-nonconformities-page",
                ]
            )
        );
        // A file that times measures and prepares nothing.
        assert_eq!(
            touched("crates/kartenn-bench/src/stats_budgets.rs"),
            Reason::Measures(vec!["core.stats"])
        );
        // The rest of its code: every bench.
        let a = affected(["crates/kartenn-bench/src/main.rs"]);
        assert_eq!(a.runs.len(), BENCHES.len());
        assert!(a.runs.iter().all(|(_, r)| *r == Run::Whole));
        // Its tests, the selection, the history and this table: nothing.
        for path in [
            "crates/kartenn-bench/tests/it/only.rs",
            "crates/kartenn-bench/src/measures.rs",
            "crates/kartenn-bench/src/history.rs",
            "crates/kartenn-bench/src/affected.rs",
        ] {
            assert_eq!(touched(path), Reason::Nothing, "{path}");
        }
    }

    #[test]
    fn web_and_documentation_run_nothing() {
        let a = affected([
            "web/v2/ui/src/components/Button.vue",
            "web/shell/src/main.ts",
            "clients/python/bench/cli.py",
            "docs/adr/0041-bancs-cibles.md",
            "specs/028-bancs-cibles/tasks.md",
            "crates/kartenn-store-pg/tests/it/main.rs",
            "crates/kartenn-dev-idp/src/main.rs",
            "devenv.nix",
        ]);
        assert!(a.runs.is_empty(), "{:?}", a.runs);
        assert_eq!(a.proposed, ["bench-ui", "bench-web", "bench-cli"]);
        assert_eq!(a.unconcerned, 5);
        assert_eq!(a.plan(), "");
    }

    #[test]
    fn the_lock_runs_every_bench_and_paths_are_read_once() {
        let a = affected(["Cargo.lock", "./Cargo.lock", "", "Cargo.toml"]);
        assert_eq!(a.runs.len(), BENCHES.len());
        assert!(a.runs.iter().all(|(_, r)| *r == Run::Whole));
        assert_eq!(a.explained.lines().count(), 2, "{}", a.explained);
    }

    #[test]
    fn scripts_are_those_of_devenv() {
        let devenv = std::fs::read_to_string(root().join("devenv.nix")).unwrap();
        for bench in BENCHES {
            let script = script(bench.name);
            assert!(
                devenv.contains(&format!("scripts.{script} =\n    bench \"{}\"", bench.name)),
                "{script}: no script of devenv.nix runs {}",
                bench.name
            );
        }
        for (_, script) in PROPOSED {
            assert!(
                devenv.contains(&format!("scripts.{script} =")),
                "{script}: no such script"
            );
        }
    }
}
