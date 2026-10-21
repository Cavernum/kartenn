//! Break-glass accounts (spec 005, REQ-005-14, 15; task 005-14): created by the owner of
//! the schema, authenticated by passphrase and TOTP, a code never twice, disabled after
//! 100 consecutive failures, each use audited at high criticality.

use std::time::Duration;

use kartenn_auth::breakglass::{STEP, Verifier, code, create};
use kartenn_auth::keys::KeyRing;
use kartenn_core::period::Instant;
use kartenn_service::aggregate::FailureAggregator;
use kartenn_service::audit::Instance;
use kartenn_service::authn::{AuthnError, IdentityKind, authenticate_breakglass};
use kartenn_store_pg::audit_journal::PgAuditJournal;
use kartenn_store_pg::breakglass::Stored;
use kartenn_store_pg::write::PgStore;
use sqlx::PgPool;
use sqlx::types::time::OffsetDateTime;

/// The secret of an `otpauth://` URI (RFC 4648 base32), as an authenticator app reads it.
fn secret_of(uri: &str) -> Vec<u8> {
    let encoded = uri
        .split("secret=")
        .nth(1)
        .unwrap()
        .split('&')
        .next()
        .unwrap();
    let mut out = Vec::new();
    let (mut buffer, mut bits) = (0u32, 0);
    for c in encoded.bytes() {
        let value = u32::try_from(
            b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567"
                .iter()
                .position(|&a| a == c)
                .unwrap(),
        )
        .unwrap();
        buffer = (buffer << 5) | value;
        bits += 5;
        if bits >= 8 {
            bits -= 8;
            out.push(u8::try_from((buffer >> bits) & 0xff).unwrap());
        }
    }
    out
}

fn now() -> Instant {
    Instant(i64::try_from(OffsetDateTime::now_utc().unix_timestamp_nanos() / 1_000).unwrap())
}

#[kartenn_testing::pg_test]
async fn a_break_glass_account_takes_a_passphrase_and_a_fresh_code(pool: PgPool) {
    let ring = KeyRing::parse(&KeyRing::generate_line("k1")).unwrap();
    let account = create(&ring, "rescue");
    let instance = Instance::new("test");
    let store = PgStore::new(pool.clone());
    let journal = PgAuditJournal::new(pool.clone(), instance.clone());
    let aggregator = FailureAggregator::new(1_000, Duration::from_mins(1));
    let verifier = Verifier(ring.clone());
    kartenn_store_pg::breakglass::create(
        &pool,
        &Stored {
            name: "rescue",
            password_hash: &account.password_hash,
            totp_sealed: &account.totp_sealed,
            totp_key_id: &account.totp_key_id,
            valid_until: OffsetDateTime::now_utc() + Duration::from_hours(1),
        },
        &instance,
    )
    .await
    .unwrap();
    let secret = secret_of(&account.otpauth);
    let current = || format!("{:06}", code(&secret, now().0 / 1_000_000 / STEP));
    let login = |passphrase: String, otp: String| {
        let (store, journal, aggregator, verifier) = (&store, &journal, &aggregator, &verifier);
        async move {
            authenticate_breakglass(
                store,
                verifier,
                journal,
                aggregator,
                ("rescue", &passphrase, &otp),
                Some("192.0.2.1".parse().unwrap()),
                now(),
            )
            .await
        }
    };
    // One code, reused for the replay below: computed again, it could fall in the next
    // 30-second step and be a fresh code.
    let first = current();
    let principal = login(account.passphrase.clone(), first.clone())
        .await
        .unwrap();
    assert_eq!(principal.kind, IdentityKind::Breakglass);
    let used: Vec<String> = sqlx::query_scalar(
        "SELECT criticality FROM audit.event WHERE name IN ('authn.breakglass.used', 'account.breakglass.created') ORDER BY seq",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(used, ["high", "high"]);
    // The same code again: refused (RFC 6238 §5.2).
    assert!(matches!(
        login(account.passphrase.clone(), first).await,
        Err(AuthnError::Refused)
    ));
    // A wrong passphrase, an unknown account: refused, audited.
    assert!(login("wrong passphrase".into(), current()).await.is_err());
    let unknown = authenticate_breakglass(
        &store,
        &verifier,
        &journal,
        &aggregator,
        ("nobody", "x", "000000"),
        None,
        now(),
    )
    .await;
    assert!(matches!(unknown, Err(AuthnError::Refused)));
    let reasons: Vec<Option<String>> = sqlx::query_scalar(
        "SELECT reason FROM audit.event WHERE name = 'authn.login.failed' ORDER BY seq",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(
        reasons,
        [
            Some("totp".to_owned()),
            Some("secret".to_owned()),
            Some("unknown".to_owned())
        ]
    );
    // 100 consecutive failures disable it (NIST SP 800-63B §3.2.2).
    sqlx::query("UPDATE breakglass_account SET failures = 98")
        .execute(&pool)
        .await
        .unwrap();
    assert!(login("wrong".into(), current()).await.is_err());
    assert!(login("wrong".into(), current()).await.is_err());
    let (disabled, locked): (bool, i64) = sqlx::query_as(
        "SELECT (SELECT disabled_at IS NOT NULL FROM person WHERE display_name = 'rescue'),
                (SELECT count(*) FROM audit.event WHERE name = 'authn.account.locked')",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(disabled);
    assert_eq!(locked, 1);
    // Even the right proofs no longer open it.
    assert!(login(account.passphrase.clone(), current()).await.is_err());
}

#[kartenn_testing::pg_test]
async fn an_expired_account_is_refused(pool: PgPool) {
    let ring = KeyRing::parse(&KeyRing::generate_line("k1")).unwrap();
    let account = create(&ring, "old");
    let instance = Instance::new("test");
    kartenn_store_pg::breakglass::create(
        &pool,
        &Stored {
            name: "old",
            password_hash: &account.password_hash,
            totp_sealed: &account.totp_sealed,
            totp_key_id: &account.totp_key_id,
            valid_until: OffsetDateTime::now_utc() - Duration::from_secs(1),
        },
        &instance,
    )
    .await
    .unwrap();
    let secret = secret_of(&account.otpauth);
    let otp = format!("{:06}", code(&secret, now().0 / 1_000_000 / STEP));
    let outcome = authenticate_breakglass(
        &PgStore::new(pool.clone()),
        &Verifier(ring),
        &PgAuditJournal::new(pool.clone(), instance),
        &FailureAggregator::new(10, Duration::from_mins(1)),
        ("old", &account.passphrase, &otp),
        None,
        now(),
    )
    .await;
    assert!(matches!(outcome, Err(AuthnError::Refused)));
}

#[kartenn_testing::pg_test]
async fn a_code_sent_twice_at_once_opens_one_session(pool: PgPool) {
    let ring = KeyRing::parse(&KeyRing::generate_line("k1")).unwrap();
    let account = create(&ring, "rescue");
    let instance = Instance::new("test");
    kartenn_store_pg::breakglass::create(
        &pool,
        &Stored {
            name: "rescue",
            password_hash: &account.password_hash,
            totp_sealed: &account.totp_sealed,
            totp_key_id: &account.totp_key_id,
            valid_until: OffsetDateTime::now_utc() + Duration::from_hours(1),
        },
        &instance,
    )
    .await
    .unwrap();
    let otp = format!(
        "{:06}",
        code(&secret_of(&account.otpauth), now().0 / 1_000_000 / STEP)
    );
    let store = PgStore::new(pool.clone());
    let journal = PgAuditJournal::new(pool.clone(), instance);
    let aggregator = FailureAggregator::new(1_000, Duration::from_mins(1));
    let verifier = Verifier(ring);
    let login = || {
        authenticate_breakglass(
            &store,
            &verifier,
            &journal,
            &aggregator,
            ("rescue", &account.passphrase, &otp),
            None,
            now(),
        )
    };
    // Both read the account before either records its step (RFC 6238 §5.2).
    let (first, second) = tokio::join!(login(), login());
    assert_eq!(
        usize::from(first.is_ok()) + usize::from(second.is_ok()),
        1,
        "{first:?} {second:?}"
    );
}
