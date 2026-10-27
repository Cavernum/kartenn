//! The rights of the policy (spec 010, S15 amended by Q17, REQ-010-24; spec 027, P4 to
//! P7): one read for every object of the policy; the objects that define possible values
//! (axes, values, environments, flow types, services) under the settings of the policy;
//! the rules under `policy.rule.admin`. The settings of each declarant, the core's, the
//! machines' and the policy's, are written under its own `<declarant>.settings.*`
//! (REQ-027-5, 6).

use kartenn_service::authz::Grants;
use kartenn_service::operations::{DeclarationInput, RawLabels, TypeInput};
use kartenn_service::pipeline::MAX_OPERATIONS;
use kartenn_service::{Operation, Outcome, Ref, submit};
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

use crate::support::env::{Env, messages, op, rules};

/// `operations` submitted by the author of `env` under `grants`.
async fn submit_with(env: &Env, grants: &Grants, operations: Vec<Operation>) -> Outcome {
    submit(
        &env.store,
        grants,
        &env.request(operations, None),
        MAX_OPERATIONS,
    )
    .await
    .unwrap()
}

/// The edit and the application of one's own changesets of the settings of `declarant`.
fn settings(env: &Env, grants: Grants, declarant: &str) -> Grants {
    grants
        .everywhere(env.author, &format!("{declarant}.settings.edit"))
        .everywhere(env.author, &format!("{declarant}.settings.approval.self"))
}

/// Applied, else a panic naming the outcome.
fn applied(outcome: &Outcome) -> Uuid {
    match outcome {
        Outcome::Applied { changeset, .. } => *changeset,
        other => panic!("not applied: {other:?}"),
    }
}

/// Refused as a write of an object without its permission, `permission` named.
fn refused_for(outcome: &Outcome, permission: &str) {
    assert_eq!(rules(outcome), ["core.object.forbidden"], "{permission}");
    assert!(
        messages(outcome).contains(permission),
        "{permission}: {}",
        messages(outcome)
    );
}

/// The closed-list value `key` added to the property `property`.
fn add_value(property: Uuid, key: &str) -> Operation {
    Operation::AddEnumValue {
        local: None,
        property: Ref::Id(property),
        key: key.into(),
        labels: RawLabels::new(),
    }
}

/// The administrator's field `custom.contrat`, a text carried by the addresses.
fn declare_contract() -> Operation {
    Operation::DeclareProperty {
        local: None,
        declaration: DeclarationInput {
            key: "custom.contrat".into(),
            value_type: TypeInput::Text {
                min_len: 0,
                max_len: 64,
                pattern: None,
            },
            cardinality: kartenn_core::values::Cardinality::Single,
            carriers: vec!["address".into()],
            inherit: None,
            module: None,
            obligations: Vec::new(),
            expected: None,
            labels: None,
        },
    }
}

#[kartenn_testing::pg_test]
async fn the_catalogue_is_that_of_s15(pool: PgPool) {
    let _env = Env::new(&pool).await;
    for name in [
        "policy.catalogue.read",
        "policy.verdict.read",
        "policy.flow.request",
        "policy.settings.edit",
        "policy.settings.apply",
        "policy.settings.approval.self",
        "policy.rule.admin",
        "policy.inference.admin",
    ] {
        assert!(kartenn_core::authz::permission(name).is_some(), "{name}");
    }
    // A role entry follows the naming rule of spec 018 (K12, R2, R5).
    let module = kartenn_service::module::Module::permissions(&kartenn_policy::Policy);
    assert!(
        kartenn_core::authz::catalogue_problems(module).is_empty(),
        "{:?}",
        kartenn_core::authz::catalogue_problems(module)
    );
    // The settings are global (spec 027, P5).
    for p in module.iter().filter(|p| p.name.contains(".settings.")) {
        assert!(p.global_only, "{}", p.name);
    }
    assert!(kartenn_core::authz::permission("policy.axis.write").is_none());
    assert!(kartenn_core::authz::permission("policy.matrix.admin").is_none());
    // Every object of the policy is read under one permission, written under its family's.
    assert_eq!(
        kartenn_service::reads::read_permission("policy.signal"),
        "policy.catalogue.read"
    );
    assert_eq!(
        kartenn_core::authz::type_edit_permission("policy.condition"),
        "policy.rule.admin"
    );
    assert_eq!(
        kartenn_core::authz::type_edit_permission("policy.value"),
        "policy.settings.edit"
    );
    // A type without declared permissions keeps the convention.
    assert_eq!(
        kartenn_core::authz::type_edit_permission("flow.flow"),
        "flow.edit"
    );
}

#[kartenn_testing::pg_test]
async fn a_family_is_administered_under_its_own_permission(pool: PgPool) {
    let env = Env::new(&pool).await;
    let flow_type = || {
        vec![op(
            "flow_type.create",
            json!({ "key": "adm", "labels": { "fr": "Adm" } }),
        )]
    };
    let rules_only = Grants::new().everywhere(env.author, "policy.rule.admin");
    let refused = submit_with(&env, &rules_only, flow_type()).await;
    refused_for(&refused, "policy.settings.edit");
    let admin = settings(&env, Grants::new(), "policy");
    applied(&submit_with(&env, &admin, flow_type()).await);
}

/// REQ-027-5: a security team with the settings of the policy creates an environment, a
/// service and a value of an axis; the settings of the machines and of the core stay out
/// of its reach, the missing permission named.
#[kartenn_testing::pg_test]
async fn acceptance_of_req_027_5_a_team_keeps_to_its_settings(pool: PgPool) {
    let env = Env::new(&pool).await;
    env.applied(vec![op("examples.install", json!({}))]).await;
    let team = settings(&env, Grants::new(), "policy");
    applied(
        &submit_with(
            &env,
            &team,
            vec![op(
                "environment.create",
                json!({ "key": "preprod", "labels": { "fr": "Préproduction" } }),
            )],
        )
        .await,
    );
    applied(
        &submit_with(
            &env,
            &team,
            vec![
                op(
                    "flow_type.create",
                    json!({ "local": 1, "key": "annuaire", "labels": { "fr": "Annuaire" } }),
                ),
                op(
                    "service.create",
                    json!({ "key": "ldaps", "labels": { "fr": "LDAPS" },
                            "flow_type": { "local": 1 },
                            "entries": [{ "transport": "tcp", "ports": 636 }] }),
                ),
            ],
        )
        .await,
    );
    let sensitivity = env.property("policy.sensitivity").await;
    applied(&submit_with(&env, &team, vec![add_value(sensitivity, "pci")]).await);
    applied(
        &submit_with(
            &env,
            &team,
            vec![op(
                "value.describe",
                json!({ "property": "policy.sensitivity", "value": "pci",
                        "description": "Données de cartes de paiement." }),
            )],
        )
        .await,
    );
    let media = env.property("machines.interface_media").await;
    refused_for(
        &submit_with(&env, &team, vec![add_value(media, "sfp28")]).await,
        "machines.settings.edit",
    );
    refused_for(
        &submit_with(&env, &team, vec![declare_contract()]).await,
        "core.settings.edit",
    );
}

/// REQ-027-5, 6: the settings of the core declare a `custom.*` field, not an environment
/// of the policy.
#[kartenn_testing::pg_test]
async fn acceptance_of_req_027_5_the_core_does_not_write_the_policy(pool: PgPool) {
    let env = Env::new(&pool).await;
    let core = settings(&env, Grants::new(), "core");
    refused_for(
        &submit_with(
            &env,
            &core,
            vec![op(
                "environment.create",
                json!({ "key": "recette", "labels": { "fr": "Recette" } }),
            )],
        )
        .await,
        "policy.settings.edit",
    );
    applied(&submit_with(&env, &core, vec![declare_contract()]).await);
}

/// REQ-027-6 (P6): the sensitivity of a property of a module is the core's.
#[kartenn_testing::pg_test]
async fn acceptance_of_req_027_6_sensitivity_is_the_cores(pool: PgPool) {
    let env = Env::new(&pool).await;
    let fqdn = env.property("machines.fqdn").await;
    let mark = || {
        vec![Operation::SetSensitivity {
            property: Ref::Id(fqdn),
            sensitive: true,
        }]
    };
    let machines = settings(&env, Grants::new(), "machines");
    refused_for(
        &submit_with(&env, &machines, mark()).await,
        "core.settings.edit",
    );
    let core = settings(&env, Grants::new(), "core");
    applied(&submit_with(&env, &core, mark()).await);
}

/// REQ-027-6: the settings of the machines relabel a value of their closed list, as an
/// audited changeset (REQ-018-9).
#[kartenn_testing::pg_test]
async fn acceptance_of_req_027_6_a_module_relabels_its_values(pool: PgPool) {
    let env = Env::new(&pool).await;
    let media = env.property("machines.interface_media").await;
    let relabel = || {
        vec![Operation::UpdateEnumValue {
            property: Ref::Id(media),
            key: "wifi".into(),
            labels: RawLabels::from([
                ("fr".into(), "Wi-Fi 6".into()),
                ("en".into(), "Wi-Fi 6".into()),
            ]),
        }]
    };
    let policy = settings(&env, Grants::new(), "policy");
    refused_for(
        &submit_with(&env, &policy, relabel()).await,
        "machines.settings.edit",
    );
    let machines = settings(&env, Grants::new(), "machines");
    let changeset = applied(&submit_with(&env, &machines, relabel()).await);
    let (label, by): (String, Uuid) = sqlx::query_as(
        "SELECT labels->>'fr', changeset_id FROM property_enum_value
          WHERE property_id = $1 AND key = 'wifi' AND valid_period @> now()",
    )
    .bind(media)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!((label.as_str(), by), ("Wi-Fi 6", changeset));
    let audited: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM audit.event
          WHERE name = 'changeset.state.applied' AND changeset_id = $1",
    )
    .bind(changeset)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(audited, 1);
}
