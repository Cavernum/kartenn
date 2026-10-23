//! The RBAC schema: bindings well formed, at most two active secrets per service account,
//! break-glass accounts out of reach of the application (spec 005, REQ-005-11, 14, 18;
//! task 005-3).

use sqlx::PgPool;
use sqlx::types::Uuid;

async fn account(pool: &PgPool, kind: &str) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO person (id, display_name, kind) VALUES (gen_random_uuid(), 'a', $1) RETURNING id",
    )
    .bind(kind)
    .fetch_one(pool)
    .await
    .unwrap()
}

/// Subject kind, identity, issuer, group, scope kind, scope, accepted.
type Case<'a> = (
    &'a str,
    Option<Uuid>,
    Option<&'a str>,
    Option<&'a str>,
    &'a str,
    Option<Uuid>,
    bool,
);

#[kartenn_testing::pg_test]
async fn bindings_name_their_subject_and_scope_consistently(pool: PgPool) {
    let insert = "INSERT INTO binding (id, subject_kind, subject_id, issuer, group_name, role_id,
                  scope_kind, scope_id, valid_period, changeset_id)
                  VALUES (gen_random_uuid(), $1, $2, $3, $4, gen_random_uuid(), $5, $6,
                          tstzrange(now(), NULL), gen_random_uuid())";
    let id = Some(Uuid::now_v7());
    let issuer = Some("https://idp.example.invalid");
    let cases: [Case<'_>; 7] = [
        ("identity", id, None, None, "global", None, true),
        ("group", None, issuer, Some("netops"), "entity", id, true),
        ("issuer", None, issuer, None, "domain", id, true),
        ("identity", None, None, None, "global", None, false),
        ("group", None, issuer, None, "global", None, false),
        ("issuer", id, issuer, None, "global", None, false),
        ("identity", id, None, None, "entity", None, false),
    ];
    for (kind, subject, iss, group, scope, scope_id, ok) in cases {
        let result = sqlx::query(insert)
            .bind(kind)
            .bind(subject)
            .bind(iss)
            .bind(group)
            .bind(scope)
            .bind(scope_id)
            .execute(&pool)
            .await;
        assert_eq!(result.is_ok(), ok, "{kind} {scope}: {result:?}");
    }
}

#[kartenn_testing::pg_test]
async fn a_service_account_holds_two_active_secrets_at_most(pool: PgPool) {
    let svc = account(&pool, "service_account").await;
    let add = || {
        sqlx::query(
            "INSERT INTO service_secret (id, account_id, digest, key_id)
             VALUES (gen_random_uuid(), $1, decode(repeat('ab', 32), 'hex'), 'k1')",
        )
        .bind(svc)
        .execute(&pool)
    };
    add().await.unwrap();
    add().await.unwrap();
    assert!(add().await.is_err(), "a third active secret");
    sqlx::query("UPDATE service_secret SET revoked_at = now() WHERE account_id = $1 AND ctid = (SELECT min(ctid) FROM service_secret WHERE account_id = $1)")
        .bind(svc)
        .execute(&pool)
        .await
        .unwrap();
    add().await.unwrap();
}

#[kartenn_testing::pg_test]
async fn the_application_cannot_create_a_break_glass_account(pool: PgPool) {
    let glass = account(&pool, "breakglass").await;
    sqlx::query(
        "INSERT INTO breakglass_account (account_id, password_hash, totp_secret, totp_key_id, valid_until)
         VALUES ($1, '$argon2id$v=19$m=65536,t=3,p=4$c2FsdA$aGFzaA', '\\x00', 'k1', now() + interval '1 hour')",
    )
    .bind(glass)
    .execute(&pool)
    .await
    .unwrap();
    let mut tx = pool.begin().await.unwrap();
    sqlx::query("SET LOCAL ROLE kartenn_app")
        .execute(&mut *tx)
        .await
        .unwrap();
    sqlx::query("UPDATE breakglass_account SET last_totp_step = 5, failures = 1")
        .execute(&mut *tx)
        .await
        .unwrap();
    for forbidden in [
        "UPDATE breakglass_account SET password_hash = '$argon2id$x'",
        "UPDATE breakglass_account SET valid_until = now() + interval '10 years'",
        "INSERT INTO breakglass_account SELECT * FROM breakglass_account",
        "DELETE FROM breakglass_account",
        "UPDATE person SET kind = 'person'",
        "UPDATE service_secret SET digest = digest",
    ] {
        sqlx::query("SAVEPOINT s").execute(&mut *tx).await.unwrap();
        let err = sqlx::query(forbidden).execute(&mut *tx).await.unwrap_err();
        assert!(
            err.to_string().contains("permission denied"),
            "{forbidden}: {err}"
        );
        sqlx::query("ROLLBACK TO SAVEPOINT s")
            .execute(&mut *tx)
            .await
            .unwrap();
    }
}

/// R5 (spec 018, task 018-1): a role entry may be a permission of two segments
/// (`flow.read`), not a single word nor a name ending in a dot.
#[kartenn_testing::pg_test]
async fn a_role_entry_may_omit_the_module_resource(pool: PgPool) {
    let insert = "INSERT INTO role_permission (role_id, permission, valid_period, changeset_id)
                  VALUES (gen_random_uuid(), $1, tstzrange(now(), NULL), gen_random_uuid())";
    let cases = [
        ("flow.read", true),
        ("flow.waiver.grant", true),
        ("core.address.read", true),
        ("prop.custom.rack.write", true),
        ("prop.*.write", true),
        ("*", true),
        ("flow", false),
        ("flow.", false),
        ("Flow.read", false),
    ];
    for (permission, ok) in cases {
        let result = sqlx::query(insert).bind(permission).execute(&pool).await;
        assert_eq!(result.is_ok(), ok, "{permission}: {result:?}");
    }
}
