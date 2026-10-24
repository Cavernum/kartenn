//! The catalogues of permissions and rules, merged with those of the modules of the build
//! (spec 008, REQ-008-8; task 008-6). Each test runs in its own process (nextest): the
//! catalogue of a test without the module never saw it.

use crate::support::kit_module as kit;

use kartenn_service::authz::Unrestricted;
use kartenn_service::pipeline::MAX_OPERATIONS;
use kartenn_service::rbac::sync_roles;
use kartenn_service::{ChangesetRequest, Operation, Outcome, submit};
use kartenn_store_pg::write::PgStore;
use sqlx::PgPool;
use sqlx::types::Uuid;

async fn set_role(pool: &PgPool, store: &PgStore, entries: &[&str]) -> Outcome {
    let author: Uuid = sqlx::query_scalar(
        "INSERT INTO person (id, display_name) VALUES (gen_random_uuid(), 't') RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    let request = ChangesetRequest {
        author,
        motive: "test".into(),
        period: None,
        operations: vec![Operation::SetRole {
            key: "kit_reader".into(),
            labels: [("fr".into(), "Kit reader".into())].into(),
            entries: entries.iter().map(|e| (*e).to_owned()).collect(),
        }],
        simulate: false,
        idempotency_key: None,
        apply_own: true,
    };
    submit(store, &Unrestricted, &request, MAX_OPERATIONS)
        .await
        .unwrap()
}

async fn entries(pool: &PgPool, role: &str) -> Vec<String> {
    sqlx::query_scalar(
        "SELECT p.permission FROM role r JOIN role_permission p ON p.role_id = r.id
         WHERE r.key = $1 AND upper_inf(p.valid_period) ORDER BY 1",
    )
    .bind(role)
    .fetch_all(pool)
    .await
    .unwrap()
}

#[kartenn_testing::pg_test]
async fn a_role_with_a_module_permission(pool: PgPool) {
    let store = kit::store(&pool);
    let outcome = set_role(&pool, &store, &["kit.group.read", "kit.item.read"]).await;
    assert!(matches!(outcome, Outcome::Applied { .. }), "{outcome:?}");
    assert_eq!(
        entries(&pool, "kit_reader").await,
        ["kit.group.read", "kit.item.read"]
    );
    // The delivered roles that read read the objects of the module; the module's rules
    // read back from the journal.
    let roles = sync_roles(&store, MAX_OPERATIONS).await.unwrap();
    assert!(matches!(roles, Some(Outcome::Applied { .. })), "{roles:?}");
    let reader = entries(&pool, "reader").await;
    assert!(reader.contains(&"kit.group.read".to_owned()), "{reader:?}");
    assert!(reader.contains(&"kit.item.read".to_owned()), "{reader:?}");
    assert!(
        !entries(&pool, "editor")
            .await
            .contains(&"kit.group.edit".to_owned())
    );
    // The approver applies the changesets of others on the objects of the module too.
    assert!(
        entries(&pool, "approver")
            .await
            .contains(&"kit.group.apply".to_owned())
    );
    assert!(kartenn_core::rules::rule("kit.object.invalid").is_some());
    assert!(kartenn_core::authz::permission("kit.group.edit").is_some());
}

#[kartenn_testing::pg_test]
async fn unknown_module_permission_refused(pool: PgPool) {
    let store = PgStore::new(pool.clone());
    let outcome = set_role(&pool, &store, &["kit.group.read"]).await;
    match outcome {
        Outcome::Refused { violations, .. } => {
            let rules: Vec<_> = violations.iter().map(|v| v.rule.id).collect();
            assert_eq!(rules, ["core.role.invalid"]);
        }
        other => panic!("not refused: {other:?}"),
    }
    assert!(kartenn_core::rules::rule("kit.object.invalid").is_none());
}

/// A module whose operation `ask` requires a permission, and whose types are read and
/// written under others (ADR 0026, amendment 3).
#[derive(Debug)]
struct Requiring(
    &'static [(&'static str, &'static str)],
    &'static [kartenn_core::authz::TypePermissions],
);

static OWN_TYPES: &[kartenn_service::module::ObjectType] = &[kartenn_service::module::ObjectType {
    kind: "req.thing",
    collection: "req-things",
    parent: None,
    owner_required: false,
    carries_site: false,
    domain_anchored: false,
}];

static OWN: kartenn_core::authz::Permission = kartenn_core::authz::Permission {
    name: "req.thing.request",
    summary: "Demander une chose.",
    global_only: false,
};
static OWN_PERMISSIONS: &[&kartenn_core::authz::Permission] = &[&OWN];
static OWN_OPERATIONS: &[kartenn_service::module::OperationType] =
    &[kartenn_service::module::OperationType {
        name: "ask",
        defines: None,
    }];

impl kartenn_service::module::Module for Requiring {
    fn name(&self) -> &'static str {
        "req"
    }
    fn version(&self) -> &'static str {
        "0.0.1"
    }
    fn permissions(&self) -> &'static [&'static kartenn_core::authz::Permission] {
        OWN_PERMISSIONS
    }
    fn operations(&self) -> &'static [kartenn_service::module::OperationType] {
        OWN_OPERATIONS
    }
    fn operation_permissions(&self) -> &'static [(&'static str, &'static str)] {
        self.0
    }
    fn object_types(&self) -> &'static [kartenn_service::module::ObjectType] {
        OWN_TYPES
    }
    fn type_permissions(&self) -> &'static [kartenn_core::authz::TypePermissions] {
        self.1
    }
}

/// ADR 0026, amendment 3: a module requires only its own permissions, of its own
/// operations; otherwise the registry, and the server, refuse to start.
#[test]
fn a_required_permission_must_be_the_module_own() {
    use kartenn_service::module::{ModuleRegistry, RegistryError};
    let registry = |declared: &'static [(&'static str, &'static str)]| {
        ModuleRegistry::new(vec![std::sync::Arc::new(Requiring(declared, &[]))])
    };
    assert!(registry(&[("ask", "req.thing.request")]).is_ok());
    let refused: [&'static [(&'static str, &'static str)]; 5] = [
        // Absent from the catalogue of the module.
        &[("ask", "req.thing.approve")],
        // A permission of the core, or of another module.
        &[("ask", "core.address.edit")],
        &[("ask", "kit.item.request")],
        // An operation the module does not accept.
        &[("forge", "req.thing.request")],
        // The same operation twice.
        &[("ask", "req.thing.request"), ("ask", "req.thing.request")],
    ];
    for declared in refused {
        assert!(
            matches!(
                registry(declared),
                Err(RegistryError::UnknownPermission { .. })
            ),
            "{declared:?}"
        );
    }
    // The permissions of a type, likewise: of the module, for one of its types.
    let typed = |kind, read, edit| {
        let types: &'static [kartenn_core::authz::TypePermissions] =
            Box::leak(Box::new([kartenn_core::authz::TypePermissions {
                kind,
                read,
                edit,
            }]));
        ModuleRegistry::new(vec![std::sync::Arc::new(Requiring(&[], types))])
    };
    assert!(typed("req.thing", "req.thing.request", "req.thing.request").is_ok());
    for (kind, read, edit) in [
        ("req.thing", "req.thing.request", "core.address.edit"),
        ("req.thing", "kit.item.read", "req.thing.request"),
        ("req.other", "req.thing.request", "req.thing.request"),
    ] {
        assert!(
            matches!(
                typed(kind, read, edit),
                Err(RegistryError::UnknownPermission { .. })
            ),
            "{kind} {read} {edit}"
        );
    }
}
