//! Use cases and the changeset write pipeline of kartenn (spec 004, ADR 0006).
//!
//! Every write is a [`ChangesetRequest`]: operations validated, then applied atomically
//! through a storage port ([`Store`], [`WriteTx`]) implemented by an adapter
//! (`kartenn-store-pg`). This module owns the orchestration; the adapter owns SQL.

pub mod aggregate;
pub mod audit;
pub mod authn;
pub mod authz;
mod cascade;
pub mod declarations;
pub mod expected;
mod global;
pub mod import;
pub mod module;
pub mod observability;
pub mod operations;
pub mod pipeline;
pub mod port;
pub mod rbac;
pub mod reads;
mod resolution;
pub mod review;
mod rights;
pub mod undo;

pub use operations::{ChangesetRequest, ObjectKind, Operation, Ref};
pub use pipeline::{Outcome, RequestError, submit, submit_in, submit_with};
pub use port::{Replay, Store, StoreError, WriteError, WriteTx};
