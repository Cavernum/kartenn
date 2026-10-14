//! Operations of a changeset (REQ-004-1 to 5).

use kartenn_core::objects::{AddressUsage, Nature, RangeUsage};
use kartenn_core::values::{Cardinality, Raw};
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
    /// Property declaration (spec 003).
    Property,
    /// Value of a closed list.
    EnumValue,
}

/// Type of a declared property and its parameters, as received (003/design).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TypeInput {
    /// Closed list.
    Enum,
    /// Text.
    Text {
        /// Minimum length in code points.
        #[serde(default)]
        min_len: usize,
        /// Maximum length in code points.
        max_len: usize,
        /// Anchored pattern.
        #[serde(default)]
        pattern: Option<String>,
    },
    /// Integer.
    Int {
        /// Minimum.
        min: i64,
        /// Maximum.
        max: i64,
    },
    /// Boolean.
    Bool,
    /// Calendar date.
    Date,
    /// Host name.
    Fqdn,
    /// Reference to an object kind.
    Ref {
        /// Target kind.
        target: String,
    },
    /// Type compiled by a module; only module synchronisation declares it (P1, P12).
    #[serde(skip_deserializing)]
    Module {
        /// Type identifier.
        id: String,
    },
}

/// One raw value of an assignment, as received: text (closed-list key, text, host name,
/// date, reference), integer or boolean (REQ-003-8).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum RawInput {
    /// Boolean.
    Bool(bool),
    /// Integer.
    Int(i64),
    /// Text.
    Text(String),
}

impl RawInput {
    /// The raw scalar the value types parse.
    #[must_use]
    pub fn to_raw(&self) -> Raw {
        match self {
            Self::Bool(b) => Raw::Bool(*b),
            Self::Int(i) => Raw::Int(*i),
            Self::Text(t) => Raw::Text(t.clone()),
        }
    }
}

impl ObjectKind {
    /// Name of the kind, as carriers and references name it.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Domain => "domain",
            Self::Prefix => "prefix",
            Self::Range => "range",
            Self::Address => "address",
            Self::Property => "property",
            Self::EnumValue => "enum_value",
        }
    }

    /// Whether objects of this kind belong to the addressing tree.
    #[must_use]
    pub const fn is_addressing(self) -> bool {
        matches!(
            self,
            Self::Domain | Self::Prefix | Self::Range | Self::Address
        )
    }
}

/// Content of a property declaration, shared by declaration and update.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeclarationInput {
    /// `<namespace>.<name>`.
    pub key: String,
    /// Type and parameters.
    #[serde(flatten)]
    pub value_type: TypeInput,
    /// One value or a set.
    pub cardinality: Cardinality,
    /// Object types that carry it.
    pub carriers: Vec<String>,
    /// Inheritance tree, if heritable.
    #[serde(default)]
    pub inherit: Option<String>,
    /// Module declaring it; never read from the API, set by module synchronisation only
    /// (P12). `None` means an administrator.
    #[serde(skip_deserializing, default)]
    pub module: Option<String>,
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
    /// Declares a property (REQ-003-1 to 3).
    DeclareProperty {
        /// Local name for later operations.
        local: Option<u32>,
        /// Declaration.
        #[serde(flatten)]
        declaration: DeclarationInput,
    },
    /// Replaces the current declaration of a property from the start of the changeset
    /// period, if compatible with its use (REQ-003-6).
    UpdateProperty {
        /// Property.
        id: Ref,
        /// New declaration; the key never changes.
        #[serde(flatten)]
        declaration: DeclarationInput,
    },
    /// Adds a value to a closed list (REQ-003-5, 7).
    AddEnumValue {
        /// Local name for later operations.
        local: Option<u32>,
        /// Property of type `enum`.
        property: Ref,
        /// Key, `[a-z0-9-]`.
        key: String,
    },
    /// Assigns a value to a property of an object over the changeset period, replacing
    /// any assignment there (REQ-003-4, 8, 12).
    AssignProperty {
        /// Object.
        object: Ref,
        /// Its kind.
        kind: ObjectKind,
        /// Property.
        property: Ref,
        /// One value, or the elements of a set.
        values: Vec<RawInput>,
    },
    /// Assigns "no value", which stops inheritance (REQ-003-13).
    ClearProperty {
        /// Object.
        object: Ref,
        /// Its kind.
        kind: ObjectKind,
        /// Property.
        property: Ref,
    },
    /// Removes the assignment of a property from an object over the changeset period;
    /// the object inherits again.
    UnassignProperty {
        /// Object.
        object: Ref,
        /// Its kind.
        kind: ObjectKind,
        /// Property.
        property: Ref,
    },
    /// Closes an object (D33), a property or a closed-list value (REQ-003-6).
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
            | Self::CreateAddress { local, .. }
            | Self::DeclareProperty { local, .. }
            | Self::AddEnumValue { local, .. } => *local,
            Self::Close { .. }
            | Self::UpdateProperty { .. }
            | Self::AssignProperty { .. }
            | Self::ClearProperty { .. }
            | Self::UnassignProperty { .. } => None,
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
