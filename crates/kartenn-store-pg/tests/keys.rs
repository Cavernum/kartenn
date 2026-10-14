//! Natural keys and external identifiers resolved at a date (REQ-002-40, 41).

use kartenn_store_pg::keys::{
    SlugKind, address_by_value, bind_external, by_external, by_slug, prefix_by_value,
};
use sqlx::PgPool;
use sqlx::types::Uuid;
use sqlx::types::time::OffsetDateTime;

const MARCH_2026: i64 = 1_772_323_200;
const JULY_2026: i64 = 1_782_864_000;
const MARCH_2025: i64 = 1_740_787_200;

fn day(unix: i64) -> OffsetDateTime {
    OffsetDateTime::from_unix_timestamp(unix).unwrap()
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn slugs_resolve_at_a_date_and_follow_renames(pool: PgPool) -> sqlx::Result<()> {
    let id = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO addressing_domain (id, slug, valid_period, changeset_id) VALUES
         ($1, 'prod', tstzrange('2026-01-01', '2026-06-01'), gen_random_uuid()),
         ($1, 'prod-eu', tstzrange('2026-06-01', NULL), gen_random_uuid())",
    )
    .bind(id)
    .execute(&pool)
    .await?;
    assert_eq!(
        by_slug(&pool, SlugKind::Domain, "prod", day(MARCH_2026)).await?,
        Some(id)
    );
    assert_eq!(
        by_slug(&pool, SlugKind::Domain, "prod", day(JULY_2026)).await?,
        None
    );
    assert_eq!(
        by_slug(&pool, SlugKind::Domain, "prod-eu", day(JULY_2026)).await?,
        Some(id),
        "same UUID after rename"
    );
    assert_eq!(
        by_slug(&pool, SlugKind::Entity, "prod-eu", day(JULY_2026)).await?,
        None,
        "per kind"
    );
    Ok(())
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn addressing_keys_are_per_domain(pool: PgPool) -> sqlx::Result<()> {
    let (d1, d2, p1, a1) = (
        Uuid::now_v7(),
        Uuid::now_v7(),
        Uuid::now_v7(),
        Uuid::now_v7(),
    );
    sqlx::query(
        "INSERT INTO prefix (id, domain_id, prefix, nature, public, registry_version, valid_period, changeset_id)
         VALUES ($1, $2, '10.1.2.0/24', 'network', false, 'v', tstzrange('2026-01-01', NULL), gen_random_uuid());
         ",
    )
    .bind(p1)
    .bind(d1)
    .execute(&pool)
    .await?;
    sqlx::query(
        "INSERT INTO address (id, domain_id, address, usage, public, registry_version, valid_period, changeset_id)
         VALUES ($1, $2, '10.1.2.3', 'host', false, 'v', tstzrange('2026-01-01', NULL), gen_random_uuid())",
    )
    .bind(a1)
    .bind(d1)
    .execute(&pool)
    .await?;
    let at = day(MARCH_2026);
    assert_eq!(
        prefix_by_value(&pool, d1, "10.1.2.0/24", at).await?,
        Some(p1)
    );
    assert_eq!(prefix_by_value(&pool, d2, "10.1.2.0/24", at).await?, None);
    assert_eq!(address_by_value(&pool, d1, "10.1.2.3", at).await?, Some(a1));
    assert_eq!(
        address_by_value(&pool, d1, "10.1.2.3", day(MARCH_2025)).await?,
        None
    );
    Ok(())
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn external_identifiers(pool: PgPool) -> sqlx::Result<()> {
    let (o1, o2) = (Uuid::now_v7(), Uuid::now_v7());
    bind_external(&pool, "legacy-ipam", "NET-42", o1, "prefix").await?;
    assert!(
        bind_external(&pool, "legacy-ipam", "NET-42", o2, "prefix")
            .await
            .is_err(),
        "REQ-002-41"
    );
    bind_external(&pool, "cmdb", "NET-42", o2, "prefix").await?;
    assert_eq!(
        by_external(&pool, "legacy-ipam", "NET-42").await?,
        Some((o1, "prefix".to_owned()))
    );
    assert_eq!(
        by_external(&pool, "cmdb", "NET-42").await?.map(|(o, _)| o),
        Some(o2)
    );
    assert_eq!(by_external(&pool, "cmdb", "NET-43").await?, None);
    Ok(())
}
