//! Monthly history partitions and the index of the system period (spec 004, REQ-004-17,
//! 38; task 004-25).

use kartenn_store_pg::maintenance::history_partitions;
use sqlx::PgPool;

/// Partitions of the history of `prefix`, by name.
async fn partitions(pool: &PgPool) -> Vec<String> {
    sqlx::query_scalar(
        "SELECT c.relname::text FROM pg_inherits i JOIN pg_class c ON c.oid = i.inhrelid
         WHERE i.inhparent = 'prefix_history'::regclass ORDER BY 1",
    )
    .fetch_all(pool)
    .await
    .unwrap()
}

async fn month(pool: &PgPool, offset: i32) -> String {
    sqlx::query_scalar(
        "SELECT to_char(date_trunc('month', now()) + make_interval(months => $1), 'YYYYMM')",
    )
    .bind(offset)
    .fetch_one(pool)
    .await
    .unwrap()
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn partitions_are_kept_ahead_and_used(pool: PgPool) {
    // A month already holding rows in the default partition is skipped, not refused.
    let busy = month(&pool, 4).await;
    sqlx::query(
        "INSERT INTO prefix_history_default
             (id, domain_id, prefix, nature, point_to_point, public, registry_version,
              valid_period, sys_start, changeset_id, anycast, sys_end)
         VALUES (gen_random_uuid(), gen_random_uuid(), '10.0.0.0/8', 'container', false, false,
                 'x', tstzrange(now(), NULL), now(), gen_random_uuid(), false,
                 date_trunc('month', now()) + interval '4 months' + interval '1 day')",
    )
    .execute(&pool)
    .await
    .unwrap();
    history_partitions(&pool, 6).await.unwrap();
    let names = partitions(&pool).await;
    for offset in [0, 1, 2, 3, 5] {
        let wanted = format!("prefix_history_{}", month(&pool, offset).await);
        assert!(names.contains(&wanted), "{wanted} in {names:?}");
    }
    assert!(
        !names.contains(&format!("prefix_history_{busy}")),
        "{names:?}"
    );
    // Idempotent.
    history_partitions(&pool, 6).await.unwrap();
    assert_eq!(partitions(&pool).await, names);

    // A version replaced from now on lands in the partition of the current month.
    sqlx::raw_sql(
        "INSERT INTO addressing_domain (id, slug, valid_period, changeset_id)
         VALUES (gen_random_uuid(), 'lab', tstzrange(now(), NULL), gen_random_uuid())",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::raw_sql("DELETE FROM addressing_domain")
        .execute(&pool)
        .await
        .unwrap();
    let landed: String = sqlx::query_scalar(
        "SELECT tableoid::regclass::text FROM addressing_domain_history LIMIT 1",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        landed,
        format!("addressing_domain_history_{}", month(&pool, 0).await)
    );
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn every_history_has_its_system_period_index(pool: PgPool) {
    let missing: Vec<String> = sqlx::query_scalar(
        "SELECT h.relname::text FROM pg_class h
         WHERE h.relkind = 'p' AND h.relname LIKE '%\\_history'
           AND NOT EXISTS (SELECT FROM pg_indexes i
                           WHERE i.tablename = h.relname AND i.indexdef LIKE '%gist%sys_start, sys_end%')
         ORDER BY 1",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert!(missing.is_empty(), "{missing:?}");
    // A table versioned from now on gets it too.
    sqlx::raw_sql(
        "CREATE TABLE demo (id uuid NOT NULL, valid_period tstzrange NOT NULL,
                            sys_start timestamptz NOT NULL DEFAULT now(), changeset_id uuid NOT NULL);
         CALL kartenn_make_versioned('demo');",
    )
    .execute(&pool)
    .await
    .unwrap();
    let indexed: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT FROM pg_indexes WHERE tablename = 'demo_history'
                                                 AND indexdef LIKE '%gist%')",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(indexed);
}
