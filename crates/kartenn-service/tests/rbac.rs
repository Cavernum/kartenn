//! RBAC on a small tree: owner and site of every object, then the decisions of the
//! PostgreSQL authorizer (spec 005, REQ-005-18 to 20, 23; tasks 005-3, 005-4).

use std::collections::HashMap;

use kartenn_core::objects::{AddressUsage, Nature};
use kartenn_service::authz::{Authorizer, Subject, Unrestricted};
use kartenn_service::declarations::{core_declarations, sync_modules};
use kartenn_service::operations::RawInput;
use kartenn_service::pipeline::MAX_OPERATIONS;
use kartenn_service::{ChangesetRequest, ObjectKind, Operation, Outcome, Ref, submit};
use kartenn_store_pg::rbac::PgAuthorizer;
use kartenn_store_pg::write::PgStore;
use sqlx::PgPool;
use sqlx::types::Uuid;

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

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
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
            "INSERT INTO role (id, key, label, valid_period, changeset_id)
             VALUES ($1, $2, $2, tstzrange(now() - interval '1 day', NULL), gen_random_uuid())",
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

    /// Binds `role` on a scope (`global` or the name of an object of the tree) to an
    /// identity, a group or a whole issuer, until `until` days from now (unbounded with
    /// `None`).
    async fn bind(&self, subject: Binding<'_>, role: Uuid, scope: &str, until: Option<i32>) {
        let (kind, id, issuer, group) = match subject {
            Binding::Identity(id) => ("identity", Some(id), None, None),
            Binding::Group(issuer, group) => ("group", None, Some(issuer), Some(group)),
            Binding::Issuer(issuer) => ("issuer", None, Some(issuer), None),
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
            "INSERT INTO binding (id, subject_kind, subject_id, issuer, group_name, role_id,
                                  scope_kind, scope_id, valid_period, changeset_id)
             VALUES (gen_random_uuid(), $1, $2, $3, $4, $5, $6, $7,
                     tstzrange(now() - interval '2 days',
                               CASE WHEN $8::int IS NULL THEN NULL
                                    ELSE now() + make_interval(days => $8::int) END),
                     gen_random_uuid())",
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
    Issuer(&'a str),
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

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
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
        t.allowed(&who, "core.address.write", &OBJECTS)
            .await
            .is_empty()
    );
    // A binding past its period grants nothing.
    let late = person(None, &[]);
    t.bind(Binding::Identity(late.id), reader, "global", Some(-1))
        .await;
    assert!(t.allowed(&late, read, &OBJECTS).await.is_empty());
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
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
    // (without owner) included; a whole-issuer binding serves all its people.
    t.attach(IDP, None).await;
    t.bind(Binding::Issuer(IDP), reader, "global", None).await;
    assert_eq!(
        t.allowed(&person(Some(IDP), &[]), read, &OBJECTS).await,
        OBJECTS.to_vec()
    );
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn patterns_and_global_permissions(pool: PgPool) {
    let t = Tree::new(&pool).await;
    let editor = t.role("editor", &["prop.*.write"]).await;
    let admin = t.role("admin", &["*"]).await;
    let who = person(None, &[]);
    t.bind(Binding::Identity(who.id), editor, "global", None)
        .await;
    let objects = ["10.5.0.7"];
    assert_eq!(
        t.allowed(&who, "prop.core.site.write", &objects).await,
        objects
    );
    // A sensitive property is spared by the pattern (003/P17).
    sqlx::query("UPDATE property_declaration SET sensitive = true WHERE key = 'core.site'")
        .execute(&pool)
        .await
        .unwrap();
    assert!(
        t.allowed(&who, "prop.core.site.write", &objects)
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
