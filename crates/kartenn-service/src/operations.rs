//! Operations of a changeset (REQ-004-1 to 5).

use kartenn_core::objects::{AddressUsage, Nature, RangeUsage};
use kartenn_core::properties::Expected;
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

/// Kind of an object known by a slug (REQ-002-47).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Named {
    /// Addressing domain.
    Domain,
    /// Entity.
    Entity,
    /// Site.
    Site,
}

impl Named {
    /// The kind of object.
    #[must_use]
    pub const fn kind(self) -> ObjectKind {
        match self {
            Self::Domain => ObjectKind::Domain,
            Self::Entity => ObjectKind::Entity,
            Self::Site => ObjectKind::Site,
        }
    }
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
    /// Entity (organisation), in a tree (REQ-002-8).
    Entity,
    /// Site, in a tree (REQ-002-9).
    Site,
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
            Self::Entity => "entity",
            Self::Site => "site",
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
    /// Scopes where a resolved value is mandatory (REQ-003-16).
    #[serde(default)]
    pub obligations: Vec<ObligationInput>,
    /// Source of expected values and its mode (REQ-003-21 to 24).
    #[serde(default)]
    pub expected: Option<Expected>,
}

/// Scope of an obligation, as received (REQ-003-16, P4).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "scope", rename_all = "snake_case")]
pub enum ObligationInput {
    /// Every carrier object of an addressing domain.
    Domain {
        /// Domain.
        id: Uuid,
    },
    /// Every carrier object owned by an entity.
    Entity {
        /// Entity.
        id: Uuid,
    },
    /// Every object of a kind.
    ObjectType {
        /// Kind.
        kind: String,
    },
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
    /// An operation on its own period instead of the changeset's (REQ-004-2).
    Within {
        /// Period of the operation.
        period: PeriodSpec,
        /// The operation.
        operation: Box<Operation>,
    },
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
    /// Defines a role, or replaces its label and entries (spec 005, REQ-005-17, 25):
    /// catalogue permissions, `prop.<key>.<action>`, `prop.*.<action>`, `*`.
    SetRole {
        /// Stable key.
        key: String,
        /// Label.
        label: String,
        /// Entries.
        entries: Vec<String>,
    },
    /// Closes a role and its bindings (005).
    CloseRole {
        /// Key.
        key: String,
    },
    /// Binds a role on a scope to a subject, over the operation's period (005,
    /// REQ-005-18, 25, 26).
    Bind {
        /// Local name of the binding.
        local: Option<u32>,
        /// Who.
        subject: BindingSubject,
        /// Role key.
        role: String,
        /// Where.
        scope: BindingScope,
    },
    /// Ends a binding (005).
    Unbind {
        /// The binding.
        id: Uuid,
    },
    /// Attaches an OIDC issuer to an entity, or everywhere without entity (005, K11, K13).
    AttachIssuer {
        /// Issuer identifier (`iss`).
        issuer: String,
        /// Entity; `None`: everywhere.
        #[serde(default)]
        entity: Option<Ref>,
    },
    /// Detaches an issuer from an entity, or from everywhere (005).
    DetachIssuer {
        /// Issuer identifier.
        issuer: String,
        /// Entity; `None`: the global attachment.
        #[serde(default)]
        entity: Option<Ref>,
    },
    /// Disables or enables an identity, revokes its sessions (005, REQ-005-3).
    SetIdentityState {
        /// The identity.
        identity: Uuid,
        /// `Some(true)` disables, `Some(false)` enables.
        #[serde(default)]
        disabled: Option<bool>,
        /// Revokes the sessions opened until now.
        #[serde(default)]
        revoke_sessions: bool,
    },
    /// Creates a service account with its first secret (005, REQ-005-11): only the digest
    /// of a secret generated outside the pipeline travels.
    CreateServiceAccount {
        /// Local name of the account.
        local: Option<u32>,
        /// Name, the Basic user (no `:`).
        name: String,
        /// Its first secret.
        secret: SecretRecord,
    },
    /// Adds a secret to a service account, for a rotation (005, REQ-005-11).
    AddServiceSecret {
        /// The account.
        account: Uuid,
        /// The secret.
        secret: SecretRecord,
    },
    /// Revokes a secret of a service account (005).
    RevokeServiceSecret {
        /// The account.
        account: Uuid,
        /// The secret.
        secret: Uuid,
    },
    /// Creates or replaces, by name, an approval policy (REQ-004-20, 40): it holds the
    /// changesets that match every trigger it defines.
    SetApprovalPolicy {
        /// Unique name.
        name: String,
        /// At least this many operations.
        #[serde(default)]
        min_operations: Option<u32>,
        /// At least this many objects in the differential.
        #[serde(default)]
        min_diff_objects: Option<u64>,
        /// One of these properties changes, directly or by effect.
        #[serde(default)]
        property_keys: Vec<String>,
        /// The changeset touches one of these addressing domains.
        #[serde(default)]
        domains: Vec<Uuid>,
        /// The author is one of these people.
        #[serde(default)]
        authors: Vec<Uuid>,
        /// The changeset touches one of these scopes (entity, site or prefix, with their
        /// subtrees) (005, REQ-005-27).
        #[serde(default)]
        scopes: Vec<BindingScope>,
        /// The author holds one of these roles (005).
        #[serde(default)]
        author_roles: Vec<String>,
        /// The author belongs to one of these groups, as (issuer, group) (005).
        #[serde(default)]
        author_groups: Vec<(String, String)>,
        /// The changeset binds one of these roles (005, REQ-005-25).
        #[serde(default)]
        bound_roles: Vec<String>,
        /// A disabled policy holds nothing.
        #[serde(default = "enabled")]
        enabled: bool,
    },
    /// Over the period of the changeset, gives an object (or one of its assignments) back
    /// the versions asserted just before changeset `before` (REQ-004-34): the operation of
    /// an inverse changeset.
    Restore {
        /// Kind of the object.
        kind: ObjectKind,
        /// Object.
        id: Ref,
        /// The property, to restore an assignment of the object rather than the object.
        #[serde(default)]
        property: Option<Uuid>,
        /// The changeset whose effect is undone.
        before: Uuid,
    },
    /// Records that an object is known to a source under an identifier (REQ-002-41):
    /// unique per source.
    BindExternal {
        /// Object.
        object: Ref,
        /// Kind of the object.
        kind: ObjectKind,
        /// Source (a connector).
        source: String,
        /// Identifier in the source.
        external_id: String,
    },
    /// Creates an entity (REQ-002-8).
    CreateEntity {
        /// Local name for later operations.
        local: Option<u32>,
        /// Technical identifier.
        slug: String,
        /// Parent entity; none for a root.
        #[serde(default)]
        parent: Option<Ref>,
    },
    /// Creates a site (REQ-002-9).
    CreateSite {
        /// Local name for later operations.
        local: Option<u32>,
        /// Technical identifier.
        slug: String,
        /// Parent site; none for a root.
        #[serde(default)]
        parent: Option<Ref>,
    },
    /// Attaches an entity to another parent, or makes it a root, over the period of the
    /// changeset (REQ-002-8).
    MoveEntity {
        /// Entity.
        id: Ref,
        /// New parent; none for a root.
        #[serde(default)]
        parent: Option<Ref>,
        /// As for [`Operation::Close`].
        #[serde(default)]
        expected: Option<Uuid>,
    },
    /// Attaches a site to another parent, or makes it a root (REQ-002-9).
    MoveSite {
        /// Site.
        id: Ref,
        /// New parent; none for a root.
        #[serde(default)]
        parent: Option<Ref>,
        /// As for [`Operation::Close`].
        #[serde(default)]
        expected: Option<Uuid>,
    },
    /// Marks a property sensitive, or removes the mark: reading its values is then audited
    /// at high criticality (spec 003, REQ-003-27; spec 012, REQ-012-18).
    SetSensitivity {
        /// The property.
        property: Ref,
        /// Sensitive or not.
        sensitive: bool,
    },
    /// Changes the slug of a domain, an entity or a site over the period of the changeset;
    /// its UUID stays (REQ-002-47).
    Rename {
        /// What is renamed.
        kind: Named,
        /// The object.
        id: Ref,
        /// New slug.
        slug: String,
        /// As for [`Operation::Close`].
        #[serde(default)]
        expected: Option<Uuid>,
    },
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
    /// Guards the changeset on the version of an object (REQ-004-11; 006/design « Routes
    /// par ressource »): writes nothing, refuses with a conflict if the current version
    /// differs. Needs the right to read the object.
    Expect {
        /// Kind of the object.
        kind: ObjectKind,
        /// Object.
        id: Ref,
        /// Changeset that last wrote the object or its properties, as the caller saw it.
        expected: Uuid,
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
        /// Also close what depends on the object (REQ-004-26): without it, a closure that
        /// would leave dependents invalid is refused (REQ-004-25).
        #[serde(default)]
        cascade: bool,
    },
    /// Modifies a prefix over the period of the changeset (REQ-002-17, 18; REQ-004-14):
    /// the fields given change, the others keep their value in each version.
    UpdatePrefix {
        /// Prefix.
        id: Ref,
        /// The domain never changes (REQ-002-12): given, it is refused.
        #[serde(default)]
        domain: Option<Ref>,
        /// New value, `address/length` (resizing; same identifier).
        #[serde(default)]
        prefix: Option<String>,
        /// New nature.
        #[serde(default)]
        nature: Option<Nature>,
        /// Point-to-point network or not.
        #[serde(default)]
        point_to_point: Option<bool>,
        /// Anycast network or not.
        #[serde(default)]
        anycast: Option<bool>,
        /// As for [`Operation::Close`].
        #[serde(default)]
        expected: Option<Uuid>,
    },
    /// Changes the usage of an address over the period of the changeset (REQ-004-14).
    UpdateAddress {
        /// Address.
        id: Ref,
        /// The domain never changes (REQ-002-12): given, it is refused.
        #[serde(default)]
        domain: Option<Ref>,
        /// New usage.
        usage: AddressUsage,
        /// As for [`Operation::Close`].
        #[serde(default)]
        expected: Option<Uuid>,
    },
    /// Changes the usage of a range over the period of the changeset (REQ-004-14).
    UpdateRange {
        /// Range.
        id: Ref,
        /// The domain never changes (REQ-002-12): given, it is refused.
        #[serde(default)]
        domain: Option<Ref>,
        /// New usage.
        usage: RangeUsage,
        /// As for [`Operation::Close`].
        #[serde(default)]
        expected: Option<Uuid>,
    },
}

impl Operation {
    /// The operation itself, without the period that wraps it.
    #[must_use]
    pub fn inner(&self) -> &Self {
        match self {
            Self::Within { operation, .. } => operation.inner(),
            other => other,
        }
    }

    /// The period of the operation, when it has its own (REQ-004-2); the innermost wins.
    #[must_use]
    pub fn own_period(&self) -> Option<PeriodSpec> {
        match self {
            Self::Within { period, operation } => operation.own_period().or(Some(*period)),
            _ => None,
        }
    }

    /// Local name this operation defines, if any.
    #[must_use]
    pub fn defines(&self) -> Option<u32> {
        match self {
            Self::Within { operation, .. } => operation.defines(),
            Self::CreateDomain { local, .. }
            | Self::CreatePrefix { local, .. }
            | Self::CreateRange { local, .. }
            | Self::CreateAddress { local, .. }
            | Self::CreateEntity { local, .. }
            | Self::CreateSite { local, .. }
            | Self::DeclareProperty { local, .. }
            | Self::Bind { local, .. }
            | Self::CreateServiceAccount { local, .. }
            | Self::AddEnumValue { local, .. } => *local,
            Self::AddServiceSecret { .. }
            | Self::RevokeServiceSecret { .. }
            | Self::SetRole { .. }
            | Self::CloseRole { .. }
            | Self::Unbind { .. }
            | Self::AttachIssuer { .. }
            | Self::DetachIssuer { .. }
            | Self::SetIdentityState { .. }
            | Self::Close { .. }
            | Self::Expect { .. }
            | Self::UpdatePrefix { .. }
            | Self::UpdateAddress { .. }
            | Self::UpdateRange { .. }
            | Self::MoveEntity { .. }
            | Self::MoveSite { .. }
            | Self::Rename { .. }
            | Self::SetSensitivity { .. }
            | Self::SetApprovalPolicy { .. }
            | Self::Restore { .. }
            | Self::BindExternal { .. }
            | Self::UpdateProperty { .. }
            | Self::AssignProperty { .. }
            | Self::ClearProperty { .. }
            | Self::UnassignProperty { .. } => None,
        }
    }
}

/// The digest of a generated secret of a service account (005, K8).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SecretRecord {
    /// Identifier.
    pub id: Uuid,
    /// HMAC-SHA-256 of the secret, 64 hexadecimal digits.
    pub digest: String,
    /// The key of the ring that made the digest.
    pub key_id: String,
    /// Expiry (microseconds since the epoch), if any.
    #[serde(default)]
    pub expires_at: Option<i64>,
}

/// Who a binding serves (005, K4).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum BindingSubject {
    /// One identity (person, service or break-glass account).
    Identity {
        /// Its identifier.
        id: Uuid,
    },
    /// The members of a group of an issuer.
    Group {
        /// Issuer.
        issuer: String,
        /// Group.
        group: String,
    },
    /// Every identity of an issuer (K5).
    Issuer {
        /// Issuer.
        issuer: String,
    },
}

/// Where a binding acts (005, K14): its scope and what lies below.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum BindingScope {
    /// Everywhere.
    Global,
    /// An entity and its descendants.
    Entity {
        /// The entity.
        id: Ref,
    },
    /// A site and its descendants.
    Site {
        /// The site.
        id: Ref,
    },
    /// An addressing domain.
    Domain {
        /// The domain.
        id: Ref,
    },
    /// A prefix and what it holds.
    Prefix {
        /// The prefix.
        id: Ref,
    },
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

const fn enabled() -> bool {
    true
}
