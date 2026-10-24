//! Storage port of the write pipeline (hexagonal architecture, ADR 0001).

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::future::Future;

use kartenn_core::inheritance::Assigned;
use kartenn_core::properties::{CurrentAssignment, Declaration, InheritTree, ObligationScope};
use kartenn_core::values::Value;

use kartenn_core::addr::{Addr, Prefix};
use kartenn_core::objects::{
    Address, AddressUsage, IpRange, Nature, PrefixObject, RangeUsage, Slug,
};
use kartenn_core::period::Period;
use kartenn_core::rules::Violation;
use uuid::Uuid;

use crate::operations::{ChangesetRequest, ObjectKind, Operation};

/// Infrastructure failure: the changeset is neither applied nor refused by a rule.
#[derive(Debug, thiserror::Error)]
#[error("storage failure: {0}")]
pub struct StoreError(pub String);

/// A modification of an addressing object over a period (004-8): the fields given change.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Change {
    /// A prefix.
    Prefix {
        /// New value and its public class (resizing).
        value: Option<(Prefix, bool)>,
        /// New nature.
        nature: Option<Nature>,
        /// Point-to-point or not.
        point_to_point: Option<bool>,
        /// Anycast or not.
        anycast: Option<bool>,
    },
    /// The usage of an address.
    Address(AddressUsage),
    /// The usage of a range.
    Range(RangeUsage),
    /// The parent of an entity or a site; none for a root.
    Parent(Option<Uuid>),
    /// The slug of a domain, an entity or a site (REQ-002-47).
    Slug(Slug),
    /// Whether a property is sensitive (REQ-003-27).
    Sensitive(bool),
    /// The labels of a property or a closed-list value (spec 018, REQ-018-9).
    Labels(kartenn_core::labels::Labels),
    /// An administrator's obligation on a property, added or removed (REQ-008-28).
    Obligation {
        /// Where the property is required.
        scope: kartenn_core::properties::ObligationScope,
        /// Added, or removed.
        required: bool,
    },
}

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

/// An object as it was valid at a date and known at another.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectRead {
    /// `domain`, `prefix`, `range`, `address`, `entity` or `site`.
    pub kind: String,
    /// Attributes, by column.
    pub fields: serde_json::Value,
    /// Resolved value of each property the object carries or inherits, by key (closed-list
    /// keys); `None` for an explicit "no value".
    pub properties: BTreeMap<String, Option<serde_json::Value>>,
    /// Keys of `properties` declared sensitive (spec 003, REQ-003-27): reading them is
    /// audited (spec 012, REQ-012-18).
    pub sensitive: BTreeSet<String>,
    /// Keys of sensitive properties whose value the reader may not see (005,
    /// REQ-005-21): absent from `properties`.
    pub masked: BTreeSet<String>,
}

/// The resolution of one property for one object, explained (REQ-003-14, REQ-016-1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resolved {
    /// The object.
    pub object: Uuid,
    /// Its kind (`domain`…, or a qualified type of a module).
    pub kind: String,
    /// Property key.
    pub property: String,
    /// Whether the property is declared sensitive (REQ-003-27).
    pub sensitive: bool,
    /// Resolved value (closed-list keys); `None` when absent, "no value" or masked.
    pub value: Option<serde_json::Value>,
    /// Object carrying the applicable assignment; `None` when nothing applies.
    pub origin: Option<Uuid>,
    /// Objects walked from the object to its origin, both included.
    pub chain: Vec<Uuid>,
    /// Provenance of the applicable assignment: `input` or `source:<id>@<version>`.
    pub provenance: Option<String>,
    /// The value is sensitive and the reader may not see it (REQ-005-21).
    pub masked: bool,
}

/// What a consultation of the audit journal asks for (spec 012, REQ-012-17).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AuditQuery {
    /// Recorded from this time on.
    pub from: Option<kartenn_core::period::Instant>,
    /// Recorded before this time.
    pub to: Option<kartenn_core::period::Instant>,
    /// Of this event name.
    pub name: Option<String>,
    /// By this actor.
    pub actor: Option<Uuid>,
    /// About this object.
    pub object: Option<Uuid>,
    /// Recorded before the event of this sequence number (the cursor of a page).
    pub before: Option<i64>,
    /// At most this many events, the most recent first.
    pub limit: u32,
}

/// Reads that the service audits (spec 012, REQ-012-17, 18); the API (006) serves them.
pub trait Reader: Send + Sync {
    /// The object `id` valid at `valid_at`, as asserted at `as_of` (now when `None`).
    fn object(
        &self,
        id: Uuid,
        valid_at: kartenn_core::period::Instant,
        as_of: Option<kartenn_core::period::Instant>,
    ) -> impl Future<Output = Result<Option<ObjectRead>, StoreError>> + Send;

    /// The properties `keys` of the `objects` resolved and explained at `valid_at`, as
    /// asserted at `as_of` (REQ-016-1): one per existing object and declared key, in the
    /// order of `objects` then `keys`; nothing filtered by rights.
    fn resolved_many(
        &self,
        objects: &[Uuid],
        keys: &[String],
        valid_at: kartenn_core::period::Instant,
        as_of: Option<kartenn_core::period::Instant>,
    ) -> impl Future<Output = Result<Vec<Resolved>, StoreError>> + Send;

    /// Audit events matching `query`, as exported (JSON).
    fn audit_events(
        &self,
        query: &AuditQuery,
    ) -> impl Future<Output = Result<Vec<serde_json::Value>, StoreError>> + Send;

    /// How many events match `query` (its `limit` aside), and the sequence number of the
    /// most recent of them.
    fn audit_extent(
        &self,
        query: &AuditQuery,
    ) -> impl Future<Output = Result<(u64, Option<i64>), StoreError>> + Send;
}

/// What is written to the audit for a changeset (REQ-004-31).
#[derive(Debug, Clone)]
pub struct Journal<'a> {
    /// Changeset identifier.
    pub id: Uuid,
    /// The request.
    pub request: &'a ChangesetRequest,
    /// `applied`, `refused`, `pending` or `failed_revalidation`.
    pub state: &'static str,
    /// `direct`, `approval` or `proposal` (kept when a pending changeset is decided).
    pub kind: &'static str,
    /// Who validated a pending changeset.
    pub validator: Option<Uuid>,
    /// What a pending changeset waits for.
    pub pending: Option<&'a Pending>,
    /// Violations (errors and warnings).
    pub violations: &'a [Violation],
    /// Addressing domains written by the changeset (event ordering key, ADR 0018).
    pub domains: &'a [Uuid],
    /// Lock keys of module operations (ADR 0026, clause 14): with the domains, the
    /// ordering keys of the changeset's events (REQ-016-14, ADR 0018 amendment 4).
    pub keys: &'a [String],
    /// Objects created or modified.
    pub objects: &'a [Uuid],
    /// Rules evaluated anew on objects, as `(object, rule-id prefix)`: their anomalies that
    /// no longer fire are closed, and only those (REQ-002-35, 36). An empty prefix covers
    /// every rule (a closed object).
    pub reevaluated: &'a [(Uuid, &'static str)],
    /// Differential of resolved values (REQ-003-19); its detail is the one recorded in
    /// the transaction by [`WriteTx::record_diff`].
    pub diff: &'a [DiffCount],
    /// Closures added by a cascade (REQ-004-26), in the order of the expansion.
    pub cascaded: &'a [(ObjectKind, Uuid)],
    /// Audit events written with the journal, in its transaction (REQ-012-8, 11).
    pub audit: &'a [kartenn_core::audit::AuditEvent],
    /// The requester a service account wrote for (REQ-016-12), an identity `requester`.
    pub on_behalf_of: Option<Uuid>,
}

/// What a pending changeset waits for (004/design, § Approbation et propositions).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Pending {
    /// The approval policy that holds it, for an approval.
    pub policy: Option<Uuid>,
    /// Objects of its operations and of its differential.
    pub perimeter: Vec<Uuid>,
    /// The edit permissions it requires, with the objects concerned, and the review
    /// permissions its modules name: whoever applies it needs their application (spec 024,
    /// REQ-024-11; [`kartenn_core::authz::verb_permission`]).
    pub required: Vec<(String, Vec<Uuid>)>,
    /// A module requires its review (REQ-016-10): its author does not apply it.
    #[serde(default)]
    pub review: bool,
    /// Why its author could not apply it when asking to (spec 024, REQ-024-13): the
    /// message and the objects of its `changeset.approval.self`, answered again on a
    /// replay.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<(String, Vec<Uuid>)>,
    /// The author's issuer and groups when submitting (005, REQ-005-22): an approval
    /// checks the author's rights again with them, the groups living in the token.
    #[serde(default)]
    pub author: Option<crate::authz::Subject>,
    /// The requester a service account submitted it for (REQ-016-12), given to the
    /// modules again when it is validated (ADR 0026, amendment 3).
    #[serde(default)]
    pub on_behalf_of: Option<Uuid>,
}

/// A changeset journaled as pending, or decided since.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingChangeset {
    /// Identifier.
    pub id: Uuid,
    /// The request as submitted.
    pub request: ChangesetRequest,
    /// `approval` or `proposal`.
    pub kind: String,
    /// Current state.
    pub state: String,
    /// What it waits for.
    pub pending: Pending,
}

/// One element of an import (REQ-004-28): an operation, with the identifier in the source
/// of the object it creates.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ImportItem {
    /// Identifier in the source; required for an operation that creates an object.
    #[serde(default)]
    pub external_id: Option<String>,
    /// The operation; its local references hold for the whole import.
    pub operation: Operation,
}

/// A batch of an import, applied as one changeset.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportBatch {
    /// Position.
    pub index: i32,
    /// Elements, in order.
    pub items: Vec<ImportItem>,
    /// `queued`, `applied` or `failed`.
    pub state: String,
    /// The changeset that applied it.
    pub changeset: Option<Uuid>,
}

/// An import and its batches.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportRecord {
    /// Identifier.
    pub id: Uuid,
    /// Its author.
    pub author: Uuid,
    /// The author's issuer and groups when starting or resuming it (005, REQ-005-22):
    /// its batches run in a background task, without the token where the groups live,
    /// and are submitted with them, as the approval of a pending changeset is checked.
    pub author_subject: Option<crate::authz::Subject>,
    /// The source (connector).
    pub source: String,
    /// `uploading`, `ready`, `running`, `paused`, `done` or `cancelled`.
    pub state: String,
    /// Report: the failure that paused it, if any.
    pub report: serde_json::Value,
    /// Its batches, in order.
    pub batches: Vec<ImportBatch>,
}

/// Where an import stands, without its elements.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportProgress {
    /// State, as in [`ImportRecord`].
    pub state: String,
    /// Batches applied.
    pub applied: usize,
    /// All its batches.
    pub batches: usize,
    /// Report: the failure that paused it, if any.
    pub report: serde_json::Value,
}

/// What the store knows of an applied changeset to undo it (REQ-004-34, 35).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UndoFacts {
    /// Earliest start of its periods, in microseconds since the epoch.
    pub start: i64,
    /// Objects and assignments it wrote.
    pub touched: Vec<Touched>,
    /// What it wrote that is not undone: declarations, closed-list values, accounts
    /// (kind, id).
    pub unsupported: Vec<(String, Uuid)>,
    /// Roles, bindings and issuer attachments it wrote (spec 005, REQ-005-25, K16).
    pub rbac: Vec<RbacTouched>,
}

/// A role, a binding or an issuer attachment written by a changeset to undo (spec 005,
/// K16).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RbacTouched {
    /// Which one.
    pub row: RbacRow,
    /// As it was in force before the changeset, from the start of the undo; `None`: the
    /// changeset created it.
    pub before: Option<RbacBefore>,
    /// One is in force after the changeset, from the start of the undo.
    pub after: bool,
    /// A later changeset that wrote it there: a conflict.
    pub later: Option<Uuid>,
}

/// The identity of a role, a binding or an issuer attachment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RbacRow {
    /// A role, by key.
    Role {
        /// Key.
        key: String,
    },
    /// A binding.
    Binding {
        /// Its identifier.
        id: Uuid,
    },
    /// The attachment of an issuer to an entity, or everywhere.
    Attachment {
        /// Issuer.
        issuer: String,
        /// Entity; `None`: everywhere.
        entity: Option<Uuid>,
    },
}

/// A role, a binding or an attachment as it was before a changeset.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RbacBefore {
    /// Its application period then.
    pub period: crate::operations::PeriodSpec,
    /// What it was.
    pub state: RbacState,
}

/// The content of a role or a binding before a changeset.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RbacState {
    /// A role.
    Role {
        /// Labels by language.
        labels: crate::operations::RawLabels,
        /// Entries.
        entries: Vec<String>,
    },
    /// A binding.
    Binding {
        /// Who.
        subject: crate::operations::BindingSubject,
        /// Role key.
        role: String,
        /// Where.
        scope: crate::operations::BindingScope,
    },
    /// An attachment: nothing beyond its identity.
    Attachment,
}

/// An object, or an assignment of it, written by a changeset to undo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Touched {
    /// Kind of the object.
    pub kind: ObjectKind,
    /// Object.
    pub id: Uuid,
    /// The property, for an assignment.
    pub property: Option<Uuid>,
    /// No version was asserted before the changeset: it created it.
    pub created: bool,
    /// A later changeset that wrote it over the period: a conflict.
    pub later: Option<Uuid>,
}

/// An approval policy (REQ-004-20, 40): it holds a changeset when every trigger it
/// defines matches.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApprovalPolicy {
    /// Identifier.
    pub id: Uuid,
    /// Unique name.
    pub name: String,
    /// At least this many operations.
    pub min_operations: Option<u32>,
    /// At least this many objects in the differential.
    pub min_diff_objects: Option<u64>,
    /// One of these properties changes, directly or by effect.
    pub property_keys: Vec<String>,
    /// The changeset touches one of these addressing domains.
    pub domains: Vec<Uuid>,
    /// The author is one of these people.
    pub authors: Vec<Uuid>,
    /// The changeset touches an object of one of these scopes (005, REQ-005-27): `entity`,
    /// `site` or `prefix`, with its subtree; with `domains`, one perimeter trigger.
    pub scopes: Vec<(String, Uuid)>,
    /// The author holds one of these roles (005); with `authors`, one author trigger.
    pub author_roles: Vec<String>,
    /// The author belongs to one of these groups (issuer, group) (005).
    pub author_groups: Vec<(String, String)>,
    /// The changeset binds one of these roles (005, REQ-005-25: a second administrator
    /// for any binding of the administrator role).
    pub bound_roles: Vec<String>,
    /// Measures of modules and their thresholds (`<module>.<measure>`, REQ-016-7): each
    /// reaches its threshold.
    pub module_triggers: Vec<(String, u64)>,
    /// Disabled policies hold nothing.
    pub enabled: bool,
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
    /// The same key is being processed by another transaction (006/REQ-006-15).
    InProgress,
}

/// A store able to open write transactions.
pub trait Store: Send + Sync {
    /// The modules of the build, whose operations its changesets accept (ADR 0026).
    fn modules(&self) -> &crate::module::ModuleRegistry {
        crate::module::ModuleRegistry::empty()
    }

    /// Transaction type.
    type Tx: WriteTx + Send;

    /// Opens a transaction.
    fn begin(&self) -> impl Future<Output = Result<Self::Tx, StoreError>> + Send;

    /// Records an audit event in a transaction of its own (REQ-012-11), for a refusal
    /// that writes nothing else.
    fn audit(
        &self,
        event: &kartenn_core::audit::AuditEvent,
    ) -> impl Future<Output = Result<(), StoreError>> + Send;

    /// Journals a changeset that was not applied, in a transaction of its own.
    fn journal_refused(
        &self,
        journal: &Journal<'_>,
    ) -> impl Future<Output = Result<(), StoreError>> + Send;

    /// A changeset journaled as pending, whatever its state since.
    fn pending(
        &self,
        id: Uuid,
    ) -> impl Future<Output = Result<Option<PendingChangeset>, StoreError>> + Send;

    /// Proposals pending, oldest first.
    fn pending_proposals(
        &self,
    ) -> impl Future<Output = Result<Vec<PendingChangeset>, StoreError>> + Send;

    /// Records a new import and its batches, all queued.
    fn create_import(
        &self,
        record: &ImportRecord,
    ) -> impl Future<Output = Result<(), StoreError>> + Send;

    /// An import with its batches.
    fn import(
        &self,
        id: Uuid,
    ) -> impl Future<Output = Result<Option<ImportRecord>, StoreError>> + Send;

    /// Records the outcome of a batch.
    fn set_batch(
        &self,
        import: Uuid,
        index: i32,
        state: &str,
        changeset: Option<Uuid>,
    ) -> impl Future<Output = Result<(), StoreError>> + Send;

    /// Records the state and report of an import if its state is one of `from`; false
    /// otherwise (cancelled meanwhile, say).
    fn set_import(
        &self,
        import: Uuid,
        from: &[&str],
        state: &str,
        report: &serde_json::Value,
    ) -> impl Future<Output = Result<bool, StoreError>> + Send;

    /// Where an import stands, without reading its elements.
    fn import_progress(
        &self,
        import: Uuid,
    ) -> impl Future<Output = Result<Option<ImportProgress>, StoreError>> + Send;

    /// The objects known to `source` by some of `external_ids` (REQ-002-41), by
    /// identifier.
    fn external_objects(
        &self,
        source: &str,
        external_ids: &[String],
    ) -> impl Future<Output = Result<HashMap<String, Uuid>, StoreError>> + Send;

    /// The changeset of `author` applied with `motive`, if any: a batch of an import that
    /// a stop kept from being recorded (REQ-004-29).
    fn applied_changeset(
        &self,
        author: Uuid,
        motive: &str,
    ) -> impl Future<Output = Result<Option<Uuid>, StoreError>> + Send;

    /// What an applied changeset wrote, to undo it; `None` if it is not applied.
    fn undo_facts(
        &self,
        changeset: Uuid,
    ) -> impl Future<Output = Result<Option<UndoFacts>, StoreError>> + Send;
}

/// One write transaction. Every write runs under a savepoint, so that a refused write
/// leaves the transaction usable and every violation can be reported (REQ-004-7).
pub trait WriteTx {
    /// The objects among `objects` on which `subject` lacks `permission`, as the RBAC sees
    /// them in this transaction, the objects it created or changed included (spec 005,
    /// REQ-005-22).
    fn rbac_outside(
        &mut self,
        subject: &crate::authz::Subject,
        permission: &str,
        objects: &[Uuid],
    ) -> impl Future<Output = Result<Vec<Uuid>, StoreError>> + Send;

    /// Writes an administration operation of the RBAC over `period` (005, REQ-005-25).
    fn rbac_write(
        &mut self,
        op: &crate::rbac::RbacOp,
        period: Option<Period>,
        changeset: Uuid,
    ) -> impl Future<Output = Result<(), WriteError>> + Send;

    /// The entries of the role `key` in force now, if it exists.
    fn role_entries(
        &mut self,
        key: &str,
    ) -> impl Future<Output = Result<Option<Vec<String>>, StoreError>> + Send;

    /// The labels of the role `key` in force now, if it exists (spec 018).
    fn role_labels(
        &mut self,
        key: &str,
    ) -> impl Future<Output = Result<Option<crate::operations::RawLabels>, StoreError>> + Send;

    /// The roles not delivered with kartenn, in force now, that hold one of `permissions`
    /// or a property permission of a retired verb (`prop.<key>.write`,
    /// `prop.<key>.propose`), by key (spec 018, REQ-018-4; spec 024, REQ-024-9).
    fn custom_roles_citing(
        &mut self,
        permissions: &[&str],
    ) -> impl Future<Output = Result<Vec<crate::rbac::StoredRole>, StoreError>> + Send;

    /// The scope object of a binding in force (its entity, site, domain or prefix, or
    /// [`crate::rbac::RBAC_OBJECT`] for a global one), if it exists.
    fn binding_scope(
        &mut self,
        binding: Uuid,
    ) -> impl Future<Output = Result<Option<Uuid>, StoreError>> + Send;

    /// Whether one of `objects` lies in the scope `kind` (`entity`, `site`, `prefix`) `scope`
    /// or below it, as this transaction sees them (005, REQ-005-27).
    fn scope_touched(
        &mut self,
        kind: &str,
        scope: Uuid,
        objects: &[Uuid],
    ) -> impl Future<Output = Result<bool, StoreError>> + Send;

    /// The keys of the roles `subject` holds now, on any scope (005, REQ-005-27).
    fn roles_of(
        &mut self,
        subject: &crate::authz::Subject,
    ) -> impl Future<Output = Result<Vec<String>, StoreError>> + Send;

    /// Takes the advisory locks of the domains, in order, and of public space if needed
    /// (ADR 0015).
    fn lock(
        &mut self,
        domains: &[Uuid],
        public_space: bool,
    ) -> impl Future<Output = Result<(), StoreError>> + Send;

    /// Takes the named lock keys of module operations, after the domains, until the end
    /// of the transaction (REQ-008-27; amendments of ADR 0015). A key is held through its
    /// bucket, one of a fixed number (1024) of hash buckets: the same key always falls in
    /// the same bucket, the buckets are taken in ascending order, and a transaction holds
    /// at most one lock per bucket however many keys it names. Two keys sharing a bucket
    /// serialize needlessly, never too little.
    fn lock_keys(&mut self, keys: &[String])
    -> impl Future<Output = Result<(), StoreError>> + Send;

    /// The transaction as a module writes in it (ADR 0026, clause 6).
    fn module_tx(&mut self) -> &mut dyn crate::module::ModuleTx;

    /// Sets who stands behind the changeset of this transaction, as its modules read it
    /// (ADR 0026, amendment 3); only the pipeline sets it.
    fn set_module_actors(&mut self, actors: crate::module::ModuleActors);

    /// Holds the idempotency key for the transaction (advisory lock held until its end)
    /// without waiting: [`Replay::InProgress`] if another holds it; otherwise looks up a
    /// live earlier submission.
    fn replay(
        &mut self,
        request: &ChangesetRequest,
    ) -> impl Future<Output = Result<Replay, StoreError>> + Send;

    /// Version of an object: the last changeset that wrote it or its property
    /// assignments, whatever the period (006/design « Version d'un objet »); its row
    /// stays locked until the end of the transaction, so the comparison holds at commit.
    fn version(
        &mut self,
        id: Uuid,
    ) -> impl Future<Output = Result<Option<Uuid>, StoreError>> + Send;

    /// Records that `changeset` wrote the objects `ids` or their assignments.
    fn touch(
        &mut self,
        ids: &[Uuid],
        changeset: Uuid,
    ) -> impl Future<Output = Result<(), StoreError>> + Send;

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

    /// Applies a modification to an addressing object over `period` ("FOR PORTION OF",
    /// REQ-004-14): each version keeps its other fields; touching versions of the same
    /// state merge (004/design).
    fn modify(
        &mut self,
        kind: ObjectKind,
        id: Uuid,
        change: &Change,
        period: Option<Period>,
        changeset: Uuid,
    ) -> impl Future<Output = Result<(), WriteError>> + Send;

    /// Gives an object, or its assignment of `property`, back the versions asserted just
    /// before changeset `before`, over `period` (REQ-004-34).
    fn restore(
        &mut self,
        kind: ObjectKind,
        id: Uuid,
        property: Option<Uuid>,
        before: Uuid,
        period: Option<Period>,
        changeset: Uuid,
    ) -> impl Future<Output = Result<(), WriteError>> + Send;

    /// Objects that depend on `id` from the start of `period` on, and close with it in
    /// a cascade (REQ-004-26): a network's ranges and addresses, a domain's prefixes,
    /// ranges and addresses, an entity's or a site's children.
    fn dependents(
        &mut self,
        kind: ObjectKind,
        id: Uuid,
        period: Option<Period>,
    ) -> impl Future<Output = Result<Vec<(ObjectKind, Uuid)>, StoreError>> + Send;

    /// Reference assignments to `target` from the start of `period` on: object, its kind
    /// and the property.
    fn referencing(
        &mut self,
        target: Uuid,
        period: Option<Period>,
    ) -> impl Future<Output = Result<Vec<(Uuid, ObjectKind, Uuid)>, StoreError>> + Send;

    /// Records the identifier of an object in a source (REQ-002-41).
    fn bind_external(
        &mut self,
        object: Uuid,
        kind: ObjectKind,
        source: &str,
        external_id: &str,
    ) -> impl Future<Output = Result<(), WriteError>> + Send;

    /// Writes a new entity or site.
    fn insert_anchor(
        &mut self,
        kind: ObjectKind,
        id: Uuid,
        slug: &str,
        parent: Option<Uuid>,
        period: Option<Period>,
        changeset: Uuid,
    ) -> impl Future<Output = Result<(), WriteError>> + Send;

    /// Parents named by the versions of an entity or site over part of `period` that do
    /// not exist over the whole of those versions.
    fn missing_parents(
        &mut self,
        kind: ObjectKind,
        id: Uuid,
        period: Option<Period>,
    ) -> impl Future<Output = Result<Vec<Uuid>, StoreError>> + Send;

    /// Whether an entity or site is among its own ancestors, following the parents of
    /// every version over part of `period`.
    fn in_cycle(
        &mut self,
        kind: ObjectKind,
        id: Uuid,
        period: Option<Period>,
    ) -> impl Future<Output = Result<bool, StoreError>> + Send;

    /// Entities or sites whose parent is `id` at some instant from the start of `period`
    /// on.
    fn children(
        &mut self,
        kind: ObjectKind,
        id: Uuid,
        period: Option<Period>,
    ) -> impl Future<Output = Result<Vec<Uuid>, StoreError>> + Send;

    /// Prefixes among `prefixes` that are containers while carrying an assignment of the
    /// property `key`, at some instant over `period`.
    fn containers_carrying(
        &mut self,
        key: &str,
        prefixes: &[Uuid],
        period: Option<Period>,
    ) -> impl Future<Output = Result<Vec<Uuid>, StoreError>> + Send;

    /// Versions of a prefix over part of `period`, with their public class.
    fn prefix_versions(
        &mut self,
        id: Uuid,
        period: Option<Period>,
    ) -> impl Future<Output = Result<Vec<(PrefixObject, bool)>, StoreError>> + Send;

    /// Versions of an address over part of `period`, with their public class.
    fn address_versions(
        &mut self,
        id: Uuid,
        period: Option<Period>,
    ) -> impl Future<Output = Result<Vec<(Address, bool)>, StoreError>> + Send;

    /// Versions of a range over part of `period`.
    fn range_versions(
        &mut self,
        id: Uuid,
        period: Option<Period>,
    ) -> impl Future<Output = Result<Vec<IpRange>, StoreError>> + Send;

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

    /// Inserts a property declaration (REQ-003-1 to 3) with its labels (spec 018).
    fn insert_declaration(
        &mut self,
        id: Uuid,
        declaration: &Declaration,
        labels: &kartenn_core::labels::Labels,
        period: Option<Period>,
        changeset: Uuid,
    ) -> impl Future<Output = Result<(), WriteError>> + Send;

    /// The declaration of a property valid at the start of `period`.
    fn declaration(
        &mut self,
        id: Uuid,
        period: Option<Period>,
    ) -> impl Future<Output = Result<Option<Declaration>, StoreError>> + Send;

    /// The declaration of the property `key` valid at the start of `period`, with its
    /// identifier.
    fn declaration_by_key(
        &mut self,
        key: &str,
        period: Option<Period>,
    ) -> impl Future<Output = Result<Option<(Uuid, Declaration)>, StoreError>> + Send;

    /// Keys of the closed lists that borrow the values of `key` at the start of `period`
    /// (REQ-016-2), sorted.
    fn borrowers_of(
        &mut self,
        key: &str,
        period: Option<Period>,
    ) -> impl Future<Output = Result<Vec<String>, StoreError>> + Send;

    /// Replaces the version of a declaration valid at the start of `period`, from that
    /// start to the end of the version (REQ-003-6); its labels are `labels`, or kept.
    fn replace_declaration(
        &mut self,
        id: Uuid,
        declaration: &Declaration,
        labels: Option<&kartenn_core::labels::Labels>,
        period: Option<Period>,
        changeset: Uuid,
    ) -> impl Future<Output = Result<(), WriteError>> + Send;

    /// Current declarations of a namespace, with the keys of their closed-list values.
    fn declarations_in(
        &mut self,
        namespace: &str,
    ) -> impl Future<Output = Result<Vec<(Uuid, Declaration, Vec<String>)>, StoreError>> + Send;

    /// The labels in force of the declarations of a namespace and of their closed-list
    /// values (spec 018, R10): `<property>` or `<property>/<value>`, the labels, and
    /// whether the system wrote the version (a synchronisation), not an administrator.
    fn labels_in(
        &mut self,
        namespace: &str,
    ) -> impl Future<Output = Result<Vec<(String, crate::operations::RawLabels, bool)>, StoreError>> + Send;

    /// Inserts a closed-list value (REQ-003-5, 7) with its labels (spec 018).
    fn insert_enum_value(
        &mut self,
        id: Uuid,
        property: Uuid,
        key: &str,
        labels: &kartenn_core::labels::Labels,
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

    /// [`WriteTx::object_view`] of each of `objects`, read together, in their order; those
    /// that do not exist then are left out (the objects of a changeset, REQ-010-54).
    fn object_views(
        &mut self,
        objects: &[Uuid],
        period: Option<Period>,
    ) -> impl Future<Output = Result<Vec<ObjectView>, StoreError>> + Send;

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

    /// Properties heritable along a tree at the start of `period`, with their tree.
    fn heritable(
        &mut self,
        period: Option<Period>,
    ) -> impl Future<Output = Result<Vec<(Uuid, InheritTree)>, StoreError>> + Send;

    /// The objects and the objects below them in their trees (addressing, entities, sites
    /// or a module), from the start of `period` on, each once: one read for every object
    /// of a changeset, not one per object.
    fn subtree(
        &mut self,
        objects: &[Uuid],
        period: Option<Period>,
    ) -> impl Future<Output = Result<Vec<Uuid>, StoreError>> + Send;

    /// Every object of the given kinds, from the start of `period` on.
    fn objects_of_kinds(
        &mut self,
        kinds: &[String],
        period: Option<Period>,
    ) -> impl Future<Output = Result<Vec<Uuid>, StoreError>> + Send;

    /// `(property, object)` pairs whose origin is one of `origins` from the start of
    /// `period` on, each once.
    fn inheritors(
        &mut self,
        origins: &[Uuid],
        period: Option<Period>,
    ) -> impl Future<Output = Result<Vec<(Uuid, Uuid)>, StoreError>> + Send;

    /// Whether the changeset changed when `object` carries an assignment of `property`
    /// (a value or an explicit clearing) from the start of `period` on; when it did not,
    /// only values changed and no origin moves.
    fn assignment_presence_changed(
        &mut self,
        object: Uuid,
        property: Uuid,
        period: Option<Period>,
    ) -> impl Future<Output = Result<bool, StoreError>> + Send;

    /// Objects whose origin of `property` is `origin` from the start of `period` on.
    fn heirs(
        &mut self,
        property: Uuid,
        origin: Uuid,
        period: Option<Period>,
    ) -> impl Future<Output = Result<Vec<Uuid>, StoreError>> + Send;

    /// Recomputes the origin of a property heritable along `tree` for `targets` (those
    /// of the tree) from the start of `period` on, and writes the segments that changed
    /// (003/design, § Résolution matérialisée). Returns the number of objects whose
    /// origin changed.
    fn refresh_origins(
        &mut self,
        property: Uuid,
        tree: &InheritTree,
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

    /// Records the differential of a heritable `property` on the heirs of `origins`, whose
    /// value changed while no origin moved, from the start of `period` on; `excluded` are
    /// the objects the changeset creates or whose origin it recomputed.
    fn record_value_diff(
        &mut self,
        property: Uuid,
        origins: &[Uuid],
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

    /// Records a section of the differential contributed by a module (REQ-008-26), stored
    /// with the journal of an applied changeset.
    fn record_section(
        &mut self,
        section: &crate::module::DiffSection,
    ) -> impl Future<Output = Result<(), StoreError>> + Send;

    /// First `limit` entries of the differential recorded so far.
    fn diff_sample(
        &mut self,
        limit: i64,
    ) -> impl Future<Output = Result<Vec<DiffItem>, StoreError>> + Send;

    /// Locks a journaled changeset until the end of the transaction and gives its state
    /// (validations of the same changeset are serialised); `None` if unknown.
    fn lock_changeset(
        &mut self,
        id: Uuid,
    ) -> impl Future<Output = Result<Option<String>, StoreError>> + Send;

    /// Enabled approval policies.
    fn approval_policies(
        &mut self,
    ) -> impl Future<Output = Result<Vec<ApprovalPolicy>, StoreError>> + Send;

    /// Identifier of the approval policy named `name`, if any.
    fn policy_named(
        &mut self,
        name: &str,
    ) -> impl Future<Output = Result<Option<Uuid>, StoreError>> + Send;

    /// Creates or replaces an approval policy.
    fn set_policy(
        &mut self,
        policy: &ApprovalPolicy,
    ) -> impl Future<Output = Result<(), WriteError>> + Send;

    /// Records an audit event with the action of this transaction: if it cannot be
    /// written, the transaction fails (REQ-012-11).
    fn audit(
        &mut self,
        event: &kartenn_core::audit::AuditEvent,
    ) -> impl Future<Output = Result<(), StoreError>> + Send;

    /// Journals the applied changeset in this transaction.
    fn journal_applied(
        &mut self,
        journal: &Journal<'_>,
    ) -> impl Future<Output = Result<(), StoreError>> + Send;

    /// Writes to the outbox the events emitted by the background task `job`
    /// (`<module>.<task>`) in this transaction, ordered under the key `<module>:<task>`
    /// (REQ-016-15).
    fn journal_job(&mut self, job: &str) -> impl Future<Output = Result<(), StoreError>> + Send;

    /// Ends a pending changeset without applying it (`rejected`, `withdrawn`), with the
    /// motive of a rejection and its events (outbox, audit); `false` if it is no longer
    /// pending. The modules learn of it in this transaction (REQ-016-9).
    fn close_pending(
        &mut self,
        id: Uuid,
        state: &'static str,
        by: Uuid,
        rejection: Option<&str>,
        audit: &kartenn_core::audit::AuditEvent,
    ) -> impl Future<Output = Result<bool, StoreError>> + Send;

    /// Expires the changesets pending past their deadline, with their events (outbox and
    /// audit, the latter attributed to the system).
    fn expire_pending(
        &mut self,
    ) -> impl Future<Output = Result<Vec<PendingChangeset>, StoreError>> + Send;

    /// Writes to the outbox the events the modules emitted when the pending changeset
    /// `changeset` ended (REQ-016-9, 15), ordered under `changeset:<id>`.
    fn journal_ended(
        &mut self,
        changeset: Uuid,
    ) -> impl Future<Output = Result<(), StoreError>> + Send;

    /// Commits.
    fn commit(self) -> impl Future<Output = Result<(), StoreError>> + Send;

    /// Rolls back.
    fn rollback(self) -> impl Future<Output = Result<(), StoreError>> + Send;
}
