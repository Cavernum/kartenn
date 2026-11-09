//! Kubernetes module of kartenn (spec 019), pure domain: names and labels of Kubernetes
//! objects (`names`), label selectors (`selector`), the climb of `ownerReferences` to the
//! workload of a pod (`owner`, 019/X3), the projection of an observed object on its
//! tracked fields (`tracked`, 019/X4), and the rules of the module (`rules`). No database,
//! no network: the handler, the storage and the connector build on these types.

pub mod names;
pub mod owner;
pub mod rules;
pub mod selector;
pub mod tracked;

pub use names::{DnsLabel, DnsSubdomain, LabelKey, LabelValue, Labels};
pub use owner::{Owned, OwnerChainError, OwnerReference, WorkloadKind, WorkloadRef, workload_of};
pub use selector::{Requirement, Selector};
