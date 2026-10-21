//! Dated reads before and after a retroactive correction (REQ-004-17, 18).

use kartenn_store_pg::temporal::{AsOf, history_of, select_at};
use sqlx::types::Uuid;
use sqlx::types::time::OffsetDateTime;
use sqlx::{PgPool, Row};

async fn setup(pool: &PgPool) -> sqlx::Result<()> {
    sqlx::raw_sql(
        "CREATE TABLE item (
            id uuid NOT NULL, usage text NOT NULL, valid_period tstzrange NOT NULL,
            sys_start timestamptz NOT NULL DEFAULT now(), changeset_id uuid NOT NULL,
            PRIMARY KEY (id, valid_period WITHOUT OVERLAPS));
         CALL kartenn_make_versioned('item');",
    )
    .execute(pool)
    .await?;
    Ok(())
}

async fn now(pool: &PgPool) -> sqlx::Result<OffsetDateTime> {
    sqlx::query_scalar("SELECT clock_timestamp()")
        .fetch_one(pool)
        .await
}

async fn usage_at(
    pool: &PgPool,
    valid_at: OffsetDateTime,
    as_of: Option<OffsetDateTime>,
) -> sqlx::Result<Vec<String>> {
    let (sql, past) = match as_of {
        None => (select_at("item", &["usage"], AsOf::Now, "").unwrap(), None),
        Some(t) => (
            select_at("item", &["usage"], AsOf::Past, "").unwrap(),
            Some(t),
        ),
    };
    let mut q = sqlx::query_scalar(sql).bind(valid_at);
    if let Some(t) = past {
        q = q.bind(t);
    }
    q.fetch_all(pool).await
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn as_of_sees_what_was_asserted(pool: PgPool) -> sqlx::Result<()> {
    setup(&pool).await?;
    let id = Uuid::now_v7();
    // Changeset 1: the item is a host since 2026-01-01.
    sqlx::query("INSERT INTO item (id, usage, valid_period, changeset_id) VALUES ($1, 'host', tstzrange('2026-01-01', NULL), $2)")
        .bind(id)
        .bind(Uuid::now_v7())
        .execute(&pool)
        .await?;
    let between = now(&pool).await?;
    // Changeset 2: retroactive correction, it was in fact reserved over the whole period.
    sqlx::query("UPDATE item SET usage = 'reserved', changeset_id = $2 WHERE id = $1")
        .bind(id)
        .bind(Uuid::now_v7())
        .execute(&pool)
        .await?;
    let march = OffsetDateTime::from_unix_timestamp(1_772_323_200).unwrap(); // 2026-03-01
    assert_eq!(
        usage_at(&pool, march, None).await?,
        vec!["reserved"],
        "current assertion"
    );
    assert_eq!(
        usage_at(&pool, march, Some(between)).await?,
        vec!["host"],
        "what kartenn said before the correction"
    );
    assert_eq!(
        usage_at(&pool, march, Some(now(&pool).await?)).await?,
        vec!["reserved"]
    );
    let before_any = OffsetDateTime::from_unix_timestamp(0).unwrap();
    assert!(
        usage_at(&pool, march, Some(before_any)).await?.is_empty(),
        "nothing asserted in 1970"
    );
    let december = OffsetDateTime::from_unix_timestamp(1_764_547_200).unwrap(); // 2025-12-01
    assert!(
        usage_at(&pool, december, None).await?.is_empty(),
        "not valid before 2026"
    );

    let versions = sqlx::query(history_of("item", &["usage"]).unwrap())
        .bind(id)
        .fetch_all(&pool)
        .await?;
    assert_eq!(versions.len(), 2);
    assert_eq!(versions[0].get::<String, _>("usage"), "host");
    assert!(
        versions[0]
            .get::<Option<OffsetDateTime>, _>("sys_end")
            .is_some()
    );
    assert_eq!(versions[1].get::<String, _>("usage"), "reserved");
    assert!(
        versions[1]
            .get::<Option<OffsetDateTime>, _>("sys_end")
            .is_none(),
        "current version"
    );
    assert_ne!(
        versions[0].get::<Uuid, _>("changeset_id"),
        versions[1].get::<Uuid, _>("changeset_id")
    );
    Ok(())
}
