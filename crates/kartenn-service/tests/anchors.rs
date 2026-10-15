//! Entities, sites and the owner of objects (spec 002, REQ-002-8 to 12, 20, 38; task
//! 002-22; ADR 0014).

use kartenn_core::objects::{AddressUsage, Nature};
use kartenn_service::authz::{Authorizer, Grants, Unrestricted};
use kartenn_service::declarations::{core_declarations, sync_modules};
use kartenn_service::operations::RawInput;
use kartenn_service::pipeline::MAX_OPERATIONS;
use kartenn_service::{ChangesetRequest, ObjectKind, Operation, Outcome, Ref, submit};
use kartenn_store_pg::explain::explain;
use kartenn_store_pg::write::PgStore;
use sqlx::PgPool;
use sqlx::types::Uuid;
use sqlx::types::time::OffsetDateTime;

struct Fixture {
    pool: PgPool,
    store: PgStore,
    author: Uuid,
}

impl Fixture {
    async fn new(pool: &PgPool) -> Self {
        let author = sqlx::query_scalar(
            "INSERT INTO person (id, display_name) VALUES (gen_random_uuid(), 't') RETURNING id",
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

    /// Declares `core.owner` and `core.site`, as the migration job does.
    async fn core(&self) {
        for outcome in sync_modules(
            &self.store,
            &Unrestricted,
            &[core_declarations()],
            MAX_OPERATIONS,
        )
        .await
        .unwrap()
        {
            assert!(matches!(outcome, Outcome::Applied { .. }), "{outcome:?}");
        }
    }

    async fn submit_as(&self, authorizer: &impl Authorizer, operations: Vec<Operation>) -> Outcome {
        let request = ChangesetRequest {
            author: self.author,
            motive: "test".into(),
            period: None,
            operations,
            simulate: false,
            idempotency_key: None,
        };
        submit(&self.store, authorizer, &request, MAX_OPERATIONS)
            .await
            .unwrap()
    }

    async fn applied(&self, operations: Vec<Operation>) {
        let outcome = self.submit_as(&Unrestricted, operations).await;
        assert!(matches!(outcome, Outcome::Applied { .. }), "{outcome:?}");
    }

    async fn refused_as(
        &self,
        authorizer: &impl Authorizer,
        operations: Vec<Operation>,
    ) -> Vec<&'static str> {
        match self.submit_as(authorizer, operations).await {
            Outcome::Refused { violations, .. } => violations.iter().map(|v| v.rule.id).collect(),
            other => panic!("expected refused, got {other:?}"),
        }
    }

    async fn refused(&self, operations: Vec<Operation>) -> Vec<&'static str> {
        self.refused_as(&Unrestricted, operations).await
    }

    async fn id(&self, table: &str, column: &str, value: &str) -> Uuid {
        sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
            "SELECT id FROM {table} WHERE {column}::text = $1 LIMIT 1"
        )))
        .bind(value)
        .fetch_one(&self.pool)
        .await
        .unwrap()
    }

    async fn property(&self, key: &str) -> Uuid {
        self.id("property_declaration", "key", key).await
    }
}

fn entity(local: u32, slug: &str, parent: Option<Ref>) -> Operation {
    Operation::CreateEntity {
        local: Some(local),
        slug: slug.into(),
        parent,
    }
}

fn prefix(domain: Ref, value: &str, nature: Nature) -> Operation {
    Operation::CreatePrefix {
        local: None,
        domain,
        prefix: value.into(),
        nature,
        point_to_point: false,
        anycast: false,
    }
}

fn assign(object: Uuid, kind: ObjectKind, property: Uuid, target: Uuid) -> Operation {
    Operation::AssignProperty {
        object: Ref::Id(object),
        kind,
        property: Ref::Id(property),
        values: vec![RawInput::Text(target.to_string())],
    }
}

fn close(kind: ObjectKind, id: Uuid) -> Operation {
    Operation::Close {
        kind,
        id: Ref::Id(id),
        expected: None,
    }
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn trees_refuse_cycles_and_missing_parents(pool: PgPool) {
    let f = Fixture::new(&pool).await;
    f.applied(vec![
        entity(1, "group", None),
        entity(2, "branch", Some(Ref::Local(1))),
        entity(3, "team", Some(Ref::Local(2))),
        Operation::CreateSite {
            local: Some(4),
            slug: "region".into(),
            parent: None,
        },
        Operation::CreateSite {
            local: Some(5),
            slug: "campus".into(),
            parent: Some(Ref::Local(4)),
        },
    ])
    .await;
    let group = f.id("entity", "slug", "group").await;
    let team = f.id("entity", "slug", "team").await;
    // REQ-002-8: attaching an entity to one of its descendants.
    assert_eq!(
        f.refused(vec![Operation::MoveEntity {
            id: Ref::Id(group),
            parent: Some(Ref::Id(team)),
            expected: None,
        }])
        .await,
        vec!["core.anchor.cycle"]
    );
    // A root again is fine.
    f.applied(vec![Operation::MoveEntity {
        id: Ref::Id(team),
        parent: None,
        expected: None,
    }])
    .await;
    // REQ-002-9: the same for sites.
    let region = f.id("site", "slug", "region").await;
    let campus = f.id("site", "slug", "campus").await;
    assert_eq!(
        f.refused(vec![Operation::MoveSite {
            id: Ref::Id(region),
            parent: Some(Ref::Id(campus)),
            expected: None,
        }])
        .await,
        vec!["core.anchor.cycle"]
    );
    // A parent must exist: a closed entity cannot receive children.
    f.applied(vec![close(ObjectKind::Entity, team)]).await;
    assert_eq!(
        f.refused(vec![entity(1, "late", Some(Ref::Id(team)))])
            .await,
        vec!["changeset.operation.no_version"]
    );
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn anchors_in_use_stay_open(pool: PgPool) {
    let f = Fixture::new(&pool).await;
    f.core().await;
    let site = f.property("core.site").await;
    let owner = f.property("core.owner").await;
    f.applied(vec![
        entity(1, "group", None),
        entity(2, "branch", Some(Ref::Local(1))),
        Operation::CreateSite {
            local: Some(3),
            slug: "campus".into(),
            parent: None,
        },
    ])
    .await;
    let group = f.id("entity", "slug", "group").await;
    let campus = f.id("site", "slug", "campus").await;
    // REQ-002-11: a domain needs its owner, in the same changeset.
    assert!(
        f.refused(vec![Operation::CreateDomain {
            local: Some(1),
            slug: "lab".into(),
        }])
        .await
        .contains(&"prop.assignment.required")
    );
    f.applied(vec![
        Operation::CreateDomain {
            local: Some(1),
            slug: "lab".into(),
        },
        Operation::AssignProperty {
            object: Ref::Local(1),
            kind: ObjectKind::Domain,
            property: Ref::Id(owner),
            values: vec![RawInput::Text(group.to_string())],
        },
        prefix(Ref::Local(1), "10.1.2.0/24", Nature::Network),
    ])
    .await;
    let network = f.id("prefix", "prefix", "10.1.2.0/24").await;
    f.applied(vec![assign(network, ObjectKind::Prefix, site, campus)])
        .await;
    // REQ-002-10: the site of a network, the parent of an entity, the owner of a domain.
    let used = f.refused(vec![close(ObjectKind::Site, campus)]).await;
    assert_eq!(used, vec!["core.anchor.in_use"]);
    assert_eq!(
        f.refused(vec![close(ObjectKind::Entity, group)]).await,
        vec!["core.anchor.in_use"]
    );
    // Detach first, then close.
    f.applied(vec![
        Operation::UnassignProperty {
            object: Ref::Id(network),
            kind: ObjectKind::Prefix,
            property: Ref::Id(site),
        },
        close(ObjectKind::Site, campus),
    ])
    .await;
}

/// Entities `group`, `branch`, `vendor`, site `campus`; domain `lab` owned by `group`,
/// with a container `10.0.0.0/8` and the networks `10.5.0.0/24` (owned by `branch`) and
/// `10.6.0.0/24`, holding `10.5.0.7` and `10.6.0.7`.
async fn owned(f: &Fixture) {
    f.core().await;
    f.applied(vec![
        entity(1, "group", None),
        entity(2, "branch", Some(Ref::Local(1))),
        entity(3, "vendor", None),
        Operation::CreateSite {
            local: Some(4),
            slug: "campus".into(),
            parent: None,
        },
    ])
    .await;
    let owner = f.property("core.owner").await;
    let group = f.id("entity", "slug", "group").await;
    let branch = f.id("entity", "slug", "branch").await;
    let address = |value: &str| Operation::CreateAddress {
        local: None,
        domain: Ref::Local(1),
        address: value.into(),
        usage: AddressUsage::Host,
    };
    f.applied(vec![
        Operation::CreateDomain {
            local: Some(1),
            slug: "lab".into(),
        },
        Operation::AssignProperty {
            object: Ref::Local(1),
            kind: ObjectKind::Domain,
            property: Ref::Id(owner),
            values: vec![RawInput::Text(group.to_string())],
        },
        prefix(Ref::Local(1), "10.0.0.0/8", Nature::Container),
        prefix(Ref::Local(1), "10.5.0.0/24", Nature::Network),
        prefix(Ref::Local(1), "10.6.0.0/24", Nature::Network),
        address("10.5.0.7"),
        address("10.6.0.7"),
    ])
    .await;
    let network = f.id("prefix", "prefix", "10.5.0.0/24").await;
    f.applied(vec![assign(network, ObjectKind::Prefix, owner, branch)])
        .await;
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn sites_on_networks_and_fixed_domains(pool: PgPool) {
    let f = Fixture::new(&pool).await;
    owned(&f).await;
    let site = f.property("core.site").await;
    let campus = f.id("site", "slug", "campus").await;
    let container = f.id("prefix", "prefix", "10.0.0.0/8").await;
    let network = f.id("prefix", "prefix", "10.6.0.0/24").await;
    // REQ-002-20: a network takes a site, a container does not, in either order.
    assert_eq!(
        f.refused(vec![assign(container, ObjectKind::Prefix, site, campus)])
            .await,
        vec!["core.prefix.site_on_container"]
    );
    f.applied(vec![assign(network, ObjectKind::Prefix, site, campus)])
        .await;
    let to_container = Operation::UpdatePrefix {
        id: Ref::Id(network),
        domain: None,
        prefix: None,
        nature: Some(Nature::Container),
        point_to_point: None,
        anycast: None,
        expected: None,
    };
    assert!(
        f.refused(vec![to_container])
            .await
            .contains(&"core.prefix.site_on_container")
    );
    // REQ-002-12: the domain of a prefix never changes.
    let domain = f.id("addressing_domain", "slug", "lab").await;
    assert_eq!(
        f.refused(vec![Operation::UpdatePrefix {
            id: Ref::Id(network),
            domain: Some(Ref::Id(domain)),
            prefix: None,
            nature: None,
            point_to_point: None,
            anycast: None,
            expected: None,
        }])
        .await,
        vec!["core.domain.immutable"]
    );
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn the_owner_is_resolved_and_guarded(pool: PgPool) {
    let f = Fixture::new(&pool).await;
    owned(&f).await;
    let owner = f.property("core.owner").await;
    let domain = f.id("addressing_domain", "slug", "lab").await;
    let network = f.id("prefix", "prefix", "10.5.0.0/24").await;
    let near = f.id("address", "host(address)", "10.5.0.7").await;
    let far = f.id("address", "host(address)", "10.6.0.7").await;
    let origin = |object| {
        let pool = pool.clone();
        async move {
            explain(&pool, object, owner, OffsetDateTime::now_utc(), None)
                .await
                .unwrap()
                .unwrap()
        }
    };
    // REQ-002-38: through the network that has one, else the domain; the container
    // without owner is walked through.
    let near_owner = origin(near).await;
    assert_eq!(near_owner.origin, Some(network));
    let far_owner = origin(far).await;
    assert_eq!(far_owner.origin, Some(domain));
    assert!(far_owner.chain.len() >= 3, "{:?}", far_owner.chain);
    // A vendor's container over 10.6.0.0/24 takes the network and its address.
    let vendor = f.id("entity", "slug", "vendor").await;
    f.applied(vec![prefix(
        Ref::Id(domain),
        "10.6.0.0/16",
        Nature::Container,
    )])
    .await;
    let taken = f.id("prefix", "prefix", "10.6.0.0/16").await;
    f.applied(vec![assign(taken, ObjectKind::Prefix, owner, vendor)])
        .await;
    assert_eq!(origin(far).await.origin, Some(taken));
    // ADR 0014: closing it gives them back to the domain's owner; without the right on
    // them, the manoeuvre is refused.
    assert!(
        f.refused_as(&Grants::new(), vec![close(ObjectKind::Prefix, taken)])
            .await
            .contains(&"prop.assignment.forbidden_effect")
    );
}
