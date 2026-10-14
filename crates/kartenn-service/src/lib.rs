//! Use cases and the changeset write pipeline of kartenn (spec 004, ADR 0006).
//!
//! Every write is a [`ChangesetRequest`]: operations validated, then applied atomically
//! through a storage port ([`Store`], [`WriteTx`]) implemented by an adapter
//! (`kartenn-store-pg`). This module owns the orchestration; the adapter owns SQL.

mod global;
pub mod operations;
pub mod pipeline;
pub mod port;

pub use operations::{ChangesetRequest, ObjectKind, Operation, Ref};
pub use pipeline::{Outcome, RequestError, submit};
pub use port::{Replay, Store, StoreError, WriteError, WriteTx};
