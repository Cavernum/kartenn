//! What the subjects share: the connector of the clusters, a service account member of the
//! group they designate (019/X5, X10), which alone writes their mirrors by `kube.sync`.

use kartenn_service::Operation;
use sqlx::PgPool;
use sqlx::types::Uuid;

/// A connector group `key` with a new service account as its only member: `(account,
/// group)`.
pub(crate) async fn connector(pool: &PgPool, key: &str) -> (Uuid, Uuid) {
    let account: Uuid = sqlx::query_scalar(
        "INSERT INTO person (id, display_name, kind)
         VALUES (gen_random_uuid(), $1, 'service_account') RETURNING id",
    )
    .bind(key)
    .fetch_one(pool)
    .await
    .unwrap();
    let group: Uuid = sqlx::query_scalar(
        "INSERT INTO rbac_group (id, key, labels, valid_period, changeset_id)
         VALUES (gen_random_uuid(), $1, '{}', tstzrange(now() - interval '1 day', NULL),
                 gen_random_uuid())
         RETURNING id",
    )
    .bind(key)
    .fetch_one(pool)
    .await
    .unwrap();
    member(pool, group, account).await;
    (account, group)
}

/// `identity` made a direct member of `group`.
pub(crate) async fn member(pool: &PgPool, group: Uuid, identity: Uuid) {
    sqlx::query(
        "INSERT INTO group_member (id, group_id, member_kind, identity_id, valid_period,
                                   changeset_id)
         VALUES (gen_random_uuid(), $1, 'identity', $2,
                 tstzrange(now() - interval '1 day', NULL), gen_random_uuid())",
    )
    .bind(group)
    .bind(identity)
    .execute(pool)
    .await
    .unwrap();
}

/// Whether `operations` are a batch of the connector, which its account submits.
pub(crate) fn batch(operations: &[Operation]) -> bool {
    !operations.is_empty()
        && operations
            .iter()
            .all(|o| matches!(o, Operation::Module(m) if m.module == "kube" && m.name == "sync"))
}
