//! Operations of a changeset (REQ-004-1 to 5).

use kartenn_core::objects::{AddressUsage, Nature, RangeUsage};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Designation of an object inside a changeset: an existing UUID, or the local name of an
/// object created by an earlier operation of the same changeset (REQ-004-4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Ref {
    /// Existing object.
    Id(Uuid),
    /// Object created earlier in the changeset under this local name.
    Local(u32),
}

/// Kind of an object targeted by a generic operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObjectKind {
    /// Addressing domain.
    Domain,
    /// Prefix.
    Prefix,
    /// Range of addresses.
    Range,
    /// Address.
    Address,
}

/// Application-time period of an operation, in microseconds since the epoch; `None` end
/// means unbounded. Validated when the changeset is decoded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PeriodSpec {
    /// Start, included.
    pub start: i64,
    /// End, excluded; `None` for unbounded.
    pub end: Option<i64>,
}

/// One operation of a changeset.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Operation {
    /// Creates an addressing domain.
    CreateDomain {
        /// Local name for later operations.
        local: Option<u32>,
        /// Technical identifier.
        slug: String,
    },
    /// Creates a prefix.
    CreatePrefix {
        /// Local name for later operations.
        local: Option<u32>,
        /// Domain.
        domain: Ref,
        /// `address/length`.
        prefix: String,
        /// Container or network.
        nature: Nature,
        /// Point-to-point network.
        #[serde(default)]
        point_to_point: bool,
        /// Anycast network (REQ-002-46, 49).
        #[serde(default)]
        anycast: bool,
    },
    /// Creates a range of addresses inside a network.
    CreateRange {
        /// Local name for later operations.
        local: Option<u32>,
        /// Domain.
        domain: Ref,
        /// First address.
        first: String,
        /// Last address, included.
        last: String,
        /// DHCP, reserved or static.
        usage: RangeUsage,
    },
    /// Creates an address.
    CreateAddress {
        /// Local name for later operations.
        local: Option<u32>,
        /// Domain.
        domain: Ref,
        /// Address.
        address: String,
        /// Usage.
        usage: AddressUsage,
    },
    /// Ends an object (REQ-004-15); nothing before the end is erased.
    Close {
        /// Kind of the object.
        kind: ObjectKind,
        /// Object.
        id: Ref,
        /// Changeset that last wrote the object, as the caller saw it (REQ-004-11):
        /// a different current version is a conflict.
        #[serde(default)]
        expected: Option<Uuid>,
    },
}

impl Operation {
    /// Local name this operation defines, if any.
    #[must_use]
    pub const fn defines(&self) -> Option<u32> {
        match self {
            Self::CreateDomain { local, .. }
            | Self::CreatePrefix { local, .. }
            | Self::CreateRange { local, .. }
            | Self::CreateAddress { local, .. } => *local,
            Self::Close { .. } => None,
        }
    }
}

/// A changeset submitted by an authenticated person (REQ-004-1).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChangesetRequest {
    /// Author (person identifier).
    pub author: Uuid,
    /// Why; mandatory.
    pub motive: String,
    /// Default application period; `None` means `[transaction time, ∞)` (REQ-004-3).
    pub period: Option<PeriodSpec>,
    /// Operations, applied in order.
    pub operations: Vec<Operation>,
    /// Validate and report without writing anything (REQ-004-9).
    #[serde(default)]
    pub simulate: bool,
    /// Idempotency key (REQ-004-10): a second submission with the same key by the same
    /// author within 24 h returns the first result.
    #[serde(default)]
    pub idempotency_key: Option<String>,
}
