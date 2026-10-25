//! RBAC on a small tree: owner and site of every object, then the decisions of the
//! PostgreSQL authorizer (spec 005, REQ-005-18 to 20, 23; tasks 005-3, 005-4).

use std::collections::HashMap;

use kartenn_core::objects::{AddressUsage, Nature};
use kartenn_service::audit::{Instance, RequestContext};
use kartenn_service::authz::{Authorizer, Subject, Unrestricted};
use kartenn_service::declarations::{core_declarations, sync_modules};
use kartenn_service::expected::Sources;
use kartenn_service::operations::{BindingScope, GroupMember, RawInput};
use kartenn_service::pipeline::MAX_OPERATIONS;
use kartenn_service::pipeline::PendingKind;
use kartenn_service::rbac::{RENAME_MOTIVE, rename_permissions, sync_roles};
use kartenn_service::reads::{ReadAudit, read_object};
use kartenn_service::undo::inverse;
use kartenn_service::{ChangesetRequest, ObjectKind, Operation, Outcome, Ref, submit, submit_in};
use kartenn_store_pg::audit_journal::PgAuditJournal;
use kartenn_store_pg::filter::{Condition, filter, filter_in};
use kartenn_store_pg::rbac::{PgAuthorizer, ReadScope};
use kartenn_store_pg::tree::{Node, children_in};
use kartenn_store_pg::write::PgStore;
use sqlx::PgPool;
use sqlx::types::Uuid;
use sqlx::types::time::OffsetDateTime;

/// Entities `group` > `branch`, `vendor`; sites `campus` > `building`; domain `lab` owned
/// by `group`, with the container `10.0.0.0/8`, the networks `10.5.0.0/24` (owned by
/// `branch`, site `building`) and `10.6.0.0/24`, the addresses `10.5.0.7` and `10.6.0.7`;
/// domain `other` owned by `vendor`, with `172.16.0.0/24` and `172.16.0.9`.
struct Tree {
    pool: PgPool,
    store: PgStore,
    author: Uuid,
    ids: HashMap<&'static str, Uuid>,
}

impl Tree {
    async fn new(pool: &PgPool) -> Self {
        let author = sqlx::query_scalar(
            "INSERT INTO person (id, display_name) VALUES (gen_random_uuid(), 't') RETURNING id",
        )
        .fetch_one(pool)
        .await
        .unwrap();
        let mut t = Self {
            pool: pool.clone(),
            store: PgStore::new(pool.clone()),
            author,
            ids: HashMap::new(),
        };
        for outcome in sync_modules(
            &t.store,
            &Unrestricted,
            &[core_declarations()],
            MAX_OPERATIONS,
        )
        .await
        .unwrap()
        {
            assert!(matches!(outcome, Outcome::Applied { .. }), "{outcome:?}");
        }
        t.build().await;
        t
    }

    async fn applied(&self, operations: Vec<Operation>) {
        let request = ChangesetRequest {
            author: self.author,
            motive: "test".into(),
            period: None,
            operations,
            simulate: false,
            idempotency_key: None,
            apply_own: true,
        };
        let outcome = submit(&self.store, &Unrestricted, &request, MAX_OPERATIONS)
            .await
            .unwrap();
        assert!(matches!(outcome, Outcome::Applied { .. }), "{outcome:?}");
    }

    async fn find(&self, table: &str, column: &str, value: &str) -> Uuid {
        sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
            "SELECT id FROM {table} WHERE {column}::text = $1 LIMIT 1"
        )))
        .bind(value)
        .fetch_one(&self.pool)
        .await
        .unwrap()
    }

    #[allow(clippy::too_many_lines)] // the whole tree, in order
    async fn build(&mut self) {
        let entity = |local, slug: &str, parent| Operation::CreateEntity {
            local: Some(local),
            slug: slug.into(),
            parent,
        };
        self.applied(vec![
            entity(1, "group", None),
            entity(2, "branch", Some(Ref::Local(1))),
            entity(3, "vendor", None),
            Operation::CreateSite {
                local: Some(4),
                slug: "campus".into(),
                parent: None,
            },
            Operation::CreateSite {
                local: Some(5),
                slug: "building".into(),
                parent: Some(Ref::Local(4)),
            },
        ])
        .await;
        for (name, table, column) in [
            ("group", "entity", "slug"),
            ("branch", "entity", "slug"),
            ("vendor", "entity", "slug"),
            ("campus", "site", "slug"),
            ("building", "site", "slug"),
        ] {
            let id = self.find(table, column, name).await;
            self.ids.insert(name, id);
        }
        let owner = self.find("property_declaration", "key", "core.owner").await;
        let site = self.find("property_declaration", "key", "core.site").await;
        let prefix = |domain, value: &str, nature| Operation::CreatePrefix {
            local: None,
            domain,
            prefix: value.into(),
            nature,
            point_to_point: false,
            anycast: false,
        };
        let address = |domain, value: &str| Operation::CreateAddress {
            local: None,
            domain,
            address: value.into(),
            usage: AddressUsage::Host,
        };
        let assign = |object, kind, property, value: Uuid| Operation::AssignProperty {
            object,
            kind,
            property: Ref::Id(property),
            values: vec![RawInput::Text(value.to_string())],
        };
        self.applied(vec![
            Operation::CreateDomain {
                local: Some(1),
                slug: "lab".into(),
            },
            assign(Ref::Local(1), ObjectKind::Domain, owner, self.ids["group"]),
            prefix(Ref::Local(1), "10.0.0.0/8", Nature::Container),
            prefix(Ref::Local(1), "10.5.0.0/24", Nature::Network),
            prefix(Ref::Local(1), "10.6.0.0/24", Nature::Network),
            address(Ref::Local(1), "10.5.0.7"),
            address(Ref::Local(1), "10.6.0.7"),
            Operation::CreateDomain {
                local: Some(2),
                slug: "other".into(),
            },
            assign(Ref::Local(2), ObjectKind::Domain, owner, self.ids["vendor"]),
            prefix(Ref::Local(2), "172.16.0.0/24", Nature::Network),
            address(Ref::Local(2), "172.16.0.9"),
        ])
        .await;
        for (name, table, column) in [
            ("lab", "addressing_domain", "slug"),
            ("other", "addressing_domain", "slug"),
            ("10.0.0.0/8", "prefix", "prefix"),
            ("10.5.0.0/24", "prefix", "prefix"),
            ("10.6.0.0/24", "prefix", "prefix"),
            ("172.16.0.0/24", "prefix", "prefix"),
            ("10.5.0.7", "address", "host(address)"),
            ("10.6.0.7", "address", "host(address)"),
            ("172.16.0.9", "address", "host(address)"),
        ] {
            let id = self.find(table, column, name).await;
            self.ids.insert(name, id);
        }
        let network = self.ids["10.5.0.0/24"];
        self.applied(vec![
            assign(
                Ref::Id(network),
                ObjectKind::Prefix,
                owner,
                self.ids["branch"],
            ),
            assign(
                Ref::Id(network),
                ObjectKind::Prefix,
                site,
                self.ids["building"],
            ),
        ])
        .await;
    }

    async fn anchor(&self, function: &str, object: &str) -> Option<Uuid> {
        sqlx::query_scalar(sqlx::AssertSqlSafe(format!("SELECT {function}($1, now())")))
            .bind(self.ids[object])
            .fetch_one(&self.pool)
            .await
            .unwrap()
    }
}

#[kartenn_testing::pg_test]
async fn every_object_has_its_owner_and_site(pool: PgPool) {
    let t = Tree::new(&pool).await;
    let id = |name: &str| Some(t.ids[name]);
    for (object, owner) in [
        ("group", id("group")),
        ("branch", id("branch")),
        ("campus", None),
        ("lab", id("group")),
        ("10.0.0.0/8", id("group")),
        ("10.5.0.0/24", id("branch")),
        ("10.5.0.7", id("branch")),
        ("10.6.0.7", id("group")),
        ("172.16.0.9", id("vendor")),
    ] {
        assert_eq!(
            t.anchor("kartenn_owner", object).await,
            owner,
            "owner of {object}"
        );
    }
    for (object, site) in [
        ("building", id("building")),
        ("10.5.0.0/24", id("building")),
        ("10.5.0.7", id("building")),
        ("10.6.0.7", None),
        ("10.0.0.0/8", None),
        ("lab", None),
        ("group", None),
    ] {
        assert_eq!(
            t.anchor("kartenn_site", object).await,
            site,
            "site of {object}"
        );
    }
}

const IDP: &str = "https://idp.example.invalid";
const VENDOR_IDP: &str = "https://vendor.example.invalid";

impl Tree {
    /// A role holding `entries`.
    async fn role(&self, key: &str, entries: &[&str]) -> Uuid {
        let id = Uuid::now_v7();
        sqlx::query(
            "INSERT INTO role (id, key, labels, valid_period, changeset_id)
             VALUES ($1, $2, jsonb_build_object('fr', $2), tstzrange(now() - interval '1 day', NULL), gen_random_uuid())",
        )
        .bind(id)
        .bind(key)
        .execute(&self.pool)
        .await
        .unwrap();
        for entry in entries {
            sqlx::query(
                "INSERT INTO role_permission (role_id, permission, valid_period, changeset_id)
                 VALUES ($1, $2, tstzrange(now() - interval '1 day', NULL), gen_random_uuid())",
            )
            .bind(id)
            .bind(entry)
            .execute(&self.pool)
            .await
            .unwrap();
        }
        id
    }

    /// Binds `role` on a scope (`global` or the name of an object of the tree), until
    /// `until` days from now (unbounded with `None`), to a new group of kartenn whose
    /// member is an identity or a group of an issuer (spec 025).
    async fn bind(&self, subject: Binding<'_>, role: Uuid, scope: &str, until: Option<i32>) {
        let (kind, id, issuer, group) = match subject {
            Binding::Identity(id) => ("identity", Some(id), None, None),
            Binding::Group(issuer, group) => ("idp_group", None, Some(issuer), Some(group)),
        };
        let (scope_kind, scope_id) = match scope {
            "global" => ("global", None),
            name => {
                let kind = match name {
                    "group" | "branch" | "vendor" => "entity",
                    "campus" | "building" => "site",
                    "lab" | "other" => "domain",
                    _ => "prefix",
                };
                (kind, Some(self.ids[name]))
            }
        };
        sqlx::query(
            "WITH g AS (
                INSERT INTO rbac_group (id, key, labels, valid_period, changeset_id)
                VALUES (gen_random_uuid(), 't-' || replace(gen_random_uuid()::text, '-', ''),
                        '{\"fr\": \"t\"}', tstzrange(now() - interval '2 days', NULL),
                        gen_random_uuid())
                RETURNING id),
             m AS (
                INSERT INTO group_member (id, group_id, member_kind, identity_id, issuer,
                                          group_name, valid_period, changeset_id)
                SELECT gen_random_uuid(), g.id, $1, $2, $3, $4,
                       tstzrange(now() - interval '2 days', NULL), gen_random_uuid() FROM g)
             INSERT INTO binding (id, group_id, role_id, scope_kind, scope_id, valid_period,
                                  changeset_id)
             SELECT gen_random_uuid(), g.id, $5, $6, $7,
                    tstzrange(now() - interval '2 days',
                              CASE WHEN $8::int IS NULL THEN NULL
                                   ELSE now() + make_interval(days => $8::int) END),
                    gen_random_uuid()
             FROM g",
        )
        .bind(kind)
        .bind(id)
        .bind(issuer)
        .bind(group)
        .bind(role)
        .bind(scope_kind)
        .bind(scope_id)
        .bind(until)
        .execute(&self.pool)
        .await
        .unwrap();
    }

    /// Attaches `issuer` to an entity of the tree, or everywhere with `None`.
    async fn attach(&self, issuer: &str, entity: Option<&str>) {
        sqlx::query(
            "INSERT INTO issuer_attachment (issuer, entity_id, valid_period, changeset_id)
             VALUES ($1, $2, tstzrange(now() - interval '1 day', NULL), gen_random_uuid())",
        )
        .bind(issuer)
        .bind(entity.map(|e| self.ids[e]))
        .execute(&self.pool)
        .await
        .unwrap();
    }

    /// The names among `objects` on which `subject` holds `permission`.
    async fn allowed(
        &self,
        subject: &Subject,
        permission: &str,
        objects: &[&'static str],
    ) -> Vec<&'static str> {
        let ids: Vec<Uuid> = objects.iter().map(|o| self.ids[o]).collect();
        let denied = PgAuthorizer::new(self.pool.clone())
            .authorize_many(subject, permission, &ids)
            .await
            .unwrap();
        objects
            .iter()
            .filter(|o| !denied.contains(&self.ids[*o]))
            .copied()
            .collect()
    }
}

enum Binding<'a> {
    Identity(Uuid),
    Group(&'a str, &'a str),
}

const OBJECTS: [&str; 7] = [
    "10.5.0.7",
    "10.6.0.7",
    "172.16.0.9",
    "10.5.0.0/24",
    "10.0.0.0/8",
    "campus",
    "branch",
];

fn person(issuer: Option<&str>, groups: &[&str]) -> Subject {
    Subject {
        id: Uuid::now_v7(),
        issuer: issuer.map(str::to_owned),
        groups: groups.iter().map(|g| (*g).to_owned()).collect(),
    }
}

#[kartenn_testing::pg_test]
async fn each_scope_reaches_its_subtree(pool: PgPool) {
    let t = Tree::new(&pool).await;
    let reader = t.role("reader", &["core.address.read"]).await;
    let read = "core.address.read";
    for (scope, expected) in [
        (
            "group",
            vec![
                "10.5.0.7",
                "10.6.0.7",
                "10.5.0.0/24",
                "10.0.0.0/8",
                "branch",
            ],
        ),
        ("branch", vec!["10.5.0.7", "10.5.0.0/24", "branch"]),
        ("campus", vec!["10.5.0.7", "10.5.0.0/24", "campus"]),
        ("other", vec!["172.16.0.9"]),
        ("10.5.0.0/24", vec!["10.5.0.7", "10.5.0.0/24"]),
        ("global", OBJECTS.to_vec()),
    ] {
        let who = person(None, &[]);
        t.bind(Binding::Identity(who.id), reader, scope, None).await;
        assert_eq!(
            t.allowed(&who, read, &OBJECTS).await,
            expected,
            "scope {scope}"
        );
    }
    // REQ-005-19: no binding, nothing; a permission the role lacks, nothing.
    assert!(
        t.allowed(&person(None, &[]), read, &OBJECTS)
            .await
            .is_empty()
    );
    let who = person(None, &[]);
    t.bind(Binding::Identity(who.id), reader, "global", None)
        .await;
    assert!(
        t.allowed(&who, "core.address.edit", &OBJECTS)
            .await
            .is_empty()
    );
    // A binding past its period grants nothing.
    let late = person(None, &[]);
    t.bind(Binding::Identity(late.id), reader, "global", Some(-1))
        .await;
    assert!(t.allowed(&late, read, &OBJECTS).await.is_empty());
}

#[kartenn_testing::pg_test]
async fn an_issuer_is_confined_to_its_entities(pool: PgPool) {
    let t = Tree::new(&pool).await;
    let reader = t.role("reader", &["core.address.read"]).await;
    let read = "core.address.read";
    // A group of the vendor's IdP, bound everywhere, reaches the vendor's objects only.
    t.bind(Binding::Group(VENDOR_IDP, "ops"), reader, "global", None)
        .await;
    let vendor = person(Some(VENDOR_IDP), &["ops"]);
    assert!(
        t.allowed(&vendor, read, &OBJECTS).await.is_empty(),
        "no attachment, no right"
    );
    t.attach(VENDOR_IDP, Some("vendor")).await;
    assert_eq!(t.allowed(&vendor, read, &OBJECTS).await, ["172.16.0.9"]);
    assert!(
        t.allowed(&person(Some(VENDOR_IDP), &["sales"]), read, &OBJECTS)
            .await
            .is_empty(),
        "another group"
    );
    // The internal IdP, attached globally, reaches everything its bindings give, sites
    // (without owner) included; no binding serves a whole issuer (spec 025, G3).
    t.attach(IDP, None).await;
    t.bind(Binding::Group(IDP, "staff"), reader, "global", None)
        .await;
    assert_eq!(
        t.allowed(&person(Some(IDP), &["staff"]), read, &OBJECTS)
            .await,
        OBJECTS.to_vec()
    );
    assert!(
        t.allowed(&person(Some(IDP), &[]), read, &OBJECTS)
            .await
            .is_empty(),
        "an identity in no group holds nothing, its issuer attached"
    );
    // A group of an issuer serves the identities of that issuer only (REQ-025-4).
    assert!(
        t.allowed(&person(Some(VENDOR_IDP), &["staff"]), read, &OBJECTS)
            .await
            .is_empty()
    );
}

#[kartenn_testing::pg_test]
async fn patterns_and_global_permissions(pool: PgPool) {
    let t = Tree::new(&pool).await;
    let editor = t.role("editor", &["prop.*.edit"]).await;
    let admin = t.role("admin", &["*"]).await;
    let who = person(None, &[]);
    t.bind(Binding::Identity(who.id), editor, "global", None)
        .await;
    let objects = ["10.5.0.7"];
    assert_eq!(
        t.allowed(&who, "prop.core.site.edit", &objects).await,
        objects
    );
    // A sensitive property is spared by the pattern (003/P17).
    sqlx::query("UPDATE property_declaration SET sensitive = true WHERE key = 'core.site'")
        .execute(&pool)
        .await
        .unwrap();
    assert!(
        t.allowed(&who, "prop.core.site.edit", &objects)
            .await
            .is_empty()
    );
    // `*` on an entity grants everything there, but a global-only permission nowhere.
    let delegate = person(None, &[]);
    t.bind(Binding::Identity(delegate.id), admin, "group", None)
        .await;
    assert_eq!(
        t.allowed(&delegate, "prop.core.site.read", &objects).await,
        objects
    );
    assert!(
        t.allowed(&delegate, "core.role.admin", &objects)
            .await
            .is_empty()
    );
    let root = person(None, &[]);
    t.bind(Binding::Identity(root.id), admin, "global", None)
        .await;
    assert_eq!(t.allowed(&root, "core.role.admin", &objects).await, objects);
}

/// The children of `prefix`, as names, kept if `keep`.
async fn children_named(t: &Tree, prefix: &str, scope: &ReadScope) -> Vec<Uuid> {
    let mut ids: Vec<Uuid> = children_in(&t.pool, t.ids[prefix], now(), scope)
        .await
        .unwrap()
        .iter()
        .map(Node::id)
        .collect();
    ids.sort();
    ids
}

fn now() -> OffsetDateTime {
    OffsetDateTime::now_utc()
}

#[kartenn_testing::pg_test]
async fn lists_hold_what_the_decision_allows(pool: PgPool) {
    // REQ-005-24: for every scope, a list keeps exactly the objects the unitary decision
    // allows, kind by kind.
    let t = Tree::new(&pool).await;
    let reader = t
        .role("reader", &["core.address.read", "core.prefix.read"])
        .await;
    let addresses_only = t.role("addresses", &["core.address.read"]).await;
    let authorizer = PgAuthorizer::new(pool.clone());
    let all = children_named(&t, "10.0.0.0/8", &ReadScope::all()).await;
    assert!(all.len() >= 2, "the container holds its networks");
    let lab_network = children_named(&t, "10.5.0.0/24", &ReadScope::all()).await;
    for (role, scope) in [
        (reader, "global"),
        (reader, "branch"),
        (reader, "campus"),
        (reader, "10.5.0.0/24"),
        (reader, "other"),
        (addresses_only, "group"),
    ] {
        let who = person(None, &[]);
        t.bind(Binding::Identity(who.id), role, scope, None).await;
        let read_scope = authorizer.read_scope(&who, now()).await.unwrap();
        for (prefix, expected_from) in [("10.0.0.0/8", &all), ("10.5.0.0/24", &lab_network)] {
            let listed = children_named(&t, prefix, &read_scope).await;
            let mut allowed = Vec::new();
            for id in expected_from {
                let kind: String =
                    sqlx::query_scalar("SELECT kind FROM carrier_object WHERE id = $1 LIMIT 1")
                        .bind(id)
                        .fetch_one(&pool)
                        .await
                        .unwrap();
                let denied = authorizer
                    .authorize_many(&who, &format!("core.{kind}.read"), &[*id])
                    .await
                    .unwrap();
                if denied.is_empty() {
                    allowed.push(*id);
                }
            }
            assert_eq!(listed, allowed, "children of {prefix}, scope {scope}");
            if scope == "other" {
                assert!(listed.is_empty(), "another domain: nothing of `lab`");
            }
            if scope == "global" {
                assert_eq!(&listed, expected_from, "everything");
            }
        }
        // The property filter: the objects of `lab` that inherit an owner.
        let owner = t.find("property_declaration", "key", "core.owner").await;
        let mut filtered = filter_in(
            &pool,
            owner,
            t.ids["lab"],
            &Condition::Present,
            now(),
            None,
            100,
            &read_scope,
        )
        .await
        .unwrap();
        filtered.sort();
        let unfiltered = filter(
            &pool,
            owner,
            t.ids["lab"],
            &Condition::Present,
            now(),
            None,
            100,
        )
        .await
        .unwrap();
        let mut allowed = Vec::new();
        for id in unfiltered {
            let kind: String =
                sqlx::query_scalar("SELECT kind FROM carrier_object WHERE id = $1 LIMIT 1")
                    .bind(id)
                    .fetch_one(&pool)
                    .await
                    .unwrap();
            if authorizer
                .authorize_many(&who, &format!("core.{kind}.read"), &[id])
                .await
                .unwrap()
                .is_empty()
            {
                allowed.push(id);
            }
        }
        allowed.sort();
        assert_eq!(filtered, allowed, "filter, scope {scope}");
    }
}

#[kartenn_testing::pg_test]
async fn a_read_needs_its_right_and_masks_sensitive_values(pool: PgPool) {
    let t = Tree::new(&pool).await;
    let authorizer = PgAuthorizer::new(pool.clone());
    let journal = PgAuditJournal::new(pool.clone(), Instance::new("test"));
    let reader = t.role("reader", &["core.prefix.read"]).await;
    let site_reader = t.role("site", &["prop.core.site.read"]).await;
    sqlx::query("UPDATE property_declaration SET sensitive = true WHERE key = 'core.site'")
        .execute(&pool)
        .await
        .unwrap();
    let network = t.ids["10.5.0.0/24"];
    let at =
        kartenn_core::period::Instant(i64::try_from(now().unix_timestamp_nanos() / 1_000).unwrap());
    let read = |who: Subject| {
        let (authorizer, journal, store) = (&authorizer, &journal, &t.store);
        async move {
            read_object(
                store,
                authorizer,
                journal,
                &RequestContext::of(&who),
                network,
                at,
                None,
                ReadAudit::default(),
            )
            .await
            .unwrap()
        }
    };
    let events = |name: &'static str| {
        let pool = pool.clone();
        async move {
            sqlx::query_scalar::<_, i64>("SELECT count(*) FROM audit.event WHERE name = $1")
                .bind(name)
                .fetch_one(&pool)
                .await
                .unwrap()
        }
    };
    // No right: answered as absent, the refusal audited.
    assert_eq!(read(person(None, &[])).await, None);
    assert_eq!(events("authz.access.denied").await, 1);
    // Reader: the network, its site masked, no sensitive read recorded.
    let who = person(None, &[]);
    t.bind(Binding::Identity(who.id), reader, "global", None)
        .await;
    let seen = read(who.clone()).await.unwrap();
    assert!(!seen.properties.contains_key("core.site"));
    assert!(seen.masked.contains("core.site"));
    assert_eq!(events("data.sensitive.read").await, 0);
    // With the right on the value: shown, and its read recorded.
    t.bind(Binding::Identity(who.id), site_reader, "global", None)
        .await;
    let seen = read(who).await.unwrap();
    assert!(seen.properties.contains_key("core.site"));
    assert!(seen.masked.is_empty());
    assert_eq!(events("data.sensitive.read").await, 1);
}

impl Tree {
    /// A person with an identity in the database (changesets reference their author).
    async fn identity(&self) -> Subject {
        let id: Uuid = sqlx::query_scalar(
            "INSERT INTO person (id, display_name) VALUES (gen_random_uuid(), 'p') RETURNING id",
        )
        .fetch_one(&self.pool)
        .await
        .unwrap();
        Subject::from(id)
    }

    /// Submits `operations` as `who`, under the RBAC.
    async fn submit_as(&self, who: &Subject, operations: Vec<Operation>) -> Outcome {
        let request = ChangesetRequest {
            author: who.id,
            motive: "test".into(),
            period: None,
            operations,
            simulate: false,
            idempotency_key: None,
            apply_own: true,
        };
        submit_in(
            &self.store,
            &PgAuthorizer::new(self.pool.clone()),
            &Sources::none(),
            &RequestContext::of(who),
            &request,
            MAX_OPERATIONS,
        )
        .await
        .unwrap()
    }
}

fn new_address(domain: Uuid, value: &str) -> Operation {
    Operation::CreateAddress {
        local: None,
        domain: Ref::Id(domain),
        address: value.into(),
        usage: AddressUsage::Host,
    }
}

fn rules_of(outcome: &Outcome) -> Vec<&'static str> {
    match outcome {
        Outcome::Refused { violations, .. } => violations.iter().map(|v| v.rule.id).collect(),
        _ => Vec::new(),
    }
}

#[kartenn_testing::pg_test]
async fn writes_need_the_right_of_their_kind_in_their_scope(pool: PgPool) {
    let t = Tree::new(&pool).await;
    let editor = t
        .role(
            "editor",
            &[
                "core.address.edit",
                "core.address.apply_own",
                "core.prefix.edit",
                "core.prefix.apply_own",
                "prop.*.edit",
                "prop.*.apply_own",
            ],
        )
        .await;
    let lab = t.ids["lab"];
    // An editor of `branch` creates in its network, not in the group's.
    let who = t.identity().await;
    t.bind(Binding::Identity(who.id), editor, "branch", None)
        .await;
    let outcome = t.submit_as(&who, vec![new_address(lab, "10.5.0.9")]).await;
    assert!(matches!(outcome, Outcome::Applied { .. }), "{outcome:?}");
    let outcome = t.submit_as(&who, vec![new_address(lab, "10.6.0.9")]).await;
    assert_eq!(rules_of(&outcome), ["core.object.forbidden"]);
    // Nor close an object out of its scope.
    let outcome = t
        .submit_as(
            &who,
            vec![Operation::Close {
                kind: ObjectKind::Address,
                id: Ref::Id(t.ids["10.6.0.7"]),
                expected: None,
                cascade: false,
            }],
        )
        .await;
    assert_eq!(rules_of(&outcome), ["core.object.forbidden"]);
    // A network and its address created together: seen from the transaction, the new
    // address belongs to the group, which the editor of `group` may write.
    let group_editor = t.identity().await;
    t.bind(Binding::Identity(group_editor.id), editor, "group", None)
        .await;
    let outcome = t
        .submit_as(
            &group_editor,
            vec![
                Operation::CreatePrefix {
                    local: None,
                    domain: Ref::Id(lab),
                    prefix: "10.7.0.0/24".into(),
                    nature: Nature::Network,
                    point_to_point: false,
                    anycast: false,
                },
                new_address(lab, "10.7.0.1"),
            ],
        )
        .await;
    assert!(matches!(outcome, Outcome::Applied { .. }), "{outcome:?}");
}

#[kartenn_testing::pg_test]
async fn a_right_to_edit_alone_makes_a_proposal(pool: PgPool) {
    let t = Tree::new(&pool).await;
    let proposer = t.role("proposer", &["core.address.edit"]).await;
    let who = t.identity().await;
    t.bind(Binding::Identity(who.id), proposer, "branch", None)
        .await;
    let lab = t.ids["lab"];
    match t.submit_as(&who, vec![new_address(lab, "10.5.0.9")]).await {
        Outcome::Pending { kind, .. } => assert_eq!(kind, PendingKind::Proposal),
        other => panic!("expected a proposal, got {other:?}"),
    }
    // Out of its scope, not even a proposal.
    let outcome = t.submit_as(&who, vec![new_address(lab, "10.6.0.9")]).await;
    assert_eq!(rules_of(&outcome), ["core.object.forbidden"]);
}

impl Tree {
    /// A binding of `role` on `scope` to a new group whose member is `subject`.
    async fn bind_op(&self, subject: Uuid, role: &str, scope: BindingScope) -> Operation {
        let group: String = sqlx::query_scalar(kartenn_testing::GROUP_OF)
            .bind(subject)
            .fetch_one(&self.pool)
            .await
            .unwrap();
        Operation::Bind {
            local: None,
            group,
            role: role.into(),
            scope,
        }
    }
}

/// The binding serving the only group of `identity`.
const BINDING_OF: &str = "SELECT b.id FROM binding b
     JOIN group_member m ON m.group_id = b.group_id WHERE m.identity_id = $1";

fn set_role(key: &str, entries: &[&str]) -> Operation {
    Operation::SetRole {
        key: key.into(),
        labels: [("fr".into(), key.into())].into(),
        entries: entries.iter().map(|e| (*e).to_owned()).collect(),
    }
}

#[kartenn_testing::pg_test]
async fn the_delivered_roles_are_synchronised_once(pool: PgPool) {
    let t = Tree::new(&pool).await;
    let first = sync_roles(&t.store, MAX_OPERATIONS).await.unwrap();
    assert!(matches!(first, Some(Outcome::Applied { .. })), "{first:?}");
    assert!(
        sync_roles(&t.store, MAX_OPERATIONS)
            .await
            .unwrap()
            .is_none()
    );
    let audited: i64 =
        sqlx::query_scalar("SELECT count(*) FROM audit.event WHERE name = 'rbac.role.changed'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(audited, 4, "each delivered role, audited");
    let keys: Vec<String> = sqlx::query_scalar("SELECT key FROM role WHERE builtin ORDER BY key")
        .fetch_all(&pool)
        .await
        .unwrap();
    assert_eq!(keys, ["administrator", "approver", "editor", "reader"]);
    // A delivered role changes by the synchronisation only, even for an administrator.
    let root = t.identity().await;
    let admin = t.role("root", &["*"]).await;
    t.bind(Binding::Identity(root.id), admin, "global", None)
        .await;
    let outcome = t
        .submit_as(&root, vec![set_role("reader", &["core.address.read"])])
        .await;
    assert_eq!(rules_of(&outcome), ["core.role.builtin"]);
}

/// REQ-018-13: the delivered roles carry their French and English labels, written again
/// by the synchronisation when they differ.
#[kartenn_testing::pg_test]
async fn builtin_roles_have_french_and_english_labels(pool: PgPool) {
    let t = Tree::new(&pool).await;
    sync_roles(&t.store, MAX_OPERATIONS).await.unwrap();
    let reader = || async {
        sqlx::query_scalar::<_, serde_json::Value>(
            "SELECT labels FROM role WHERE key = 'reader' AND valid_period @> now()",
        )
        .fetch_one(&pool)
        .await
        .unwrap()
    };
    assert_eq!(
        reader().await,
        serde_json::json!({"fr": "Lecteur", "en": "Reader"})
    );
    // A role of a former version, labelled in French only, is labelled again.
    sqlx::query("UPDATE role SET labels = '{\"fr\": \"Lecteur\"}' WHERE key = 'reader'")
        .execute(&pool)
        .await
        .unwrap();
    let again = sync_roles(&t.store, MAX_OPERATIONS).await.unwrap();
    assert!(matches!(again, Some(Outcome::Applied { .. })), "{again:?}");
    assert_eq!(reader().await["en"], "Reader");
    for role in kartenn_core::authz::BUILTIN_ROLES {
        assert!(
            !role.labels.fr.is_empty() && !role.labels.en.is_empty(),
            "{}",
            role.key
        );
    }
}

#[kartenn_testing::pg_test]
async fn a_role_names_known_permissions_only(pool: PgPool) {
    let t = Tree::new(&pool).await;
    let root = t.identity().await;
    let admin = t.role("root", &["*"]).await;
    t.bind(Binding::Identity(root.id), admin, "global", None)
        .await;
    // Outside the catalogue, or a property no one declared (REQ-005-17).
    for entries in [&["core.nothing.read"][..], &["prop.custom.nowhere.edit"]] {
        let outcome = t.submit_as(&root, vec![set_role("odd", entries)]).await;
        assert_eq!(rules_of(&outcome), ["core.role.invalid"], "{entries:?}");
    }
    // An old name of spec 018, with no alias (REQ-018-3).
    for (old, _) in kartenn_core::authz::RENAMED_PERMISSIONS {
        let outcome = t.submit_as(&root, vec![set_role("odd", &[old])]).await;
        assert_eq!(rules_of(&outcome), ["core.role.invalid"], "{old}");
    }
    // A retired verb of spec 024, with no alias (REQ-024-9).
    for old in [
        "core.address.write",
        "core.address.propose",
        "core.changeset.approve",
        "prop.*.write",
    ] {
        let outcome = t.submit_as(&root, vec![set_role("odd", &[old])]).await;
        assert_eq!(rules_of(&outcome), ["core.role.invalid"], "{old}");
    }
    // A declared property, a pattern, or a property declared by the same changeset.
    let outcome = t
        .submit_as(
            &root,
            vec![
                Operation::DeclareProperty {
                    local: None,
                    declaration: kartenn_service::operations::DeclarationInput {
                        key: "custom.rack".into(),
                        value_type: kartenn_service::operations::TypeInput::Int {
                            min: 0,
                            max: 100,
                        },
                        cardinality: kartenn_core::values::Cardinality::Single,
                        carriers: vec!["address".into()],
                        inherit: None,
                        module: None,
                        obligations: Vec::new(),
                        expected: None,
                        labels: None,
                    },
                },
                set_role(
                    "racks",
                    &[
                        "prop.core.owner.read",
                        "prop.*.read",
                        "prop.custom.rack.edit",
                    ],
                ),
            ],
        )
        .await;
    assert!(matches!(outcome, Outcome::Applied { .. }), "{outcome:?}");
}

/// The read of the flow module, as a build with the module declares it, and its old name.
fn flow_read() -> (&'static str, &'static str) {
    static FLOW_READ: kartenn_core::authz::Permission = kartenn_core::authz::Permission {
        name: "flow.read",
        summary: "Lire les flux.",
        global_only: false,
    };
    kartenn_core::authz::register(&[&FLOW_READ]);
    *kartenn_core::authz::RENAMED_PERMISSIONS
        .iter()
        .find(|(_, new)| *new == FLOW_READ.name)
        .expect("a renamed read")
}

/// The custom roles holding an old name, as an instance before spec 018 stored them:
/// `old` with an unchanged entry, and `both` holding the old and the new name.
async fn roles_before_the_rename(t: &Tree, old: &str, new: &str) -> (Subject, Uuid) {
    let role = t.role("flows", &[old, "core.address.read"]).await;
    t.role("both", &[old, new]).await;
    let who = t.identity().await;
    t.bind(Binding::Identity(who.id), role, "global", None)
        .await;
    (who, role)
}

/// REQ-018-4, Q-018-3: the migration job renames the old names of the custom roles in one
/// system changeset, the bindings giving the same rights; a second run submits nothing.
#[kartenn_testing::pg_test]
async fn custom_roles_take_the_new_names_once(pool: PgPool) {
    let t = Tree::new(&pool).await;
    let (old, new) = flow_read();
    let (who, _) = roles_before_the_rename(&t, old, new).await;
    let objects = ["group", "lab", "10.5.0.7"];
    assert_eq!(t.allowed(&who, old, &objects).await, objects);
    let audited = || async {
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM audit.event WHERE name = 'rbac.role.changed'",
        )
        .fetch_one(&pool)
        .await
        .unwrap()
    };
    let before = audited().await;

    let outcome = rename_permissions(&t.store, MAX_OPERATIONS).await.unwrap();
    let Some(Outcome::Applied { changeset, .. }) = outcome else {
        panic!("{outcome:?}");
    };
    let (author, motive): (Uuid, String) =
        sqlx::query_as("SELECT author_id, motive FROM changeset WHERE id = $1")
            .bind(changeset)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(author, kartenn_service::declarations::SYSTEM_AUTHOR);
    assert_eq!(motive, RENAME_MOTIVE);
    assert_eq!(audited().await, before + 2, "each renamed role, audited");
    for (key, entries) in [
        ("flows", vec!["core.address.read", new]),
        ("both", vec![new]),
    ] {
        let role = kartenn_store_pg::rbac_read::role(&pool, key)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(role.entries, entries, "{key}");
    }
    // The same rights, under the new name only: no alias.
    assert_eq!(t.allowed(&who, new, &objects).await, objects);
    assert!(t.allowed(&who, old, &objects).await.is_empty());
    assert!(
        rename_permissions(&t.store, MAX_OPERATIONS)
            .await
            .unwrap()
            .is_none()
    );
}

/// REQ-018-4, 5: the history keeps the old name; a read of the role before the rename
/// gives it.
#[kartenn_testing::pg_test]
async fn a_role_read_before_the_rename_keeps_the_old_name(pool: PgPool) {
    let t = Tree::new(&pool).await;
    let (old, new) = flow_read();
    let (_, role) = roles_before_the_rename(&t, old, new).await;
    rename_permissions(&t.store, MAX_OPERATIONS).await.unwrap();
    let earlier: Vec<String> = sqlx::query_scalar(
        "SELECT permission FROM role_permission
         WHERE role_id = $1 AND valid_period @> now() - interval '1 hour' ORDER BY permission",
    )
    .bind(role)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(earlier, ["core.address.read", old]);
    let recorded: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM role_permission_history WHERE role_id = $1 AND permission = $2",
    )
    .bind(role)
    .bind(old)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(recorded, 1, "the version before the rename, kept as it was");
}

/// REQ-024-9: the migration job replaces the retired verbs of the custom roles in one
/// system changeset: `write` gives `edit` and `apply_own`, `propose` gives `edit`,
/// `core.changeset.approve` every `apply`; a second run submits nothing.
#[kartenn_testing::pg_test]
async fn custom_roles_take_the_verbs_of_spec_024_once(pool: PgPool) {
    let t = Tree::new(&pool).await;
    t.role(
        "writers",
        &["core.address.read", "core.address.write", "prop.*.write"],
    )
    .await;
    t.role(
        "proposers",
        &["core.prefix.propose", "prop.core.owner.propose"],
    )
    .await;
    t.role("approvers", &["core.changeset.approve"]).await;
    let outcome = rename_permissions(&t.store, MAX_OPERATIONS).await.unwrap();
    assert!(
        matches!(outcome, Some(Outcome::Applied { .. })),
        "{outcome:?}"
    );
    let entries = |key: &'static str| {
        let pool = pool.clone();
        async move {
            kartenn_store_pg::rbac_read::role(&pool, key)
                .await
                .unwrap()
                .unwrap()
                .entries
        }
    };
    assert_eq!(
        entries("writers").await,
        [
            "core.address.apply_own",
            "core.address.edit",
            "core.address.read",
            "prop.*.apply_own",
            "prop.*.edit"
        ]
    );
    assert_eq!(
        entries("proposers").await,
        ["core.prefix.edit", "prop.core.owner.edit"]
    );
    let approvers = entries("approvers").await;
    for apply in ["core.address.apply", "core.entity.apply", "prop.*.apply"] {
        assert!(approvers.iter().any(|e| e == apply), "{approvers:?}");
    }
    assert!(
        approvers
            .iter()
            .all(|e| e.rsplit('.').next() == Some("apply")),
        "{approvers:?}"
    );
    assert!(
        rename_permissions(&t.store, MAX_OPERATIONS)
            .await
            .unwrap()
            .is_none()
    );
}

#[kartenn_testing::pg_test]
async fn an_administrator_binds_and_unbinds(pool: PgPool) {
    let t = Tree::new(&pool).await;
    let root = t.identity().await;
    let admin = t.role("root", &["*"]).await;
    t.bind(Binding::Identity(root.id), admin, "global", None)
        .await;
    let user = t.identity().await;
    let outcome = t
        .submit_as(
            &root,
            vec![
                set_role("auditor", &["core.address.read"]),
                t.bind_op(
                    user.id,
                    "auditor",
                    BindingScope::Entity {
                        id: Ref::Id(t.ids["branch"]),
                    },
                )
                .await,
            ],
        )
        .await;
    assert!(matches!(outcome, Outcome::Applied { .. }), "{outcome:?}");
    assert_eq!(
        t.allowed(&user, "core.address.read", &["10.5.0.7", "10.6.0.7"])
            .await,
        ["10.5.0.7"]
    );
    let binding: Uuid = sqlx::query_scalar(BINDING_OF)
        .bind(user.id)
        .fetch_one(&pool)
        .await
        .unwrap();
    let outcome = t
        .submit_as(&root, vec![Operation::Unbind { id: binding }])
        .await;
    assert!(matches!(outcome, Outcome::Applied { .. }), "{outcome:?}");
    assert!(
        t.allowed(&user, "core.address.read", &["10.5.0.7"])
            .await
            .is_empty()
    );
    // REQ-005-25: who held the role, and until when, stays readable.
    let ended: bool =
        sqlx::query_scalar("SELECT NOT upper_inf(valid_period) FROM binding WHERE id = $1")
            .bind(binding)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(ended, "the binding is ended, not erased");
    // Each change of a binding audited on it (REQ-005-28).
    let audited: Vec<(Vec<Uuid>, Option<String>)> = sqlx::query_as(
        "SELECT target_ids, details ->> 'role' FROM audit.event
         WHERE name = 'rbac.binding.changed' ORDER BY seq",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(audited.len(), 2, "{audited:?}");
    assert_eq!(audited[1].0, [binding]);
    assert!(audited[0].1.is_some() && audited[1].1.is_none());
}

#[kartenn_testing::pg_test]
async fn a_delegate_cannot_escalate(pool: PgPool) {
    let t = Tree::new(&pool).await;
    let root = t.identity().await;
    let admin = t.role("root", &["*"]).await;
    t.bind(Binding::Identity(root.id), admin, "global", None)
        .await;
    let outcome = t
        .submit_as(
            &root,
            vec![
                set_role("auditor", &["core.address.read"]),
                set_role(
                    "writer",
                    &[
                        "core.address.read",
                        "core.address.edit",
                        "core.address.apply_own",
                    ],
                ),
            ],
        )
        .await;
    assert!(matches!(outcome, Outcome::Applied { .. }), "{outcome:?}");
    // The delegate administers the group and reads its addresses.
    let delegate = t.identity().await;
    let manager = t
        .role("manager", &["core.rbac.admin", "core.address.read"])
        .await;
    t.bind(Binding::Identity(delegate.id), manager, "group", None)
        .await;
    let user = t.identity().await;
    let entity = |name: &str| BindingScope::Entity {
        id: Ref::Id(t.ids[name]),
    };
    // Within its scope, a role whose entries it holds.
    let outcome = t
        .submit_as(
            &delegate,
            vec![t.bind_op(user.id, "auditor", entity("branch")).await],
        )
        .await;
    assert!(matches!(outcome, Outcome::Applied { .. }), "{outcome:?}");
    sync_roles(&t.store, MAX_OPERATIONS).await.unwrap();
    // `None`: applied; a role it holds entirely may be delegated, its own included.
    for (role, scope, rule) in [
        ("auditor", entity("vendor"), Some("core.rbac.forbidden")),
        ("auditor", BindingScope::Global, Some("core.rbac.forbidden")),
        ("writer", entity("branch"), Some("core.binding.escalation")),
        (
            "administrator",
            entity("branch"),
            Some("core.binding.escalation"),
        ),
        ("manager", entity("branch"), None),
    ] {
        let outcome = t
            .submit_as(&delegate, vec![t.bind_op(user.id, role, scope).await])
            .await;
        match rule {
            Some(rule) => assert_eq!(rules_of(&outcome), [rule], "{role} on {scope:?}"),
            None => assert!(matches!(outcome, Outcome::Applied { .. }), "{outcome:?}"),
        }
    }
}

#[kartenn_testing::pg_test]
async fn an_undo_cannot_escalate(pool: PgPool) {
    let t = Tree::new(&pool).await;
    let root = t.identity().await;
    let admin = t.role("root", &["*"]).await;
    t.bind(Binding::Identity(root.id), admin, "global", None)
        .await;
    let user = t.identity().await;
    let branch = BindingScope::Entity {
        id: Ref::Id(t.ids["branch"]),
    };
    let outcome = t
        .submit_as(
            &root,
            vec![
                set_role(
                    "writer",
                    &[
                        "core.address.read",
                        "core.address.edit",
                        "core.address.apply_own",
                    ],
                ),
                t.bind_op(user.id, "writer", branch).await,
            ],
        )
        .await;
    assert!(matches!(outcome, Outcome::Applied { .. }), "{outcome:?}");
    let binding: Uuid = sqlx::query_scalar(BINDING_OF)
        .bind(user.id)
        .fetch_one(&pool)
        .await
        .unwrap();
    let Outcome::Applied {
        changeset: unbound, ..
    } = t
        .submit_as(&root, vec![Operation::Unbind { id: binding }])
        .await
    else {
        panic!("the unbinding is applied");
    };
    // A delegate administers the branch but cannot write addresses: the inverse binds
    // `writer` again, under the same checks as a binding of its own (spec 005, K16).
    let delegate = t.identity().await;
    let manager = t
        .role("manager", &["core.rbac.admin", "core.address.read"])
        .await;
    t.bind(Binding::Identity(delegate.id), manager, "group", None)
        .await;
    let undo = inverse(&t.store, unbound, delegate.id)
        .await
        .unwrap()
        .unwrap();
    let outcome = t
        .submit_as(&delegate, undo.request.operations.clone())
        .await;
    assert_eq!(rules_of(&outcome), ["core.binding.escalation"]);
    let outcome = t.submit_as(&root, undo.request.operations).await;
    assert!(matches!(outcome, Outcome::Applied { .. }), "{outcome:?}");
    assert_eq!(
        t.allowed(&user, "core.address.edit", &["10.5.0.7"]).await,
        ["10.5.0.7"]
    );
}

#[kartenn_testing::pg_test]
async fn issuers_and_identities_need_global_administration(pool: PgPool) {
    let t = Tree::new(&pool).await;
    let root = t.identity().await;
    let admin = t.role("root", &["*"]).await;
    t.bind(Binding::Identity(root.id), admin, "global", None)
        .await;
    let delegate = t.identity().await;
    let local_admin = t.role("local", &["*"]).await;
    t.bind(Binding::Identity(delegate.id), local_admin, "group", None)
        .await;
    let user = t.identity().await;
    let operations = || {
        vec![
            Operation::AttachIssuer {
                issuer: VENDOR_IDP.into(),
                entity: Some(Ref::Id(t.ids["vendor"])),
            },
            Operation::SetIdentityState {
                identity: user.id,
                disabled: Some(true),
                revoke_sessions: true,
            },
        ]
    };
    // `*` on an entity is not the global scope these permissions need.
    let outcome = t.submit_as(&delegate, operations()).await;
    assert_eq!(
        rules_of(&outcome),
        ["core.rbac.forbidden", "core.rbac.forbidden"]
    );
    let outcome = t.submit_as(&root, operations()).await;
    assert!(matches!(outcome, Outcome::Applied { .. }), "{outcome:?}");
    let (disabled, revoked): (bool, bool) = sqlx::query_as(
        "SELECT disabled_at IS NOT NULL, revoked_after IS NOT NULL FROM person WHERE id = $1",
    )
    .bind(user.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(disabled && revoked);
    let attached: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM issuer_attachment WHERE issuer = $1 AND valid_period @> now()",
    )
    .bind(VENDOR_IDP)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(attached, 1);
    // A break-glass account, kartenn itself: beyond the reach of a changeset (REQ-005-14).
    let rescue: Uuid = sqlx::query_scalar(
        "INSERT INTO person (id, display_name, kind) VALUES (gen_random_uuid(), 'rescue', 'breakglass')
         RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let system: Uuid = sqlx::query_scalar("SELECT id FROM person WHERE kind = 'system'")
        .fetch_one(&pool)
        .await
        .unwrap();
    for identity in [rescue, system] {
        let outcome = t
            .submit_as(
                &root,
                vec![Operation::SetIdentityState {
                    identity,
                    disabled: Some(true),
                    revoke_sessions: false,
                }],
            )
            .await;
        assert!(!matches!(outcome, Outcome::Applied { .. }), "{outcome:?}");
    }
    let untouched: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM person WHERE id = ANY($1) AND disabled_at IS NULL",
    )
    .bind(vec![rescue, system])
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(untouched, 2);
}

#[kartenn_testing::pg_test]
async fn binding_the_administrator_role_waits_for_a_second_administrator(pool: PgPool) {
    let t = Tree::new(&pool).await;
    let root = t.identity().await;
    let admin = t.role("root", &["*"]).await;
    t.bind(Binding::Identity(root.id), admin, "global", None)
        .await;
    t.role("reader_too", &["core.address.read"]).await;
    let outcome = t
        .submit_as(
            &root,
            vec![Operation::SetApprovalPolicy {
                name: "second-administrator".into(),
                min_operations: None,
                min_diff_objects: None,
                property_keys: Vec::new(),
                domains: Vec::new(),
                authors: Vec::new(),
                scopes: Vec::new(),
                author_roles: Vec::new(),
                author_groups: Vec::new(),
                bound_roles: vec!["root".into()],
                module_triggers: Vec::new(),
                enabled: true,
            }],
        )
        .await;
    assert!(matches!(outcome, Outcome::Applied { .. }), "{outcome:?}");
    let newcomer = t.identity().await;
    // REQ-005-25: any binding of the administrator role waits; another role does not.
    match t
        .submit_as(
            &root,
            vec![t.bind_op(newcomer.id, "root", BindingScope::Global).await],
        )
        .await
    {
        Outcome::Pending { kind, .. } => assert_eq!(kind, PendingKind::Approval),
        other => panic!("expected an approval, got {other:?}"),
    }
    let outcome = t
        .submit_as(
            &root,
            vec![
                t.bind_op(newcomer.id, "reader_too", BindingScope::Global)
                    .await,
            ],
        )
        .await;
    assert!(matches!(outcome, Outcome::Applied { .. }), "{outcome:?}");
}

fn policy_op(name: &str, scopes: Vec<BindingScope>, author_roles: Vec<String>) -> Operation {
    Operation::SetApprovalPolicy {
        name: name.into(),
        min_operations: None,
        min_diff_objects: None,
        property_keys: Vec::new(),
        domains: Vec::new(),
        authors: Vec::new(),
        scopes,
        author_roles,
        author_groups: Vec::new(),
        bound_roles: Vec::new(),
        module_triggers: Vec::new(),
        enabled: true,
    }
}

#[kartenn_testing::pg_test]
async fn policies_hold_by_scope_and_by_role(pool: PgPool) {
    let t = Tree::new(&pool).await;
    let root = t.identity().await;
    let admin = t.role("root", &["*"]).await;
    t.bind(Binding::Identity(root.id), admin, "global", None)
        .await;
    let branch = BindingScope::Entity {
        id: Ref::Id(t.ids["branch"]),
    };
    let outcome = t
        .submit_as(&root, vec![policy_op("branch", vec![branch], Vec::new())])
        .await;
    assert!(matches!(outcome, Outcome::Applied { .. }), "{outcome:?}");
    let lab = t.ids["lab"];
    // In the branch: held for approval; in the group's other network: applied.
    match t.submit_as(&root, vec![new_address(lab, "10.5.0.9")]).await {
        Outcome::Pending { kind, .. } => assert_eq!(kind, PendingKind::Approval),
        other => panic!("expected an approval, got {other:?}"),
    }
    let outcome = t.submit_as(&root, vec![new_address(lab, "10.6.0.9")]).await;
    assert!(matches!(outcome, Outcome::Applied { .. }), "{outcome:?}");
    // By role: the contractors' changesets wait, whoever they touch.
    let contractor = t
        .role(
            "contractor",
            &["core.address.edit", "core.address.apply_own"],
        )
        .await;
    let outcome = t
        .submit_as(
            &root,
            vec![policy_op(
                "contractors",
                Vec::new(),
                vec!["contractor".into()],
            )],
        )
        .await;
    assert!(matches!(outcome, Outcome::Applied { .. }), "{outcome:?}");
    let worker = t.identity().await;
    t.bind(Binding::Identity(worker.id), contractor, "group", None)
        .await;
    match t
        .submit_as(&worker, vec![new_address(lab, "10.6.0.10")])
        .await
    {
        Outcome::Pending { kind, .. } => assert_eq!(kind, PendingKind::Approval),
        other => panic!("expected an approval, got {other:?}"),
    }
}

fn set_group(key: &str) -> Operation {
    Operation::SetGroup {
        key: key.into(),
        labels: [("fr".into(), key.into())].into(),
    }
}

fn add_member(group: &str, id: Uuid) -> Operation {
    Operation::AddMember {
        local: None,
        group: group.into(),
        member: GroupMember::Identity { id },
    }
}

fn bind_group(group: &str, role: &str, scope: BindingScope) -> Operation {
    Operation::Bind {
        local: None,
        group: group.into(),
        role: role.into(),
        scope,
    }
}

impl Tree {
    /// The membership of `identity` in the group `group`, in force now.
    async fn membership(&self, group: &str, identity: Uuid) -> Option<Uuid> {
        sqlx::query_scalar(
            "SELECT m.id FROM group_member m
             JOIN rbac_group g ON g.id = m.group_id AND g.valid_period @> now()
             WHERE g.key = $1 AND m.identity_id = $2 AND m.valid_period @> now()",
        )
        .bind(group)
        .bind(identity)
        .fetch_optional(&self.pool)
        .await
        .unwrap()
    }

    /// A root, holding every permission everywhere.
    async fn root(&self) -> Subject {
        let root = self.identity().await;
        let admin = self.role("root", &["*"]).await;
        self.bind(Binding::Identity(root.id), admin, "global", None)
            .await;
        root
    }
}

// REQ-025-2, 4, 8: the rights of an identity are those of the groups it belongs to,
// directly or by a group of its issuer, while it belongs to them.
#[kartenn_testing::pg_test]
async fn rights_come_and_go_with_the_memberships(pool: PgPool) {
    let t = Tree::new(&pool).await;
    let root = t.root().await;
    let outcome = t
        .submit_as(
            &root,
            vec![
                set_role("auditor", &["core.address.read"]),
                set_group("auditors"),
                bind_group("auditors", "auditor", BindingScope::Global),
                Operation::AddMember {
                    local: None,
                    group: "auditors".into(),
                    member: GroupMember::IdpGroup {
                        issuer: IDP.into(),
                        group: "audit".into(),
                    },
                },
            ],
        )
        .await;
    assert!(matches!(outcome, Outcome::Applied { .. }), "{outcome:?}");
    let (read, objects) = ("core.address.read", ["10.5.0.7"]);
    let user = t.identity().await;
    assert!(t.allowed(&user, read, &objects).await.is_empty());
    t.attach(IDP, None).await;
    let audit = person(Some(IDP), &["audit"]);
    assert_eq!(t.allowed(&audit, read, &objects).await, objects);
    assert!(
        t.allowed(&person(Some(VENDOR_IDP), &["audit"]), read, &objects)
            .await
            .is_empty(),
        "the group of another issuer"
    );
    // A member directly, until removed; a membership over a past period gives nothing.
    let outcome = t
        .submit_as(&root, vec![add_member("auditors", user.id)])
        .await;
    assert!(matches!(outcome, Outcome::Applied { .. }), "{outcome:?}");
    assert_eq!(t.allowed(&user, read, &objects).await, objects);
    let twice = t
        .submit_as(&root, vec![add_member("auditors", user.id)])
        .await;
    assert_eq!(rules_of(&twice), ["core.group.invalid"]);
    let membership = t.membership("auditors", user.id).await.unwrap();
    let outcome = t
        .submit_as(&root, vec![Operation::RemoveMember { id: membership }])
        .await;
    assert!(matches!(outcome, Outcome::Applied { .. }), "{outcome:?}");
    sqlx::query(
        "INSERT INTO group_member (id, group_id, member_kind, identity_id, valid_period,
                                   changeset_id)
         SELECT gen_random_uuid(), id, 'identity', $1,
                tstzrange(now() - interval '2 days', now() - interval '1 day'), gen_random_uuid()
         FROM rbac_group WHERE key = 'auditors'",
    )
    .bind(user.id)
    .execute(&pool)
    .await
    .unwrap();
    assert!(t.allowed(&user, read, &objects).await.is_empty());
    // Unknown groups.
    let user_id = user.id;
    for (op, rule) in [
        (add_member("nobody", user_id), "core.group.unknown"),
        (
            bind_group("nobody", "auditor", BindingScope::Global),
            "core.binding.group_closed",
        ),
    ] {
        assert_eq!(rules_of(&t.submit_as(&root, vec![op]).await), [rule]);
    }
    // Closing the group ends its bindings and its memberships.
    let outcome = t
        .submit_as(
            &root,
            vec![Operation::CloseGroup {
                key: "auditors".into(),
            }],
        )
        .await;
    assert!(matches!(outcome, Outcome::Applied { .. }), "{outcome:?}");
    assert!(t.allowed(&audit, read, &objects).await.is_empty());
    let left: i64 = sqlx::query_scalar(
        "SELECT (SELECT count(*) FROM binding WHERE group_id = g.id AND valid_period @> now())
              + (SELECT count(*) FROM group_member WHERE group_id = g.id
                 AND valid_period @> now())
         FROM rbac_group g WHERE g.key = 'auditors'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(left, 0);
}

// INV-27, REQ-025-7, 8 (G5): adding or removing a member of a group, oneself included, or
// closing it, asks what creating each of its bindings asks; an empty group asks
// `core.rbac.admin` somewhere.
#[kartenn_testing::pg_test]
async fn a_delegate_cannot_escalate_through_a_group(pool: PgPool) {
    let t = Tree::new(&pool).await;
    let root = t.root().await;
    let entity = |name: &str| BindingScope::Entity {
        id: Ref::Id(t.ids[name]),
    };
    let outcome = t
        .submit_as(
            &root,
            vec![
                set_role("auditor", &["core.address.read"]),
                set_role("writer", &["core.address.read", "core.address.edit"]),
                set_group("everywhere"),
                bind_group("everywhere", "auditor", BindingScope::Global),
                set_group("vendor-team"),
                bind_group("vendor-team", "auditor", entity("vendor")),
                set_group("branch-writers"),
                bind_group("branch-writers", "writer", entity("branch")),
                set_group("branch-team"),
                bind_group("branch-team", "auditor", entity("branch")),
                set_group("empty"),
            ],
        )
        .await;
    assert!(matches!(outcome, Outcome::Applied { .. }), "{outcome:?}");
    // The delegate administers the entity `group` (`branch` below) and reads addresses.
    let delegate = t.identity().await;
    let manager = t
        .role("manager", &["core.rbac.admin", "core.address.read"])
        .await;
    t.bind(Binding::Identity(delegate.id), manager, "group", None)
        .await;
    let user = t.identity().await;
    let groups = ["everywhere", "vendor-team", "branch-writers", "branch-team"];
    let outcome = t
        .submit_as(
            &root,
            groups.iter().map(|g| add_member(g, user.id)).collect(),
        )
        .await;
    assert!(matches!(outcome, Outcome::Applied { .. }), "{outcome:?}");
    let refused = Some("core.group.escalation");
    for (group, rule) in [
        // Wider than the delegate's scope, or outside it.
        ("everywhere", refused),
        ("vendor-team", refused),
        // In its scope, a role holding a permission the delegate lacks.
        ("branch-writers", refused),
        ("branch-team", None),
        ("empty", None),
    ] {
        let newcomer = t.identity().await.id;
        let mut operations = vec![add_member(group, newcomer), add_member(group, delegate.id)];
        if let Some(id) = t.membership(group, user.id).await {
            operations.push(Operation::RemoveMember { id });
        }
        operations.push(Operation::CloseGroup { key: group.into() });
        for op in operations {
            let outcome = t.submit_as(&delegate, vec![op.clone()]).await;
            match rule {
                Some(rule) => assert_eq!(rules_of(&outcome), [rule], "{group}: {op:?}"),
                None => assert!(
                    matches!(outcome, Outcome::Applied { .. }),
                    "{group}: {op:?}: {outcome:?}"
                ),
            }
        }
    }
    // Without `core.rbac.admin` anywhere, not even an empty group.
    let nobody = t.identity().await;
    assert_eq!(
        rules_of(&t.submit_as(&nobody, vec![set_group("mine")]).await),
        ["core.rbac.forbidden"]
    );
    let outcome = t
        .submit_as(
            &delegate,
            vec![
                set_group("mine"),
                bind_group("mine", "auditor", entity("branch")),
                add_member("mine", user.id),
            ],
        )
        .await;
    assert!(matches!(outcome, Outcome::Applied { .. }), "{outcome:?}");
}
