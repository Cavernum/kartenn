//! Every refusal of a login or of an access token is audited (spec 005, REQ-005-28; task
//! 005-15): one `authn.login.failed` with its coded reason, its method and its issuer, the
//! claimed identifier as a digest only.

use std::time::Duration;

use kartenn_auth::oidc::LoginError;
use kartenn_auth::tokens::TokenError;
use kartenn_core::audit::Source;
use kartenn_core::period::Instant;
use kartenn_service::aggregate::FailureAggregator;
use kartenn_service::audit::Instance;
use kartenn_service::authn::{Failure, Method, failed};
use kartenn_store_pg::audit_journal::PgAuditJournal;
use kartenn_store_pg::write::PgStore;
use sqlx::PgPool;

const ISSUER: &str = "https://idp.example.invalid";

#[kartenn_testing::pg_test]
async fn each_refusal_is_audited_with_its_reason(pool: PgPool) {
    let store = PgStore::new(pool.clone());
    let journal = PgAuditJournal::new(pool.clone(), Instance::new("test"));
    // No aggregation here: one event per failure.
    let aggregator = FailureAggregator::new(1_000, Duration::from_mins(1));
    let logins = [
        LoginError::UnknownIssuer,
        LoginError::Unavailable("timeout".into()),
        LoginError::Transaction,
        LoginError::State,
        LoginError::Issuer,
        LoginError::Refused("access_denied".into()),
        LoginError::Exchange("invalid_grant".into()),
        LoginError::IdToken(TokenError::Nonce),
        LoginError::IdToken(TokenError::Signature),
        LoginError::IdToken(TokenError::Expired),
        LoginError::IdToken(TokenError::Audience),
        LoginError::Subject,
    ];
    let tokens = [
        TokenError::Malformed,
        TokenError::NotAccessToken,
        TokenError::UnknownIssuer,
        TokenError::Algorithm,
        TokenError::UnknownKey,
        TokenError::Expired,
        TokenError::Audience,
        TokenError::Claims("no subject".into()),
        TokenError::Keys("unreachable".into()),
    ];
    let refusals = logins
        .iter()
        .map(|e| (Method::OidcSession, e.reason()))
        .chain(tokens.iter().map(|e| (Method::Bearer, e.reason())));
    let mut expected = Vec::new();
    for (k, (method, reason)) in refusals.enumerate() {
        failed(
            &store,
            &journal,
            &aggregator,
            Failure {
                claimed: "u-42",
                method,
                issuer: Some(ISSUER),
                reason,
            },
            Source::default(),
            Instant(1_800_000_000_000_000 + i64::try_from(k).unwrap()),
        )
        .await
        .unwrap();
        expected.push((method.as_str().to_owned(), reason.to_owned()));
    }
    let recorded: Vec<(String, String, String, String)> = sqlx::query_as(
        "SELECT auth_method, reason, issuer, claimed FROM audit.event
         WHERE name = 'authn.login.failed' ORDER BY seq",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(
        recorded
            .iter()
            .map(|(m, r, _, _)| (m.clone(), r.clone()))
            .collect::<Vec<_>>(),
        expected
    );
    assert!(
        recorded
            .iter()
            .all(|(_, _, issuer, claimed)| issuer == ISSUER
                && claimed.len() == 64
                && !claimed.contains("u-42"))
    );
    // The reasons the design names are all among them.
    for reason in ["state", "iss", "nonce", "signature", "expired", "audience"] {
        assert!(
            expected.iter().any(|(_, r)| r == reason),
            "{reason} never coded"
        );
    }
}
