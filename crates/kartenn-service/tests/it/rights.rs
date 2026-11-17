//! Rights on properties: direct permission and permission on the effect (spec 003,
//! REQ-003-17, 18; task 003-11), one test per edge case of the design.

use kartenn_core::objects::{AddressUsage, Nature};
use kartenn_core::values::Cardinality;
use kartenn_service::authz::{Grants, Unrestricted};
use kartenn_service::operations::{DeclarationInput, RawInput, TypeInput};
use kartenn_service::pipeline::{MAX_OPERATIONS, PendingKind};
use kartenn_service::{ChangesetRequest, ObjectKind, Operation, Outcome, Ref, submit};
use kartenn_store_pg::write::PgStore;
use sqlx::PgPool;
use sqlx::types::Uuid;

const EDIT: &str = "prop.custom.environment.edit";
const OWN: &str = "prop.custom.environment.approval.self";

/// Edit and apply one's own changesets everywhere (spec 024): what `write` gave.
fn writer(who: Uuid) -> Grants {
    Grants::new().everywhere(who, EDIT).everywhere(who, OWN)
}

struct Fixture {
    pool: PgPool,
    store: PgStore,
    admin: Uuid,
    contractor: Uuid,
    domain: Uuid,
    env: Uuid,
}

impl Fixture {
    /// `10.0.0.0/8` in `{prod}`, `10.5.0.0/16` in `{dev}` with the network `10.5.0.0/24`
    /// and the address `10.5.0.1` below, written by an administrator.
    async fn new(pool: &PgPool) -> Self {
        let person = |name: &'static str| {
            sqlx::query_scalar(
                "INSERT INTO person (id, display_name) VALUES (gen_random_uuid(), $1) RETURNING id",
            )
            .bind(name)
            .fetch_one(pool)
        };
        let mut f = Self {
            pool: pool.clone(),
            store: PgStore::new(pool.clone()),
            admin: person("admin").await.unwrap(),
            contractor: person("contractor").await.unwrap(),
            domain: Uuid::nil(),
            env: Uuid::nil(),
        };
        let declaration = DeclarationInput {
            key: "custom.environment".into(),
            value_type: TypeInput::ENUM,
            cardinality: Cardinality::Set,
            carriers: vec!["prefix".into(), "address".into()],
            inherit: Some("addressing".into()),
            module: None,
            obligations: Vec::new(),
            expected: None,
            labels: None,
        };
        let assign = |local, value: &str| Operation::AssignProperty {
            object: Ref::Local(local),
            kind: ObjectKind::Prefix,
            property: Ref::Local(2),
            values: vec![RawInput::Text(value.into())],
        };
        let outcome = f
            .submit(
                &Unrestricted,
                f.admin,
                vec![
                    Operation::CreateDomain {
                        local: Some(1),
                        slug: "prod".into(),
                    },
                    Operation::DeclareProperty {
                        local: Some(2),
                        declaration,
                    },
                    Operation::AddEnumValue {
                        local: None,
                        property: Ref::Local(2),
                        key: "prod".into(),
                        labels: std::collections::BTreeMap::new(),
                    },
                    Operation::AddEnumValue {
                        local: None,
                        property: Ref::Local(2),
                        key: "dev".into(),
                        labels: std::collections::BTreeMap::new(),
                    },
                    prefix(3, Ref::Local(1), "10.0.0.0/8", Nature::Container),
                    prefix(4, Ref::Local(1), "10.5.0.0/16", Nature::Container),
                    prefix(5, Ref::Local(1), "10.5.0.0/24", Nature::Network),
                    Operation::CreateAddress {
                        local: None,
                        domain: Ref::Local(1),
                        address: "10.5.0.1".into(),
                        usage: AddressUsage::Host,
                    },
                    assign(3, "prod"),
                    assign(4, "dev"),
                ],
            )
            .await;
        assert!(matches!(outcome, Outcome::Applied { .. }), "{outcome:?}");
        f.domain = sqlx::query_scalar("SELECT id FROM addressing_domain")
            .fetch_one(pool)
            .await
            .unwrap();
        f.env = sqlx::query_scalar("SELECT id FROM property_declaration")
            .fetch_one(pool)
            .await
            .unwrap();
        f
    }

    async fn submit<A: kartenn_service::authz::Authorizer>(
        &self,
        authorizer: &A,
        author: Uuid,
        operations: Vec<Operation>,
    ) -> Outcome {
        let request = ChangesetRequest {
            author,
            motive: "test".into(),
            period: None,
            operations,
            simulate: false,
            idempotency_key: None,
            approval_self: true,
        };
        submit(&self.store, authorizer, &request, MAX_OPERATIONS)
            .await
            .unwrap()
    }

    /// Refused by the rights: the objects refused.
    /// `grants` plus the right to write the objects themselves (005) and to read them, so
    /// that a refusal names them (INV-63, task 002-29d4): these tests are about the rights
    /// on the properties and their effects.
    fn with_objects(&self, grants: &Grants) -> Grants {
        ["prefix", "range", "address"]
            .iter()
            .fold(grants.clone(), |g, kind| {
                g.everywhere(self.contractor, &format!("core.{kind}.edit"))
                    .everywhere(self.contractor, &format!("core.{kind}.approval.self"))
                    .everywhere(self.contractor, &format!("core.{kind}.read"))
            })
    }

    async fn forbidden(&self, grants: &Grants, operations: Vec<Operation>) -> Vec<Uuid> {
        let grants = self.with_objects(grants);
        match self.submit(&grants, self.contractor, operations).await {
            Outcome::Refused { violations, .. } => {
                assert_eq!(violations.len(), 1, "{violations:?}");
                assert_eq!(violations[0].rule.id, "prop.assignment.forbidden_effect");
                let mut subjects = violations[0].subjects.clone();
                subjects.sort();
                subjects
            }
            other => panic!("expected refused, got {other:?}"),
        }
    }

    async fn applied(&self, grants: &Grants, operations: Vec<Operation>) {
        let outcome = self
            .submit(&self.with_objects(grants), self.contractor, operations)
            .await;
        assert!(matches!(outcome, Outcome::Applied { .. }), "{outcome:?}");
    }

    async fn id(&self, value: &str) -> Uuid {
        sqlx::query_scalar(
            "SELECT id FROM addressing_object WHERE value = $1::inet AND kind <> 'domain' LIMIT 1",
        )
        .bind(value)
        .fetch_one(&self.pool)
        .await
        .unwrap()
    }

    fn assign(&self, object: Ref, kind: ObjectKind, value: &str) -> Operation {
        Operation::AssignProperty {
            object,
            kind,
            property: Ref::Id(self.env),
            values: vec![RawInput::Text(value.into())],
        }
    }
}

fn prefix(local: u32, domain: Ref, value: &str, nature: Nature) -> Operation {
    Operation::CreatePrefix {
        local: Some(local),
        domain,
        prefix: value.into(),
        nature,
        point_to_point: false,
        anycast: false,
    }
}

fn sorted(mut ids: Vec<Uuid>) -> Vec<Uuid> {
    ids.sort();
    ids
}

#[kartenn_testing::pg_test]
async fn an_assignment_needs_the_property_right_on_the_object_and_its_effects(pool: PgPool) {
    let f = Fixture::new(&pool).await;
    let net = f.id("10.5.0.0/24").await;
    let addr = f.id("10.5.0.1").await;
    // REQ-003-17: allowed to touch the network, but not the property → refused.
    let none = Grants::new();
    assert_eq!(
        f.forbidden(
            &none,
            vec![f.assign(Ref::Id(net), ObjectKind::Prefix, "prod")]
        )
        .await,
        sorted(vec![net, addr])
    );
    // The right on the network alone does not cover the address whose value changes.
    let network_only = Grants::new().on(f.contractor, EDIT, [net]);
    assert_eq!(
        f.forbidden(
            &network_only,
            vec![f.assign(Ref::Id(net), ObjectKind::Prefix, "prod")]
        )
        .await,
        vec![addr]
    );
    let both = Grants::new()
        .on(f.contractor, EDIT, [net, addr])
        .on(f.contractor, OWN, [net, addr]);
    f.applied(
        &both,
        vec![f.assign(Ref::Id(net), ObjectKind::Prefix, "prod")],
    )
    .await;
}

#[kartenn_testing::pg_test]
async fn structure_changes_need_the_right_only_when_values_change(pool: PgPool) {
    let f = Fixture::new(&pool).await;
    let none = Grants::new();
    // Edge case: a container without assignment between `{prod}` and `{dev}` changes no
    // resolved value → no right needed (REQ-003-18 acceptance).
    f.applied(
        &none,
        vec![prefix(
            1,
            Ref::Id(f.domain),
            "10.5.0.0/20",
            Nature::Container,
        )],
    )
    .await;
    // A new address inheriting `{dev}` is a new object → accepted.
    f.applied(
        &none,
        vec![Operation::CreateAddress {
            local: None,
            domain: Ref::Id(f.domain),
            address: "10.5.0.2".into(),
            usage: AddressUsage::Host,
        }],
    )
    .await;
    // Edge case: closing the `{dev}` container under `{prod}` moves everything below to
    // `{prod}` → refused, listing them.
    let dev = f.id("10.5.0.0/16").await;
    let close = || {
        vec![Operation::Close {
            kind: ObjectKind::Prefix,
            id: Ref::Id(dev),
            expected: None,
            cascade: false,
        }]
    };
    let below = sorted(vec![
        f.id("10.5.0.0/20").await,
        f.id("10.5.0.0/24").await,
        f.id("10.5.0.1").await,
        f.id("10.5.0.2").await,
    ]);
    assert_eq!(f.forbidden(&none, close()).await, below);
    // With the right to edit only, a proposal (spec 024, REQ-024-10).
    let propose = Grants::new().everywhere(f.contractor, EDIT);
    match f
        .submit(&f.with_objects(&propose), f.contractor, close())
        .await
    {
        Outcome::Pending { kind, .. } => assert_eq!(kind, PendingKind::Proposal),
        other => panic!("expected a proposal, got {other:?}"),
    }
    f.applied(&writer(f.contractor), close()).await;
}

#[kartenn_testing::pg_test]
async fn new_objects_need_the_direct_right_only(pool: PgPool) {
    let f = Fixture::new(&pool).await;
    // Edge case: a changeset creating a network and its addresses and assigning the
    // network: no effect check on new objects, but the assignment needs the right.
    let operations = || {
        vec![
            prefix(1, Ref::Id(f.domain), "10.6.0.0/24", Nature::Network),
            Operation::CreateAddress {
                local: None,
                domain: Ref::Id(f.domain),
                address: "10.6.0.1".into(),
                usage: AddressUsage::Host,
            },
            f.assign(Ref::Local(1), ObjectKind::Prefix, "dev"),
        ]
    };
    let refused = f.forbidden(&Grants::new(), operations()).await;
    assert_eq!(refused.len(), 1, "only the assigned network");
    f.applied(&writer(f.contractor), operations()).await;
}

/// The right to write the prefixes, ranges and addresses, to apply them, and to read the
/// prefixes and ranges only: the addresses are out of the contractor's reading.
fn blind_to_addresses(f: &Fixture, grants: Grants) -> Grants {
    ["prefix", "range", "address"]
        .iter()
        .fold(grants, |g, kind| {
            let g = g
                .everywhere(f.contractor, &format!("core.{kind}.edit"))
                .everywhere(f.contractor, &format!("core.{kind}.approval.self"));
            if *kind == "address" {
                g
            } else {
                g.everywhere(f.contractor, &format!("core.{kind}.read"))
            }
        })
}

/// A second address below the network, out of the contractor's reading as the first.
async fn another_address(f: &Fixture) {
    let outcome = f
        .submit(
            &Unrestricted,
            f.admin,
            vec![Operation::CreateAddress {
                local: None,
                domain: Ref::Id(f.domain),
                address: "10.5.0.7".into(),
                usage: AddressUsage::Host,
            }],
        )
        .await;
    assert!(matches!(outcome, Outcome::Applied { .. }), "{outcome:?}");
}

/// A violation whole: rule, message and objects.
fn whole(v: &kartenn_core::rules::Violation) -> (&'static str, String, Vec<Uuid>) {
    (v.rule.id, v.message.clone(), sorted(v.subjects.clone()))
}

/// INV-63 (task 002-29d6, point 1): the refusal of the right on the effects of an
/// assignment names the objects its author reads, and counts only them: one who writes a
/// property on a prefix he reads does not learn how many addresses out of his reading it
/// reaches. One address hidden or two, the same refusal.
#[kartenn_testing::pg_test]
async fn a_refusal_counts_only_the_objects_its_author_reads(pool: PgPool) {
    let f = Fixture::new(&pool).await;
    let net = f.id("10.5.0.0/24").await;
    let assign = || vec![f.assign(Ref::Id(net), ObjectKind::Prefix, "prod")];
    let refusals = |grants: Grants| {
        let grants = blind_to_addresses(&f, grants);
        let operations = assign();
        let f = &f;
        async move {
            match f.submit(&grants, f.contractor, operations).await {
                Outcome::Refused { violations, .. } => {
                    violations.iter().map(whole).collect::<Vec<_>>()
                }
                other => panic!("expected refused, got {other:?}"),
            }
        }
    };
    let network_only = || Grants::new().on(f.contractor, EDIT, [net]);
    let (hidden, named) = (
        refusals(network_only()).await,
        refusals(Grants::new()).await,
    );
    assert_eq!(
        hidden,
        [(
            "prop.assignment.forbidden_effect",
            format!("objects you do not read need {EDIT}"),
            Vec::new()
        )]
    );
    assert_eq!(
        named,
        [(
            "prop.assignment.forbidden_effect",
            format!("1 object(s) need {EDIT}"),
            vec![net]
        )]
    );
    another_address(&f).await;
    assert_eq!(
        refusals(network_only()).await,
        hidden,
        "two addresses hidden"
    );
    assert_eq!(refusals(Grants::new()).await, named, "two addresses hidden");
}

/// INV-63 (task 002-29d6, point 2): the reason a changeset its author asked to apply is
/// held for (`changeset.approval.self.missing`), answered at its submission and stored for
/// its replays, names only the objects its author reads, and counts only them; the rights
/// it requires keep every object, for whoever applies it (REQ-024-11).
#[kartenn_testing::pg_test]
async fn the_reason_a_changeset_is_held_for_names_only_what_its_author_reads(pool: PgPool) {
    let f = Fixture::new(&pool).await;
    let net = f.id("10.5.0.0/24").await;
    let addr = f.id("10.5.0.1").await;
    let grants = blind_to_addresses(
        &f,
        Grants::new()
            .everywhere(f.contractor, EDIT)
            .on(f.contractor, OWN, [net]),
    );
    let held = || async {
        let outcome = f
            .submit(
                &grants,
                f.contractor,
                vec![f.assign(Ref::Id(net), ObjectKind::Prefix, "prod")],
            )
            .await;
        let Outcome::Pending {
            changeset,
            reason: Some(reason),
            ..
        } = outcome
        else {
            panic!("expected a pending changeset with its reason, got {outcome:?}")
        };
        let stored: serde_json::Value =
            sqlx::query_scalar("SELECT required_rights -> 'reason' FROM changeset WHERE id = $1")
                .bind(changeset)
                .fetch_one(&f.pool)
                .await
                .unwrap();
        let required: serde_json::Value =
            sqlx::query_scalar("SELECT required_rights -> 'required' FROM changeset WHERE id = $1")
                .bind(changeset)
                .fetch_one(&f.pool)
                .await
                .unwrap();
        (whole(&reason), stored, required)
    };
    let (reason, stored, required) = held().await;
    let expected = (
        "changeset.approval.self.missing",
        format!("its author lacks approval.self: objects you do not read need {OWN}"),
        Vec::new(),
    );
    assert_eq!(reason, expected);
    assert_eq!(
        stored,
        serde_json::json!({
            "code": expected.0, "message": expected.1, "objects": []
        })
    );
    assert!(
        required.to_string().contains(&addr.to_string()),
        "the rights required keep the address: {required}"
    );
    another_address(&f).await;
    let (again, stored_again, _) = held().await;
    assert_eq!(
        (again, stored_again),
        (reason, stored),
        "two addresses hidden"
    );
}

/// INV-63 (task 002-29d6, point 3): the validator who lacks the application of objects he
/// does not read is refused naming none of them, nor counting them; the changeset stays
/// pending, its rights required whole.
#[kartenn_testing::pg_test]
async fn a_validator_is_told_only_the_objects_he_reads(pool: PgPool) {
    let f = Fixture::new(&pool).await;
    let net = f.id("10.5.0.0/24").await;
    let validator = f.admin;
    let author = blind_to_addresses(&f, Grants::new().everywhere(f.contractor, EDIT));
    let request = ChangesetRequest {
        author: f.contractor,
        motive: "test".into(),
        period: None,
        operations: vec![f.assign(Ref::Id(net), ObjectKind::Prefix, "prod")],
        simulate: false,
        idempotency_key: None,
        approval_self: false,
    };
    let Outcome::Pending { changeset, .. } = submit(&f.store, &author, &request, MAX_OPERATIONS)
        .await
        .unwrap()
    else {
        panic!("expected a proposal")
    };
    let refused = || async {
        let grants = author
            .clone()
            .on(validator, "prop.custom.environment.apply", [net])
            .everywhere(validator, "core.prefix.read");
        match kartenn_service::review::validate_pending(
            &f.store,
            &grants,
            &kartenn_service::expected::Sources::none(),
            changeset,
            &validator.into(),
            MAX_OPERATIONS,
        )
        .await
        {
            Err(kartenn_service::RequestError::Review(v)) => {
                let mut v = whole(&v);
                v.1 = v.1.replace(&validator.to_string(), "<validator>");
                v
            }
            other => panic!("expected a refused validation, got {other:?}"),
        }
    };
    let first = refused().await;
    assert_eq!(
        first,
        (
            "changeset.approval.forbidden",
            "<validator> lacks the right to validate on objects it does not read".to_owned(),
            Vec::new()
        )
    );
    // The same once a second address is hidden below the network: a new proposal.
    another_address(&f).await;
    assert_eq!(refused().await, first);
}

/// Task 002-29d6, point 5: a refusal names the objects in force over the period of its
/// changeset, not now: an address that lives from tomorrow, which the contractor reads
/// but may not close, is named.
#[kartenn_testing::pg_test]
async fn a_refusal_names_the_objects_in_force_at_its_period(pool: PgPool) {
    let f = Fixture::new(&pool).await;
    let now = i64::try_from(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_micros(),
    )
    .unwrap();
    let day = 86_400_000_000;
    let at = |start: i64, author: Uuid, operations: Vec<Operation>| ChangesetRequest {
        author,
        motive: "test".into(),
        period: Some(kartenn_service::operations::PeriodSpec { start, end: None }),
        operations,
        simulate: false,
        idempotency_key: None,
        approval_self: true,
    };
    let created = submit(
        &f.store,
        &Unrestricted,
        &at(
            now + day,
            f.admin,
            vec![Operation::CreateAddress {
                local: None,
                domain: Ref::Id(f.domain),
                address: "10.5.0.9".into(),
                usage: AddressUsage::Host,
            }],
        ),
        MAX_OPERATIONS,
    )
    .await
    .unwrap();
    assert!(matches!(created, Outcome::Applied { .. }), "{created:?}");
    let tomorrow = f.id("10.5.0.9").await;
    let reader = Grants::new().everywhere(f.contractor, "core.address.read");
    let close = at(
        now + 2 * day,
        f.contractor,
        vec![Operation::Close {
            kind: ObjectKind::Address,
            id: Ref::Id(tomorrow),
            expected: None,
            cascade: false,
        }],
    );
    match submit(&f.store, &reader, &close, MAX_OPERATIONS)
        .await
        .unwrap()
    {
        Outcome::Refused { violations, .. } => assert_eq!(
            violations.iter().map(whole).collect::<Vec<_>>(),
            [(
                "core.object.forbidden",
                "1 object(s) need core.address.edit".to_owned(),
                vec![tomorrow]
            )]
        ),
        other => panic!("expected refused, got {other:?}"),
    }
}
