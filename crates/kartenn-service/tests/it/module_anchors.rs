//! Anchors, access control and properties of the objects of modules (spec 008, REQ-008-6,
//! 7; task 008-5): `core.owner` inherited along the tree of the module, `core.site` taken
//! from the nearest ancestor, the domain of an anchored type, and module types as
//! carriers and targets of properties.

use crate::support::kit_module as kit;

use std::collections::HashMap;

use kartenn_core::objects::Nature;
use kartenn_core::values::Cardinality;
use kartenn_service::authz::{Authorizer, Subject, Unrestricted};
use kartenn_service::declarations::{core_declarations_for, sync_modules};
use kartenn_service::operations::{DeclarationInput, ModuleKind, PeriodSpec, RawInput, TypeInput};
use kartenn_service::pipeline::MAX_OPERATIONS;
use kartenn_service::{ChangesetRequest, ObjectKind, Operation, Outcome, Ref, submit};
use kartenn_store_pg::rbac::PgAuthorizer;
use kartenn_store_pg::write::PgStore;
use serde_json::json;
use sqlx::PgPool;
use sqlx::types::Uuid;
use sqlx::types::time::OffsetDateTime;

/// Entities `group` > `branch`, `vendor`; sites `campus` > `building`; domain `lab` owned
/// by `group`, with the network `10.5.0.0/24`. Groups of the kit: `g-branch` (owner
/// `branch`) with the item `i-branch`; `g-vendor` (owner `vendor`) with `i-vendor`;
/// `g-site` (owner `group`, site `building`) with `i-site`; `g-lab` (owner `vendor`, in the
/// domain `lab`).
struct Kit {
    pool: PgPool,
    store: PgStore,
    author: Uuid,
    ids: HashMap<&'static str, Uuid>,
}

fn group() -> ObjectKind {
    ObjectKind::Module(ModuleKind::parse("kit.group").unwrap())
}

fn item() -> ObjectKind {
    ObjectKind::Module(ModuleKind::parse("kit.item").unwrap())
}

fn assign(object: Ref, kind: ObjectKind, property: Uuid, value: Uuid) -> Operation {
    Operation::AssignProperty {
        object,
        kind,
        property: Ref::Id(property),
        values: vec![RawInput::Text(value.to_string())],
    }
}

fn rules(outcome: &Outcome) -> Vec<&'static str> {
    match outcome {
        Outcome::Refused { violations, .. } => violations.iter().map(|v| v.rule.id).collect(),
        other => panic!("not refused: {other:?}"),
    }
}

impl Kit {
    async fn new(pool: &PgPool) -> Self {
        let author = sqlx::query_scalar(
            "INSERT INTO person (id, display_name) VALUES (gen_random_uuid(), 't') RETURNING id",
        )
        .fetch_one(pool)
        .await
        .unwrap();
        let store = kit::store(pool);
        let declarations = [core_declarations_for(&kit::registry())];
        for outcome in sync_modules(&store, &Unrestricted, &declarations, MAX_OPERATIONS)
            .await
            .unwrap()
        {
            assert!(matches!(outcome, Outcome::Applied { .. }), "{outcome:?}");
        }
        let mut k = Self {
            pool: pool.clone(),
            store,
            author,
            ids: HashMap::new(),
        };
        k.build().await;
        k
    }

    async fn submit_at(&self, operations: Vec<Operation>, period: Option<PeriodSpec>) -> Outcome {
        let request = ChangesetRequest {
            author: self.author,
            motive: "test".into(),
            period,
            operations,
            simulate: false,
            idempotency_key: None,
        };
        submit(&self.store, &Unrestricted, &request, MAX_OPERATIONS)
            .await
            .unwrap()
    }

    async fn submit(&self, operations: Vec<Operation>) -> Outcome {
        self.submit_at(operations, None).await
    }

    async fn applied(&self, operations: Vec<Operation>) {
        let outcome = self.submit(operations).await;
        assert!(matches!(outcome, Outcome::Applied { .. }), "{outcome:?}");
    }

    async fn key(&self, key: &str) -> Uuid {
        sqlx::query_scalar("SELECT id FROM property_declaration WHERE key = $1")
            .bind(key)
            .fetch_one(&self.pool)
            .await
            .unwrap()
    }

    /// The objects of the kit, in creation order.
    async fn kit_objects(&self) -> Vec<Uuid> {
        sqlx::query_scalar("SELECT id FROM module_object ORDER BY sys_start, id")
            .fetch_all(&self.pool)
            .await
            .unwrap()
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

    /// A group (local 1) with `owner`, and an item below it (local 2), in one changeset;
    /// their identifiers.
    async fn group_and_item(&self, owner: &str, extra: Vec<Operation>) -> (Uuid, Uuid) {
        let before = self.kit_objects().await.len();
        let mut operations = vec![
            kit::op("create_group", json!({"local": 1})),
            assign(
                Ref::Local(1),
                group(),
                self.key("core.owner").await,
                self.ids[owner],
            ),
            kit::op("create_item", json!({"local": 2, "parent": {"local": 1}})),
        ];
        operations.extend(extra);
        self.applied(operations).await;
        let all = self.kit_objects().await;
        let new: Vec<Uuid> = all[before..].to_vec();
        let kinds: Vec<String> =
            sqlx::query_scalar("SELECT kind FROM module_object WHERE id = ANY($1) ORDER BY kind")
                .bind(&new)
                .fetch_all(&self.pool)
                .await
                .unwrap();
        assert_eq!(kinds, ["kit.group", "kit.item"]);
        let parent: Option<Uuid> = sqlx::query_scalar(
            "SELECT parent_id FROM module_object WHERE id = ANY($1) AND kind = 'kit.item'",
        )
        .bind(&new)
        .fetch_one(&self.pool)
        .await
        .unwrap();
        let parent = parent.expect("the item has its group");
        let child = *new.iter().find(|id| **id != parent).unwrap();
        (parent, child)
    }

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
        for (name, table) in [
            ("group", "entity"),
            ("branch", "entity"),
            ("vendor", "entity"),
            ("campus", "site"),
            ("building", "site"),
        ] {
            let id = self.find(table, "slug", name).await;
            self.ids.insert(name, id);
        }
        let owner = self.key("core.owner").await;
        self.applied(vec![
            Operation::CreateDomain {
                local: Some(1),
                slug: "lab".into(),
            },
            assign(Ref::Local(1), ObjectKind::Domain, owner, self.ids["group"]),
            Operation::CreatePrefix {
                local: None,
                domain: Ref::Local(1),
                prefix: "10.5.0.0/24".into(),
                nature: Nature::Network,
                point_to_point: false,
                anycast: false,
            },
        ])
        .await;
        let lab = self.find("addressing_domain", "slug", "lab").await;
        self.ids.insert("lab", lab);
        let network = self.find("prefix", "prefix", "10.5.0.0/24").await;
        self.ids.insert("10.5.0.0/24", network);

        let (g, i) = self.group_and_item("branch", vec![]).await;
        self.ids.insert("g-branch", g);
        self.ids.insert("i-branch", i);
        let (g, i) = self.group_and_item("vendor", vec![]).await;
        self.ids.insert("g-vendor", g);
        self.ids.insert("i-vendor", i);
        let site = self.key("core.site").await;
        let building = self.ids["building"];
        let (g, i) = self
            .group_and_item(
                "group",
                vec![assign(Ref::Local(1), group(), site, building)],
            )
            .await;
        self.ids.insert("g-site", g);
        self.ids.insert("i-site", i);
        self.applied(vec![
            kit::op("create_group", json!({"local": 1, "domain": {"id": lab}})),
            assign(Ref::Local(1), group(), owner, self.ids["vendor"]),
        ])
        .await;
        let g_lab: Uuid =
            sqlx::query_scalar("SELECT id FROM module_object WHERE domain_id IS NOT NULL")
                .fetch_one(&self.pool)
                .await
                .unwrap();
        self.ids.insert("g-lab", g_lab);
    }

    async fn anchor(&self, function: &str, object: &str) -> Option<Uuid> {
        sqlx::query_scalar(sqlx::AssertSqlSafe(format!("SELECT {function}($1, now())")))
            .bind(self.ids[object])
            .fetch_one(&self.pool)
            .await
            .unwrap()
    }

    /// A role holding `entries`, bound to a new person on the object `scope` of `kind`.
    async fn reader(&self, entries: &[&str], kind: &str, scope: &str) -> Subject {
        let role = Uuid::now_v7();
        sqlx::query(
            "INSERT INTO role (id, key, labels, valid_period, changeset_id)
             VALUES ($1, $2, jsonb_build_object('fr', $2), tstzrange(now() - interval '1 day', NULL), gen_random_uuid())",
        )
        .bind(role)
        .bind(format!("r{}", role.simple()))
        .execute(&self.pool)
        .await
        .unwrap();
        for entry in entries {
            sqlx::query(
                "INSERT INTO role_permission (role_id, permission, valid_period, changeset_id)
                 VALUES ($1, $2, tstzrange(now() - interval '1 day', NULL), gen_random_uuid())",
            )
            .bind(role)
            .bind(entry)
            .execute(&self.pool)
            .await
            .unwrap();
        }
        let who = Subject {
            id: Uuid::now_v7(),
            issuer: None,
            groups: Vec::new(),
        };
        sqlx::query(
            "INSERT INTO binding (id, subject_kind, subject_id, role_id, scope_kind, scope_id,
                                  valid_period, changeset_id)
             VALUES (gen_random_uuid(), 'identity', $1, $2, $3, $4,
                     tstzrange(now() - interval '1 day', NULL), gen_random_uuid())",
        )
        .bind(who.id)
        .bind(role)
        .bind(kind)
        .bind(self.ids[scope])
        .execute(&self.pool)
        .await
        .unwrap();
        who
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

    /// The names among `objects` that `subject` may read, by the read scope of the lists
    /// (`kartenn_scopes` and `kartenn_visible`, under `<type>.read`).
    async fn readable(&self, subject: &Subject, objects: &[&'static str]) -> Vec<&'static str> {
        let scope = PgAuthorizer::new(self.pool.clone())
            .with_modules(kit::registry())
            .read_scope(subject, OffsetDateTime::now_utc())
            .await
            .unwrap()
            .to_json()
            .unwrap();
        let mut out = Vec::new();
        for object in objects {
            let yes: bool = sqlx::query_scalar(
                "WITH s AS (SELECT * FROM kartenn_scopes($2))
                 SELECT coalesce((SELECT bool_or(kartenn_visible(
                                      m.id, m.domain_id, NULL, now(), s.global, s.entities,
                                      s.sites, s.domains, s.prefix_domains, s.prefixes,
                                      s.confinement))
                                  FROM s WHERE s.kind = m.kind), false)
                 FROM module_object m WHERE m.id = $1",
            )
            .bind(self.ids[object])
            .bind(&scope)
            .fetch_one(&self.pool)
            .await
            .unwrap();
            if yes {
                out.push(*object);
            }
        }
        out
    }
}

const GROUPS: [&str; 4] = ["g-branch", "g-vendor", "g-site", "g-lab"];
const ITEMS: [&str; 3] = ["i-branch", "i-vendor", "i-site"];

#[kartenn_testing::pg_test]
async fn a_reader_on_an_entity_sees_its_module_objects(pool: PgPool) {
    let k = Kit::new(&pool).await;
    let reads = ["kit.group.read", "kit.item.read"];
    let who = k.reader(&reads, "entity", "branch").await;
    assert_eq!(
        k.allowed(&who, "kit.group.read", &GROUPS).await,
        ["g-branch"]
    );
    assert_eq!(k.allowed(&who, "kit.item.read", &ITEMS).await, ["i-branch"]);
    // The lists read under `<type>.read`, kind by kind.
    assert_eq!(k.readable(&who, &GROUPS).await, ["g-branch"]);
    assert_eq!(k.readable(&who, &ITEMS).await, ["i-branch"]);
    // A parent entity reaches the objects of its descendants; another entity, none.
    let parent = k.reader(&reads, "entity", "group").await;
    assert_eq!(
        k.allowed(&parent, "kit.group.read", &GROUPS).await,
        ["g-branch", "g-site"]
    );
    let other = k.reader(&["kit.group.read"], "entity", "vendor").await;
    assert_eq!(
        k.allowed(&other, "kit.item.read", &ITEMS).await,
        Vec::<&str>::new()
    );
}

#[kartenn_testing::pg_test]
async fn a_site_binding_covers_module_objects_of_child_sites(pool: PgPool) {
    let k = Kit::new(&pool).await;
    let building = Some(k.ids["building"]);
    assert_eq!(k.anchor("kartenn_site", "g-site").await, building);
    // The item takes the site of its group.
    assert_eq!(k.anchor("kartenn_site", "i-site").await, building);
    assert_eq!(k.anchor("kartenn_site", "g-branch").await, None);
    let who = k
        .reader(&["kit.group.read", "kit.item.read"], "site", "campus")
        .await;
    assert_eq!(k.allowed(&who, "kit.group.read", &GROUPS).await, ["g-site"]);
    assert_eq!(k.allowed(&who, "kit.item.read", &ITEMS).await, ["i-site"]);
    assert_eq!(k.readable(&who, &ITEMS).await, ["i-site"]);
}

#[kartenn_testing::pg_test]
async fn a_domain_anchored_module_object_is_in_the_domain_scope(pool: PgPool) {
    let k = Kit::new(&pool).await;
    let who = k.reader(&["kit.group.read"], "domain", "lab").await;
    assert_eq!(k.allowed(&who, "kit.group.read", &GROUPS).await, ["g-lab"]);
    assert_eq!(k.readable(&who, &GROUPS).await, ["g-lab"]);
    // A prefix scope never reaches an object of a module.
    let prefix = k.reader(&["kit.group.read"], "prefix", "10.5.0.0/24").await;
    assert!(
        k.allowed(&prefix, "kit.group.read", &GROUPS)
            .await
            .is_empty()
    );
}

#[kartenn_testing::pg_test]
async fn a_module_object_inherits_owner_from_its_parent(pool: PgPool) {
    let k = Kit::new(&pool).await;
    assert_eq!(
        k.anchor("kartenn_owner", "i-branch").await,
        Some(k.ids["branch"])
    );
    assert_eq!(
        k.anchor("kartenn_owner", "i-vendor").await,
        Some(k.ids["vendor"])
    );
    // The group changes owner: its item follows.
    let owner = k.key("core.owner").await;
    k.applied(vec![assign(
        Ref::Id(k.ids["g-branch"]),
        group(),
        owner,
        k.ids["vendor"],
    )])
    .await;
    assert_eq!(
        k.anchor("kartenn_owner", "i-branch").await,
        Some(k.ids["vendor"])
    );
    // An item may carry its own owner.
    k.applied(vec![assign(
        Ref::Id(k.ids["i-branch"]),
        item(),
        owner,
        k.ids["branch"],
    )])
    .await;
    assert_eq!(
        k.anchor("kartenn_owner", "i-branch").await,
        Some(k.ids["branch"])
    );
    // A group requires its owner; the item below one inherits it.
    let outcome = k
        .submit(vec![kit::op("create_group", json!({"local": 1}))])
        .await;
    assert_eq!(rules(&outcome), ["prop.assignment.required"]);
    k.applied(vec![kit::op(
        "create_item",
        json!({"local": 1, "parent": {"id": k.ids["g-site"]}}),
    )])
    .await;
}

#[kartenn_testing::pg_test]
async fn ref_to_a_module_type(pool: PgPool) {
    let k = Kit::new(&pool).await;
    let declare = |local, key: &str, value_type, carriers: &[&str]| Operation::DeclareProperty {
        local: Some(local),
        declaration: DeclarationInput {
            key: key.into(),
            value_type,
            cardinality: Cardinality::Single,
            carriers: carriers.iter().map(|c| (*c).to_owned()).collect(),
            inherit: None,
            module: None,
            obligations: Vec::new(),
            expected: None,
            labels: None,
        },
    };
    let text = TypeInput::Text {
        min_len: 0,
        max_len: 64,
        pattern: None,
    };
    // A type of a module carries a property of an administrator; a reference targets one,
    // declared since yesterday with an entity that lives since then.
    let yesterday = || {
        Some(PeriodSpec {
            start: (OffsetDateTime::now_utc().unix_timestamp() - 86_400) * 1_000_000,
            end: None,
        })
    };
    let outcome = k
        .submit_at(
            vec![
                declare(
                    1,
                    "custom.kit",
                    TypeInput::Ref {
                        target: "kit.group".into(),
                    },
                    &["prefix", "entity"],
                ),
                Operation::CreateEntity {
                    local: Some(2),
                    slug: "old".into(),
                    parent: None,
                },
            ],
            yesterday(),
        )
        .await;
    assert!(matches!(outcome, Outcome::Applied { .. }), "{outcome:?}");
    k.applied(vec![declare(
        1,
        "custom.contact",
        text.clone(),
        &["kit.item"],
    )])
    .await;
    let outcome = k
        .submit(vec![declare(1, "custom.nothing", text, &["kit.nothing"])])
        .await;
    assert_eq!(rules(&outcome), ["prop.declaration.invalid"]);
    let contact = k.key("custom.contact").await;
    let kit_ref = k.key("custom.kit").await;
    k.applied(vec![Operation::AssignProperty {
        object: Ref::Id(k.ids["i-site"]),
        kind: item(),
        property: Ref::Id(contact),
        values: vec![RawInput::Text("noc".into())],
    }])
    .await;
    let network = k.ids["10.5.0.0/24"];
    k.applied(vec![assign(
        Ref::Id(network),
        ObjectKind::Prefix,
        kit_ref,
        k.ids["g-site"],
    )])
    .await;
    // Not a group: refused.
    let outcome = k
        .submit(vec![assign(
            Ref::Id(network),
            ObjectKind::Prefix,
            kit_ref,
            k.ids["i-site"],
        )])
        .await;
    assert_eq!(rules(&outcome), ["prop.assignment.invalid_value"]);
    // A group that does not live over the whole period: refused; from now on, accepted.
    let old = k.find("entity", "slug", "old").await;
    let outcome = k
        .submit_at(
            vec![assign(
                Ref::Id(old),
                ObjectKind::Entity,
                kit_ref,
                k.ids["g-branch"],
            )],
            yesterday(),
        )
        .await;
    assert_eq!(rules(&outcome), ["prop.assignment.invalid_value"]);
    k.applied(vec![assign(
        Ref::Id(old),
        ObjectKind::Entity,
        kit_ref,
        k.ids["g-branch"],
    )])
    .await;
}

impl Kit {
    /// The outcome of `operations` submitted by `who`, under the rights the database
    /// grants it.
    async fn submit_by(&self, who: &Subject, operations: Vec<Operation>) -> Outcome {
        sqlx::query(
            "INSERT INTO person (id, display_name) VALUES ($1, 'w') ON CONFLICT (id) DO NOTHING",
        )
        .bind(who.id)
        .execute(&self.pool)
        .await
        .unwrap();
        let request = ChangesetRequest {
            author: who.id,
            motive: "test".into(),
            period: None,
            operations,
            simulate: false,
            idempotency_key: None,
        };
        submit(
            &self.store,
            &PgAuthorizer::new(self.pool.clone()),
            &request,
            MAX_OPERATIONS,
        )
        .await
        .unwrap()
    }
}

/// ADR 0026, amendment 3: an operation of a module requires the permission its module
/// declares for it, on the object it writes and under the same scopes as `<type>.write`;
/// the module reads who stands behind the changeset.
#[kartenn_testing::pg_test]
async fn an_operation_requires_the_permission_its_module_declares(pool: PgPool) {
    let k = Kit::new(&pool).await;
    let request = |group: Uuid| {
        vec![kit::op(
            "request_item",
            json!({"local": 1, "parent": {"id": group}}),
        )]
    };
    let before = k.kit_objects().await.len();
    // The declared permission on the entity of the group: applied.
    let requester = k.reader(&["kit.item.request"], "entity", "branch").await;
    let outcome = k.submit_by(&requester, request(k.ids["g-branch"])).await;
    assert!(matches!(outcome, Outcome::Applied { .. }), "{outcome:?}");
    let created = k.kit_objects().await[before..].to_vec();
    assert_eq!(created.len(), 1);
    let actors = kit::actors_of(created[0]).expect("the module read its actors");
    assert_eq!(actors.author, Some(requester.id));
    assert_eq!(actors.on_behalf_of, None);
    assert_eq!(actors.validator, None);
    // Another permission of the module does not stand in for it.
    let writer = k.reader(&["kit.group.write"], "entity", "branch").await;
    let refused = k.submit_by(&writer, request(k.ids["g-branch"])).await;
    assert_eq!(rules(&refused), ["core.object.forbidden"]);
    // The declared permission out of scope: refused as `<type>.write` would be.
    let elsewhere = k.reader(&["kit.item.request"], "entity", "vendor").await;
    let refused = k.submit_by(&elsewhere, request(k.ids["g-branch"])).await;
    assert_eq!(rules(&refused), ["core.object.forbidden"]);
    assert_eq!(k.kit_objects().await.len(), before + 1);
}
