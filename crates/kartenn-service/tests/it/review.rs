//! Pending changesets: approval policies, proposals and their decisions (spec 004,
//! REQ-004-19 to 24, 39 to 43; tasks 004-11 to 14).

use kartenn_core::objects::Nature;
use kartenn_core::values::Cardinality;
use kartenn_service::audit::RequestContext;
use kartenn_service::authz::{Authorizer, Grants, Subject, Unrestricted};
use kartenn_service::declarations::{ModuleDeclarations, SYSTEM_AUTHOR, sync_modules};
use kartenn_service::expected::Sources;
use kartenn_service::operations::{DeclarationInput, RawInput, TypeInput};
use kartenn_service::pipeline::{MAX_OPERATIONS, PendingKind};
use kartenn_service::review::{expire_pending, proposals_for, reject, validate_pending, withdraw};
use kartenn_service::{
    ChangesetRequest, ObjectKind, Operation, Outcome, Ref, RequestError, submit, submit_in,
};
use kartenn_store_pg::write::PgStore;
use sqlx::PgPool;
use sqlx::types::Uuid;

struct Fixture {
    pool: PgPool,
    store: PgStore,
    author: Uuid,
    approver: Uuid,
    domain: Uuid,
    network: Uuid,
    env: Uuid,
}

async fn person(pool: &PgPool, name: &str) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO person (id, display_name) VALUES (gen_random_uuid(), $1) RETURNING id",
    )
    .bind(name)
    .fetch_one(pool)
    .await
    .unwrap()
}

impl Fixture {
    /// Domain `lab`, network `10.1.0.0/20` with 600 addresses, `custom.environment`
    /// (closed list `prod`, `dev`, inherited along the addressing tree).
    async fn new(pool: &PgPool) -> Self {
        let mut f = Self {
            pool: pool.clone(),
            store: PgStore::new(pool.clone()),
            author: person(pool, "author").await,
            approver: person(pool, "approver").await,
            domain: Uuid::nil(),
            network: Uuid::nil(),
            env: Uuid::nil(),
        };
        let declaration = DeclarationInput {
            key: "custom.environment".into(),
            value_type: TypeInput::ENUM,
            cardinality: Cardinality::Single,
            carriers: vec!["prefix".into(), "address".into()],
            inherit: Some("addressing".into()),
            module: None,
            obligations: Vec::new(),
            expected: None,
            labels: None,
        };
        f.applied(vec![
            Operation::DeclareProperty {
                local: Some(1),
                declaration,
            },
            Operation::AddEnumValue {
                local: None,
                property: Ref::Local(1),
                key: "prod".into(),
                labels: std::collections::BTreeMap::new(),
            },
            Operation::AddEnumValue {
                local: None,
                property: Ref::Local(1),
                key: "dev".into(),
                labels: std::collections::BTreeMap::new(),
            },
            Operation::CreateDomain {
                local: Some(2),
                slug: "lab".into(),
            },
            Operation::CreatePrefix {
                local: None,
                domain: Ref::Local(2),
                prefix: "10.1.0.0/20".into(),
                nature: Nature::Network,
                point_to_point: false,
                anycast: false,
            },
        ])
        .await;
        f.domain = sqlx::query_scalar("SELECT id FROM addressing_domain")
            .fetch_one(pool)
            .await
            .unwrap();
        f.network = sqlx::query_scalar("SELECT id FROM prefix")
            .fetch_one(pool)
            .await
            .unwrap();
        f.env = sqlx::query_scalar("SELECT id FROM property_declaration")
            .fetch_one(pool)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO address (id, domain_id, address, usage, public, registry_version, valid_period, changeset_id)
             SELECT gen_random_uuid(), $1, '10.1.0.0'::inet + g, 'host', false, 'x',
                    tstzrange(now() - interval '1 day', NULL), gen_random_uuid()
             FROM generate_series(1, 600) g",
        )
        .bind(f.domain)
        .execute(pool)
        .await
        .unwrap();
        f
    }

    async fn submit_as(&self, authorizer: &impl Authorizer, operations: Vec<Operation>) -> Outcome {
        submit(
            &self.store,
            authorizer,
            &request(self.author, operations),
            MAX_OPERATIONS,
        )
        .await
        .unwrap()
    }

    /// A new group of kartenn whose only member is the author, by key (spec 025: a policy
    /// designates its authors by group, REQ-025-13).
    async fn author_group(&self) -> String {
        sqlx::query_scalar(kartenn_testing::GROUP_OF)
            .bind(self.author)
            .fetch_one(&self.pool)
            .await
            .unwrap()
    }

    async fn applied(&self, operations: Vec<Operation>) -> Uuid {
        match self.submit_as(&Unrestricted, operations).await {
            Outcome::Applied { changeset, .. } => changeset,
            other => panic!("expected applied, got {other:?}"),
        }
    }

    async fn pending(
        &self,
        authorizer: &impl Authorizer,
        operations: Vec<Operation>,
    ) -> (Uuid, PendingKind) {
        match self.submit_as(authorizer, operations).await {
            Outcome::Pending {
                changeset, kind, ..
            } => (changeset, kind),
            other => panic!("expected pending, got {other:?}"),
        }
    }

    fn assign(&self, object: Uuid, kind: ObjectKind, value: &str) -> Operation {
        Operation::AssignProperty {
            object: Ref::Id(object),
            kind,
            property: Ref::Id(self.env),
            values: vec![RawInput::Text(value.into())],
        }
    }

    async fn state(&self, changeset: Uuid) -> String {
        sqlx::query_scalar("SELECT state FROM changeset WHERE id = $1")
            .bind(changeset)
            .fetch_one(&self.pool)
            .await
            .unwrap()
    }

    async fn events(&self, changeset: Uuid) -> Vec<String> {
        sqlx::query_scalar(
            "SELECT payload->>'type' FROM outbox WHERE changeset_id = $1 ORDER BY seq",
        )
        .bind(changeset)
        .fetch_all(&self.pool)
        .await
        .unwrap()
    }

    async fn validate(
        &self,
        authorizer: &impl Authorizer,
        id: Uuid,
        validator: Uuid,
    ) -> Result<Outcome, RequestError> {
        validate_pending(
            &self.store,
            authorizer,
            &Sources::none(),
            id,
            &validator.into(),
            MAX_OPERATIONS,
        )
        .await
    }
}

fn rule(result: Result<impl std::fmt::Debug, RequestError>) -> &'static str {
    match result {
        Err(RequestError::Review(v)) => v.rule.id,
        other => panic!("expected a refused review, got {other:?}"),
    }
}

fn policy(name: &str, min_diff_objects: u64) -> Operation {
    Operation::SetApprovalPolicy {
        name: name.into(),
        min_operations: None,
        min_diff_objects: Some(min_diff_objects),
        property_keys: Vec::new(),
        domains: Vec::new(),
        scopes: Vec::new(),
        author_roles: Vec::new(),
        author_groups: Vec::new(),
        bound_roles: Vec::new(),
        module_triggers: Vec::new(),
        enabled: true,
    }
}

#[kartenn_testing::pg_test]
async fn a_policy_holds_large_changesets_for_an_approver(pool: PgPool) {
    let f = Fixture::new(&pool).await;
    // REQ-004-39: no policy, no wait.
    f.applied(vec![f.assign(f.network, ObjectKind::Prefix, "dev")])
        .await;
    // REQ-004-40: a policy needs a trigger.
    match f
        .submit_as(
            &Unrestricted,
            vec![Operation::SetApprovalPolicy {
                name: "empty".into(),
                min_operations: None,
                min_diff_objects: None,
                property_keys: Vec::new(),
                domains: Vec::new(),
                scopes: Vec::new(),
                author_roles: Vec::new(),
                author_groups: Vec::new(),
                bound_roles: Vec::new(),
                module_triggers: Vec::new(),
                enabled: true,
            }],
        )
        .await
    {
        Outcome::Refused { violations, .. } => {
            assert_eq!(violations[0].rule.id, "changeset.policy.invalid");
        }
        other => panic!("{other:?}"),
    }
    // Only an administrator of the policies sets one.
    match f
        .submit_as(&Grants::new(), vec![policy("large", 500)])
        .await
    {
        Outcome::Refused { violations, .. } => {
            assert_eq!(violations[0].rule.id, "changeset.policy.forbidden");
        }
        other => panic!("{other:?}"),
    }
    let admin = Grants::new().everywhere(f.author, kartenn_service::authz::POLICY_ADMIN);
    assert!(matches!(
        f.submit_as(&admin, vec![policy("large", 500)]).await,
        Outcome::Applied { .. }
    ));
    // REQ-004-20: 601 objects change → pending; nothing written.
    let (big, kind) = f
        .pending(
            &Unrestricted,
            vec![f.assign(f.network, ObjectKind::Prefix, "prod")],
        )
        .await;
    assert_eq!(kind, PendingKind::Approval);
    assert_eq!(f.state(big).await, "pending");
    assert_eq!(f.events(big).await, vec!["org.kartenn.changeset.pending"]);
    // A single address with its own value: 1 object, applied at once.
    let one: Uuid = sqlx::query_scalar("SELECT id FROM address LIMIT 1")
        .fetch_one(&pool)
        .await
        .unwrap();
    f.applied(vec![f.assign(one, ObjectKind::Address, "prod")])
        .await;
    // REQ-024-11, 12: never the author, even holding everything, and the application of
    // everything it touches; the author's own rights are checked again.
    let author_rights = Grants::new().everywhere(f.author, "prop.custom.environment.edit");
    assert_eq!(
        rule(f.validate(&Unrestricted, big, f.author).await),
        "changeset.approval.self"
    );
    assert_eq!(
        rule(f.validate(&author_rights, big, f.approver).await),
        "changeset.approval.forbidden"
    );
    assert_eq!(f.state(big).await, "pending");
    let approver = author_rights.everywhere(f.approver, "prop.custom.environment.apply");
    match f.validate(&approver, big, f.approver).await.unwrap() {
        Outcome::Applied { changeset, .. } => assert_eq!(changeset, big),
        other => panic!("{other:?}"),
    }
    assert_eq!(f.state(big).await, "applied");
    let validator: Option<Uuid> =
        sqlx::query_scalar("SELECT validator_id FROM changeset WHERE id = $1")
            .bind(big)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(validator, Some(f.approver));
    // REQ-004-24: decided once.
    assert_eq!(
        rule(f.validate(&approver, big, f.approver).await),
        "changeset.state.not_pending"
    );
}

#[kartenn_testing::pg_test]
async fn a_pending_changeset_is_revalidated_rejected_withdrawn_or_expired(pool: PgPool) {
    let f = Fixture::new(&pool).await;
    f.applied(vec![Operation::SetApprovalPolicy {
        name: "everything".into(),
        min_operations: Some(1),
        min_diff_objects: None,
        property_keys: Vec::new(),
        domains: Vec::new(),
        scopes: Vec::new(),
        author_roles: Vec::new(),
        author_groups: vec![f.author_group().await],
        bound_roles: Vec::new(),
        module_triggers: Vec::new(),
        enabled: true,
    }])
    .await;
    let approver = Grants::new()
        .everywhere(f.approver, "core.prefix.apply")
        .everywhere(f.approver, "core.address.apply");
    let container = || Operation::CreatePrefix {
        local: None,
        domain: Ref::Id(f.domain),
        prefix: "10.0.0.0/8".into(),
        nature: Nature::Container,
        point_to_point: false,
        anycast: false,
    };
    // REQ-004-22: someone else creates the same prefix meanwhile.
    let (held, _) = f.pending(&Unrestricted, vec![container()]).await;
    let other = submit(
        &f.store,
        &Unrestricted,
        &request(f.approver, vec![container()]),
        MAX_OPERATIONS,
    )
    .await
    .unwrap();
    assert!(matches!(other, Outcome::Applied { .. }), "{other:?}");
    match f.validate(&approver, held, f.approver).await.unwrap() {
        Outcome::FailedRevalidation { violations, .. } => {
            assert_eq!(violations[0].rule.id, "core.prefix.duplicate");
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(f.state(held).await, "failed_revalidation");

    // REQ-004-23: rejection with a motive, given back.
    let address = |value: &str| Operation::CreateAddress {
        local: None,
        domain: Ref::Id(f.domain),
        address: value.into(),
        usage: kartenn_core::objects::AddressUsage::Host,
    };
    let (rejected, _) = f.pending(&Unrestricted, vec![address("10.1.15.1")]).await;
    assert!(matches!(
        reject(&f.store, &approver, rejected, &f.approver.into(), " ").await,
        Err(RequestError::MotiveRequired)
    ));
    reject(
        &f.store,
        &approver,
        rejected,
        &f.approver.into(),
        "not this week",
    )
    .await
    .unwrap();
    let rejection: Option<String> =
        sqlx::query_scalar("SELECT rejection FROM changeset WHERE id = $1")
            .bind(rejected)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(rejection.as_deref(), Some("not this week"));
    assert_eq!(
        f.events(rejected).await,
        vec![
            "org.kartenn.changeset.pending",
            "org.kartenn.changeset.rejected"
        ]
    );
    // Withdrawal: by its author only.
    let (withdrawn, _) = f.pending(&Unrestricted, vec![address("10.1.15.2")]).await;
    assert_eq!(
        rule(withdraw(&f.store, withdrawn, f.approver).await),
        "changeset.state.not_author"
    );
    withdraw(&f.store, withdrawn, f.author).await.unwrap();
    assert_eq!(f.state(withdrawn).await, "withdrawn");
    // Expiration past the deadline.
    let (late, _) = f.pending(&Unrestricted, vec![address("10.1.15.3")]).await;
    let (on_time, _) = f.pending(&Unrestricted, vec![address("10.1.15.4")]).await;
    sqlx::query("UPDATE changeset SET expires_at = now() - interval '1 second' WHERE id = $1")
        .bind(late)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(expire_pending(&f.store).await.unwrap(), vec![late]);
    assert_eq!(f.state(late).await, "expired");
    assert_eq!(f.state(on_time).await, "pending");
    // Nothing of them was written.
    let written: i64 =
        sqlx::query_scalar("SELECT count(*) FROM address WHERE address <<= '10.1.15.0/24'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(written, 0);
    // The system author is never held (P12).
    let module = ModuleDeclarations {
        module: "policy".into(),
        version: "1".into(),
        properties: Vec::new(),
        labels: std::collections::BTreeMap::new(),
    };
    sync_modules(&f.store, &Unrestricted, &[module], MAX_OPERATIONS)
        .await
        .unwrap();
    let held_system: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM changeset WHERE author_id = $1 AND state = 'pending'",
    )
    .bind(SYSTEM_AUTHOR)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(held_system, 0);
}

#[kartenn_testing::pg_test]
async fn an_author_who_may_only_edit_proposes(pool: PgPool) {
    let f = Fixture::new(&pool).await;
    let maintainer = person(&pool, "maintainer").await;
    let outsider = person(&pool, "outsider").await;
    let vm: Uuid = sqlx::query_scalar("SELECT id FROM address ORDER BY address LIMIT 1")
        .fetch_one(&pool)
        .await
        .unwrap();
    let apply = "prop.custom.environment.apply";
    let edit = "prop.custom.environment.edit";
    // REQ-024-10: without edit → refused.
    match f
        .submit_as(
            &Grants::new(),
            vec![f.assign(vm, ObjectKind::Address, "dev")],
        )
        .await
    {
        Outcome::Refused { violations, .. } => {
            assert_eq!(violations[0].rule.id, "prop.assignment.forbidden_effect");
        }
        other => panic!("{other:?}"),
    }
    let rights = Grants::new()
        .on(f.author, edit, [vm])
        .on(maintainer, apply, [vm]);
    let (proposal, kind) = f
        .pending(&rights, vec![f.assign(vm, ObjectKind::Address, "dev")])
        .await;
    assert_eq!(kind, PendingKind::Proposal);
    // REQ-004-43: in the queue of who can validate it only.
    let (store, rights_ref) = (&f.store, &rights);
    let queue = |who: Uuid| async move { proposals_for(store, rights_ref, &who.into()).await };
    assert_eq!(
        queue(maintainer)
            .await
            .unwrap()
            .iter()
            .map(|p| p.id)
            .collect::<Vec<_>>(),
        vec![proposal]
    );
    assert!(queue(outsider).await.unwrap().is_empty());
    assert!(queue(f.author).await.unwrap().is_empty());
    // REQ-024-11: not the author without apply_own, not without the apply right.
    assert_eq!(
        rule(f.validate(&rights, proposal, f.author).await),
        "changeset.apply_own.missing"
    );
    assert_eq!(
        rule(f.validate(&rights, proposal, outsider).await),
        "changeset.approval.forbidden"
    );
    assert!(matches!(
        f.validate(&rights, proposal, maintainer).await.unwrap(),
        Outcome::Applied { .. }
    ));
    let value: Option<String> = sqlx::query_scalar(
        "SELECT e.key FROM property_assignment a JOIN property_enum_value e ON e.id = a.v_uuid
         WHERE a.object_id = $1",
    )
    .bind(vm)
    .fetch_optional(&pool)
    .await
    .unwrap();
    assert_eq!(value.as_deref(), Some("dev"));
    // The author is told: the applied event names them.
    let author: Option<String> = sqlx::query_scalar(
        "SELECT payload->'data'->>'author' FROM outbox
         WHERE changeset_id = $1 AND payload->>'type' = 'org.kartenn.changeset.applied'",
    )
    .bind(proposal)
    .fetch_optional(&pool)
    .await
    .unwrap();
    assert_eq!(author, Some(f.author.to_string()));
}

fn request(author: Uuid, operations: Vec<Operation>) -> ChangesetRequest {
    ChangesetRequest {
        author,
        motive: "test".into(),
        period: None,
        operations,
        simulate: false,
        idempotency_key: None,
        apply_own: true,
    }
}

#[kartenn_testing::pg_test]
async fn an_approval_checks_the_author_with_the_groups_it_had(pool: PgPool) {
    // REQ-005-22: the author's groups live in its token; the pending changeset keeps them
    // so that the approval checks the author's rights again with them.
    let f = Fixture::new(&pool).await;
    let admin = Grants::new().everywhere(f.author, kartenn_service::authz::POLICY_ADMIN);
    assert!(matches!(
        f.submit_as(&admin, vec![policy("large", 500)]).await,
        Outcome::Applied { .. }
    ));
    let rights = Grants::new()
        .group(
            "https://idp.example.invalid",
            "netops",
            "prop.custom.environment.edit",
        )
        .everywhere(f.approver, "prop.custom.environment.apply");
    let author = Subject {
        id: f.author,
        issuer: Some("https://idp.example.invalid".into()),
        groups: vec!["netops".into()],
    };
    let outcome = submit_in(
        &f.store,
        &rights,
        &Sources::none(),
        &RequestContext::of(&author),
        &request(
            f.author,
            vec![f.assign(f.network, ObjectKind::Prefix, "prod")],
        ),
        MAX_OPERATIONS,
    )
    .await
    .unwrap();
    let Outcome::Pending {
        changeset: big,
        kind: PendingKind::Approval,
        ..
    } = outcome
    else {
        panic!("{outcome:?}");
    };
    let kept: Option<String> = sqlx::query_scalar(
        "SELECT required_rights -> 'author' -> 'groups' ->> 0 FROM changeset WHERE id = $1",
    )
    .bind(big)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(kept.as_deref(), Some("netops"), "the groups are kept");
    // The approver holds no group: the author's rights come from the snapshot.
    match f.validate(&rights, big, f.approver).await.unwrap() {
        Outcome::Applied { changeset, .. } => assert_eq!(changeset, big),
        other => panic!("{other:?}"),
    }
}

/// Spec 024, REQ-024-10 to 13: the matrix of the application, author or someone else ×
/// `apply` or `apply_own` × a policy holding the changeset or not.
#[kartenn_testing::pg_test]
async fn the_application_follows_the_author_and_the_policies(pool: PgPool) {
    let f = Fixture::new(&pool).await;
    let other = person(&pool, "other").await;
    let ids: Vec<Uuid> = sqlx::query_scalar("SELECT id FROM address ORDER BY address LIMIT 8")
        .fetch_all(&pool)
        .await
        .unwrap();
    let edit = "prop.custom.environment.edit";
    let apply = "prop.custom.environment.apply";
    let own = "prop.custom.environment.apply_own";
    let submit_with = |rights: Grants, object: Uuid, apply_own: bool| {
        let f = &f;
        async move {
            let mut request = request(f.author, vec![f.assign(object, ObjectKind::Address, "dev")]);
            request.apply_own = apply_own;
            submit(&f.store, &rights, &request, MAX_OPERATIONS)
                .await
                .unwrap()
        }
    };
    let audited = |changeset: Uuid| {
        let pool = pool.clone();
        async move {
            sqlx::query_scalar::<_, String>(
                "SELECT name FROM audit.event WHERE changeset_id = $1 ORDER BY seq",
            )
            .bind(changeset)
            .fetch_all(&pool)
            .await
            .unwrap()
        }
    };
    let pending = |outcome: Outcome| match outcome {
        Outcome::Pending {
            changeset, reason, ..
        } => (changeset, reason.map(|r| r.rule.id)),
        other => panic!("expected pending, got {other:?}"),
    };

    // REQ-024-10: submitted under edit, pending; nothing applies on its own.
    let editor = Grants::new().everywhere(f.author, edit);
    let (held, reason) = pending(submit_with(editor.clone(), ids[0], false).await);
    assert_eq!(reason, None);
    // REQ-024-13: asked to apply without apply_own → pending, with its reason.
    let (refused, reason) = pending(submit_with(editor.clone(), ids[1], true).await);
    assert_eq!(reason, Some("changeset.apply_own.missing"));
    assert_eq!(f.state(refused).await, "pending");
    // REQ-024-13: under apply_own, applied in the same request, audited twice, its author
    // as its validator.
    let maintainer = editor.clone().everywhere(f.author, own);
    let Outcome::Applied {
        changeset: direct, ..
    } = submit_with(maintainer.clone(), ids[2], true).await
    else {
        panic!("applied by its author");
    };
    assert_eq!(
        audited(direct).await,
        ["changeset.state.pending", "changeset.state.applied"]
    );
    let (kind, validator): (String, Option<Uuid>) =
        sqlx::query_as("SELECT kind, validator_id FROM changeset WHERE id = $1")
            .bind(direct)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!((kind.as_str(), validator), ("direct", Some(f.author)));

    // REQ-024-11: its author validates it later under apply_own, not under apply alone.
    let applier = editor.clone().everywhere(f.author, apply);
    assert_eq!(
        rule(f.validate(&applier, held, f.author).await),
        "changeset.apply_own.missing"
    );
    assert!(matches!(
        f.validate(&maintainer, held, f.author).await.unwrap(),
        Outcome::Applied { .. }
    ));
    // Someone else under apply, not under apply_own.
    let (theirs, _) = pending(submit_with(editor.clone(), ids[3], false).await);
    let other_own = editor.clone().everywhere(other, own);
    assert_eq!(
        rule(f.validate(&other_own, theirs, other).await),
        "changeset.approval.forbidden"
    );
    let other_apply = editor.clone().everywhere(other, apply);
    assert!(matches!(
        f.validate(&other_apply, theirs, other).await.unwrap(),
        Outcome::Applied { .. }
    ));

    // REQ-024-12: a policy that holds it wins over apply_own.
    f.applied(vec![Operation::SetApprovalPolicy {
        name: "author".into(),
        min_operations: Some(1),
        min_diff_objects: None,
        property_keys: Vec::new(),
        domains: Vec::new(),
        scopes: Vec::new(),
        author_roles: Vec::new(),
        author_groups: vec![f.author_group().await],
        bound_roles: Vec::new(),
        module_triggers: Vec::new(),
        enabled: true,
    }])
    .await;
    let (approval, reason) = pending(submit_with(maintainer.clone(), ids[4], true).await);
    assert_eq!(reason, Some("changeset.approval.self"));
    assert_eq!(
        rule(f.validate(&maintainer, approval, f.author).await),
        "changeset.approval.self"
    );
    assert!(matches!(
        f.validate(&other_apply, approval, other).await.unwrap(),
        Outcome::Applied { .. }
    ));
}
