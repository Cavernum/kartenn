//! The handler of the module in the write pipeline (ADR 0026, clause 6). For now it
//! carries the references of the endpoints (REQ-011-3, task 011-4): closing an endpoint
//! of a living flow is refused, or closes the flow under the cascade. The operations
//! `flow.flow.*` come with task 011-3.

use kartenn_core::period::Period;
use kartenn_core::rules::Violation;
use kartenn_service::module::{BoxFuture, ModuleHandler, ModulePlan, ModuleTx, Reference};
use kartenn_service::operations::{ModuleKind, ModuleOperation, Ref};
use kartenn_service::port::{StoreError, WriteError};
use sqlx::PgConnection;
use uuid::Uuid;

use crate::pg;
use crate::rules::FIELD_INVALID;

/// The handler of the module.
#[derive(Debug, Clone, Copy, Default)]
pub struct Handler;

fn connection(tx: &mut dyn ModuleTx) -> Result<&mut PgConnection, StoreError> {
    tx.connection()
        .downcast_mut()
        .ok_or_else(|| StoreError("not a PostgreSQL transaction".into()))
}

impl ModuleHandler for Handler {
    fn plan(
        &self,
        operation: &ModuleOperation,
        object: Uuid,
        resolve: &dyn Fn(Ref) -> Option<Uuid>,
    ) -> Result<ModulePlan, Violation> {
        let _ = (object, resolve);
        Err(Violation::new(
            &FIELD_INVALID,
            format!("{}: unknown operation", operation.tag()),
        ))
    }

    fn apply<'a>(
        &'a self,
        tx: &'a mut dyn ModuleTx,
        plan: &'a ModulePlan,
        period: Option<Period>,
        changeset: Uuid,
    ) -> BoxFuture<'a, Result<(), WriteError>> {
        let _ = (tx, period, changeset);
        Box::pin(async move {
            Err(WriteError::Store(StoreError(format!(
                "{}: no operation of the module writes it yet",
                plan.kind
            ))))
        })
    }

    fn close<'a>(
        &'a self,
        tx: &'a mut dyn ModuleTx,
        kind: ModuleKind,
        id: Uuid,
        period: Option<Period>,
        changeset: Uuid,
    ) -> BoxFuture<'a, Result<(), WriteError>> {
        let _ = kind;
        Box::pin(async move {
            tx.close_versioned(pg::FLOW_TABLE, id, period, changeset)
                .await?;
            let (at, _) = pg::bounds(period);
            pg::close_dependents(connection(tx)?, id, at, changeset).await?;
            Ok(())
        })
    }

    fn restore<'a>(
        &'a self,
        tx: &'a mut dyn ModuleTx,
        kind: ModuleKind,
        id: Uuid,
        before: Uuid,
        period: Option<Period>,
        changeset: Uuid,
    ) -> BoxFuture<'a, Result<(), WriteError>> {
        let _ = kind;
        tx.restore_versioned(pg::FLOW_TABLE, id, before, period, changeset)
    }

    fn referrers<'a>(
        &'a self,
        tx: &'a mut dyn ModuleTx,
        reference: &'a Reference,
        target: Uuid,
        period: Option<Period>,
    ) -> BoxFuture<'a, Result<Vec<Uuid>, StoreError>> {
        Box::pin(async move {
            let (at, _) = pg::bounds(period);
            pg::referrers(connection(tx)?, reference.to, target, at).await
        })
    }
}
