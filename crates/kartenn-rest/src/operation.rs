//! The operations of a changeset as the API receives them (spec 006, REQ-006-11, 34; task
//! 006-22, arbitrage of 2026-10-19): a mirror of [`Operation`] of the service, read from
//! the body of `POST /v1/changesets` and of the chunks of an import, which draws the
//! contract. The service stays free to change: [`From`] converts each variant, and a test
//! reads one sample of each through both types.

use kartenn_core::objects::{AddressUsage, Nature, RangeUsage};
use kartenn_core::properties::{Expected, ExpectedMode};
use kartenn_core::values::Cardinality;
use kartenn_service::operations::ModuleOperation;
use kartenn_service::operations::RawLabels;
use kartenn_service::operations::{
    BindingScope, DeclarationInput, GroupMember, Named, ObligationInput, PeriodSpec, RawInput,
    SecretRecord, TypeInput,
};
use kartenn_service::port::ImportItem;
use kartenn_service::{ObjectKind, Operation, Ref};
use serde::Deserialize;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::openapi::enums::{AddressUsageSchema, NatureSchema, RangeUsageSchema};

/// An object of the changeset: an existing UUID, or the local name of an object created by
/// an earlier operation of the same changeset (REQ-004-4).
#[derive(Debug, Clone, Copy, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
#[schema(as = Ref)]
pub(crate) enum RefDto {
    /// An existing object.
    Id(Uuid),
    /// An object created earlier in the changeset under this local name.
    Local(u32),
}

impl From<RefDto> for Ref {
    fn from(r: RefDto) -> Self {
        match r {
            RefDto::Id(id) => Self::Id(id),
            RefDto::Local(n) => Self::Local(n),
        }
    }
}

fn opt(r: Option<RefDto>) -> Option<Ref> {
    r.map(Ref::from)
}

/// A kind of object known by a slug (REQ-002-47).
#[derive(Debug, Clone, Copy, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
#[schema(as = NamedKind)]
pub(crate) enum NamedDto {
    Domain,
    Entity,
    Site,
}

impl From<NamedDto> for Named {
    fn from(n: NamedDto) -> Self {
        match n {
            NamedDto::Domain => Self::Domain,
            NamedDto::Entity => Self::Entity,
            NamedDto::Site => Self::Site,
        }
    }
}

/// The kinds of object of the core an operation names: the schema of [`TargetKindDto`],
/// to which the contract of a build adds the types of its modules.
#[derive(Debug, Clone, Copy, serde::Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
#[schema(as = TargetKind)]
#[allow(dead_code)]
pub(crate) enum TargetKindSchema {
    Domain,
    Prefix,
    Range,
    Address,
    Property,
    EnumValue,
    Entity,
    Site,
}

/// The kind of the object of an operation: a kind of the core, or the qualified type of an
/// object of a module (`vlan.vlan`, REQ-008-5), which the registry checks when it plans.
#[derive(Debug, Clone, Copy)]
pub(crate) struct TargetKindDto(ObjectKind);

impl<'de> Deserialize<'de> for TargetKindDto {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let name = String::deserialize(deserializer)?;
        ObjectKind::from_name(&name).map(Self).ok_or_else(|| {
            serde::de::Error::custom(format!(
                "unknown variant `{name}`, expected one of `domain`, `prefix`, `range`, \
                 `address`, `property`, `enum_value`, `entity`, `site` or the type of an \
                 object of a module"
            ))
        })
    }
}

impl utoipa::PartialSchema for TargetKindDto {
    fn schema() -> utoipa::openapi::RefOr<utoipa::openapi::schema::Schema> {
        <TargetKindSchema as utoipa::PartialSchema>::schema()
    }
}

impl ToSchema for TargetKindDto {
    fn name() -> std::borrow::Cow<'static, str> {
        <TargetKindSchema as ToSchema>::name()
    }
}

impl From<TargetKindDto> for ObjectKind {
    fn from(k: TargetKindDto) -> Self {
        k.0
    }
}

/// The type of a declared property and its parameters (003/design).
#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
#[schema(as = PropertyType)]
pub(crate) enum TypeDto {
    /// A closed list.
    Enum {
        /// Key of another closed list of the same declarant whose values it shares.
        #[serde(default)]
        values_of: Option<String>,
    },
    /// A text.
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
    /// An integer.
    Int {
        /// Minimum.
        min: i64,
        /// Maximum.
        max: i64,
    },
    /// A boolean.
    Bool,
    /// A calendar date.
    Date,
    /// A host name.
    Fqdn,
    /// A reference to an object kind.
    Ref {
        /// Target kind.
        target: String,
    },
    /// A reference to an object of another system.
    ExternalRef,
}

impl From<TypeDto> for TypeInput {
    fn from(t: TypeDto) -> Self {
        match t {
            TypeDto::Enum { values_of } => Self::Enum { values_of },
            TypeDto::Text {
                min_len,
                max_len,
                pattern,
            } => Self::Text {
                min_len,
                max_len,
                pattern,
            },
            TypeDto::Int { min, max } => Self::Int { min, max },
            TypeDto::Bool => Self::Bool,
            TypeDto::Date => Self::Date,
            TypeDto::Fqdn => Self::Fqdn,
            TypeDto::Ref { target } => Self::Ref { target },
            TypeDto::ExternalRef => Self::ExternalRef,
        }
    }
}

/// The cardinality of a property.
#[derive(Debug, Clone, Copy, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
#[schema(as = Cardinality)]
pub(crate) enum CardinalityDto {
    /// One value.
    Single,
    /// A set of values.
    Set,
}

impl From<CardinalityDto> for Cardinality {
    fn from(c: CardinalityDto) -> Self {
        match c {
            CardinalityDto::Single => Self::Single,
            CardinalityDto::Set => Self::Set,
        }
    }
}

/// How the expected values of a source apply.
#[derive(Debug, Clone, Copy, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
#[schema(as = ExpectedMode)]
pub(crate) enum ExpectedModeDto {
    Suggest,
    Default,
    Check,
}

/// The source of the expected values of a property (REQ-003-23).
#[derive(Debug, Clone, Deserialize, ToSchema)]
#[schema(as = Expected)]
pub(crate) struct ExpectedDto {
    /// The source.
    source: String,
    /// How its values apply.
    mode: ExpectedModeDto,
}

impl From<ExpectedDto> for Expected {
    fn from(e: ExpectedDto) -> Self {
        Self {
            source: e.source,
            mode: match e.mode {
                ExpectedModeDto::Suggest => ExpectedMode::Suggest,
                ExpectedModeDto::Default => ExpectedMode::Default,
                ExpectedModeDto::Check => ExpectedMode::Check,
            },
        }
    }
}

/// Where a property is required.
#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(tag = "scope", rename_all = "snake_case")]
#[schema(as = Obligation)]
pub(crate) enum ObligationDto {
    /// In an addressing domain.
    Domain {
        /// The domain.
        id: Uuid,
    },
    /// In an entity.
    Entity {
        /// The entity.
        id: Uuid,
    },
    /// On a kind of object.
    ObjectType {
        /// The kind.
        kind: String,
    },
}

impl From<ObligationDto> for ObligationInput {
    fn from(o: ObligationDto) -> Self {
        match o {
            ObligationDto::Domain { id } => Self::Domain { id },
            ObligationDto::Entity { id } => Self::Entity { id },
            ObligationDto::ObjectType { kind } => Self::ObjectType { kind },
        }
    }
}

/// The declaration of a property (REQ-003-1 to 3).
#[derive(Debug, Clone, Deserialize, ToSchema)]
#[schema(as = Declaration)]
pub(crate) struct DeclarationDto {
    /// The key.
    key: String,
    /// The type and its parameters.
    #[serde(flatten)]
    value_type: TypeDto,
    /// One value or a set.
    cardinality: CardinalityDto,
    /// The kinds of object that carry it.
    carriers: Vec<String>,
    /// The tree it is inherited along.
    #[serde(default)]
    inherit: Option<String>,
    /// Where it is required.
    #[serde(default)]
    obligations: Vec<ObligationDto>,
    /// The source of its expected values.
    #[serde(default)]
    expected: Option<ExpectedDto>,
    /// Its labels by language, `{<tag>: <text>}` (spec 018): absent, none for a new
    /// declaration and those in force for an update.
    #[serde(default)]
    labels: Option<RawLabels>,
}

impl From<DeclarationDto> for DeclarationInput {
    fn from(d: DeclarationDto) -> Self {
        Self {
            key: d.key,
            value_type: d.value_type.into(),
            cardinality: d.cardinality.into(),
            carriers: d.carriers,
            inherit: d.inherit,
            module: None,
            obligations: d.obligations.into_iter().map(Into::into).collect(),
            expected: d.expected.map(Into::into),
            labels: d.labels,
        }
    }
}

/// One raw value of an assignment: text (closed-list key, text, host name, date,
/// reference), integer or boolean (REQ-003-8).
#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(untagged)]
#[schema(as = RawValue)]
pub(crate) enum RawDto {
    Bool(bool),
    Int(i64),
    Text(String),
    /// A reference to an object of another system: a value of `core.external_system`,
    /// the identifier there, a label.
    External {
        system: String,
        id: String,
        #[serde(default)]
        label: Option<String>,
    },
}

impl From<RawDto> for RawInput {
    fn from(r: RawDto) -> Self {
        match r {
            RawDto::Bool(b) => Self::Bool(b),
            RawDto::Int(i) => Self::Int(i),
            RawDto::Text(t) => Self::Text(t),
            RawDto::External { system, id, label } => Self::External { system, id, label },
        }
    }
}

/// A period, in microseconds since the epoch: `[start, end)`, unbounded without `end`.
#[derive(Debug, Clone, Copy, Deserialize, ToSchema)]
#[schema(as = Period)]
pub(crate) struct PeriodDto {
    /// Start, included.
    start: i64,
    /// End, excluded.
    end: Option<i64>,
}

impl From<PeriodDto> for PeriodSpec {
    fn from(p: PeriodDto) -> Self {
        Self {
            start: p.start,
            end: p.end,
        }
    }
}

/// The digest of a secret of a service account, generated outside the pipeline (005, K8).
#[derive(Debug, Clone, Deserialize, ToSchema)]
#[schema(as = SecretRecord)]
pub(crate) struct SecretDto {
    /// Identifier.
    id: Uuid,
    /// HMAC-SHA-256 of the secret, 64 hexadecimal digits.
    digest: String,
    /// The key of the ring that made the digest.
    key_id: String,
    /// Expiry, in microseconds since the epoch.
    #[serde(default)]
    expires_at: Option<i64>,
}

impl From<SecretDto> for SecretRecord {
    fn from(s: SecretDto) -> Self {
        Self {
            id: s.id,
            digest: s.digest,
            key_id: s.key_id,
            expires_at: s.expires_at,
        }
    }
}

/// A trigger of an approval policy on a measure of a module (REQ-016-7).
#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
#[schema(as = ModuleTrigger)]
pub(crate) struct ModuleTriggerDto {
    /// `<module>.<measure>`, declared by a module of the build.
    measure: String,
    /// The policy holds from this value up.
    threshold: u64,
}

/// A member of a group of kartenn (spec 025, G2, G6): an identity or a group of an
/// issuer, never another group of kartenn.
#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[schema(as = GroupMember)]
pub(crate) enum MemberDto {
    /// One identity.
    Identity {
        /// Its identifier.
        id: Uuid,
    },
    /// The identities of an issuer whose session carries the group `group`.
    IdpGroup {
        /// Issuer.
        issuer: String,
        /// Group, as the issuer serves it.
        group: String,
    },
}

impl From<MemberDto> for GroupMember {
    fn from(m: MemberDto) -> Self {
        match m {
            MemberDto::Identity { id } => Self::Identity { id },
            MemberDto::IdpGroup { issuer, group } => Self::IdpGroup { issuer, group },
        }
    }
}

/// Where a binding acts (005, K14): its scope and what lies below.
#[derive(Debug, Clone, Copy, Deserialize, ToSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[schema(as = BindingScope)]
pub(crate) enum ScopeDto {
    /// Everywhere.
    Global,
    /// An entity and its descendants.
    Entity {
        /// The entity.
        id: RefDto,
    },
    /// A site and its descendants.
    Site {
        /// The site.
        id: RefDto,
    },
    /// An addressing domain.
    Domain {
        /// The domain.
        id: RefDto,
    },
    /// A prefix and what it holds.
    Prefix {
        /// The prefix.
        id: RefDto,
    },
}

impl From<ScopeDto> for BindingScope {
    fn from(s: ScopeDto) -> Self {
        match s {
            ScopeDto::Global => Self::Global,
            ScopeDto::Entity { id } => Self::Entity { id: id.into() },
            ScopeDto::Site { id } => Self::Site { id: id.into() },
            ScopeDto::Domain { id } => Self::Domain { id: id.into() },
            ScopeDto::Prefix { id } => Self::Prefix { id: id.into() },
        }
    }
}

const fn enabled() -> bool {
    true
}

/// An operation of a changeset (004, REQ-004-1 to 5), named by `op`: one of the core, or
/// one of a module in its envelope, `<module>.<operation>` (REQ-008-4), whose other
/// members the module reads when it plans it.
#[derive(Debug, Clone)]
pub(crate) enum OperationDto {
    /// An operation of the core.
    Core(CoreOperationDto),
    /// An operation of a module.
    Module(ModuleOperation),
}

impl<'de> Deserialize<'de> for OperationDto {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = serde_json::Value::deserialize(deserializer)?;
        let qualified = value
            .get("op")
            .and_then(serde_json::Value::as_str)
            .and_then(|tag| tag.split_once('.'))
            .map(|(module, name)| (module.to_owned(), name.to_owned()));
        match (qualified, value) {
            (Some((module, name)), serde_json::Value::Object(mut body)) => {
                body.remove("op");
                Ok(Self::Module(ModuleOperation { module, name, body }))
            }
            (_, value) => CoreOperationDto::deserialize(value)
                .map(Self::Core)
                .map_err(serde::de::Error::custom),
        }
    }
}

impl utoipa::PartialSchema for OperationDto {
    fn schema() -> utoipa::openapi::RefOr<utoipa::openapi::schema::Schema> {
        use utoipa::openapi::schema::{ObjectBuilder, Schema, Type};
        let mut schema = <CoreOperationDto as utoipa::PartialSchema>::schema();
        if let utoipa::openapi::RefOr::T(Schema::OneOf(one_of)) = &mut schema {
            one_of.items.push(
                ObjectBuilder::new()
                    .description(Some(
                        "An operation of a module (ADR 0026, clause 6): `op` is \
                         `<module>.<operation>`, the other members are those of the module.",
                    ))
                    .property(
                        "op",
                        ObjectBuilder::new()
                            .schema_type(Type::String)
                            .pattern(Some("^[a-z][a-z0-9_]*\\.[a-z][a-z0-9_]*$"))
                            .description(Some("The module, then the operation.")),
                    )
                    .required("op")
                    .additional_properties(Some(
                        utoipa::openapi::schema::AdditionalProperties::FreeForm(true),
                    ))
                    .into(),
            );
        }
        schema
    }
}

impl ToSchema for OperationDto {
    fn name() -> std::borrow::Cow<'static, str> {
        "Operation".into()
    }

    fn schemas(
        schemas: &mut Vec<(
            String,
            utoipa::openapi::RefOr<utoipa::openapi::schema::Schema>,
        )>,
    ) {
        <CoreOperationDto as ToSchema>::schemas(schemas);
    }
}

impl From<OperationDto> for Operation {
    fn from(o: OperationDto) -> Self {
        match o {
            OperationDto::Core(core) => core.into(),
            OperationDto::Module(module) => Self::Module(module),
        }
    }
}

/// An operation of the core, named by `op`.
#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(tag = "op", rename_all = "snake_case")]
pub(crate) enum CoreOperationDto {
    /// An operation on its own period instead of the changeset's (REQ-004-2).
    Within {
        /// The period of the operation.
        period: PeriodDto,
        /// The operation.
        #[schema(no_recursion)]
        operation: Box<OperationDto>,
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
        domain: RefDto,
        /// `address/length`.
        prefix: String,
        /// Container or network.
        #[schema(value_type = NatureSchema)]
        nature: Nature,
        /// Point-to-point network.
        #[serde(default)]
        point_to_point: bool,
        /// Anycast network.
        #[serde(default)]
        anycast: bool,
    },
    /// Creates a range.
    CreateRange {
        /// Local name for later operations.
        local: Option<u32>,
        /// Domain.
        domain: RefDto,
        /// First address.
        first: String,
        /// Last address, included.
        last: String,
        /// DHCP, reserved or static.
        #[schema(value_type = RangeUsageSchema)]
        usage: RangeUsage,
    },
    /// Creates an address.
    CreateAddress {
        /// Local name for later operations.
        local: Option<u32>,
        /// Domain.
        domain: RefDto,
        /// Address.
        address: String,
        /// Usage.
        #[schema(value_type = AddressUsageSchema)]
        usage: AddressUsage,
    },
    /// Defines a role, or replaces its labels and entries (005, REQ-005-17, 25).
    SetRole {
        /// Stable key.
        key: String,
        /// Labels by language, `{<tag>: <text>}`: the default language at least.
        labels: RawLabels,
        /// Entries.
        entries: Vec<String>,
    },
    /// Closes a role and its bindings (005).
    CloseRole {
        /// Key.
        key: String,
    },
    /// Defines a group of kartenn, or replaces its labels (spec 025, REQ-025-1).
    SetGroup {
        /// Stable key.
        key: String,
        /// Labels by language, `{<tag>: <text>}`: the default language at least.
        labels: RawLabels,
    },
    /// Closes a group, its bindings and its memberships (spec 025, REQ-025-8).
    CloseGroup {
        /// Key.
        key: String,
    },
    /// Adds a member to a group (spec 025, REQ-025-2, 7).
    AddMember {
        /// Local name of the membership.
        local: Option<u32>,
        /// The group, by key.
        group: String,
        /// Who.
        member: MemberDto,
    },
    /// Ends a membership (spec 025, REQ-025-7).
    RemoveMember {
        /// The membership.
        id: Uuid,
    },
    /// Binds a role on a scope to a group of kartenn (005, REQ-005-25, 26; spec 025,
    /// REQ-025-4, 6).
    Bind {
        /// Local name of the binding.
        local: Option<u32>,
        /// The group, by key.
        group: String,
        /// Role key.
        role: String,
        /// Where.
        scope: ScopeDto,
    },
    /// Ends a binding (005).
    Unbind {
        /// The binding.
        id: Uuid,
    },
    /// Attaches an OIDC issuer to an entity, or everywhere without entity (005).
    AttachIssuer {
        /// Issuer identifier (`iss`).
        issuer: String,
        /// Entity; absent: everywhere.
        #[serde(default)]
        entity: Option<RefDto>,
    },
    /// Detaches an issuer from an entity, or from everywhere (005).
    DetachIssuer {
        /// Issuer identifier.
        issuer: String,
        /// Entity; absent: the global attachment.
        #[serde(default)]
        entity: Option<RefDto>,
    },
    /// Disables or enables an identity, revokes its sessions (005, REQ-005-3).
    SetIdentityState {
        /// The identity.
        identity: Uuid,
        /// `true` disables, `false` enables.
        #[serde(default)]
        disabled: Option<bool>,
        /// Revokes the sessions opened until now.
        #[serde(default)]
        revoke_sessions: bool,
    },
    /// Creates a service account with its first secret (005, REQ-005-11).
    CreateServiceAccount {
        /// Local name of the account.
        local: Option<u32>,
        /// Name, the Basic user (no `:`).
        name: String,
        /// Its first secret.
        secret: SecretDto,
    },
    /// Adds a secret to a service account (005, REQ-005-11).
    AddServiceSecret {
        /// The account.
        account: Uuid,
        /// The secret.
        secret: SecretDto,
    },
    /// Revokes a secret of a service account (005).
    RevokeServiceSecret {
        /// The account.
        account: Uuid,
        /// The secret.
        secret: Uuid,
    },
    /// Creates or replaces, by name, an approval policy (REQ-004-20, 40).
    SetApprovalPolicy {
        /// Unique name.
        name: String,
        /// At least this many operations.
        #[serde(default)]
        min_operations: Option<u32>,
        /// At least this many objects in the differential.
        #[serde(default)]
        min_diff_objects: Option<u64>,
        /// One of these properties changes.
        #[serde(default)]
        property_keys: Vec<String>,
        /// The changeset touches one of these addressing domains.
        #[serde(default)]
        domains: Vec<Uuid>,
        /// The changeset touches one of these scopes (005, REQ-005-27).
        #[serde(default)]
        scopes: Vec<ScopeDto>,
        /// The author holds one of these roles through one of his groups (005, 025).
        #[serde(default)]
        author_roles: Vec<String>,
        /// The author is a member of one of these groups of kartenn, by key, directly or
        /// through a group of his issuer; no account is an author (025, REQ-025-13).
        #[serde(default)]
        author_groups: Vec<String>,
        /// The changeset binds one of these roles (005, REQ-005-25).
        #[serde(default)]
        bound_roles: Vec<String>,
        /// Measures of modules reach these thresholds (REQ-016-7).
        #[serde(default)]
        module_triggers: Vec<ModuleTriggerDto>,
        /// A disabled policy holds nothing.
        #[serde(default = "enabled")]
        enabled: bool,
    },
    /// Gives an object, or one of its assignments, back the versions asserted just before
    /// changeset `before` (REQ-004-34).
    Restore {
        /// Kind of the object.
        kind: TargetKindDto,
        /// Object.
        id: RefDto,
        /// The property, to restore an assignment rather than the object.
        #[serde(default)]
        property: Option<Uuid>,
        /// The changeset whose effect is undone.
        before: Uuid,
    },
    /// Records that an object is known to a source under an identifier (REQ-002-41).
    BindExternal {
        /// Object.
        object: RefDto,
        /// Kind of the object.
        kind: TargetKindDto,
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
        parent: Option<RefDto>,
    },
    /// Creates a site (REQ-002-9).
    CreateSite {
        /// Local name for later operations.
        local: Option<u32>,
        /// Technical identifier.
        slug: String,
        /// Parent site; none for a root.
        #[serde(default)]
        parent: Option<RefDto>,
    },
    /// Attaches an entity to another parent, or makes it a root (REQ-002-8).
    MoveEntity {
        /// Entity.
        id: RefDto,
        /// New parent; none for a root.
        #[serde(default)]
        parent: Option<RefDto>,
        /// The changeset that last wrote the object, as the caller saw it.
        #[serde(default)]
        expected: Option<Uuid>,
    },
    /// Attaches a site to another parent, or makes it a root (REQ-002-9).
    MoveSite {
        /// Site.
        id: RefDto,
        /// New parent; none for a root.
        #[serde(default)]
        parent: Option<RefDto>,
        /// The changeset that last wrote the object, as the caller saw it.
        #[serde(default)]
        expected: Option<Uuid>,
    },
    /// Marks a property sensitive, or removes the mark (003, REQ-003-27).
    SetSensitivity {
        /// The property.
        property: RefDto,
        /// Sensitive or not.
        sensitive: bool,
    },
    /// Requires a property in a scope, as an administrator; the synchronisation of the
    /// module that declares it keeps it (008, REQ-008-28).
    AddObligation {
        /// The property.
        property: RefDto,
        /// Where it becomes required.
        #[serde(flatten)]
        scope: ObligationDto,
    },
    /// Removes an obligation an administrator added (008, REQ-008-28).
    RemoveObligation {
        /// The property.
        property: RefDto,
        /// Where it was required.
        #[serde(flatten)]
        scope: ObligationDto,
    },
    /// Changes the slug of a domain, an entity or a site; its UUID stays (REQ-002-47).
    Rename {
        /// What is renamed.
        kind: NamedDto,
        /// The object.
        id: RefDto,
        /// New slug.
        slug: String,
        /// The changeset that last wrote the object, as the caller saw it.
        #[serde(default)]
        expected: Option<Uuid>,
    },
    /// Declares a property (REQ-003-1 to 3).
    DeclareProperty {
        /// Local name for later operations.
        local: Option<u32>,
        /// Declaration.
        #[serde(flatten)]
        declaration: DeclarationDto,
    },
    /// Replaces the current declaration of a property (REQ-003-6).
    UpdateProperty {
        /// Property.
        id: RefDto,
        /// New declaration; the key never changes.
        #[serde(flatten)]
        declaration: DeclarationDto,
    },
    /// Adds a value to a closed list (REQ-003-5, 7).
    AddEnumValue {
        /// Local name for later operations.
        local: Option<u32>,
        /// Property of type `enum`.
        property: RefDto,
        /// Key, `[a-z0-9-]`.
        key: String,
        /// Labels by language, `{<tag>: <text>}` (spec 018).
        #[serde(default)]
        labels: RawLabels,
    },
    /// Replaces the labels of a closed-list value; its key never changes (spec 018,
    /// REQ-018-9).
    UpdateEnumValue {
        /// Property of type `enum`.
        property: RefDto,
        /// Key of the value.
        key: String,
        /// Labels by language, `{<tag>: <text>}`; empty, the value has none.
        labels: RawLabels,
    },
    /// Assigns a value to a property of an object (REQ-003-4, 8, 12).
    AssignProperty {
        /// Object.
        object: RefDto,
        /// Its kind.
        kind: TargetKindDto,
        /// Property.
        property: RefDto,
        /// One value, or the elements of a set.
        values: Vec<RawDto>,
    },
    /// Assigns "no value", which stops inheritance (REQ-003-13).
    ClearProperty {
        /// Object.
        object: RefDto,
        /// Its kind.
        kind: TargetKindDto,
        /// Property.
        property: RefDto,
    },
    /// Removes the assignment of a property from an object; it inherits again.
    UnassignProperty {
        /// Object.
        object: RefDto,
        /// Its kind.
        kind: TargetKindDto,
        /// Property.
        property: RefDto,
    },
    /// Guards the changeset on the version of an object (REQ-004-11): writes nothing.
    Expect {
        /// Kind of the object.
        kind: TargetKindDto,
        /// Object.
        id: RefDto,
        /// The changeset that last wrote the object or its properties.
        expected: Uuid,
    },
    /// Closes an object (D33), a property or a closed-list value (REQ-003-6).
    Close {
        /// Kind of the object.
        kind: TargetKindDto,
        /// Object.
        id: RefDto,
        /// The changeset that last wrote the object, as the caller saw it (REQ-004-11).
        #[serde(default)]
        expected: Option<Uuid>,
        /// Also close what depends on the object (REQ-004-26).
        #[serde(default)]
        cascade: bool,
    },
    /// Modifies a prefix (REQ-002-17, 18; REQ-004-14): the fields given change.
    UpdatePrefix {
        /// Prefix.
        id: RefDto,
        /// The domain never changes (REQ-002-12): given, it is refused.
        #[serde(default)]
        domain: Option<RefDto>,
        /// New value, `address/length`.
        #[serde(default)]
        prefix: Option<String>,
        /// New nature.
        #[serde(default)]
        #[schema(value_type = Option<NatureSchema>)]
        nature: Option<Nature>,
        /// Point-to-point network or not.
        #[serde(default)]
        point_to_point: Option<bool>,
        /// Anycast network or not.
        #[serde(default)]
        anycast: Option<bool>,
        /// The changeset that last wrote the object, as the caller saw it.
        #[serde(default)]
        expected: Option<Uuid>,
    },
    /// Changes the usage of an address (REQ-004-14).
    UpdateAddress {
        /// Address.
        id: RefDto,
        /// The domain never changes (REQ-002-12): given, it is refused.
        #[serde(default)]
        domain: Option<RefDto>,
        /// New usage.
        #[schema(value_type = AddressUsageSchema)]
        usage: AddressUsage,
        /// The changeset that last wrote the object, as the caller saw it.
        #[serde(default)]
        expected: Option<Uuid>,
    },
    /// Changes the usage of a range (REQ-004-14).
    UpdateRange {
        /// Range.
        id: RefDto,
        /// The domain never changes (REQ-002-12): given, it is refused.
        #[serde(default)]
        domain: Option<RefDto>,
        /// New usage.
        #[schema(value_type = RangeUsageSchema)]
        usage: RangeUsage,
        /// The changeset that last wrote the object, as the caller saw it.
        #[serde(default)]
        expected: Option<Uuid>,
    },
}

impl From<CoreOperationDto> for Operation {
    #[allow(clippy::too_many_lines)]
    fn from(o: CoreOperationDto) -> Self {
        match o {
            CoreOperationDto::Within { period, operation } => Self::Within {
                period: period.into(),
                operation: Box::new((*operation).into()),
            },
            CoreOperationDto::CreateDomain { local, slug } => Self::CreateDomain { local, slug },
            CoreOperationDto::CreatePrefix {
                local,
                domain,
                prefix,
                nature,
                point_to_point,
                anycast,
            } => Self::CreatePrefix {
                local,
                domain: domain.into(),
                prefix,
                nature,
                point_to_point,
                anycast,
            },
            CoreOperationDto::CreateRange {
                local,
                domain,
                first,
                last,
                usage,
            } => Self::CreateRange {
                local,
                domain: domain.into(),
                first,
                last,
                usage,
            },
            CoreOperationDto::CreateAddress {
                local,
                domain,
                address,
                usage,
            } => Self::CreateAddress {
                local,
                domain: domain.into(),
                address,
                usage,
            },
            CoreOperationDto::SetRole {
                key,
                labels,
                entries,
            } => Self::SetRole {
                key,
                labels,
                entries,
            },
            CoreOperationDto::CloseRole { key } => Self::CloseRole { key },
            CoreOperationDto::SetGroup { key, labels } => Self::SetGroup { key, labels },
            CoreOperationDto::CloseGroup { key } => Self::CloseGroup { key },
            CoreOperationDto::AddMember {
                local,
                group,
                member,
            } => Self::AddMember {
                local,
                group,
                member: member.into(),
            },
            CoreOperationDto::RemoveMember { id } => Self::RemoveMember { id },
            CoreOperationDto::Bind {
                local,
                group,
                role,
                scope,
            } => Self::Bind {
                local,
                group,
                role,
                scope: scope.into(),
            },
            CoreOperationDto::Unbind { id } => Self::Unbind { id },
            CoreOperationDto::AttachIssuer { issuer, entity } => Self::AttachIssuer {
                issuer,
                entity: opt(entity),
            },
            CoreOperationDto::DetachIssuer { issuer, entity } => Self::DetachIssuer {
                issuer,
                entity: opt(entity),
            },
            CoreOperationDto::SetIdentityState {
                identity,
                disabled,
                revoke_sessions,
            } => Self::SetIdentityState {
                identity,
                disabled,
                revoke_sessions,
            },
            CoreOperationDto::CreateServiceAccount {
                local,
                name,
                secret,
            } => Self::CreateServiceAccount {
                local,
                name,
                secret: secret.into(),
            },
            CoreOperationDto::AddServiceSecret { account, secret } => Self::AddServiceSecret {
                account,
                secret: secret.into(),
            },
            CoreOperationDto::RevokeServiceSecret { account, secret } => {
                Self::RevokeServiceSecret { account, secret }
            }
            CoreOperationDto::SetApprovalPolicy {
                name,
                min_operations,
                min_diff_objects,
                property_keys,
                domains,
                scopes,
                author_roles,
                author_groups,
                bound_roles,
                module_triggers,
                enabled,
            } => Self::SetApprovalPolicy {
                name,
                min_operations,
                min_diff_objects,
                property_keys,
                domains,
                scopes: scopes.into_iter().map(Into::into).collect(),
                author_roles,
                author_groups,
                bound_roles,
                module_triggers: module_triggers
                    .into_iter()
                    .map(|t| kartenn_service::operations::ModuleTrigger {
                        measure: t.measure,
                        threshold: t.threshold,
                    })
                    .collect(),
                enabled,
            },
            CoreOperationDto::Restore {
                kind,
                id,
                property,
                before,
            } => Self::Restore {
                kind: kind.into(),
                id: id.into(),
                property,
                before,
            },
            CoreOperationDto::BindExternal {
                object,
                kind,
                source,
                external_id,
            } => Self::BindExternal {
                object: object.into(),
                kind: kind.into(),
                source,
                external_id,
            },
            CoreOperationDto::CreateEntity {
                local,
                slug,
                parent,
            } => Self::CreateEntity {
                local,
                slug,
                parent: opt(parent),
            },
            CoreOperationDto::CreateSite {
                local,
                slug,
                parent,
            } => Self::CreateSite {
                local,
                slug,
                parent: opt(parent),
            },
            CoreOperationDto::MoveEntity {
                id,
                parent,
                expected,
            } => Self::MoveEntity {
                id: id.into(),
                parent: opt(parent),
                expected,
            },
            CoreOperationDto::MoveSite {
                id,
                parent,
                expected,
            } => Self::MoveSite {
                id: id.into(),
                parent: opt(parent),
                expected,
            },
            CoreOperationDto::SetSensitivity {
                property,
                sensitive,
            } => Self::SetSensitivity {
                property: property.into(),
                sensitive,
            },
            CoreOperationDto::AddObligation { property, scope } => Self::AddObligation {
                property: property.into(),
                scope: scope.into(),
            },
            CoreOperationDto::RemoveObligation { property, scope } => Self::RemoveObligation {
                property: property.into(),
                scope: scope.into(),
            },
            CoreOperationDto::Rename {
                kind,
                id,
                slug,
                expected,
            } => Self::Rename {
                kind: kind.into(),
                id: id.into(),
                slug,
                expected,
            },
            CoreOperationDto::DeclareProperty { local, declaration } => Self::DeclareProperty {
                local,
                declaration: declaration.into(),
            },
            CoreOperationDto::UpdateProperty { id, declaration } => Self::UpdateProperty {
                id: id.into(),
                declaration: declaration.into(),
            },
            CoreOperationDto::AddEnumValue {
                local,
                property,
                key,
                labels,
            } => Self::AddEnumValue {
                local,
                property: property.into(),
                key,
                labels,
            },
            CoreOperationDto::UpdateEnumValue {
                property,
                key,
                labels,
            } => Self::UpdateEnumValue {
                property: property.into(),
                key,
                labels,
            },
            CoreOperationDto::AssignProperty {
                object,
                kind,
                property,
                values,
            } => Self::AssignProperty {
                object: object.into(),
                kind: kind.into(),
                property: property.into(),
                values: values.into_iter().map(Into::into).collect(),
            },
            CoreOperationDto::ClearProperty {
                object,
                kind,
                property,
            } => Self::ClearProperty {
                object: object.into(),
                kind: kind.into(),
                property: property.into(),
            },
            CoreOperationDto::UnassignProperty {
                object,
                kind,
                property,
            } => Self::UnassignProperty {
                object: object.into(),
                kind: kind.into(),
                property: property.into(),
            },
            CoreOperationDto::Expect { kind, id, expected } => Self::Expect {
                kind: kind.into(),
                id: id.into(),
                expected,
            },
            CoreOperationDto::Close {
                kind,
                id,
                expected,
                cascade,
            } => Self::Close {
                kind: kind.into(),
                id: id.into(),
                expected,
                cascade,
            },
            CoreOperationDto::UpdatePrefix {
                id,
                domain,
                prefix,
                nature,
                point_to_point,
                anycast,
                expected,
            } => Self::UpdatePrefix {
                id: id.into(),
                domain: opt(domain),
                prefix,
                nature,
                point_to_point,
                anycast,
                expected,
            },
            CoreOperationDto::UpdateAddress {
                id,
                domain,
                usage,
                expected,
            } => Self::UpdateAddress {
                id: id.into(),
                domain: opt(domain),
                usage,
                expected,
            },
            CoreOperationDto::UpdateRange {
                id,
                domain,
                usage,
                expected,
            } => Self::UpdateRange {
                id: id.into(),
                domain: opt(domain),
                usage,
                expected,
            },
        }
    }
}

/// One element of an import (REQ-004-28): an operation, with the identifier in the source
/// of the object it creates.
#[derive(Debug, Clone, Deserialize, ToSchema)]
#[schema(as = ImportElement)]
pub(crate) struct ImportItemDto {
    /// Identifier in the source; required for an operation that creates an object.
    #[serde(default)]
    external_id: Option<String>,
    /// The operation; its local references hold for the whole import.
    operation: OperationDto,
}

impl From<ImportItemDto> for ImportItem {
    fn from(i: ImportItemDto) -> Self {
        Self {
            external_id: i.external_id,
            operation: i.operation.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::*;

    /// The name of each operation of the service: a new one breaks this match, and the
    /// test below then wants its sample.
    const fn name(operation: &Operation) -> &'static str {
        match operation {
            Operation::Within { .. } => "within",
            // The envelope of the module operations comes with the routes of the modules.
            Operation::Module(_) => "module",
            Operation::CreateDomain { .. } => "create_domain",
            Operation::CreatePrefix { .. } => "create_prefix",
            Operation::CreateRange { .. } => "create_range",
            Operation::CreateAddress { .. } => "create_address",
            Operation::SetRole { .. } => "set_role",
            Operation::CloseRole { .. } => "close_role",
            Operation::SetGroup { .. } => "set_group",
            Operation::CloseGroup { .. } => "close_group",
            Operation::AddMember { .. } => "add_member",
            Operation::RemoveMember { .. } => "remove_member",
            Operation::Bind { .. } => "bind",
            Operation::Unbind { .. } => "unbind",
            Operation::AttachIssuer { .. } => "attach_issuer",
            Operation::DetachIssuer { .. } => "detach_issuer",
            Operation::SetIdentityState { .. } => "set_identity_state",
            Operation::CreateServiceAccount { .. } => "create_service_account",
            Operation::AddServiceSecret { .. } => "add_service_secret",
            Operation::RevokeServiceSecret { .. } => "revoke_service_secret",
            Operation::SetApprovalPolicy { .. } => "set_approval_policy",
            Operation::Restore { .. } => "restore",
            Operation::BindExternal { .. } => "bind_external",
            Operation::CreateEntity { .. } => "create_entity",
            Operation::CreateSite { .. } => "create_site",
            Operation::MoveEntity { .. } => "move_entity",
            Operation::MoveSite { .. } => "move_site",
            Operation::SetSensitivity { .. } => "set_sensitivity",
            Operation::AddObligation { .. } => "add_obligation",
            Operation::RemoveObligation { .. } => "remove_obligation",
            Operation::Rename { .. } => "rename",
            Operation::DeclareProperty { .. } => "declare_property",
            Operation::UpdateProperty { .. } => "update_property",
            Operation::AddEnumValue { .. } => "add_enum_value",
            Operation::UpdateEnumValue { .. } => "update_enum_value",
            Operation::AssignProperty { .. } => "assign_property",
            Operation::ClearProperty { .. } => "clear_property",
            Operation::UnassignProperty { .. } => "unassign_property",
            Operation::Expect { .. } => "expect",
            Operation::Close { .. } => "close",
            Operation::UpdatePrefix { .. } => "update_prefix",
            Operation::UpdateAddress { .. } => "update_address",
            Operation::UpdateRange { .. } => "update_range",
        }
    }

    /// One sample of each operation, its optional members given.
    #[allow(clippy::too_many_lines)]
    fn samples() -> Vec<Value> {
        let id = "0190a000-0000-7000-8000-000000000001";
        let other = "0190a000-0000-7000-8000-000000000002";
        let secret = json!({"id": id, "digest": "ab", "key_id": "k1", "expires_at": 5});
        let declaration = json!({
            "key": "vlan", "type": "int", "min": 1, "max": 4094, "cardinality": "single",
            "carriers": ["prefix"], "inherit": "addressing",
            "obligations": [{"scope": "domain", "id": id}, {"scope": "object_type", "kind": "prefix"}],
            "expected": {"source": "ipam", "mode": "check"},
            "labels": {"fr": "VLAN", "en": "VLAN"},
        });
        let mut declare = declaration.clone();
        declare["op"] = json!("declare_property");
        declare["local"] = json!(3);
        let mut update = json!({"key": "env", "type": "text", "min_len": 1, "max_len": 8,
            "pattern": "[a-z]+", "cardinality": "set", "carriers": ["site"]});
        update["op"] = json!("update_property");
        update["id"] = json!({"id": id});
        vec![
            json!({"op": "within", "period": {"start": 1, "end": 2},
                "operation": {"op": "create_domain", "local": 1, "slug": "lab"}}),
            json!({"op": "create_domain", "local": 1, "slug": "lab"}),
            json!({"op": "create_prefix", "local": 2, "domain": {"local": 1},
                "prefix": "10.0.0.0/8", "nature": "container", "point_to_point": true, "anycast": true}),
            json!({"op": "create_range", "local": null, "domain": {"id": id},
                "first": "10.0.0.1", "last": "10.0.0.9", "usage": "dhcp"}),
            json!({"op": "create_address", "local": 4, "domain": {"id": id},
                "address": "10.0.0.1", "usage": "gateway"}),
            json!({"op": "set_role", "key": "r", "labels": {"fr": "R"}, "entries": ["*"]}),
            json!({"op": "close_role", "key": "r"}),
            json!({"op": "set_group", "key": "g", "labels": {"fr": "G"}}),
            json!({"op": "close_group", "key": "g"}),
            json!({"op": "add_member", "local": 10, "group": "g",
                "member": {"kind": "identity", "id": id}}),
            json!({"op": "add_member", "local": null, "group": "g",
                "member": {"kind": "idp_group", "issuer": "i", "group": "netops"}}),
            json!({"op": "remove_member", "id": id}),
            json!({"op": "bind", "local": 5, "group": "g",
                "role": "r", "scope": {"kind": "entity", "id": {"local": 2}}}),
            json!({"op": "unbind", "id": id}),
            json!({"op": "attach_issuer", "issuer": "i", "entity": {"id": id}}),
            json!({"op": "detach_issuer", "issuer": "i"}),
            json!({"op": "set_identity_state", "identity": id, "disabled": true, "revoke_sessions": true}),
            json!({"op": "create_service_account", "local": 6, "name": "ci", "secret": secret}),
            json!({"op": "add_service_secret", "account": id, "secret": secret}),
            json!({"op": "revoke_service_secret", "account": id, "secret": other}),
            json!({"op": "set_approval_policy", "name": "p", "min_operations": 3,
                "min_diff_objects": 9, "property_keys": ["vlan"], "domains": [id],
                "scopes": [{"kind": "global"}, {"kind": "prefix", "id": {"id": id}}],
                "author_roles": ["r"], "author_groups": ["g"], "bound_roles": ["admin"],
                "enabled": false}),
            json!({"op": "restore", "kind": "enum_value", "id": {"id": id}, "property": other, "before": id}),
            json!({"op": "bind_external", "object": {"id": id}, "kind": "site", "source": "s",
                "external_id": "x"}),
            json!({"op": "create_entity", "local": 7, "slug": "e", "parent": {"local": 8}}),
            json!({"op": "create_site", "local": 9, "slug": "s"}),
            json!({"op": "move_entity", "id": {"id": id}, "parent": {"id": other}, "expected": id}),
            json!({"op": "move_site", "id": {"id": id}}),
            json!({"op": "set_sensitivity", "property": {"id": id}, "sensitive": true}),
            json!({"op": "rename", "kind": "site", "id": {"id": id}, "slug": "n", "expected": other}),
            declare,
            update,
            json!({"op": "add_enum_value", "local": 1, "property": {"id": id}, "key": "k",
                "labels": {"fr": "K"}}),
            json!({"op": "update_enum_value", "property": {"id": id}, "key": "k",
                "labels": {"fr": "K", "en": "K"}}),
            json!({"op": "assign_property", "object": {"id": id}, "kind": "prefix",
                "property": {"local": 3}, "values": [true, 4, "x"]}),
            json!({"op": "clear_property", "object": {"id": id}, "kind": "domain", "property": {"id": other}}),
            json!({"op": "unassign_property", "object": {"id": id}, "kind": "range", "property": {"id": other}}),
            json!({"op": "expect", "kind": "address", "id": {"id": id}, "expected": other}),
            json!({"op": "close", "kind": "property", "id": {"id": id}, "expected": other, "cascade": true}),
            json!({"op": "update_prefix", "id": {"id": id}, "domain": {"id": other},
                "prefix": "10.0.0.0/9", "nature": "network", "point_to_point": false,
                "anycast": true, "expected": other}),
            json!({"op": "update_address", "id": {"id": id}, "usage": "vip", "expected": other}),
            json!({"op": "update_range", "id": {"id": id}, "domain": {"id": other}, "usage": "static"}),
            json!({"op": "kit.make_thing", "local": 1, "name": "x"}),
            json!({"op": "close", "kind": "kit.thing", "id": {"id": id}}),
        ]
    }

    #[test]
    fn each_operation_reads_alike_through_the_mirror() {
        let mut named = std::collections::BTreeSet::new();
        for sample in samples() {
            let direct: Operation = serde_json::from_value(sample.clone()).unwrap();
            let mirrored: OperationDto = serde_json::from_value(sample.clone()).unwrap();
            assert_eq!(Operation::from(mirrored), direct, "{sample}");
            named.insert(name(&direct));
        }
        assert_eq!(named.len(), 41, "one sample per operation of the service");
        let item = json!({"external_id": "x", "operation": samples()[1]});
        let direct: ImportItem = serde_json::from_value(item.clone()).unwrap();
        let mirrored: ImportItemDto = serde_json::from_value(item).unwrap();
        assert_eq!(ImportItem::from(mirrored), direct);
    }
}
