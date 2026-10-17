//! RBAC on a small tree: owner and site of every object, then the decisions of the
//! PostgreSQL authorizer (spec 005, REQ-005-18 to 20, 23; tasks 005-3, 005-4).

use std::collections::HashMap;

use kartenn_core::objects::{AddressUsage, Nature};
use kartenn_service::authz::Unrestricted;
use kartenn_service::declarations::{core_declarations, sync_modules};
use kartenn_service::operations::RawInput;
use kartenn_service::pipeline::MAX_OPERATIONS;
use kartenn_service::{ChangesetRequest, ObjectKind, Operation, Outcome, Ref, submit};
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
