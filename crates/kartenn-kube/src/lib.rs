//! Kubernetes module of kartenn (spec 019): names and labels of Kubernetes objects
//! (`names`), label selectors (`selector`), the climb of `ownerReferences` to the workload
//! of a pod (`owner`, 019/X3), the projection of an observed object on its tracked fields
//! (`tracked`, 019/X4), and the rules of the module (`rules`), all without database; the
//! module as the core sees it (`module`, ADR 0026), and, with the feature `pg`, its
//! handler in the write pipeline and its storage in PostgreSQL.

mod labels;
pub mod module;
pub mod names;
pub mod owner;
pub mod rules;
pub mod selector;
pub mod tracked;

#[cfg(feature = "pg")]
pub mod handler;
#[cfg(feature = "pg")]
mod pg;

pub use module::Kube;
pub use names::{DnsLabel, DnsSubdomain, LabelKey, LabelValue, Labels};
pub use owner::{Owned, OwnerChainError, OwnerReference, WorkloadKind, WorkloadRef, workload_of};
pub use selector::{Requirement, Selector};
