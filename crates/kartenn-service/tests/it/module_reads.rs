//! The reads a module asks before the writes, among several operations (INV-63; tasks
//! 002-29c2, 29d2, 29d7): an operation refused the read of an object it cites answers by
//! that refusal alone, while the others keep their own refusals, at submission as at the
//! revalidation of a proposal with operations of review joined (REQ-016-11).

use kartenn_service::audit::RequestContext;
use kartenn_service::authz::{Grants, Unrestricted};
use kartenn_service::expected::Sources;
use kartenn_service::pipeline::{MAX_OPERATIONS, PendingKind};
use kartenn_service::review::validate_pending_with;
use kartenn_service::{ChangesetRequest, Operation, Outcome, Ref, submit};
use kartenn_store_pg::write::PgStore;
use serde_json::json;
use sqlx::PgPool;
use sqlx::types::Uuid;

use crate::support::dummy_module as dummy;

const EDIT: &str = "dummy.thing.edit";
const READ: &str = "dummy.thing.read";
const ADDRESS_READ: &str = "core.address.read";

/// Two things and an address, written by an administrator.
struct World {
    store: PgStore,
    pool: PgPool,
    author: Uuid,
    validator: Uuid,
    first: Uuid,
    second: Uuid,
    address: Uuid,
}

async fn person(pool: &PgPool) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO person (id, display_name) VALUES (gen_random_uuid(), 'tester') RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap()
}

fn request(author: Uuid, operations: Vec<Operation>) -> ChangesetRequest {
    ChangesetRequest {
        author,
        motive: "test".into(),
        period: None,
        operations,
        simulate: false,
        idempotency_key: None,
        approval_self: true,
    }
}

impl World {
    async fn new(pool: &PgPool) -> Self {
        let store = dummy::store(pool).await;
        let admin = person(pool).await;
        let outcome = submit(
            &store,
            &Unrestricted,
            &request(
                admin,
                vec![
                    Operation::CreateDomain {
                        local: Some(1),
                        slug: "lab".into(),
                    },
                    Operation::CreatePrefix {
                        local: None,
                        domain: Ref::Local(1),
                        prefix: "192.0.2.0/24".into(),
                        nature: kartenn_core::objects::Nature::Network,
                        point_to_point: false,
                        anycast: false,
                    },
                    Operation::CreateAddress {
                        local: None,
                        domain: Ref::Local(1),
                        address: "192.0.2.1".into(),
                        usage: kartenn_core::objects::AddressUsage::Host,
                    },
                    dummy::create_thing(json!({"label": "first"})),
                    dummy::create_thing(json!({"label": "second"})),
                ],
            ),
            MAX_OPERATIONS,
        )
        .await
        .unwrap();
        assert!(matches!(outcome, Outcome::Applied { .. }), "{outcome:?}");
        let thing = |label: &'static str| {
            sqlx::query_scalar("SELECT id FROM dummy.thing WHERE label = $1")
                .bind(label)
                .fetch_one(pool)
        };
        Self {
            store,
            pool: pool.clone(),
            author: person(pool).await,
            validator: person(pool).await,
            first: thing("first").await.unwrap(),
            second: thing("second").await.unwrap(),
            address: sqlx::query_scalar(
                "SELECT id FROM addressing_object WHERE value = '192.0.2.1'::inet",
            )
            .fetch_one(pool)
            .await
            .unwrap(),
        }
    }

    /// The update of `thing`, citing the address when `cites`.
    fn update(&self, thing: Uuid, label: &str, cites: bool) -> Operation {
        let mut fields = json!({"id": Ref::Id(thing), "label": label});
        if cites {
            fields["reads"] = json!(Ref::Id(self.address));
        }
        dummy::update_thing(fields)
    }

    /// The labels of the living things.
    async fn labels(&self) -> Vec<String> {
        sqlx::query_scalar("SELECT label FROM dummy.thing ORDER BY label")
            .fetch_all(&self.pool)
            .await
            .unwrap()
    }
}

/// The violations of a refusal, or of a revalidation failed, whole.
fn refusals(outcome: &Outcome) -> Vec<(&'static str, String, Vec<Uuid>)> {
    let (Outcome::Refused { violations, .. } | Outcome::FailedRevalidation { violations, .. }) =
        outcome
    else {
        panic!("expected a refusal, got {outcome:?}")
    };
    violations
        .iter()
        .map(|v| (v.rule.id, v.message.clone(), v.subjects.clone()))
        .collect()
}

/// The refusal of the read of the address, alone: it names nothing.
fn unread() -> (&'static str, String, Vec<Uuid>) {
    (
        "core.object.forbidden",
        format!("objects you do not read need {ADDRESS_READ}"),
        Vec::new(),
    )
}

fn unedited(thing: Uuid) -> (&'static str, String, Vec<Uuid>) {
    (
        "core.object.forbidden",
        format!("1 object(s) need {EDIT}"),
        vec![thing],
    )
}

/// An author who reads the things but neither edits them nor reads the address: the
/// operation that cites it answers by the refusal of its read, without the edit of its
/// thing; another operation keeps the refusal of its edit, on another thing or on the
/// same one.
#[kartenn_testing::pg_test]
async fn an_unread_operation_leaves_the_refusals_of_the_others(pool: PgPool) {
    let w = World::new(&pool).await;
    let reader = Grants::new().everywhere(w.author, READ);
    let refused = |operations| async {
        let outcome = submit(
            &w.store,
            &reader,
            &request(w.author, operations),
            MAX_OPERATIONS,
        )
        .await
        .unwrap();
        refusals(&outcome)
    };
    assert_eq!(
        refused(vec![w.update(w.first, "a", true)]).await,
        [unread()],
        "the read alone"
    );
    assert_eq!(
        refused(vec![
            w.update(w.first, "a", true),
            w.update(w.second, "b", false)
        ])
        .await,
        [unedited(w.second), unread()],
        "another thing"
    );
    assert_eq!(
        refused(vec![
            w.update(w.first, "a", true),
            w.update(w.first, "b", false)
        ])
        .await,
        [unedited(w.first), unread()],
        "the same thing"
    );
    assert_eq!(w.labels().await, ["first", "second"]);
}

/// A proposal its author could write, revalidated once he lost the edit of the things and
/// the read of the address, with an operation of review joined by its validator (checked
/// under the validator alone, REQ-016-11): the operation that cites the address answers
/// by the refusal of its read alone, the other keeps the refusal of its edit; nothing is
/// written.
#[kartenn_testing::pg_test]
async fn a_revalidation_with_joined_operations_keeps_each_refusal(pool: PgPool) {
    let w = World::new(&pool).await;
    let writer = Grants::new()
        .everywhere(w.author, EDIT)
        .everywhere(w.author, READ)
        .everywhere(w.author, ADDRESS_READ);
    for (other, label) in [(w.second, "another thing"), (w.first, "the same thing")] {
        let outcome = submit(
            &w.store,
            &writer,
            &request(
                w.author,
                vec![
                    dummy::create_thing(json!({"label": "review-me", "local": 1})),
                    w.update(w.first, "a", true),
                    w.update(other, "b", false),
                ],
            ),
            MAX_OPERATIONS,
        )
        .await
        .unwrap();
        let Outcome::Pending {
            changeset,
            kind: PendingKind::Proposal,
            ..
        } = outcome
        else {
            panic!("expected a forced proposal, got {outcome:?}")
        };
        let grants = Grants::new()
            .everywhere(w.author, READ)
            .everywhere(w.validator, "dummy.thing.apply")
            .everywhere(w.validator, "dummy.thing.review")
            .everywhere(w.validator, READ);
        let validated = validate_pending_with(
            &w.store,
            &grants,
            &Sources::none(),
            changeset,
            &w.validator.into(),
            &RequestContext::person(w.validator),
            vec![dummy::update_thing(
                json!({"id": Ref::Local(1), "label": "reviewed"}),
            )],
            MAX_OPERATIONS,
        )
        .await
        .unwrap();
        assert_eq!(refusals(&validated), [unedited(other), unread()], "{label}");
        assert_eq!(w.labels().await, ["first", "second"], "{label}");
    }
}
