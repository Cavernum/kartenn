//! Service accounts end to end (spec 005, REQ-005-11 to 13; task 005-10): created by a
//! changeset with a generated secret, authenticated by it, rotated, revoked, expired,
//! disabled; every refusal audited with the claimed name as a digest only.

use std::time::Duration;

use kartenn_auth::keys::KeyRing;
use kartenn_auth::secrets::generate;
use kartenn_core::period::Instant;
use kartenn_service::aggregate::FailureAggregator;
use kartenn_service::audit::Instance;
use kartenn_service::authn::{AuthnError, IdentityKind, authenticate_service};
use kartenn_service::authz::Unrestricted;
use kartenn_service::operations::SecretRecord;
use kartenn_service::pipeline::MAX_OPERATIONS;
use kartenn_service::{ChangesetRequest, Operation, Outcome, submit};
use kartenn_store_pg::audit_journal::PgAuditJournal;
use kartenn_store_pg::write::PgStore;
use sqlx::PgPool;
use sqlx::types::Uuid;

fn now() -> Instant {
    Instant(
        i64::try_from(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_micros(),
        )
        .unwrap(),
    )
}

struct Fixture {
    pool: PgPool,
    store: PgStore,
    ring: KeyRing,
    journal: PgAuditJournal,
    aggregator: FailureAggregator,
    admin: Uuid,
}

impl Fixture {
    async fn new(pool: &PgPool) -> Self {
        let admin = sqlx::query_scalar(
            "INSERT INTO person (id, display_name) VALUES (gen_random_uuid(), 'admin') RETURNING id",
        )
        .fetch_one(pool)
        .await
        .unwrap();
        Self {
            pool: pool.clone(),
            store: PgStore::new(pool.clone()),
            ring: KeyRing::parse(&KeyRing::generate_line("k1")).unwrap(),
            journal: PgAuditJournal::new(pool.clone(), Instance::new("test")),
            aggregator: FailureAggregator::new(10, Duration::from_mins(1)),
            admin,
        }
    }

    async fn apply(&self, operations: Vec<Operation>) -> Outcome {
        let request = ChangesetRequest {
            author: self.admin,
            motive: "test".into(),
            period: None,
            operations,
            simulate: false,
            idempotency_key: None,
        };
        submit(&self.store, &Unrestricted, &request, MAX_OPERATIONS)
            .await
            .unwrap()
    }

    async fn login(&self, name: &str, secret: &str) -> Result<Uuid, AuthnError> {
        authenticate_service(
            &self.store,
            &self.ring,
            &self.journal,
            &self.aggregator,
            name,
            secret,
            Some("192.0.2.7".parse().unwrap()),
            now(),
        )
        .await
        .map(|p| {
            assert_eq!(p.kind, IdentityKind::ServiceAccount);
            p.identity
        })
    }

    async fn failures(&self) -> Vec<(Option<String>, Option<String>)> {
        sqlx::query_as(
            "SELECT claimed, reason FROM audit.event WHERE name = 'authn.login.failed' ORDER BY seq",
        )
        .fetch_all(&self.pool)
        .await
        .unwrap()
    }
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn a_service_account_lives_by_its_secrets(pool: PgPool) {
    let f = Fixture::new(&pool).await;
    let first = generate(&f.ring, None);
    let outcome = f
        .apply(vec![Operation::CreateServiceAccount {
            local: None,
            name: "svc-backup".into(),
            secret: first.record.clone(),
        }])
        .await;
    assert!(matches!(outcome, Outcome::Applied { .. }), "{outcome:?}");
    let account = f.login("svc-backup", &first.plaintext).await.unwrap();

    // A wrong secret, an unknown account: one answer, the reason in the audit only.
    assert!(matches!(
        f.login("svc-backup", "guess").await,
        Err(AuthnError::Refused)
    ));
    assert!(matches!(
        f.login("nobody", &first.plaintext).await,
        Err(AuthnError::Refused)
    ));
    let failures = f.failures().await;
    assert_eq!(failures.len(), 2);
    assert_eq!(failures[0].1.as_deref(), Some("secret"));
    assert_eq!(failures[1].1.as_deref(), Some("unknown"));
    let claimed = failures[0].0.clone().unwrap();
    assert_eq!(claimed.len(), 64, "a digest");
    assert!(!claimed.contains("svc-backup"), "never the name itself");

    // Rotation: two secrets work; a third active one is refused.
    let second = generate(&f.ring, None);
    let outcome = f
        .apply(vec![Operation::AddServiceSecret {
            account,
            secret: second.record.clone(),
        }])
        .await;
    assert!(matches!(outcome, Outcome::Applied { .. }), "{outcome:?}");
    assert!(f.login("svc-backup", &first.plaintext).await.is_ok());
    assert!(f.login("svc-backup", &second.plaintext).await.is_ok());
    let outcome = f
        .apply(vec![Operation::AddServiceSecret {
            account,
            secret: generate(&f.ring, None).record,
        }])
        .await;
    assert!(matches!(outcome, Outcome::Refused { .. }), "{outcome:?}");
    // Revoking the first: refused at the next request.
    let outcome = f
        .apply(vec![Operation::RevokeServiceSecret {
            account,
            secret: first.record.id,
        }])
        .await;
    assert!(matches!(outcome, Outcome::Applied { .. }), "{outcome:?}");
    assert!(f.login("svc-backup", &first.plaintext).await.is_err());
    assert!(f.login("svc-backup", &second.plaintext).await.is_ok());

    // An expired secret is refused.
    let expired = generate(&f.ring, Some(now().0 - 1_000_000));
    let outcome = f
        .apply(vec![Operation::AddServiceSecret {
            account,
            secret: expired.record.clone(),
        }])
        .await;
    assert!(matches!(outcome, Outcome::Applied { .. }), "{outcome:?}");
    assert!(f.login("svc-backup", &expired.plaintext).await.is_err());

    // A disabled account is refused at once.
    let outcome = f
        .apply(vec![Operation::SetIdentityState {
            identity: account,
            disabled: Some(true),
            revoke_sessions: false,
        }])
        .await;
    assert!(matches!(outcome, Outcome::Applied { .. }), "{outcome:?}");
    assert!(f.login("svc-backup", &second.plaintext).await.is_err());
    let reasons: Vec<Option<String>> = f.failures().await.into_iter().map(|(_, r)| r).collect();
    assert_eq!(
        &reasons[2..],
        [
            Some("revoked".to_owned()),
            Some("expired".to_owned()),
            Some("disabled".to_owned())
        ]
    );
    // The secret never reaches the database.
    let leaked: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM audit.event WHERE to_jsonb(audit.event.*)::text LIKE '%' || $1 || '%'",
    )
    .bind(&second.plaintext)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(leaked, 0);
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn a_person_is_created_at_its_first_login(pool: PgPool) {
    use kartenn_service::authn::{Method, logged_in};
    let f = Fixture::new(&pool).await;
    let login = |issuer: &'static str, name: &'static str| {
        let f = &f;
        async move {
            logged_in(
                &f.store,
                &f.journal,
                &f.aggregator,
                (issuer, "u-42", Some(name)),
                vec!["netops".into()],
                Method::OidcSession,
                kartenn_core::audit::Source::default(),
                now(),
            )
            .await
        }
    };
    let first = login("https://idp.example.invalid", "A. Person")
        .await
        .unwrap();
    assert_eq!(first.groups, ["netops"]);
    let again = login("https://idp.example.invalid", "A. Person-Renamed")
        .await
        .unwrap();
    assert_eq!(
        again.identity, first.identity,
        "the same identity (REQ-005-1)"
    );
    let (name, email): (String, Option<String>) =
        sqlx::query_as("SELECT display_name, email FROM person WHERE id = $1")
            .bind(first.identity)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        (name.as_str(), email),
        ("A. Person-Renamed", None),
        "a name, no e-mail (K3)"
    );
    // The same subject at another issuer: another identity (K2).
    let other = login("https://vendor.example.invalid", "A. Person")
        .await
        .unwrap();
    assert_ne!(other.identity, first.identity);
    let succeeded: i64 =
        sqlx::query_scalar("SELECT count(*) FROM audit.event WHERE name = 'authn.login.succeeded'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(succeeded, 3);
    // Disabled: refused, audited.
    sqlx::query("UPDATE person SET disabled_at = now() WHERE id = $1")
        .bind(first.identity)
        .execute(&pool)
        .await
        .unwrap();
    assert!(matches!(
        login("https://idp.example.invalid", "A. Person").await,
        Err(AuthnError::Refused)
    ));
    assert_eq!(
        f.failures().await.last().unwrap().1.as_deref(),
        Some("disabled")
    );
}

/// Whether `person`, in a session opened at `started`, may act.
async fn check(
    f: &Fixture,
    person: &kartenn_service::authn::Principal,
    cache: &kartenn_service::authn::StateCache,
    started: Instant,
) -> Result<(), AuthnError> {
    cache
        .check(
            &f.store,
            &f.journal,
            person,
            Some(started),
            kartenn_core::audit::Source::default(),
        )
        .await
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn a_revoked_session_is_cut_at_the_next_request(pool: PgPool) {
    use kartenn_service::authn::{Method, StateCache, logged_in};
    let f = Fixture::new(&pool).await;
    let person = logged_in(
        &f.store,
        &f.journal,
        &f.aggregator,
        ("https://idp.example.invalid", "u-7", Some("B. Person")),
        Vec::new(),
        Method::OidcSession,
        kartenn_core::audit::Source::default(),
        now(),
    )
    .await
    .unwrap();
    let opened = now();
    let cache = StateCache::new(Duration::from_secs(5));
    assert!(check(&f, &person, &cache, opened).await.is_ok());
    std::thread::sleep(Duration::from_millis(2));
    let outcome = f
        .apply(vec![Operation::SetIdentityState {
            identity: person.identity,
            disabled: None,
            revoke_sessions: true,
        }])
        .await;
    assert!(matches!(outcome, Outcome::Applied { .. }), "{outcome:?}");
    // This instance applied the change: it forgets the identity, the next request is cut.
    cache.forget(person.identity);
    assert!(matches!(
        check(&f, &person, &cache, opened).await,
        Err(AuthnError::Refused)
    ));
    let reason: Option<String> = sqlx::query_scalar(
        "SELECT reason FROM audit.event WHERE name = 'authz.access.denied' ORDER BY seq DESC LIMIT 1",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(reason.as_deref(), Some("session.revoked"));
    // A session opened after the revocation is fine.
    assert!(check(&f, &person, &cache, now()).await.is_ok());
    // Another instance, without cache: a disabled identity is refused at once.
    let other = StateCache::new(Duration::ZERO);
    sqlx::query("UPDATE person SET disabled_at = now() WHERE id = $1")
        .bind(person.identity)
        .execute(&pool)
        .await
        .unwrap();
    assert!(matches!(
        check(&f, &person, &other, now()).await,
        Err(AuthnError::Refused)
    ));
}

#[test]
fn verifying_a_secret_costs_microseconds() {
    let ring = KeyRing::parse(&KeyRing::generate_line("k1")).unwrap();
    let secret = generate(&ring, None);
    let started = std::time::Instant::now();
    for _ in 0..1_000 {
        let _ = kartenn_service::authn::SecretHasher::digest(&ring, "k1", &secret.plaintext);
    }
    let each = started.elapsed() / 1_000;
    assert!(
        each < Duration::from_millis(1),
        "{each:?} per verification (REQ-005-12)"
    );
    let _: &SecretRecord = &secret.record;
}
