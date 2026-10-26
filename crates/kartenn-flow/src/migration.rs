//! The flows a machine is an endpoint of, closed by the migration job (task 011-14,
//! F21): a machine is no longer an endpoint, and the project is not frozen, so no alias
//! keeps them. One system changeset per batch of operations closes each flow living or to
//! come whose last version names a machine, by its identifier; the history keeps them. A
//! request pending a review that names a machine is refused at its application
//! (`flow.endpoint.machine`). A second run finds nothing and submits nothing.

use kartenn_service::authz::Unrestricted;
use kartenn_service::declarations::SYSTEM_AUTHOR;
use kartenn_service::{
    ChangesetRequest, ObjectKind, Operation, Outcome, Ref, RequestError, Store, StoreError,
};
use sqlx::PgPool;
use uuid::Uuid;

use crate::module::FLOW;

/// The motive of the changesets that close the flows of a machine.
pub const MACHINE_MOTIVE: &str = "flow endpoints without machine 011";

/// The flows living now or to come whose last version has a machine as an endpoint, by
/// identifier.
///
/// # Errors
///
/// A failure of the database.
pub async fn machine_flows(pool: &PgPool) -> Result<Vec<Uuid>, StoreError> {
    sqlx::query_scalar(
        "SELECT DISTINCT ON (f.id) f.id FROM flow.flow f
          WHERE (f.source_kind = 'machine' OR f.destination_kind = 'machine')
            AND (upper(f.valid_period) IS NULL OR upper(f.valid_period) > now())
          ORDER BY f.id",
    )
    .fetch_all(pool)
    .await
    .map_err(|e| StoreError(e.to_string()))
}

/// Closes the flows of [`machine_flows`] (F21), by system changesets of at most
/// `max_operations` closures each; called by the migration job. Gives the flows closed and
/// the outcome of each changeset, nothing when no flow names a machine.
///
/// # Errors
///
/// [`RequestError`] on infrastructure failure.
///
/// # Panics
///
/// Never: the type of a flow is a qualified name.
pub async fn close_machine_flows<S: Store>(
    store: &S,
    pool: &PgPool,
    max_operations: usize,
) -> Result<(Vec<Uuid>, Vec<Outcome>), RequestError> {
    let flows = machine_flows(pool).await?;
    let kind = ObjectKind::from_name(FLOW).expect("the type of a flow is declared");
    let mut outcomes = Vec::new();
    for batch in flows.chunks(max_operations.max(1)) {
        let operations = batch
            .iter()
            .map(|id| Operation::Close {
                kind,
                id: Ref::Id(*id),
                expected: None,
                cascade: false,
            })
            .collect();
        let request = ChangesetRequest {
            author: SYSTEM_AUTHOR,
            motive: MACHINE_MOTIVE.into(),
            period: None,
            operations,
            simulate: false,
            idempotency_key: None,
            approval_self: true,
        };
        outcomes
            .push(kartenn_service::submit(store, &Unrestricted, &request, max_operations).await?);
    }
    Ok((flows, outcomes))
}
