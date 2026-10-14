//! Last-line-of-defence constraints of the core objects (spec 002, task 002-15).

use sqlx::PgPool;
use sqlx::types::Uuid;

const FOREVER: &str = "tstzrange('2026-01-01', NULL)";

async fn prefix(
    pool: &PgPool,
    domain: Uuid,
    value: &str,
    nature: &str,
    p2p: bool,
    period: &str,
) -> sqlx::Result<()> {
    sqlx::query(sqlx::AssertSqlSafe(format!(
        "INSERT INTO prefix (id, domain_id, prefix, nature, point_to_point, public, registry_version, valid_period, changeset_id)
         VALUES (gen_random_uuid(), $1, $2::cidr, $3, $4, false, '2025-10-09', {period}, gen_random_uuid())"
    )))
    .bind(domain)
    .bind(value)
    .bind(nature)
    .bind(p2p)
    .execute(pool)
    .await
    .map(|_| ())
}

async fn address(
    pool: &PgPool,
    domain: Uuid,
    value: &str,
    usage: &str,
    public: bool,
) -> sqlx::Result<()> {
    sqlx::query(
        "INSERT INTO address (id, domain_id, address, usage, public, registry_version, valid_period, changeset_id)
         VALUES (gen_random_uuid(), $1, $2::inet, $3, $4, '2025-10-09', tstzrange('2026-01-01', NULL), gen_random_uuid())",
    )
    .bind(domain)
    .bind(value)
    .bind(usage)
    .bind(public)
    .execute(pool)
    .await
    .map(|_| ())
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn prefixes_are_unique_per_domain_and_period(pool: PgPool) -> sqlx::Result<()> {
    let (d1, d2) = (Uuid::now_v7(), Uuid::now_v7());
    prefix(
        &pool,
        d1,
        "10.0.0.0/8",
        "container",
        false,
        "tstzrange('2026-01-01', '2026-07-01')",
    )
    .await?;
    assert!(
        prefix(
            &pool,
            d1,
            "10.0.0.0/8",
            "container",
            false,
            "tstzrange('2026-03-01', '2027-01-01')"
        )
        .await
        .is_err(),
        "REQ-002-15"
    );
    prefix(
        &pool,
        d1,
        "10.0.0.0/8",
        "container",
        false,
        "tstzrange('2026-07-01', '2027-01-01')",
    )
    .await?;
    prefix(&pool, d2, "10.0.0.0/8", "container", false, FOREVER).await?;
    prefix(&pool, d1, "10.1.0.0/16", "container", false, FOREVER).await?;
    assert!(
        prefix(&pool, d1, "10.1.2.5/24", "network", false, FOREVER)
            .await
            .is_err(),
        "cidr refuses host bits"
    );
    Ok(())
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn only_networks_are_point_to_point(pool: PgPool) -> sqlx::Result<()> {
    let d = Uuid::now_v7();
    prefix(&pool, d, "192.0.2.0/31", "network", true, FOREVER).await?;
    assert!(
        prefix(&pool, d, "198.51.100.0/31", "container", true, FOREVER)
            .await
            .is_err()
    );
    Ok(())
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn addresses_are_hosts_unique_per_domain_and_public_space(pool: PgPool) -> sqlx::Result<()> {
    let (d1, d2) = (Uuid::now_v7(), Uuid::now_v7());
    address(&pool, d1, "10.1.2.3", "host", false).await?;
    assert!(
        address(&pool, d1, "10.1.2.3", "host", false).await.is_err(),
        "REQ-002-26"
    );
    address(&pool, d2, "10.1.2.3", "host", false).await?;
    assert!(
        address(&pool, d1, "10.1.2.0/24", "host", false)
            .await
            .is_err(),
        "an address is a host value"
    );
    address(&pool, d1, "45.0.0.10", "host", true).await?;
    assert!(
        address(&pool, d2, "45.0.0.10", "host", true).await.is_err(),
        "REQ-002-32"
    );
    address(&pool, d1, "45.0.0.53", "anycast", true).await?;
    address(&pool, d2, "45.0.0.53", "anycast", true).await?;
    assert!(
        address(&pool, d1, "10.9.9.9", "printer", false)
            .await
            .is_err(),
        "closed list of usages"
    );
    Ok(())
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn slugs_are_unique_per_type_and_period(pool: PgPool) -> sqlx::Result<()> {
    let insert = |slug: &str, period: &str| {
        let pool = pool.clone();
        let slug = slug.to_owned();
        let sql = format!(
            "INSERT INTO addressing_domain (id, slug, valid_period, changeset_id)
             VALUES (gen_random_uuid(), $1, {period}, gen_random_uuid())"
        );
        async move {
            sqlx::query(sqlx::AssertSqlSafe(sql))
                .bind(slug)
                .execute(&pool)
                .await
        }
    };
    insert("prod", "tstzrange('2026-01-01', '2026-06-01')").await?;
    assert!(
        insert("prod", "tstzrange('2026-03-01', NULL)")
            .await
            .is_err(),
        "REQ-002-47"
    );
    insert("prod", "tstzrange('2026-06-01', NULL)").await?;
    for bad in ["Prod", "-x", "a_b"] {
        assert!(insert(bad, FOREVER).await.is_err(), "{bad}");
    }
    Ok(())
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn external_ids_are_unique_per_source(pool: PgPool) -> sqlx::Result<()> {
    let q = "INSERT INTO external_id VALUES ('legacy-ipam', 'NET-42', $1, 'prefix')";
    sqlx::query(q).bind(Uuid::now_v7()).execute(&pool).await?;
    assert!(
        sqlx::query(q)
            .bind(Uuid::now_v7())
            .execute(&pool)
            .await
            .is_err(),
        "REQ-002-41"
    );
    Ok(())
}
