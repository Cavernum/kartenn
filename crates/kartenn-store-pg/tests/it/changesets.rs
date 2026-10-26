//! Changeset, audit and outbox tables (spec 004, task 004-5).

use sqlx::{PgPool, Row};

async fn person(pool: &PgPool) -> sqlx::Result<sqlx::types::Uuid> {
    sqlx::query_scalar(
        "INSERT INTO person (id, display_name) VALUES (gen_random_uuid(), 'tester') RETURNING id",
    )
    .fetch_one(pool)
    .await
}

async fn changeset(
    pool: &PgPool,
    author: sqlx::types::Uuid,
    state: &str,
) -> sqlx::Result<sqlx::types::Uuid> {
    sqlx::query_scalar(
        "INSERT INTO changeset (id, author_id, motive, state, kind, default_period, expires_at, request)
         VALUES (gen_random_uuid(), $1, 'test', $2, 'direct', tstzrange(now(), NULL),
                 CASE WHEN $2 = 'pending' THEN now() + interval '30 days' END,
                 CASE WHEN $2 = 'pending' THEN '{}'::jsonb END)
         RETURNING id",
    )
    .bind(author)
    .bind(state)
    .fetch_one(pool)
    .await
}

#[kartenn_testing::pg_test]
async fn changeset_constraints(pool: PgPool) -> sqlx::Result<()> {
    let author = person(&pool).await?;
    assert!(changeset(&pool, author, "applied").await.is_ok());
    assert!(changeset(&pool, author, "pending").await.is_ok());
    assert!(
        changeset(&pool, author, "lost").await.is_err(),
        "unknown state"
    );
    // A pending changeset keeps its request, to be replayed (REQ-004-22).
    let without_request = sqlx::query(
        "INSERT INTO changeset (id, author_id, motive, state, kind, default_period, expires_at)
         VALUES (gen_random_uuid(), $1, 'test', 'pending', 'approval', tstzrange(now(), NULL),
                 now() + interval '30 days')",
    )
    .bind(author)
    .execute(&pool)
    .await;
    assert!(without_request.is_err());
    let empty_motive = sqlx::query(
        "INSERT INTO changeset (id, author_id, motive, state, kind, default_period)
         VALUES (gen_random_uuid(), $1, '', 'applied', 'direct', tstzrange(now(), NULL))",
    )
    .bind(author)
    .execute(&pool)
    .await;
    assert!(empty_motive.is_err(), "REQ-004-1: motive required");
    let id = changeset(&pool, author, "pending").await?;
    let self_validated = sqlx::query("UPDATE changeset SET validator_id = author_id WHERE id = $1")
        .bind(id)
        .execute(&pool)
        .await;
    assert!(
        self_validated.is_ok(),
        "REQ-024-11: the author applies their own changeset under approval.self"
    );
    let rejected_without_reason =
        sqlx::query("UPDATE changeset SET state = 'rejected' WHERE id = $1")
            .bind(id)
            .execute(&pool)
            .await;
    assert!(
        rejected_without_reason.is_err(),
        "REQ-004-23: rejection carries a motive"
    );
    Ok(())
}

/// Spec 024, migration 0063: the requirements of the changesets pending before it name
/// edit permissions, those of the other changesets are untouched. Its `UPDATE` is run
/// again on rows written as before.
#[kartenn_testing::pg_test]
async fn pending_requirements_take_the_edit_verb(pool: PgPool) -> sqlx::Result<()> {
    let migration = include_str!("../../migrations/0063_permission_verbs.sql");
    let update = &migration[migration.find("UPDATE changeset").expect("the rewrite")..];
    let author = person(&pool).await?;
    let object = sqlx::types::Uuid::now_v7();
    let before = serde_json::json!({
        "policy": null,
        "perimeter": [object],
        "required": [
            ["core.address.write", [object]],
            ["prop.custom.rack.write", [object]],
            ["flow.review", [object]]
        ]
    });
    let pending = changeset(&pool, author, "pending").await?;
    let decided = changeset(&pool, author, "pending").await?;
    for id in [pending, decided] {
        sqlx::query("UPDATE changeset SET required_rights = $2 WHERE id = $1")
            .bind(id)
            .bind(&before)
            .execute(&pool)
            .await?;
    }
    sqlx::query("UPDATE changeset SET state = 'withdrawn' WHERE id = $1")
        .bind(decided)
        .execute(&pool)
        .await?;
    sqlx::raw_sql(sqlx::AssertSqlSafe(update))
        .execute(&pool)
        .await?;
    let required = |id| {
        sqlx::query_scalar::<_, serde_json::Value>(
            "SELECT required_rights -> 'required' FROM changeset WHERE id = $1",
        )
        .bind(id)
        .fetch_one(&pool)
    };
    assert_eq!(
        required(pending).await?,
        serde_json::json!([
            ["core.address.edit", [object]],
            ["prop.custom.rack.edit", [object]],
            ["flow.review", [object]]
        ])
    );
    assert_eq!(required(decided).await?, before["required"]);
    Ok(())
}

/// Task 024-11, migration 0071: the journal of a changeset takes `approval.self` (formerly
/// `apply_own`): the code and the message of its reason, its request; a changeset that
/// never named it is untouched. Its `UPDATE`s are run again on rows written as before.
#[kartenn_testing::pg_test]
async fn the_journal_takes_approval_self(pool: PgPool) -> sqlx::Result<()> {
    let migration = include_str!("../../migrations/0071_approval_self.sql");
    let updates = &migration[migration.find("UPDATE changeset").expect("the rewrite")..];
    let author = person(&pool).await?;
    let object = sqlx::types::Uuid::now_v7();
    let held = serde_json::json!({
        "policy": null,
        "perimeter": [object],
        "required": [["core.address.edit", [object]]],
        "reason": {
            "code": "changeset.apply_own.missing",
            "message": "its author lacks apply_own: core.address.apply_own on 1 object",
            "objects": [object]
        }
    });
    let request = serde_json::json!({"author": author, "motive": "m", "apply_own": true});
    let pending = changeset(&pool, author, "pending").await?;
    let untouched = changeset(&pool, author, "pending").await?;
    sqlx::query("UPDATE changeset SET required_rights = $2, request = $3 WHERE id = $1")
        .bind(pending)
        .bind(&held)
        .bind(&request)
        .execute(&pool)
        .await?;
    sqlx::raw_sql(sqlx::AssertSqlSafe(updates))
        .execute(&pool)
        .await?;
    let row = sqlx::query(
        "SELECT required_rights -> 'reason' AS reason, request FROM changeset WHERE id = $1",
    )
    .bind(pending)
    .fetch_one(&pool)
    .await?;
    assert_eq!(
        row.get::<serde_json::Value, _>("reason"),
        serde_json::json!({
            "code": "changeset.approval.self.missing",
            "message": "its author lacks approval.self: core.address.approval.self on 1 object",
            "objects": [object]
        })
    );
    assert_eq!(
        row.get::<serde_json::Value, _>("request"),
        serde_json::json!({"author": author, "motive": "m", "approval_self": true})
    );
    let request: serde_json::Value =
        sqlx::query_scalar("SELECT request FROM changeset WHERE id = $1")
            .bind(untouched)
            .fetch_one(&pool)
            .await?;
    assert_eq!(request, serde_json::json!({}));
    Ok(())
}

/// Spec 025, migration 0067 (REQ-025-13): the authors of the policies naming an account or
/// a group of an issuer are dropped; a policy left without a trigger is disabled. Its
/// block is run again on rows written as before.
#[kartenn_testing::pg_test]
async fn policy_authors_keep_roles_and_groups_of_kartenn(pool: PgPool) -> sqlx::Result<()> {
    let migration = include_str!("../../migrations/0067_policy_authors.sql");
    let block = &migration[migration.find("DO $$").expect("the rewrite")..];
    let person = person(&pool).await?;
    for (name, min_operations, authors) in [
        (
            "mixed",
            None,
            serde_json::json!([{"person": person}, {"role": "auditor"}, {"group": "astreinte"}]),
        ),
        (
            "volume",
            Some(10),
            serde_json::json!([{"group": ["https://idp.example.invalid", "netops"]}]),
        ),
        ("someone", None, serde_json::json!([{"person": person}])),
        ("kept", None, serde_json::json!([{"group": "astreinte"}])),
    ] {
        sqlx::query(
            "INSERT INTO approval_policy (id, name, min_operations, authors)
             VALUES (gen_random_uuid(), $1, $2, $3)",
        )
        .bind(name)
        .bind(min_operations)
        .bind(authors)
        .execute(&pool)
        .await?;
    }
    sqlx::raw_sql(sqlx::AssertSqlSafe(block))
        .execute(&pool)
        .await?;
    let rows: Vec<(String, serde_json::Value, bool)> =
        sqlx::query_as("SELECT name, authors, enabled FROM approval_policy ORDER BY name")
            .fetch_all(&pool)
            .await?;
    assert_eq!(
        rows,
        [
            (
                "kept".to_owned(),
                serde_json::json!([{"group": "astreinte"}]),
                true
            ),
            (
                "mixed".to_owned(),
                serde_json::json!([{"role": "auditor"}, {"group": "astreinte"}]),
                true
            ),
            ("someone".to_owned(), serde_json::json!([]), false),
            ("volume".to_owned(), serde_json::json!([]), true),
        ]
    );
    Ok(())
}

#[kartenn_testing::pg_test]
async fn idempotency_keys_are_unique_per_identity(pool: PgPool) -> sqlx::Result<()> {
    let author = person(&pool).await?;
    let cs = changeset(&pool, author, "applied").await?;
    let insert =
        "INSERT INTO idempotency_key VALUES ($1, 'k1', '\\x01', $2, now() + interval '24 hours')";
    sqlx::query(insert)
        .bind(author)
        .bind(cs)
        .execute(&pool)
        .await?;
    assert!(
        sqlx::query(insert)
            .bind(author)
            .bind(cs)
            .execute(&pool)
            .await
            .is_err()
    );
    Ok(())
}

#[kartenn_testing::pg_test]
async fn outbox_records_the_writing_transaction(pool: PgPool) -> sqlx::Result<()> {
    let author = person(&pool).await?;
    let cs = changeset(&pool, author, "applied").await?;
    let mut tx = pool.begin().await?;
    sqlx::query("INSERT INTO outbox (changeset_id, payload) VALUES ($1, '{}'), ($1, '{}')")
        .bind(cs)
        .execute(&mut *tx)
        .await?;
    let row = sqlx::query(
        "SELECT count(DISTINCT xid) AS xids, bool_and(xid = pg_current_xact_id()) AS this_tx,
                max(seq) > min(seq) AS ordered
         FROM outbox",
    )
    .fetch_one(&mut *tx)
    .await?;
    assert_eq!(row.get::<i64, _>("xids"), 1);
    assert!(row.get::<bool, _>("this_tx"));
    assert!(row.get::<bool, _>("ordered"));
    tx.commit().await?;
    Ok(())
}

#[kartenn_testing::pg_test]
async fn the_application_role_never_deletes_audit(pool: PgPool) -> sqlx::Result<()> {
    let author = person(&pool).await?;
    let cs = changeset(&pool, author, "applied").await?;
    let mut conn = pool.acquire().await?;
    sqlx::raw_sql("SET ROLE kartenn_app")
        .execute(&mut *conn)
        .await?;
    for forbidden in [
        "DELETE FROM changeset",
        "DELETE FROM outbox",
        "UPDATE changeset SET motive = 'rewritten'",
    ] {
        let err = sqlx::raw_sql(forbidden)
            .execute(&mut *conn)
            .await
            .expect_err(forbidden);
        assert!(
            err.to_string().contains("permission denied"),
            "{forbidden}: {err}"
        );
    }
    sqlx::query("UPDATE changeset SET state = 'refused' WHERE id = $1")
        .bind(cs)
        .execute(&mut *conn)
        .await?;
    Ok(())
}
