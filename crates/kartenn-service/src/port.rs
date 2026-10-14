//! Storage port of the write pipeline (hexagonal architecture, ADR 0001).

use std::collections::{BTreeMap, HashMap};
use std::future::Future;

use kartenn_core::inheritance::Assigned;
use kartenn_core::properties::{CurrentAssignment, Declaration, ObligationScope};
use kartenn_core::values::Value;

use kartenn_core::addr::{Addr, Prefix};
use kartenn_core::objects::{Address, AddressUsage, IpRange, PrefixObject};
use kartenn_core::period::Period;
use kartenn_core::rules::Violation;
use uuid::Uuid;

use crate::operations::{ChangesetRequest, ObjectKind};

/// Infrastructure failure: the changeset is neither applied nor refused by a rule.
#[derive(Debug, thiserror::Error)]
#[error("storage failure: {0}")]
pub struct StoreError(pub String);

/// Failure of one write.
#[derive(Debug, thiserror::Error)]
pub enum WriteError {
    /// A database constraint, mapped to the rule it backs (last line of defence).
    #[error("{0}")]
    Rule(Violation),
    /// The object does not exist where the operation needs it.
    #[error("object {0} does not exist at the targeted time")]
    NoVersion(Uuid),
    /// Infrastructure failure.
    #[error(transparent)]
    Store(#[from] StoreError),
}

/// What is written to the audit for a changeset (REQ-004-31).
#[derive(Debug, Clone)]
pub struct Journal<'a> {
    /// Changeset identifier.
    pub id: Uuid,
    /// The request.
    pub request: &'a ChangesetRequest,
    /// `applied` or `refused`.
    pub state: &'static str,
    /// Violations (errors and warnings).
    pub violations: &'a [Violation],
    /// Addressing domains written by the changeset (event ordering key, ADR 0018).
    pub domains: &'a [Uuid],
    /// Objects created or modified.
    pub objects: &'a [Uuid],
    /// Rules evaluated anew on objects, as `(object, rule-id prefix)`: their anomalies that
    /// no longer fire are closed, and only those (REQ-002-35, 36). An empty prefix covers
    /// every rule (a closed object).
    pub reevaluated: &'a [(Uuid, &'static str)],
    /// Differential of resolved values (REQ-003-19); its detail is the one recorded in
    /// the transaction by [`WriteTx::record_diff`].
    pub diff: &'a [DiffCount],
}

/// Use of a property, as needed to check an evolution of its declaration (REQ-003-6).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PropertyFacts {
    /// Assignments over the period.
    pub assignments: Vec<CurrentAssignment>,
    /// Objects that inherit their value from another object.
    pub inheriting: Vec<Uuid>,
    /// Current closed-list values, by key.
    pub enum_values: HashMap<String, Uuid>,
    /// Kinds of the objects referenced by the assignments.
    pub ref_kinds: HashMap<Uuid, &'static str>,
}

/// What a source of expected values sees of an object (REQ-003-21): the core object and
/// the resolved values of its properties, read only.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectView {
    /// Object.
    pub id: Uuid,
    /// Kind (`domain`, `prefix`, `range`, `address`).
    pub kind: String,
    /// Addressing domain.
    pub domain: Uuid,
    /// Prefix, first address of a range, or address; `None` for a domain.
    pub value: Option<String>,
    /// Resolved values by property key, as JSON (closed-list keys); `None` is "no value".
    pub properties: BTreeMap<String, Option<String>>,
}

/// Count of objects whose resolved value of a property goes from `before` to `after`
/// (REQ-003-19). Values are JSON texts (closed-list keys for a closed list); `None` is
/// "no value".
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct DiffCount {
    /// Property key.
    pub property: String,
    /// Value before.
    pub before: Option<String>,
    /// Value after.
    pub after: Option<String>,
    /// Objects.
    pub count: u64,
}

/// One object of the differential (REQ-003-19).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct DiffItem {
    /// Property key.
    pub property: String,
    /// Object.
    pub object: Uuid,
    /// Value before.
    pub before: Option<String>,
    /// Value after.
    pub after: Option<String>,
}

/// Earlier submission found for an idempotency key (REQ-004-10).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Replay {
    /// No live submission under this key.
    Fresh,
    /// Same key, same content: the first changeset and its state.
    Same {
        /// First changeset.
        changeset: Uuid,
        /// Its state (`applied`, `refused`…).
        state: String,
    },
    /// Same key, different content.
    Conflict,
}

/// A store able to open write transactions.
pub trait Store: Send + Sync {
    /// Transaction type.
    type Tx: WriteTx + Send;

    /// Opens a transaction.
    fn begin(&self) -> impl Future<Output = Result<Self::Tx, StoreError>> + Send;

    /// Journals a changeset that was not applied, in a transaction of its own.
    fn journal_refused(
        &self,
        journal: &Journal<'_>,
    ) -> impl Future<Output = Result<(), StoreError>> + Send;
}

/// One write transaction. Every write runs under a savepoint, so that a refused write
/// leaves the transaction usable and every violation can be reported (REQ-004-7).
pub trait WriteTx {
    /// Takes the advisory locks of the domains, in order, and of public space if needed
    /// (ADR 0015).
    fn lock(
        &mut self,
        domains: &[Uuid],
        public_space: bool,
    ) -> impl Future<Output = Result<(), StoreError>> + Send;

    /// Serialises submissions of the same idempotency key (advisory lock held until the
    /// end of the transaction) and looks up a live earlier submission.
    fn replay(
        &mut self,
        request: &ChangesetRequest,
    ) -> impl Future<Output = Result<Replay, StoreError>> + Send;

    /// Changeset that wrote the version of an object valid at the start of `period`
    /// (transaction time if `None`).
    fn current_changeset(
        &mut self,
        kind: ObjectKind,
        id: Uuid,
        period: Option<Period>,
    ) -> impl Future<Output = Result<Option<Uuid>, StoreError>> + Send;

    /// Domain of an existing addressing object.
    fn domain_of(
        &mut self,
        kind: ObjectKind,
        id: Uuid,
    ) -> impl Future<Output = Result<Option<Uuid>, StoreError>> + Send;

    /// Writes a new domain.
    fn insert_domain(
        &mut self,
        id: Uuid,
        slug: &str,
        period: Option<Period>,
        changeset: Uuid,
    ) -> impl Future<Output = Result<(), WriteError>> + Send;

    /// Writes a new prefix.
    fn insert_prefix(
        &mut self,
        prefix: &PrefixObject,
        public: bool,
        period: Option<Period>,
        changeset: Uuid,
    ) -> impl Future<Output = Result<(), WriteError>> + Send;

    /// Writes a new address.
    fn insert_address(
        &mut self,
        address: &Address,
        public: bool,
        period: Option<Period>,
        changeset: Uuid,
    ) -> impl Future<Output = Result<(), WriteError>> + Send;

    /// Ends an object at the start of `period` (transaction time if `None`).
    fn close(
        &mut self,
        kind: ObjectKind,
        id: Uuid,
        period: Option<Period>,
        changeset: Uuid,
    ) -> impl Future<Output = Result<(), WriteError>> + Send;

    /// Networks of `domain` strictly containing `prefix` over part of `period` (002/A2).
    /// With `include_equal`, a network equal to `prefix` counts (for addresses: a /32 or
    /// /128 network holds its single address).
    fn networks_containing(
        &mut self,
        domain: Uuid,
        prefix: Prefix,
        include_equal: bool,
        period: Option<Period>,
    ) -> impl Future<Output = Result<Vec<PrefixObject>, StoreError>> + Send;

    /// Prefixes of `domain` strictly inside `prefix` over part of `period`.
    fn prefixes_inside(
        &mut self,
        domain: Uuid,
        prefix: Prefix,
        period: Option<Period>,
    ) -> impl Future<Output = Result<Vec<Uuid>, StoreError>> + Send;

    /// Whether every version of the address lies, over its whole period, in networks of
    /// its domain (REQ-002-25).
    fn address_covered(
        &mut self,
        address: Uuid,
    ) -> impl Future<Output = Result<bool, StoreError>> + Send;

    /// Addresses of `domain` inside `prefix` over part of `period`.
    fn addresses_inside(
        &mut self,
        domain: Uuid,
        prefix: Prefix,
        period: Option<Period>,
    ) -> impl Future<Output = Result<Vec<Uuid>, StoreError>> + Send;

    /// Writes a new range.
    fn insert_range(
        &mut self,
        range: &IpRange,
        public: bool,
        period: Option<Period>,
        changeset: Uuid,
    ) -> impl Future<Output = Result<(), WriteError>> + Send;

    /// Whether every version of the range lies, over its whole period, in networks of its
    /// domain that contain both its bounds (REQ-002-21).
    fn range_covered(
        &mut self,
        range: Uuid,
    ) -> impl Future<Output = Result<bool, StoreError>> + Send;

    /// Ranges of `domain` overlapping `[first, last]` over part of `period`.
    fn ranges_overlapping(
        &mut self,
        domain: Uuid,
        first: Addr,
        last: Addr,
        period: Option<Period>,
    ) -> impl Future<Output = Result<Vec<IpRange>, StoreError>> + Send;

    /// Ranges of `domain` inside `prefix` over part of `period`.
    fn ranges_inside(
        &mut self,
        domain: Uuid,
        prefix: Prefix,
        period: Option<Period>,
    ) -> impl Future<Output = Result<Vec<Uuid>, StoreError>> + Send;

    /// Addresses of `domain` within `[first, last]` over part of `period`.
    fn addresses_between(
        &mut self,
        domain: Uuid,
        first: Addr,
        last: Addr,
        period: Option<Period>,
    ) -> impl Future<Output = Result<Vec<Address>, StoreError>> + Send;

    /// Public networks of other domains overlapping `prefix` over part of `period`
    /// (REQ-002-32, 46).
    fn public_networks_overlapping(
        &mut self,
        domain: Uuid,
        prefix: Prefix,
        period: Option<Period>,
    ) -> impl Future<Output = Result<Vec<PrefixObject>, StoreError>> + Send;

    /// Public addresses equal to `address` in other domains over part of `period`, with
    /// their usage.
    fn public_addresses_equal(
        &mut self,
        domain: Uuid,
        address: Addr,
        period: Option<Period>,
    ) -> impl Future<Output = Result<Vec<(Uuid, AddressUsage)>, StoreError>> + Send;

    /// Domain and value of a prefix (any version).
    fn prefix_of(
        &mut self,
        id: Uuid,
    ) -> impl Future<Output = Result<Option<(Uuid, Prefix)>, StoreError>> + Send;

    /// Inserts a property declaration (REQ-003-1 to 3).
    fn insert_declaration(
        &mut self,
        id: Uuid,
        declaration: &Declaration,
        period: Option<Period>,
        changeset: Uuid,
    ) -> impl Future<Output = Result<(), WriteError>> + Send;

    /// The declaration of a property valid at the start of `period`.
    fn declaration(
        &mut self,
        id: Uuid,
        period: Option<Period>,
    ) -> impl Future<Output = Result<Option<Declaration>, StoreError>> + Send;

    /// Replaces the version of a declaration valid at the start of `period`, from that
    /// start to the end of the version (REQ-003-6).
    fn replace_declaration(
        &mut self,
        id: Uuid,
        declaration: &Declaration,
        period: Option<Period>,
        changeset: Uuid,
    ) -> impl Future<Output = Result<(), WriteError>> + Send;

    /// Current declarations of a namespace, with the keys of their closed-list values.
    fn declarations_in(
        &mut self,
        namespace: &str,
    ) -> impl Future<Output = Result<Vec<(Uuid, Declaration, Vec<String>)>, StoreError>> + Send;

    /// Inserts a closed-list value (REQ-003-5, 7).
    fn insert_enum_value(
        &mut self,
        id: Uuid,
        property: Uuid,
        key: &str,
        period: Option<Period>,
        changeset: Uuid,
    ) -> impl Future<Output = Result<(), WriteError>> + Send;

    /// Closes the closed-list values of a property over `period`.
    fn close_enum_values_of(
        &mut self,
        property: Uuid,
        period: Option<Period>,
        changeset: Uuid,
    ) -> impl Future<Output = Result<(), StoreError>> + Send;

    /// What the compatibility checker needs about the use of a property over `period`
    /// (REQ-003-6), values decoded for the type `type_id`.
    fn property_facts(
        &mut self,
        property: Uuid,
        type_id: &str,
        period: Option<Period>,
    ) -> impl Future<Output = Result<PropertyFacts, StoreError>> + Send;

    /// Objects with an assignment of a property over part of `period`.
    fn objects_assigned(
        &mut self,
        property: Uuid,
        period: Option<Period>,
    ) -> impl Future<Output = Result<Vec<Uuid>, StoreError>> + Send;

    /// Objects whose assignment contains a closed-list value over part of `period`.
    fn objects_carrying(
        &mut self,
        enum_value: Uuid,
        period: Option<Period>,
    ) -> impl Future<Output = Result<Vec<Uuid>, StoreError>> + Send;

    /// Kind of an object of the addressing tree that exists over the whole of `period`.
    fn object_kind_covering(
        &mut self,
        id: Uuid,
        period: Option<Period>,
    ) -> impl Future<Output = Result<Option<&'static str>, StoreError>> + Send;

    /// Identifiers of closed-list values of a property valid over the whole of `period`,
    /// by key.
    fn enum_values_covering(
        &mut self,
        property: Uuid,
        keys: &[String],
        period: Option<Period>,
    ) -> impl Future<Output = Result<HashMap<String, Uuid>, StoreError>> + Send;

    /// Writes an assignment over `period`, replacing any assignment there (REQ-003-12, 13).
    #[allow(clippy::too_many_arguments)] // one per column of the assignment and its context
    fn assign(
        &mut self,
        object: Uuid,
        object_type: &str,
        property: Uuid,
        assigned: &Assigned,
        provenance: &str,
        period: Option<Period>,
        changeset: Uuid,
    ) -> impl Future<Output = Result<(), WriteError>> + Send;

    /// A frozen view of an object of the addressing tree at the start of `period`, for
    /// sources of expected values (REQ-003-21); `None` if it does not exist then.
    fn object_view(
        &mut self,
        object: Uuid,
        period: Option<Period>,
    ) -> impl Future<Output = Result<Option<ObjectView>, StoreError>> + Send;

    /// Resolved value of a property for an object at the start of `period`; `None` when
    /// absent or explicitly "no value".
    fn resolved_value(
        &mut self,
        object: Uuid,
        property: Uuid,
        declaration: &Declaration,
        period: Option<Period>,
    ) -> impl Future<Output = Result<Option<Value>, StoreError>> + Send;

    /// Version of a source whose deviations were last re-evaluated (REQ-003-24).
    fn source_version(
        &mut self,
        source: &str,
    ) -> impl Future<Output = Result<Option<String>, StoreError>> + Send;

    /// Records that the deviations of a source were re-evaluated at `version`.
    fn set_source_version(
        &mut self,
        source: &str,
        version: &str,
    ) -> impl Future<Output = Result<(), StoreError>> + Send;

    /// Properties with a source of expected values at the start of `period`.
    fn with_expected(
        &mut self,
        period: Option<Period>,
    ) -> impl Future<Output = Result<Vec<(Uuid, Declaration)>, StoreError>> + Send;

    /// Removes the assignment of a property over `period`; with `property` `None`, every
    /// assignment of the object from the start of `period` on (closure of the object).
    fn unassign(
        &mut self,
        object: Uuid,
        property: Option<Uuid>,
        period: Option<Period>,
        changeset: Uuid,
    ) -> impl Future<Output = Result<(), StoreError>> + Send;

    /// Properties heritable along the addressing tree at the start of `period`, with their
    /// carrier kinds.
    fn addressing_heritable(
        &mut self,
        period: Option<Period>,
    ) -> impl Future<Output = Result<Vec<(Uuid, Vec<String>)>, StoreError>> + Send;

    /// The object and the objects below it in the addressing tree, from the start of
    /// `period` on.
    fn subtree(
        &mut self,
        object: Uuid,
        period: Option<Period>,
    ) -> impl Future<Output = Result<Vec<Uuid>, StoreError>> + Send;

    /// Every object of the given kinds, from the start of `period` on.
    fn objects_of_kinds(
        &mut self,
        kinds: &[String],
        period: Option<Period>,
    ) -> impl Future<Output = Result<Vec<Uuid>, StoreError>> + Send;

    /// `(property, object)` pairs whose origin is `origin` from the start of `period` on.
    fn inheritors(
        &mut self,
        origin: Uuid,
        period: Option<Period>,
    ) -> impl Future<Output = Result<Vec<(Uuid, Uuid)>, StoreError>> + Send;

    /// Recomputes the origin of a heritable property for `targets` (those of a carrier
    /// kind) from the start of `period` on, and writes the segments that changed
    /// (003/design, § Résolution matérialisée). Returns the number of objects whose
    /// origin changed.
    fn refresh_origins(
        &mut self,
        property: Uuid,
        carriers: &[String],
        targets: &[Uuid],
        period: Option<Period>,
        changeset: Uuid,
    ) -> impl Future<Output = Result<usize, StoreError>> + Send;

    /// Ends every origin of a property from the start of `period` on (closed property).
    fn drop_origins(
        &mut self,
        property: Uuid,
        period: Option<Period>,
        changeset: Uuid,
    ) -> impl Future<Output = Result<(), StoreError>> + Send;

    /// Carrier objects of `scope` (among `candidates` if given) without a resolved value of
    /// `property` at some instant from the start of `period` on (REQ-003-16).
    fn objects_without_value(
        &mut self,
        property: Uuid,
        carriers: &[String],
        heritable: bool,
        scope: &ObligationScope,
        candidates: Option<&[Uuid]>,
        period: Option<Period>,
    ) -> impl Future<Output = Result<Vec<Uuid>, StoreError>> + Send;

    /// Objects carrying a reference assignment to `target` over a time when `target` does
    /// not exist (REQ-003-10).
    fn dangling_references(
        &mut self,
        target: Uuid,
    ) -> impl Future<Output = Result<Vec<Uuid>, StoreError>> + Send;

    /// Properties with obligations at the start of `period`, with their declaration.
    fn obligated(
        &mut self,
        period: Option<Period>,
    ) -> impl Future<Output = Result<Vec<(Uuid, Declaration)>, StoreError>> + Send;

    /// Records, for later restitution, the objects among `objects` whose resolved value of
    /// `property` differs before and after this transaction, from the start of `period`
    /// on (REQ-003-19); `excluded` are the objects the changeset creates.
    fn record_diff(
        &mut self,
        property: Uuid,
        objects: &[Uuid],
        heritable: bool,
        excluded: &[Uuid],
        period: Option<Period>,
    ) -> impl Future<Output = Result<(), StoreError>> + Send;

    /// Counts of the differential recorded so far.
    fn diff_summary(&mut self) -> impl Future<Output = Result<Vec<DiffCount>, StoreError>> + Send;

    /// Objects of the differential recorded so far for a property key.
    fn diff_objects(
        &mut self,
        property: &str,
    ) -> impl Future<Output = Result<Vec<Uuid>, StoreError>> + Send;

    /// First `limit` entries of the differential recorded so far.
    fn diff_sample(
        &mut self,
        limit: i64,
    ) -> impl Future<Output = Result<Vec<DiffItem>, StoreError>> + Send;

    /// Journals the applied changeset in this transaction.
    fn journal_applied(
        &mut self,
        journal: &Journal<'_>,
    ) -> impl Future<Output = Result<(), StoreError>> + Send;

    /// Commits.
    fn commit(self) -> impl Future<Output = Result<(), StoreError>> + Send;

    /// Rolls back.
    fn rollback(self) -> impl Future<Output = Result<(), StoreError>> + Send;
}
