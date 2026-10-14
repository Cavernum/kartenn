//! Last-line-of-defence constraints of the property tables (spec 003, task 003-6).

use sqlx::PgPool;
use sqlx::types::Uuid;

const FOREVER: &str = "tstzrange('2026-01-01', NULL)";

async fn exec(pool: &PgPool, sql: &str) -> sqlx::Result<()> {
    sqlx::raw_sql(sqlx::AssertSqlSafe(sql.to_owned()))
        .execute(pool)
        .await
        .map(|_| ())
}

async fn declare(pool: &PgPool, key: &str, period: &str) -> sqlx::Result<Uuid> {
    sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
        "INSERT INTO property_declaration (id, key, value_type, cardinality, carriers, inherit_tree, valid_period, changeset_id)
         VALUES (gen_random_uuid(), $1, 'enum', 'single', ARRAY['prefix', 'address'], 'addressing', {period}, gen_random_uuid())
         RETURNING id"
    )))
    .bind(key)
    .fetch_one(pool)
    .await
}

/// Inserts an assignment: `column` lists the value columns, `value` their SQL values.
async fn assign(
    pool: &PgPool,
    object: Uuid,
    property: Uuid,
    kind: &str,
    column: &str,
    value: &str,
    period: &str,
) -> sqlx::Result<()> {
    exec(
        pool,
        &format!(
            "INSERT INTO property_assignment (object_id, object_type, property_id, kind, {column}, provenance, valid_period, changeset_id)
             VALUES ('{object}', 'prefix', '{property}', '{kind}', {value}, 'input', {period}, gen_random_uuid())"
        ),
    )
    .await
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn declarations_have_unique_well_formed_keys(pool: PgPool) -> sqlx::Result<()> {
    declare(&pool, "custom.environment", FOREVER).await?;
    assert!(
        declare(&pool, "custom.environment", "tstzrange('2027-01-01', NULL)")
            .await
            .is_err(),
        "one key per period"
    );
    declare(
        &pool,
        "custom.environment",
        "tstzrange('2025-01-01', '2026-01-01')",
    )
    .await?;
    for bad in [
        "environment",
        "Custom.env",
        "custom.env-x",
        "custom.",
        "1x.env",
    ] {
        assert!(declare(&pool, bad, FOREVER).await.is_err(), "{bad}");
    }
    assert!(
        exec(
            &pool,
            &format!(
                "INSERT INTO property_declaration (id, key, value_type, cardinality, carriers, expected_mode, valid_period, changeset_id)
                 VALUES (gen_random_uuid(), 'custom.x', 'text', 'single', ARRAY['prefix'], 'check', {FOREVER}, gen_random_uuid())"
            ),
        )
        .await
        .is_err(),
        "a mode needs a source"
    );
    assert!(
        exec(
            &pool,
            &format!(
                "INSERT INTO property_declaration (id, key, value_type, cardinality, carriers, valid_period, changeset_id)
                 VALUES (gen_random_uuid(), 'custom.y', 'text', 'single', ARRAY[]::text[], {FOREVER}, gen_random_uuid())"
            ),
        )
        .await
        .is_err(),
        "at least one carrier"
    );
    Ok(())
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn closed_list_values_and_obligations(pool: PgPool) -> sqlx::Result<()> {
    let p = declare(&pool, "custom.environment", FOREVER).await?;
    let value = |key: &str| {
        format!(
            "INSERT INTO property_enum_value (id, property_id, key, valid_period, changeset_id)
             VALUES (gen_random_uuid(), '{p}', '{key}', {FOREVER}, gen_random_uuid())"
        )
    };
    exec(&pool, &value("prod")).await?;
    assert!(
        exec(&pool, &value("prod")).await.is_err(),
        "unique per property"
    );
    for bad in ["Prod", "-prod", "prod-", "pr od"] {
        assert!(exec(&pool, &value(bad)).await.is_err(), "{bad}");
    }
    let obligation = |kind: &str, id: &str, ty: &str| {
        format!(
            "INSERT INTO property_obligation (id, property_id, scope_kind, scope_id, scope_type, valid_period, changeset_id)
             VALUES (gen_random_uuid(), '{p}', '{kind}', {id}, {ty}, {FOREVER}, gen_random_uuid())"
        )
    };
    let d = format!("'{}'", Uuid::now_v7());
    exec(&pool, &obligation("domain", &d, "NULL")).await?;
    assert!(
        exec(&pool, &obligation("domain", &d, "NULL"))
            .await
            .is_err()
    );
    exec(&pool, &obligation("object_type", "NULL", "'address'")).await?;
    assert!(
        exec(&pool, &obligation("object_type", &d, "'address'"))
            .await
            .is_err()
    );
    assert!(
        exec(&pool, &obligation("entity", "NULL", "NULL"))
            .await
            .is_err()
    );
    Ok(())
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn one_assignment_per_object_and_property(pool: PgPool) -> sqlx::Result<()> {
    let p = declare(&pool, "custom.environment", FOREVER).await?;
    let o = Uuid::now_v7();
    let v = format!("'{}'", Uuid::now_v7());
    assign(&pool, o, p, "value", "v_uuid", &v, FOREVER).await?;
    assert!(
        assign(
            &pool,
            o,
            p,
            "value",
            "v_uuid",
            &v,
            "tstzrange('2027-01-01', NULL)"
        )
        .await
        .is_err(),
        "REQ-003-12: one assignment per period"
    );
    let o2 = Uuid::now_v7();
    assign(&pool, o2, p, "cleared", "v_uuid", "NULL", FOREVER).await?;
    let o3 = Uuid::now_v7();
    assert!(
        assign(&pool, o3, p, "value", "v_uuid", "NULL", FOREVER)
            .await
            .is_err(),
        "a value needs a value"
    );
    assert!(
        assign(&pool, o3, p, "cleared", "v_int", "3", FOREVER)
            .await
            .is_err(),
        "cleared carries no value"
    );
    assert!(
        assign(&pool, o3, p, "value", "v_int, v_text", "3, 'x'", FOREVER)
            .await
            .is_err(),
        "one value column"
    );
    assign(
        &pool,
        o3,
        p,
        "value",
        "v_uuids",
        &format!("ARRAY[{v}]::uuid[]"),
        FOREVER,
    )
    .await?;
    assert!(
        exec(
            &pool,
            &format!(
                "INSERT INTO property_assignment (object_id, object_type, property_id, kind, v_int, provenance, valid_period, changeset_id)
                 VALUES (gen_random_uuid(), 'prefix', '{p}', 'value', 1, 'rules', {FOREVER}, gen_random_uuid())"
            ),
        )
        .await
        .is_err(),
        "provenance is input or source:<id>@<version>"
    );

    // History is keyed by (object, property).
    exec(
        &pool,
        &format!(
            "UPDATE property_assignment SET v_uuid = gen_random_uuid() WHERE object_id = '{o}'"
        ),
    )
    .await?;
    let historised: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM property_assignment_history WHERE object_id = $1 AND property_id = $2",
    )
    .bind(o)
    .bind(p)
    .fetch_one(&pool)
    .await?;
    assert_eq!(historised, 1);
    Ok(())
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn one_origin_per_object_property_and_segment(pool: PgPool) -> sqlx::Result<()> {
    let p = declare(&pool, "custom.environment", FOREVER).await?;
    let (o, origin) = (Uuid::now_v7(), Uuid::now_v7());
    let insert = |origin: String, period: &str| {
        format!(
            "INSERT INTO property_origin (object_id, property_id, origin_object_id, valid_period, changeset_id)
             VALUES ('{o}', '{p}', {origin}, {period}, gen_random_uuid())"
        )
    };
    exec(
        &pool,
        &insert(
            format!("'{origin}'"),
            "tstzrange('2026-01-01', '2026-06-01')",
        ),
    )
    .await?;
    exec(
        &pool,
        &insert("NULL".into(), "tstzrange('2026-06-01', NULL)"),
    )
    .await?;
    assert!(
        exec(
            &pool,
            &insert(format!("'{o}'"), "tstzrange('2026-03-01', NULL)")
        )
        .await
        .is_err(),
        "segments do not overlap"
    );
    exec(&pool, "DELETE FROM property_origin").await?;
    let historised: i64 = sqlx::query_scalar("SELECT count(*) FROM property_origin_history")
        .fetch_one(&pool)
        .await?;
    assert_eq!(historised, 2);
    Ok(())
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn keyed_versioning_checks_its_key(pool: PgPool) -> sqlx::Result<()> {
    exec(
        &pool,
        "CREATE TABLE plain_keyed (a uuid, valid_period tstzrange, sys_start timestamptz, changeset_id uuid)",
    )
    .await?;
    let err = exec(
        &pool,
        "CALL kartenn_make_versioned('plain_keyed', ARRAY['a', 'b'])",
    )
    .await
    .expect_err("refused");
    assert!(
        err.to_string().contains("lacks the versioning columns b"),
        "{err}"
    );
    Ok(())
}
