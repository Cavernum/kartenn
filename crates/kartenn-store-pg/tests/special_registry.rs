//! The registries in the database classify exactly like kartenn-core (REQ-002-33, 34).

use kartenn_core::addr::{Addr, Prefix};
use kartenn_core::family::{ALL, IPV4, IPV6};
use kartenn_core::registry::special_registry;
use kartenn_store_pg::registry::{load, public_drift};
use sqlx::PgPool;
use sqlx::types::Uuid;

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn loading_is_idempotent(pool: PgPool) -> sqlx::Result<()> {
    let v4 = special_registry(IPV4).unwrap();
    assert_eq!(load(&pool, v4).await?, 26);
    assert_eq!(load(&pool, v4).await?, 0, "second load inserts nothing");
    Ok(())
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn sql_and_rust_agree_on_public_space(pool: PgPool) -> sqlx::Result<()> {
    for family in [IPV4, IPV6] {
        load(&pool, special_registry(family).unwrap()).await?;
    }
    // Every block of both registries, its first and last addresses, a little outside each
    // block, and a few ordinary addresses.
    let mut samples: Vec<Prefix> = Vec::new();
    for family in [IPV4, IPV6] {
        for e in special_registry(family).unwrap().entries() {
            samples.push(e.block);
            samples.push(Prefix::host(e.block.network()));
            samples.push(Prefix::host(e.block.last()));
            if let Some(after) = e.block.last().checked_add(1) {
                samples.push(Prefix::host(after));
            }
            if let Some(before) = e.block.network().checked_sub(1) {
                samples.push(Prefix::host(before));
            }
        }
    }
    for t in [
        "8.8.8.8",
        "10.1.2.3",
        "2a00::1",
        "fd12::1",
        "100.64.0.1",
        "192.0.0.9",
    ] {
        samples.push(Prefix::host(Addr::parse_any(&ALL, t).unwrap()));
    }
    for p in samples {
        let rust = special_registry(p.family()).unwrap().is_public(p);
        let sql: bool = sqlx::query_scalar("SELECT kartenn_is_public($1::inet, $2)")
            .bind(p.to_string())
            .bind(kartenn_core::registry::UPDATED)
            .fetch_one(&pool)
            .await?;
        assert_eq!(sql, rust, "{p}");
    }
    Ok(())
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn drift_reports_objects_whose_flag_is_stale(pool: PgPool) -> sqlx::Result<()> {
    for family in [IPV4, IPV6] {
        load(&pool, special_registry(family).unwrap()).await?;
    }
    let stale = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO address (id, domain_id, address, usage, public, registry_version, valid_period, changeset_id) VALUES
         ($1, gen_random_uuid(), '8.8.8.8', 'host', false, 'old', tstzrange(now(), NULL), gen_random_uuid()),
         (gen_random_uuid(), gen_random_uuid(), '10.0.0.1', 'host', false, 'old', tstzrange(now(), NULL), gen_random_uuid())",
    )
    .bind(stale)
    .execute(&pool)
    .await?;
    let range = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO ip_range (id, domain_id, range, usage, public, registry_version, valid_period, changeset_id)
         VALUES ($1, gen_random_uuid(), '[8.8.8.1,8.8.8.9]', 'static', false, 'old', tstzrange(now(), NULL), gen_random_uuid())",
    )
    .bind(range)
    .execute(&pool)
    .await?;
    let mut drift = public_drift(&pool, kartenn_core::registry::UPDATED).await?;
    drift.sort_by_key(|d| d.table);
    assert_eq!(
        drift
            .iter()
            .map(|d| (d.table, d.id, d.public))
            .collect::<Vec<_>>(),
        vec![("address", stale, true), ("ip_range", range, true)]
    );
    Ok(())
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn the_migration_job_loads_the_embedded_registries(pool: PgPool) -> sqlx::Result<()> {
    let count = |pool: PgPool| async move {
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM special_registry WHERE registry_version = $1",
        )
        .bind(kartenn_core::registry::UPDATED)
        .fetch_one(&pool)
        .await
    };
    kartenn_store_pg::maintenance::deploy(&pool).await.unwrap();
    let expected: usize = [IPV4, IPV6]
        .iter()
        .map(|f| special_registry(*f).unwrap().entries().len())
        .sum();
    assert_eq!(count(pool.clone()).await?, i64::try_from(expected).unwrap());
    kartenn_store_pg::maintenance::deploy(&pool).await.unwrap();
    assert_eq!(
        count(pool.clone()).await?,
        i64::try_from(expected).unwrap(),
        "a second deployment loads nothing more"
    );
    Ok(())
}
