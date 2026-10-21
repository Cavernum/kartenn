//! The offline converter (task 010-17; REQ-010-35, 53): the example of Q10, invented and
//! neutral (`tests/import/`), converted into one changeset; the differences of the
//! simulation listed; the changeset applied, and the policy stored answering as the
//! simulation said.

use std::collections::{BTreeMap, BTreeSet};

use kartenn_policy::cache::SnapshotCache;
use kartenn_policy::evaluate::{Endpoint, Requested, evaluate};
use kartenn_policy::import::convert::{Conversion, Inputs, Options, convert};
use kartenn_policy::import::simulate::{self, Answer, Expect};
use kartenn_policy::import::source;
use kartenn_policy::model::{AxisKey, AxisValue};
use kartenn_policy::snapshot::PolicySnapshot;
use serde_json::Value;
use sqlx::PgPool;
use sqlx::types::time::OffsetDateTime;

use crate::support::env::Env;

fn file(name: &str) -> String {
    std::fs::read_to_string(format!(
        "{}/tests/import/{name}",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap()
}

fn example() -> Inputs {
    Inputs {
        matrix: source::matrix(&file("matrix.csv")).unwrap(),
        ports: source::ports(&file("ports.csv")).unwrap(),
        services: source::services(&file("services.csv")).unwrap(),
        rules: source::rules(&[
            ("00-globals.toml".into(), file("rules/00-globals.toml")),
            ("10-sites.toml".into(), file("rules/10-sites.toml")),
        ])
        .unwrap(),
        sites: source::sites(&file("sites.csv")).unwrap(),
    }
}

fn count(conversion: &Conversion, op: &str) -> usize {
    conversion
        .operations
        .iter()
        .filter(|o| o["op"] == format!("policy.{op}"))
        .count()
}

/// REQ-010-35, 53: the example converted; what is left out said; the differences listed.
#[test]
fn the_example_converts_and_lists_its_differences() {
    let inputs = example();
    let conversion = convert(&inputs, &Options::default());
    assert_eq!(count(&conversion, "environment.create"), 4);
    assert_eq!(count(&conversion, "flow_type.create"), 4);
    assert_eq!(
        count(&conversion, "service.create"),
        5,
        "ping has no flow type"
    );
    assert_eq!(
        count(&conversion, "rule.create"),
        1,
        "telnet, allowed = false"
    );
    assert_eq!(
        count(&conversion, "pattern.create"),
        1,
        "for the partial cell"
    );
    assert_eq!(count(&conversion, "matrix.set"), 6);
    assert_eq!(
        count(&conversion, "inference_rule.create"),
        4,
        "the owner rule is left out"
    );
    let notices = conversion.notices.join("\n");
    assert!(notices.contains("\"ping\""), "{notices}");
    assert!(notices.contains("\"proprietaire\""), "{notices}");
    // Priorities follow the reading order; the exception is a child of its rule.
    let rules: Vec<&Value> = conversion
        .operations
        .iter()
        .filter(|o| o["op"] == "policy.inference_rule.create")
        .collect();
    assert_eq!(rules[0]["key"], "production-par-nom");
    assert_eq!(rules[1]["parent"]["local"], rules[0]["local"]);
    assert_eq!(rules[3]["key"], "site-sud-en-dev");
    assert!(rules[3]["priority"].as_i64() > rules[2]["priority"].as_i64());
    assert_eq!(rules[0]["criteria"][0]["name"][0], "^(?:prd).*$");

    // The matrix: only missing cells differ, denied by default.
    let matrix = simulate::matrix(&inputs, &conversion);
    assert!(!matrix.is_empty());
    assert!(
        matrix
            .iter()
            .all(|d| d.origin == Answer::Undefined && d.kartenn == Answer::Denied),
        "{matrix:#?}"
    );
    assert!(
        !matrix
            .iter()
            .any(|d| d.source == "prod" && d.destination == "global" && d.service == "sftp"),
        "the partial cell is to review on both sides"
    );
    // The inference: one node of a site absent from the sites of kartenn.
    let nodes = source::nodes(&file("nodes.csv")).unwrap();
    let axis = AxisKey::parse("policy.environment").unwrap();
    let differences = simulate::inference(&inputs, &nodes, &conversion, &axis).unwrap();
    assert_eq!(differences.len(), 1, "{differences:#?}");
    assert!(differences[0].path.ends_with("n50"));
    assert_eq!(
        differences[0].origin,
        Expect::Values(BTreeSet::from(["dev".to_owned()]))
    );
    assert_eq!(differences[0].kartenn, Expect::Nothing);
}

fn endpoint(environment: &str) -> Endpoint {
    Endpoint {
        values: BTreeMap::from([(
            AxisKey::parse("policy.environment").unwrap(),
            BTreeSet::from([AxisValue::new(environment)]),
        )]),
        width: 1,
        whole_domain: false,
    }
}

/// The changeset applies, and the stored policy answers as the simulation did.
#[kartenn_testing::pg_test]
async fn the_converted_changeset_applies_and_answers_as_simulated(pool: PgPool) {
    let env = Env::new(&pool).await;
    let conversion = convert(&example(), &Options::default());
    let operations = conversion
        .operations
        .iter()
        .map(|o| serde_json::from_value(o.clone()).unwrap())
        .collect();
    env.applied(operations).await;

    let mut conn = pool.acquire().await.unwrap();
    let stored = SnapshotCache::new()
        .snapshot(&mut conn, OffsetDateTime::now_utc(), None)
        .await
        .unwrap();
    let simulated = PolicySnapshot::new(conversion.parts.clone());
    let environments: BTreeSet<&String> = conversion.environments.values().collect();
    for service in &conversion.services {
        for a in &environments {
            for b in &environments {
                let answer = |snapshot: &PolicySnapshot| {
                    let e = evaluate(
                        snapshot,
                        &endpoint(a),
                        &endpoint(b),
                        &service.key,
                        Requested::Service,
                        None,
                    );
                    (e.verdict, e.outcome)
                };
                assert_eq!(
                    answer(&stored),
                    answer(&simulated),
                    "{a} -> {b} pour {}",
                    service.key
                );
            }
        }
    }
    let rules: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM policy.policy_inference_rule WHERE valid_period @> now()",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(rules, 4);
}
