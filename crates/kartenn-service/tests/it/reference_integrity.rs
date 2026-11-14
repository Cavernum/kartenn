//! Every reference of a write names an object in force (REQ-002-12, amendment of
//! 2026-11-14; task 002-29a): a reference by identifier is read before it is written, with
//! its kind, at the start of the period of its operation.

use std::collections::BTreeSet;
use std::time::{SystemTime, UNIX_EPOCH};

use kartenn_core::objects::{AddressUsage, Nature, RangeUsage};
use kartenn_core::values::Cardinality;
use kartenn_service::authz::Unrestricted;
use kartenn_service::operations::{
    BindingScope, DeclarationInput, Named, ObligationInput, PeriodSpec, RawInput, TypeInput,
};
use kartenn_service::pipeline::MAX_OPERATIONS;
use kartenn_service::{ChangesetRequest, ObjectKind, Operation, Outcome, Ref, submit};
use kartenn_store_pg::write::PgStore;
use sqlx::PgPool;
use sqlx::types::Uuid;

const UNKNOWN: &str = "core.reference.unknown";
const DAY: i64 = 86_400_000_000;
/// 2020-01-01T00:00:00Z, in microseconds.
const T0: i64 = 1_577_836_800_000_000;

struct Fixture {
    pool: PgPool,
    store: PgStore,
    author: Uuid,
}

impl Fixture {
    async fn new(pool: &PgPool) -> Self {
        let author = sqlx::query_scalar(
            "INSERT INTO person (id, display_name) VALUES (gen_random_uuid(), 'tester') RETURNING id",
        )
        .fetch_one(pool)
        .await
        .unwrap();
        Self {
            pool: pool.clone(),
            store: PgStore::new(pool.clone()),
            author,
        }
    }

    async fn submit(&self, operations: Vec<Operation>, start: Option<i64>) -> Outcome {
        let request = ChangesetRequest {
            author: self.author,
            motive: "test".into(),
            period: start.map(|start| PeriodSpec { start, end: None }),
            operations,
            simulate: false,
            idempotency_key: None,
            approval_self: true,
        };
        submit(&self.store, &Unrestricted, &request, MAX_OPERATIONS)
            .await
            .unwrap()
    }

    async fn applied(&self, operations: Vec<Operation>, start: Option<i64>) {
        let outcome = self.submit(operations, start).await;
        assert!(matches!(outcome, Outcome::Applied { .. }), "{outcome:?}");
    }

    /// The rules of a refusal, each violation checked to name its operation and field.
    async fn refused(&self, operations: Vec<Operation>, start: Option<i64>) -> Vec<&'static str> {
        match self.submit(operations, start).await {
            Outcome::Refused { violations, .. } => {
                for v in violations.iter().filter(|v| v.rule.id == UNKNOWN) {
                    assert!(v.operation.is_some(), "{v:?}");
                    let field = v.detail.as_ref().and_then(|d| d.get("field"));
                    assert!(field.is_some_and(serde_json::Value::is_string), "{v:?}");
                }
                violations.iter().map(|v| v.rule.id).collect()
            }
            other => panic!("expected a refusal, got {other:?}"),
        }
    }

    async fn id(&self, table: &str, slug: &str) -> Uuid {
        sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
            "SELECT id FROM {table} WHERE slug = $1"
        )))
        .bind(slug)
        .fetch_one(&self.pool)
        .await
        .unwrap()
    }

    async fn rows(&self) -> i64 {
        sqlx::query_scalar(
            "SELECT (SELECT count(*) FROM prefix) + (SELECT count(*) FROM ip_range)
                  + (SELECT count(*) FROM address)",
        )
        .fetch_one(&self.pool)
        .await
        .unwrap()
    }
}

fn domain(local: u32, slug: &str) -> Operation {
    Operation::CreateDomain {
        local: Some(local),
        slug: slug.into(),
    }
}

fn prefix(domain: Ref, value: &str) -> Operation {
    Operation::CreatePrefix {
        local: None,
        domain,
        prefix: value.into(),
        nature: Nature::Network,
        point_to_point: false,
        anycast: false,
    }
}

fn range(domain: Ref) -> Operation {
    Operation::CreateRange {
        local: None,
        domain,
        first: "192.0.2.10".into(),
        last: "192.0.2.20".into(),
        usage: RangeUsage::Dhcp,
    }
}

fn address(domain: Ref) -> Operation {
    Operation::CreateAddress {
        local: None,
        domain,
        address: "192.0.2.1".into(),
        usage: AddressUsage::Host,
    }
}

fn creations(domain: Ref) -> [Operation; 3] {
    [
        prefix(domain, "192.0.2.0/24"),
        range(domain),
        address(domain),
    ]
}

#[kartenn_testing::pg_test]
async fn a_creation_on_a_domain_that_is_not_one_is_refused(pool: PgPool) {
    let f = Fixture::new(&pool).await;
    // An identifier nothing bears.
    for op in creations(Ref::Id(Uuid::now_v7())) {
        assert_eq!(f.refused(vec![op], None).await, [UNKNOWN]);
    }
    // The identifier of an object of another kind.
    f.applied(
        vec![Operation::CreateEntity {
            local: None,
            slug: "team".into(),
            parent: None,
        }],
        None,
    )
    .await;
    let team = f.id("entity", "team").await;
    for op in creations(Ref::Id(team)) {
        assert_eq!(f.refused(vec![op], None).await, [UNKNOWN]);
    }
    // A domain closed.
    f.applied(vec![domain(1, "gone")], None).await;
    let gone = f.id("addressing_domain", "gone").await;
    f.applied(
        vec![Operation::Close {
            kind: ObjectKind::Domain,
            id: Ref::Id(gone),
            expected: None,
            cascade: false,
        }],
        None,
    )
    .await;
    for op in creations(Ref::Id(gone)) {
        assert_eq!(f.refused(vec![op], None).await, [UNKNOWN]);
    }
    assert_eq!(f.rows().await, 0, "nothing is written");
}

#[kartenn_testing::pg_test]
async fn legitimate_references_are_accepted(pool: PgPool) {
    let f = Fixture::new(&pool).await;
    // An object created earlier in the same changeset.
    f.applied(
        vec![domain(1, "lab"), prefix(Ref::Local(1), "192.0.2.0/24")],
        None,
    )
    .await;
    let lab = f.id("addressing_domain", "lab").await;
    f.applied(vec![prefix(Ref::Id(lab), "198.51.100.0/24")], None)
        .await;
    // Dated in the past: the domain must be in force at the start of the write.
    f.applied(vec![domain(1, "old")], Some(T0)).await;
    let old = f.id("addressing_domain", "old").await;
    assert_eq!(
        f.refused(vec![prefix(Ref::Id(old), "192.0.2.0/24")], Some(T0 - DAY))
            .await,
        [UNKNOWN]
    );
    f.applied(vec![prefix(Ref::Id(old), "192.0.2.0/24")], Some(T0 + DAY))
        .await;
    // Dated in the future: a domain that starts later cannot hold a prefix now.
    let now = i64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_micros(),
    )
    .unwrap();
    let later = now + 30 * DAY;
    f.applied(vec![domain(1, "next")], Some(later)).await;
    let next = f.id("addressing_domain", "next").await;
    assert_eq!(
        f.refused(vec![prefix(Ref::Id(next), "192.0.2.0/24")], None)
            .await,
        [UNKNOWN]
    );
    f.applied(
        vec![prefix(Ref::Id(next), "192.0.2.0/24")],
        Some(later + DAY),
    )
    .await;
}

/// The tags of every core operation, as the decoder lists them for a tag it does not know.
fn every_tag() -> BTreeSet<String> {
    let error = serde_json::from_value::<Operation>(serde_json::json!({"op": "no_such_op"}))
        .unwrap_err()
        .to_string();
    let (_, listed) = error
        .split_once("expected one of ")
        .expect("serde lists the variants");
    listed
        .split(", ")
        .map(|tag| tag.trim_matches(|c: char| c == '`' || c.is_whitespace()))
        .filter(|tag| !tag.is_empty())
        .map(str::to_owned)
        .collect()
}

fn tag(op: &Operation) -> String {
    serde_json::to_value(op).unwrap()["op"]
        .as_str()
        .unwrap()
        .to_owned()
}

/// The identifiers of the references (`Ref::Id`, `{"id": <uuid>}`) a JSON value holds.
fn ref_ids(value: &serde_json::Value, out: &mut BTreeSet<Uuid>) {
    match value {
        serde_json::Value::Object(map) => {
            if map.len() == 1
                && let Some(id) = map
                    .get("id")
                    .and_then(serde_json::Value::as_str)
                    .and_then(|s| Uuid::parse_str(s).ok())
            {
                out.insert(id);
            }
            map.values().for_each(|v| ref_ids(v, out));
        }
        serde_json::Value::Array(items) => items.iter().for_each(|v| ref_ids(v, out)),
        _ => {}
    }
}

/// The operations of the core that designate no object of the core by identifier, or only
/// the identity, the membership, the binding or the account of the RBAC, which its own rules
/// read.
const NO_REFERENCE: &[&str] = &[
    "create_domain",
    "set_role",
    "close_role",
    "set_group",
    "close_group",
    "add_member",
    "remove_member",
    "unbind",
    "set_identity_state",
    "create_service_account",
    "add_service_secret",
    "revoke_service_secret",
    "declare_property",
    // Designates an object that may be closed: the operation of an inverse changeset gives
    // back what was asserted before, an object closed since included.
    "restore",
];

/// One operation per tag with references, every reference an identifier nothing bears,
/// with the bare identifiers of objects of the core it designates too.
#[allow(clippy::too_many_lines)] // one sample per operation
fn samples() -> Vec<(Operation, Vec<Uuid>)> {
    let id = || Ref::Id(Uuid::now_v7());
    let declaration = DeclarationInput {
        key: "custom.flag".into(),
        value_type: TypeInput::Bool,
        cardinality: Cardinality::Single,
        carriers: vec!["prefix".into()],
        inherit: None,
        module: None,
        obligations: Vec::new(),
        expected: None,
        labels: None,
    };
    let obligation_domain = Uuid::now_v7();
    let policy_domain = Uuid::now_v7();
    vec![
        (
            Operation::Within {
                period: PeriodSpec {
                    start: T0,
                    end: None,
                },
                operation: Box::new(prefix(id(), "192.0.2.0/24")),
            },
            vec![],
        ),
        (prefix(id(), "192.0.2.0/24"), vec![]),
        (range(id()), vec![]),
        (address(id()), vec![]),
        (
            Operation::Bind {
                local: None,
                group: "staff".into(),
                role: "auditor".into(),
                scope: BindingScope::Domain { id: id() },
            },
            vec![],
        ),
        (
            Operation::AttachIssuer {
                issuer: "https://idp.example".into(),
                entity: Some(id()),
            },
            vec![],
        ),
        (
            Operation::DetachIssuer {
                issuer: "https://idp.example".into(),
                entity: Some(id()),
            },
            vec![],
        ),
        (
            Operation::SetApprovalPolicy {
                name: "watched".into(),
                min_operations: None,
                min_diff_objects: None,
                property_keys: Vec::new(),
                domains: vec![policy_domain],
                scopes: vec![
                    BindingScope::Entity { id: id() },
                    BindingScope::Site { id: id() },
                    BindingScope::Domain { id: id() },
                    BindingScope::Prefix { id: id() },
                ],
                author_roles: Vec::new(),
                author_groups: Vec::new(),
                bound_roles: Vec::new(),
                module_triggers: Vec::new(),
                enabled: true,
            },
            vec![policy_domain],
        ),
        (
            Operation::BindExternal {
                object: id(),
                kind: ObjectKind::Prefix,
                source: "inventory".into(),
                external_id: "x-1".into(),
            },
            vec![],
        ),
        (
            Operation::CreateEntity {
                local: None,
                slug: "child".into(),
                parent: Some(id()),
            },
            vec![],
        ),
        (
            Operation::CreateSite {
                local: None,
                slug: "child".into(),
                parent: Some(id()),
            },
            vec![],
        ),
        (
            Operation::MoveEntity {
                id: id(),
                parent: Some(id()),
                expected: None,
            },
            vec![],
        ),
        (
            Operation::MoveSite {
                id: id(),
                parent: Some(id()),
                expected: None,
            },
            vec![],
        ),
        (
            Operation::SetSensitivity {
                property: id(),
                sensitive: true,
            },
            vec![],
        ),
        (
            Operation::AddObligation {
                property: id(),
                scope: ObligationInput::Domain {
                    id: obligation_domain,
                },
            },
            vec![obligation_domain],
        ),
        (
            Operation::RemoveObligation {
                property: id(),
                scope: ObligationInput::ObjectType {
                    kind: "prefix".into(),
                },
            },
            vec![],
        ),
        (
            Operation::Rename {
                kind: Named::Domain,
                id: id(),
                slug: "renamed".into(),
                expected: None,
            },
            vec![],
        ),
        (
            Operation::UpdateProperty {
                id: id(),
                declaration,
            },
            vec![],
        ),
        (
            Operation::AddEnumValue {
                local: None,
                property: id(),
                key: "blue".into(),
                labels: [("fr".to_owned(), "bleu".to_owned())].into(),
            },
            vec![],
        ),
        (
            Operation::UpdateEnumValue {
                property: id(),
                key: "blue".into(),
                labels: [("fr".to_owned(), "bleu".to_owned())].into(),
            },
            vec![],
        ),
        (
            Operation::AssignProperty {
                object: id(),
                kind: ObjectKind::Prefix,
                property: id(),
                values: vec![RawInput::Bool(true)],
            },
            vec![],
        ),
        (
            Operation::ClearProperty {
                object: id(),
                kind: ObjectKind::Prefix,
                property: id(),
            },
            vec![],
        ),
        (
            Operation::UnassignProperty {
                object: id(),
                kind: ObjectKind::Prefix,
                property: id(),
            },
            vec![],
        ),
        (
            Operation::Expect {
                kind: ObjectKind::Prefix,
                id: id(),
                expected: Uuid::now_v7(),
            },
            vec![],
        ),
        (
            Operation::Close {
                kind: ObjectKind::Prefix,
                id: id(),
                expected: None,
                cascade: false,
            },
            vec![],
        ),
        (
            Operation::UpdatePrefix {
                id: id(),
                domain: None,
                prefix: None,
                nature: Some(Nature::Container),
                point_to_point: None,
                anycast: None,
                expected: None,
            },
            vec![],
        ),
        (
            Operation::UpdateAddress {
                id: id(),
                domain: None,
                usage: AddressUsage::Reserved,
                expected: None,
            },
            vec![],
        ),
        (
            Operation::UpdateRange {
                id: id(),
                domain: None,
                usage: RangeUsage::Reserved,
                expected: None,
            },
            vec![],
        ),
    ]
}

#[kartenn_testing::pg_test]
async fn every_reference_of_every_core_operation_is_checked(pool: PgPool) {
    let f = Fixture::new(&pool).await;
    let samples = samples();
    // Each operation of the core has a sample, or is listed without references: a new
    // operation fails here until it is classified.
    let covered: BTreeSet<String> = samples
        .iter()
        .map(|(op, _)| tag(op))
        .chain(NO_REFERENCE.iter().map(|t| (*t).to_owned()))
        .collect();
    assert_eq!(
        covered,
        every_tag(),
        "an operation of the core has no sample"
    );
    for (op, bare) in samples {
        let mut expected: BTreeSet<Uuid> = bare.into_iter().collect();
        ref_ids(&serde_json::to_value(&op).unwrap(), &mut expected);
        assert!(!expected.is_empty(), "{op:?} has no reference");
        let outcome = f.submit(vec![op.clone()], None).await;
        let Outcome::Refused { violations, .. } = outcome else {
            panic!("{op:?}: expected a refusal, got {outcome:?}")
        };
        assert!(
            violations.iter().all(|v| v.rule.id == UNKNOWN),
            "{op:?}: {violations:?}"
        );
        let named: BTreeSet<Uuid> = violations
            .iter()
            .flat_map(|v| v.subjects.iter().copied())
            .collect();
        assert_eq!(named, expected, "{op:?}: every reference is checked");
    }
}
