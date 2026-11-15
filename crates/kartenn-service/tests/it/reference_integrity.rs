//! Every reference of a write names an object in force (REQ-002-12, amendment of
//! 2026-11-14; task 002-29a): a reference by identifier is read before it is written, with
//! its kind, at the start of the period of its operation.

use std::collections::BTreeSet;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::support::{dummy_module as dummy, kit_module as kit};
use kartenn_core::objects::{AddressUsage, Nature, RangeUsage};
use kartenn_core::values::Cardinality;
use kartenn_service::authz::Unrestricted;
use kartenn_service::declarations::{core_declarations_for, sync_modules};
use kartenn_service::module::ModuleRegistry;
use kartenn_service::operations::{
    BindingScope, DeclarationInput, Named, ObligationInput, PeriodSpec, RawInput, TypeInput,
};
use kartenn_service::pipeline::MAX_OPERATIONS;
use kartenn_service::{ChangesetRequest, ObjectKind, Operation, Outcome, Ref, submit};
use kartenn_store_pg::write::PgStore;
use serde_json::{Value, json};
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
        Self::on(pool, PgStore::new(pool.clone())).await
    }

    async fn on(pool: &PgPool, store: PgStore) -> Self {
        let author = sqlx::query_scalar(
            "INSERT INTO person (id, display_name) VALUES (gen_random_uuid(), 'tester') RETURNING id",
        )
        .fetch_one(pool)
        .await
        .unwrap();
        Self {
            pool: pool.clone(),
            store,
            author,
        }
    }

    /// A build with the modules of test, the VLANs of the core and their declarations, the
    /// entities `acme` and `beta` and the site `paris`.
    async fn with_modules(pool: &PgPool) -> Self {
        let modules = build();
        kartenn_store_pg::modules::migrate(pool, &modules)
            .await
            .unwrap();
        let store = PgStore::new(pool.clone()).with_modules(modules.clone());
        for outcome in sync_modules(
            &store,
            &Unrestricted,
            &[core_declarations_for(&modules)],
            MAX_OPERATIONS,
        )
        .await
        .unwrap()
        {
            assert!(matches!(outcome, Outcome::Applied { .. }), "{outcome:?}");
        }
        let f = Self::on(pool, store).await;
        let entity = |slug: &str| Operation::CreateEntity {
            local: None,
            slug: slug.into(),
            parent: None,
        };
        f.applied(
            vec![
                entity("acme"),
                entity("beta"),
                Operation::CreateSite {
                    local: None,
                    slug: "paris".into(),
                    parent: None,
                },
            ],
            None,
        )
        .await;
        f
    }

    fn request(&self, operations: Vec<Operation>, start: Option<i64>) -> ChangesetRequest {
        ChangesetRequest {
            author: self.author,
            motive: "test".into(),
            period: start.map(|start| PeriodSpec { start, end: None }),
            operations,
            simulate: false,
            idempotency_key: None,
            approval_self: true,
        }
    }

    async fn submit(&self, operations: Vec<Operation>, start: Option<i64>) -> Outcome {
        self.run(&self.request(operations, start)).await
    }

    async fn run(&self, request: &ChangesetRequest) -> Outcome {
        submit(&self.store, &Unrestricted, request, MAX_OPERATIONS)
            .await
            .unwrap()
    }

    /// The changeset that applies `operations`.
    async fn applied(&self, operations: Vec<Operation>, start: Option<i64>) -> Uuid {
        match self.submit(operations, start).await {
            Outcome::Applied { changeset, .. } => changeset,
            other => panic!("expected applied, got {other:?}"),
        }
    }

    /// Undoes `changeset` as an author does: its inverse, without conflict, applied as it
    /// comes, on its own period (REQ-004-34).
    async fn undo(&self, changeset: Uuid) {
        let undo = kartenn_service::undo::inverse(&self.store, changeset, self.author)
            .await
            .unwrap()
            .unwrap();
        assert!(
            undo.conflicts.is_empty() && undo.unsupported.is_empty(),
            "{undo:?}"
        );
        let outcome = self.run(&undo.request).await;
        assert!(
            matches!(outcome, Outcome::Applied { .. }),
            "{:?}: {outcome:?}",
            undo.request.operations
        );
    }

    /// How many rows of `table` are in force now.
    async fn living(&self, table: &str) -> i64 {
        sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
            "SELECT count(*) FROM {table} WHERE valid_period @> now()"
        )))
        .fetch_one(&self.pool)
        .await
        .unwrap()
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

/// The modules of the build of these tests (the VLANs of the core come with the store).
fn build() -> Arc<ModuleRegistry> {
    Arc::new(ModuleRegistry::new(vec![Arc::new(dummy::Dummy), Arc::new(kit::Kit)]).unwrap())
}

/// The operation `tag` (`<module>.<name>`, the name alone for the VLANs of the core) with
/// the fields `body`.
fn module(tag: &str, body: Value) -> Operation {
    let mut object = body;
    object["op"] = tag.into();
    serde_json::from_value(object).unwrap()
}

/// A reference by identifier, as a body of a module carries it.
fn by_id(id: Uuid) -> Value {
    json!({ "id": id })
}

/// The rules, operations and objects of a refusal.
fn refusal(outcome: &Outcome) -> (BTreeSet<&'static str>, BTreeSet<usize>, BTreeSet<Uuid>) {
    let Outcome::Refused { violations, .. } = outcome else {
        panic!("expected a refusal, got {outcome:?}")
    };
    (
        violations.iter().map(|v| v.rule.id).collect(),
        violations.iter().filter_map(|v| v.operation).collect(),
        violations
            .iter()
            .flat_map(|v| v.subjects.iter().copied())
            .collect(),
    )
}

/// REQ-002-12 (amendment of 2026-11-14), task 002-29b2: only a module plan that creates its
/// object puts it out of the check; the object a plan changes is designated, read as any
/// other, and so is it by an operation of the core of the same request.
#[kartenn_testing::pg_test]
async fn the_object_a_module_operation_changes_is_in_force(pool: PgPool) {
    let f = Fixture::with_modules(&pool).await;
    let update = |id: Uuid| dummy::update_thing(json!({"id": by_id(id), "label": "b"}));
    let close = |id: Uuid| Operation::Close {
        kind: dummy::thing_kind(),
        id: Ref::Id(id),
        expected: None,
        cascade: false,
    };
    let unknown = Uuid::now_v7();
    let outcome = f.submit(vec![update(unknown), close(unknown)], None).await;
    assert_eq!(
        refusal(&outcome),
        ([UNKNOWN].into(), [0, 1].into(), [unknown].into())
    );
    // The VLANs and the groups of the core, changed by a plan too.
    for op in [
        module("update_vlan", json!({"id": by_id(unknown), "name": "v"})),
        module(
            "update_vlan_group",
            json!({"id": by_id(unknown), "slug": "g"}),
        ),
    ] {
        assert_eq!(f.refused(vec![op], None).await, [UNKNOWN]);
    }
    // A thing in force is changed, and closed; closed, it is changed no more.
    f.applied(vec![dummy::create_thing(json!({"label": "a"}))], None)
        .await;
    let thing: Uuid = sqlx::query_scalar("SELECT id FROM dummy.thing WHERE label = 'a'")
        .fetch_one(&pool)
        .await
        .unwrap();
    f.applied(vec![update(thing)], None).await;
    f.applied(vec![close(thing)], None).await;
    assert_eq!(f.refused(vec![update(thing)], None).await, [UNKNOWN]);
    // Of another kind: an entity is no thing.
    let acme = f.id("entity", "acme").await;
    assert_eq!(f.refused(vec![update(acme)], None).await, [UNKNOWN]);
}

/// REQ-002-12 (amendment of 2026-11-14), spec 026: a group of VLANs is anchored on an
/// entity, a site or a domain in force, owned by an entity in force; a VLAN is written in a
/// group in force.
#[kartenn_testing::pg_test]
async fn a_vlan_group_and_a_vlan_designate_objects_in_force(pool: PgPool) {
    let f = Fixture::with_modules(&pool).await;
    let (acme, paris) = (f.id("entity", "acme").await, f.id("site", "paris").await);
    let unknown = Uuid::now_v7();
    let group = |anchor: Value, owner: Option<Uuid>| {
        let mut body = json!({"slug": "edge", "anchor": anchor});
        if let Some(owner) = owner {
            body["owner"] = by_id(owner);
        }
        module("create_vlan_group", body)
    };
    for op in [
        // Anchored on nothing.
        group(json!({"entity": by_id(unknown)}), None),
        group(json!({"site": by_id(unknown)}), Some(acme)),
        group(json!({"domain": by_id(unknown)}), Some(acme)),
        // On an object of another kind.
        group(json!({"entity": by_id(paris)}), None),
        group(json!({"site": by_id(acme)}), Some(acme)),
        // Owned by nothing, by a site.
        group(json!({"site": by_id(paris)}), Some(unknown)),
        group(json!({"site": by_id(paris)}), Some(paris)),
    ] {
        assert_eq!(f.refused(vec![op.clone()], None).await, [UNKNOWN], "{op:?}");
    }
    assert_eq!(f.living("vlan_group").await, 0, "nothing is written");
    let vlan = |group: Uuid| {
        module(
            "create_vlan",
            json!({"group": by_id(group), "vid": 10, "name": "v10"}),
        )
    };
    // A VLAN of no group, of an entity.
    for group in [unknown, acme] {
        assert_eq!(f.refused(vec![vlan(group)], None).await, [UNKNOWN]);
    }
    // Of a group closed.
    f.applied(vec![group(json!({"entity": by_id(acme)}), None)], None)
        .await;
    let edge = f.id("vlan_group", "edge").await;
    f.applied(vec![vlan(edge)], None).await;
    f.applied(
        vec![Operation::Close {
            kind: ObjectKind::VlanGroup,
            id: Ref::Id(edge),
            expected: None,
            cascade: true,
        }],
        None,
    )
    .await;
    assert_eq!(f.refused(vec![vlan(edge)], None).await, [UNKNOWN]);
    assert_eq!(f.living("vlan").await, 0, "nothing is written");
}

/// One operation per operation type of the modules of the build, every reference an
/// identifier nothing bears; an operation of the core its plan adds is read too (the
/// domain and the property of `derive`, the address `release` closes).
fn module_samples() -> Vec<Operation> {
    let id = || by_id(Uuid::now_v7());
    vec![
        module(
            "create_vlan_group",
            json!({"slug": "g", "anchor": {"entity": id()}}),
        ),
        module(
            "update_vlan_group",
            json!({"id": id(), "anchor": {"site": id()}, "owner": id()}),
        ),
        module(
            "create_vlan",
            json!({"group": id(), "vid": 10, "name": "v10"}),
        ),
        module("update_vlan", json!({"id": id(), "group": id()})),
        dummy::create_thing(json!({
            "label": "t", "prefix": id(), "domain_ref": id(), "address_ref": id(),
            "derive": {"domain": id(), "property": id(), "value": "x"},
        })),
        dummy::update_thing(json!({"id": id(), "label": "t", "release": id()})),
        kit::op("create_group", json!({"domain": id()})),
        kit::op("create_item", json!({"parent": id(), "domain": id()})),
        kit::op("request_item", json!({"parent": id()})),
    ]
}

#[kartenn_testing::pg_test]
async fn every_reference_of_every_module_operation_is_checked(pool: PgPool) {
    let f = Fixture::with_modules(&pool).await;
    let samples = module_samples();
    // Each operation type of the build has a sample: a new one fails here until it has.
    let types: BTreeSet<String> = kartenn_service::vlan::OPERATIONS
        .iter()
        .map(|o| o.name.to_owned())
        .chain(build().modules().flat_map(|m| {
            m.operations()
                .iter()
                .map(|o| format!("{}.{}", m.name(), o.name))
        }))
        .collect();
    let covered: BTreeSet<String> = samples
        .iter()
        .map(|op| match op {
            Operation::Module(m) => m.tag(),
            other => panic!("not an operation of a module: {other:?}"),
        })
        .collect();
    assert_eq!(covered, types, "an operation of a module has no sample");
    for op in samples {
        let mut expected = BTreeSet::new();
        ref_ids(&serde_json::to_value(&op).unwrap(), &mut expected);
        let (rules, _, named) = refusal(&f.submit(vec![op.clone()], None).await);
        assert_eq!(rules, [UNKNOWN].into(), "{op:?}");
        assert_eq!(named, expected, "{op:?}: every reference is checked");
    }
}

/// The inverse of a changeset of a module (spec 004, REQ-004-34) designates what that
/// changeset wrote, in force from its start: undoing a creation closes it, undoing a change
/// gives it back, a group moved to another anchor included.
#[kartenn_testing::pg_test]
async fn the_writes_of_a_module_are_undone(pool: PgPool) {
    let f = Fixture::with_modules(&pool).await;
    let (acme, beta) = (f.id("entity", "acme").await, f.id("entity", "beta").await);
    let group = |slug: &str| {
        module(
            "create_vlan_group",
            json!({"local": 1, "slug": slug, "anchor": {"entity": by_id(acme)}}),
        )
    };
    let vlan = |group: Ref, name: &str| {
        module(
            "create_vlan",
            json!({"group": group, "vid": 10, "name": name}),
        )
    };
    // Created, then undone; the creation of a group, below.
    f.applied(vec![group("edge")], None).await;
    let edge = f.id("vlan_group", "edge").await;
    let in_edge = f.applied(vec![vlan(Ref::Id(edge), "v10")], None).await;
    let thing = f
        .applied(vec![dummy::create_thing(json!({"label": "a"}))], None)
        .await;
    f.undo(in_edge).await;
    f.undo(thing).await;
    assert_eq!(
        (f.living("vlan").await, f.living("dummy.thing").await),
        (0, 0)
    );
    // Changed, then undone.
    f.applied(vec![group("core"), vlan(Ref::Local(1), "v20")], None)
        .await;
    f.applied(vec![dummy::create_thing(json!({"label": "b"}))], None)
        .await;
    let core = f.id("vlan_group", "core").await;
    let (v20, b): (Uuid, Uuid) = sqlx::query_as(
        "SELECT (SELECT id FROM vlan WHERE name = 'v20'),
                (SELECT id FROM dummy.thing WHERE label = 'b')",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let changed = f
        .applied(
            vec![
                module("update_vlan", json!({"id": by_id(v20), "name": "v21"})),
                module(
                    "update_vlan_group",
                    json!({"id": by_id(core), "anchor": {"entity": by_id(beta)}}),
                ),
                dummy::update_thing(json!({"id": by_id(b), "label": "c"})),
            ],
            None,
        )
        .await;
    f.undo(changed).await;
    let now: (String, Uuid, String) = sqlx::query_as(
        "SELECT (SELECT name FROM vlan WHERE id = $1 AND valid_period @> now()),
                (SELECT anchor_id FROM vlan_group WHERE id = $2 AND valid_period @> now()),
                (SELECT label FROM dummy.thing WHERE id = $3 AND valid_period @> now())",
    )
    .bind(v20)
    .bind(core)
    .bind(b)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(now, ("v20".into(), acme, "b".into()));
}

/// Undoing the creation of a group of VLANs closes it with what its plan wrote for it, the
/// owner it assigned included, its VLANs created beside it too (REQ-004-34, task 004-26).
#[kartenn_testing::pg_test]
async fn the_creation_of_a_group_is_undone(pool: PgPool) {
    let f = Fixture::with_modules(&pool).await;
    let acme = f.id("entity", "acme").await;
    let group = |slug: &str| {
        module(
            "create_vlan_group",
            json!({"local": 1, "slug": slug, "anchor": {"entity": by_id(acme)}}),
        )
    };
    let vlan = module(
        "create_vlan",
        json!({"group": Ref::Local(1), "vid": 10, "name": "v10"}),
    );
    let alone = f.applied(vec![group("edge")], None).await;
    f.undo(alone).await;
    assert_eq!(f.living("vlan_group").await, 0);
    let together = f.applied(vec![group("core"), vlan], None).await;
    f.undo(together).await;
    let owners: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM property_assignment
         WHERE object_type = 'vlan_group' AND valid_period @> now()",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        (f.living("vlan_group").await, f.living("vlan").await, owners),
        (0, 0, 0)
    );
}

/// Undoing the move of a VLAN into a group created beside it gives the VLAN back to its
/// former group and closes the new one (REQ-004-34, task 004-26).
#[kartenn_testing::pg_test]
async fn a_move_into_a_new_group_is_undone(pool: PgPool) {
    let f = Fixture::with_modules(&pool).await;
    let acme = f.id("entity", "acme").await;
    let group = |slug: &str| {
        module(
            "create_vlan_group",
            json!({"local": 1, "slug": slug, "anchor": {"entity": by_id(acme)}}),
        )
    };
    f.applied(
        vec![
            group("edge"),
            module(
                "create_vlan",
                json!({"group": Ref::Local(1), "vid": 10, "name": "v10"}),
            ),
        ],
        None,
    )
    .await;
    let edge = f.id("vlan_group", "edge").await;
    let v10: Uuid = sqlx::query_scalar("SELECT id FROM vlan WHERE name = 'v10'")
        .fetch_one(&pool)
        .await
        .unwrap();
    let moved = f
        .applied(
            vec![
                group("core"),
                module(
                    "update_vlan",
                    json!({"id": by_id(v10), "group": Ref::Local(1)}),
                ),
            ],
            None,
        )
        .await;
    f.undo(moved).await;
    let now: (Uuid, i64) = sqlx::query_as(
        "SELECT (SELECT group_id FROM vlan WHERE id = $1 AND valid_period @> now()),
                (SELECT count(*) FROM vlan_group WHERE valid_period @> now())",
    )
    .bind(v10)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(now, (edge, 1));
}
