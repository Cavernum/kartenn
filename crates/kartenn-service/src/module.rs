//! The module contract (ADR 0026; spec 008, REQ-008-1): what a compiled business module
//! declares to the core, and the registry that checks the modules of a build at startup
//! and orders them along their dependencies.

use std::any::Any;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::pin::Pin;
use std::sync::{Arc, LazyLock};

use kartenn_core::authz::Permission;
use kartenn_core::period::{Instant, Period};
use kartenn_core::rules::{Rule, Violation};
use uuid::Uuid;

use crate::declarations::ModuleDeclarations;
use crate::operations::{ModuleKind, ModuleOperation, ObjectKind, Operation, Ref};
use crate::port::{StoreError, WriteError};

/// The name reserved for the core (ADR 0026, clause 1).
pub const CORE: &str = "core";

/// The collections the core serves under `/v1`: no module may declare one of them
/// (REQ-008-1). A test of `kartenn-rest` checks this list against its contract.
pub const CORE_COLLECTIONS: &[&str] = &[
    "addresses",
    "anomalies",
    "audit-events",
    "auth",
    "bindings",
    "capabilities",
    "changesets",
    "domains",
    // The drafts kept by the server (spec 024).
    "drafts",
    "entities",
    // The event stream (spec 022).
    "events",
    "graph",
    // The groups of kartenn and their members (spec 025).
    "groups",
    "identities",
    "imports",
    "issuer-attachments",
    "labels",
    "log-filters",
    "me",
    "openapi.json",
    "prefixes",
    "property-declarations",
    "property-gaps",
    "proposals",
    "ranges",
    "roles",
    // The catalogue of the rules (spec 018, REQ-018-23).
    "rules",
    "service-accounts",
    "sites",
    // The counters of the home page (spec 024, REQ-024-14).
    "stats",
];

/// A type of object of a module (ADR 0026, clause 4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectType {
    /// Qualified name, `<module>.<type>` (`vlan.vlan`).
    pub kind: &'static str,
    /// Its REST collection under `/v1` (`vlans`).
    pub collection: &'static str,
    /// The type of its parent in the tree of the module, if any.
    pub parent: Option<&'static str>,
    /// Whether `core.owner` must be set on it (otherwise inherited or optional).
    pub owner_required: bool,
    /// Whether it carries `core.site`.
    pub carries_site: bool,
    /// Whether it is anchored in an addressing domain (its writes lock that domain).
    pub domain_anchored: bool,
}

/// What closing the target of a reference does to its sources (D33, ADR 0026 clause 9).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OnClose {
    /// Refused while a source lives.
    Refuse,
    /// Closes the sources with the target, when the changeset asks for the cascade.
    Cascade,
    /// Clears the reference in the sources.
    Nullify,
}

/// A reference from objects of a module to objects of the core or of a module.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reference {
    /// The type of the source, qualified.
    pub from: &'static str,
    /// The type of the target: a core kind (`prefix`) or a qualified module type.
    pub to: &'static str,
    /// What closing the target does.
    pub on_close: OnClose,
}

/// A type of edge the module contributes to the topology graph (ADR 0026, clause 11).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EdgeType {
    /// Qualified name (`vlan.carries`).
    pub name: &'static str,
    /// The type of its origin.
    pub from: &'static str,
    /// The type of its end.
    pub to: &'static str,
}

/// An edge of the topology graph at the dates of a read (ADR 0026, clause 11): its type,
/// one of the module's [`EdgeType`]s, its ends and the period of validity it has.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Edge {
    /// The type of edge (`vlan.carries`).
    pub name: &'static str,
    /// Its origin, of the type the [`EdgeType`] names.
    pub from: Uuid,
    /// Its end.
    pub to: Uuid,
    /// When it holds.
    pub period: Period,
}

/// A SQL migration of a module, applied once in the schema of the module (REQ-008-2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleMigration {
    /// Its version, strictly increasing within the module.
    pub version: i64,
    /// A short name.
    pub name: &'static str,
    /// The SQL, usually `include_str!`.
    pub sql: &'static str,
}

/// A background task of a module (REQ-016-16): run under the lease of the core's tasks
/// (ADR 0023) by one instance at a time, every `period`, as `<module>.<name>`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JobSpec {
    /// Its name within the module, `[a-z][a-z0-9_]*`.
    pub name: &'static str,
    /// The time between two runs.
    pub period: std::time::Duration,
}

/// An approval policy a module installs on its first deployment (REQ-016-8): never
/// installed again, even disabled or closed by an administrator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DefaultPolicy {
    /// The name of the policy, unique among the policies.
    pub name: &'static str,
    /// Its triggers: a measure of the module (its local name) and a threshold.
    pub triggers: &'static [(&'static str, u64)],
}

/// A future of a module, boxed so that its traits stay object-safe.
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// An operation a module accepts in a changeset (REQ-008-4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OperationType {
    /// Its name within the module (`create_vlan`, tagged `vlan.create_vlan`).
    pub name: &'static str,
    /// The qualified type of the object it creates under its `local`, if it creates one.
    pub defines: Option<&'static str>,
}

/// An operation of a module, planned: decoded, checked locally, its references resolved.
#[derive(Debug, Clone, PartialEq)]
pub struct ModulePlan {
    /// The type of the object it writes.
    pub kind: ModuleKind,
    /// The object it writes: the identifier given by the pipeline when it creates one.
    pub object: Uuid,
    /// Whether it creates the object (its right is checked once written) or changes an
    /// existing one (checked before).
    pub creates: bool,
    /// The domains to lock, when the object is anchored in a domain (ADR 0015).
    pub domains: Vec<Uuid>,
    /// Named lock keys, taken after the domains in canonical order (REQ-008-27).
    pub keys: Vec<String>,
    /// Operations of the core it adds to the changeset, planned after it like ordinary
    /// operations (ADR 0026, clause 6).
    pub core: Vec<Operation>,
    /// What [`ModuleHandler::apply`] needs, in the module's own form.
    pub data: serde_json::Value,
}

/// A version of an object of a module, registered in `module_object` (REQ-008-3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModuleObjectRow {
    /// The object.
    pub id: Uuid,
    /// Its qualified type.
    pub kind: ModuleKind,
    /// Its parent in the tree of the module.
    pub parent: Option<Uuid>,
    /// Its domain, when anchored in one.
    pub domain: Option<Uuid>,
}

/// A row of a section of the differential: an object, its value before and after the
/// changeset, over a period (ADR 0026, clause 13).
#[derive(Debug, Clone, PartialEq)]
pub struct DiffRow {
    /// The object.
    pub object: Uuid,
    /// Its qualified type (`prefix`, `vlan.vlan`…).
    pub kind: String,
    /// What the module shows of it before the changeset; `None`: absent.
    pub before: Option<serde_json::Value>,
    /// What the module shows of it after; `None`: absent.
    pub after: Option<serde_json::Value>,
    /// The period of the change; `None`: the changeset's.
    pub period: Option<Period>,
}

/// A section of the differential, contributed by a module (REQ-008-26).
#[derive(Debug, Clone, PartialEq)]
pub struct DiffSection {
    /// Its name, `<module>.<name>`.
    pub name: String,
    /// Its rows, in the order the module gives.
    pub rows: Vec<DiffRow>,
}

/// What a module's contribution to the differential reads (ADR 0026, clause 13).
#[derive(Debug, Clone, Copy)]
pub struct DiffContext<'a> {
    /// The changeset.
    pub changeset: Uuid,
    /// The counted differential of the resolved property values; its detail is readable
    /// in the transaction.
    pub properties: &'a [crate::port::DiffCount],
    /// The objects the changeset writes, of every type (core and modules).
    pub touched: &'a [(ObjectKind, Uuid)],
    /// The sections of the modules before this one, in the order of the registry.
    pub sections: &'a [DiffSection],
}

impl DiffContext<'_> {
    /// The section named `name`, if a module before produced it.
    #[must_use]
    pub fn section(&self, name: &str) -> Option<&DiffSection> {
        self.sections.iter().find(|s| s.name == name)
    }
}

/// What the rules of a module read on the final state (ADR 0026, clause 7).
#[derive(Debug, Clone, Copy)]
pub struct CheckContext<'a> {
    /// The changeset, for the copies a module rewrites on the final state (the usage of an
    /// address copied by its attachments, 009).
    pub changeset: Uuid,
    /// The objects of the module the changeset wrote.
    pub touched: &'a [(ModuleKind, Uuid)],
    /// The objects the changeset writes, of every type (core and modules): the rules of a
    /// module on the objects of the core (`vlan.prefix.not_network`…).
    pub objects: &'a [(ObjectKind, Uuid)],
    /// Every section of the differential.
    pub sections: &'a [DiffSection],
}

/// How a pending changeset ended without being applied (REQ-016-9).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ending {
    /// Rejected by a validator, with a motive.
    Rejected,
    /// Withdrawn by its author.
    Withdrawn,
    /// Pending past its deadline.
    Expired,
}

/// What a module learns of a pending changeset that ends without being applied
/// (REQ-016-9, G11).
#[derive(Debug, Clone)]
pub struct EndContext<'a> {
    /// The changeset.
    pub changeset: Uuid,
    /// How it ended.
    pub how: Ending,
    /// The motive of a rejection.
    pub motive: Option<&'a str>,
    /// The identity that decided; `None` for an expiry.
    pub by: Option<Uuid>,
    /// The operations of the module in the changeset, in their order.
    pub operations: Vec<&'a ModuleOperation>,
}

impl CheckContext<'_> {
    /// The section named `name`, if a module produced it.
    #[must_use]
    pub fn section(&self, name: &str) -> Option<&DiffSection> {
        self.sections.iter().find(|s| s.name == name)
    }
}

/// What the rules of a module found on the final state (ADR 0026, clause 7).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Findings {
    /// The violations; the warnings become anomalies once the changeset is applied.
    pub violations: Vec<Violation>,
    /// The rules evaluated anew, as `(object, rule or prefix of rules)`: an open anomaly of
    /// such a rule on such an object that no warning raises again is closed, so that an
    /// anomaly of a module disappears once corrected (REQ-009-13).
    pub reevaluated: Vec<(Uuid, &'static str)>,
}

impl From<Vec<Violation>> for Findings {
    fn from(violations: Vec<Violation>) -> Self {
        Self {
            violations,
            reevaluated: Vec::new(),
        }
    }
}

/// Who stands behind the changeset a module writes in (ADR 0026, amendment 3): set by the
/// pipeline, never by a module. `None` for kartenn itself, or outside a changeset (a
/// background task).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ModuleActors {
    /// The author of the changeset; the identity whose rights its writes use.
    pub author: Option<Uuid>,
    /// The requester a service account submitted it for (`Kartenn-On-Behalf-Of`,
    /// REQ-016-12); never a source of rights.
    pub on_behalf_of: Option<Uuid>,
    /// The validator of a pending changeset, when it is being validated (REQ-016-11).
    pub validator: Option<Uuid>,
}

/// A versioned table of a module whose rows own labels (spec 018, R19, R24).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LabelTable {
    /// The space of the dictionary, `<module>.<collection>`.
    pub space: &'static str,
    /// The table, `<module>.<table>`, with the columns `key` and `labels`.
    pub table: &'static str,
    /// The permission that reads it, held on any scope.
    pub permission: &'static str,
}

/// The transaction of a changeset, as a module writes in it (ADR 0026, clause 6).
pub trait ModuleTx: Send {
    /// Who stands behind the changeset (ADR 0026, amendment 3): author, requester and
    /// validator, as the pipeline set them.
    fn actors(&self) -> ModuleActors;

    /// The connection of the adapter: a module's own adapter downcasts it
    /// (`sqlx::PgConnection` for `kartenn-store-pg`).
    fn connection(&mut self) -> &mut (dyn Any + Send);

    /// Registers a version of an object of the module over `period` (the changeset's when
    /// `None`), written by `changeset`.
    ///
    /// A first version is inserted; an object already registered over `period` is
    /// rewritten there (a new version, kept in the history), so that each changeset that
    /// writes an object of a module leaves its trace for undo (REQ-008-3).
    fn register(
        &mut self,
        row: ModuleObjectRow,
        period: Option<Period>,
        changeset: Uuid,
    ) -> BoxFuture<'_, Result<(), WriteError>>;

    /// Ends over `period` the versions of `id` in `table`, a versioned table of the module
    /// (`<module>.<table>`, made with `kartenn_make_versioned`), as the core ends its own
    /// objects (REQ-008-5).
    fn close_versioned<'a>(
        &'a mut self,
        table: &'a str,
        id: Uuid,
        period: Option<Period>,
        changeset: Uuid,
    ) -> BoxFuture<'a, Result<(), WriteError>>;

    /// Gives back over `period` the versions of `id` in `table` (versioned, as for
    /// [`ModuleTx::close_versioned`]) asserted just before the changeset `before`
    /// (undo, REQ-008-3).
    fn restore_versioned<'a>(
        &'a mut self,
        table: &'a str,
        id: Uuid,
        before: Uuid,
        period: Option<Period>,
        changeset: Uuid,
    ) -> BoxFuture<'a, Result<(), WriteError>>;

    /// Emits the event `event` (`<module>.<name>`, declared by [`Module::events`]) of type
    /// `org.kartenn.<module>.<name>`, with an optional CloudEvents `subject` (REQ-016-15).
    /// It reaches the outbox only with the transaction: when the changeset is applied, or
    /// when the background task ends well; a simulation or a refusal emits nothing.
    ///
    /// # Errors
    ///
    /// [`StoreError`] for an event no module of the build declares.
    fn emit(
        &mut self,
        event: &str,
        subject: Option<String>,
        data: serde_json::Value,
    ) -> Result<(), StoreError>;

    /// Notes that the module wrote a column `labels` in this transaction (spec 018, R22):
    /// the revision of the labels is incremented when the changeset is applied, and the
    /// outbox carries `org.kartenn.labels.changed`.
    fn labels_written(&mut self);
}

/// A read of the state at the dates of a request, as a module reads (ADR 0026, clauses
/// 11 and 15): outside any changeset, on the connection of the adapter.
pub trait ModuleRead: Send {
    /// The connection of the adapter: a module's own adapter downcasts it
    /// (`sqlx::PgConnection` for `kartenn-store-pg`).
    fn connection(&mut self) -> &mut (dyn Any + Send);
    /// The valid time of the read.
    fn valid_at(&self) -> Instant;
    /// The system time of the read; `None` for now.
    fn as_of(&self) -> Option<Instant>;
}

/// The behaviour of a module inside the write pipeline (ADR 0026, clause 6).
pub trait ModuleHandler: Send + Sync + fmt::Debug {
    /// Completes `operation` on the state of the transaction, before the operations are
    /// planned: the values the module defaults from existing objects (the resolved owner
    /// of a domain, the identifier of a property of the core), which [`Self::plan`], on no
    /// database, could not read. `period` is that of the operation, the changeset's when
    /// `None`. The completed operation is the one the changeset records. Nothing by
    /// default.
    fn prepare<'a>(
        &'a self,
        tx: &'a mut dyn ModuleTx,
        operation: &'a mut ModuleOperation,
        period: Option<Period>,
    ) -> BoxFuture<'a, Result<(), StoreError>> {
        let _ = (tx, operation, period);
        Box::pin(std::future::ready(Ok(())))
    }

    /// Decodes and checks `operation` locally; `object` is the identifier of the object it
    /// creates, if any; `resolve` gives the object of a reference (`None` for a local name
    /// no earlier operation defines).
    ///
    /// # Errors
    ///
    /// A violation refuses the changeset before any lock.
    fn plan(
        &self,
        operation: &ModuleOperation,
        object: Uuid,
        resolve: &dyn Fn(Ref) -> Option<Uuid>,
    ) -> Result<ModulePlan, Violation>;

    /// The rights `plan` needs besides the right to write its object: `(permission,
    /// object)` pairs for the objects it cites without writing them, such as an address
    /// of the core attached to an interface (009/M16). The core checks them like a guard,
    /// never as a proposal (`core.object.forbidden`). None by default.
    fn reads(&self, plan: &ModulePlan) -> Vec<(String, Uuid)> {
        let _ = plan;
        Vec::new()
    }

    /// Writes a planned operation over `period` (the changeset's when `None`).
    fn apply<'a>(
        &'a self,
        tx: &'a mut dyn ModuleTx,
        plan: &'a ModulePlan,
        period: Option<Period>,
        changeset: Uuid,
    ) -> BoxFuture<'a, Result<(), WriteError>>;

    /// Ends the module's own rows of `id` over `period`, once the core has ended its record
    /// (`Close`, REQ-008-5). Nothing by default: a module without rows of its own.
    fn close<'a>(
        &'a self,
        tx: &'a mut dyn ModuleTx,
        kind: ModuleKind,
        id: Uuid,
        period: Option<Period>,
        changeset: Uuid,
    ) -> BoxFuture<'a, Result<(), WriteError>> {
        let _ = (tx, kind, id, period, changeset);
        Box::pin(std::future::ready(Ok(())))
    }

    /// Gives back the module's own rows of `id` as before the changeset `before`, once the
    /// core has restored its record (`Restore`, undo, REQ-008-3). Nothing by default.
    #[allow(clippy::too_many_arguments)] // the restore of the core, for the module's rows
    fn restore<'a>(
        &'a self,
        tx: &'a mut dyn ModuleTx,
        kind: ModuleKind,
        id: Uuid,
        before: Uuid,
        period: Option<Period>,
        changeset: Uuid,
    ) -> BoxFuture<'a, Result<(), WriteError>> {
        let _ = (tx, kind, id, before, period, changeset);
        Box::pin(std::future::ready(Ok(())))
    }

    /// The living objects of the module that reference `target` by `reference` (one of the
    /// module's [`Module::references`]) from the start of `period` on, on the state of the
    /// transaction (REQ-008-10). None by default.
    fn referrers<'a>(
        &'a self,
        tx: &'a mut dyn ModuleTx,
        reference: &'a Reference,
        target: Uuid,
        period: Option<Period>,
    ) -> BoxFuture<'a, Result<Vec<Uuid>, StoreError>> {
        let _ = (tx, reference, target, period);
        Box::pin(std::future::ready(Ok(Vec::new())))
    }

    /// The write that clears `reference` in `source` (pointing at `target`), for the
    /// cascade of a reference whose policy is [`OnClose::Nullify`]: a modification of
    /// `source`, checked under its rights. `None` keeps the reference, so the closure is
    /// refused.
    fn nullify(&self, reference: &Reference, source: Uuid, target: Uuid) -> Option<ModulePlan> {
        let _ = (reference, source, target);
        None
    }

    /// The module's section of the differential, after the differential of the property
    /// values and the sections of the modules before it (REQ-008-26). None by default.
    fn diff<'a>(
        &'a self,
        tx: &'a mut dyn ModuleTx,
        context: &'a DiffContext<'a>,
    ) -> BoxFuture<'a, Result<Option<DiffSection>, StoreError>> {
        let _ = (tx, context);
        Box::pin(std::future::ready(Ok(None)))
    }

    /// The rules of the module on the final state, once every operation is written and
    /// every section of the differential produced: the context lists the objects of the
    /// module the changeset wrote, and the sections. The findings name the objects whose
    /// warnings were evaluated anew, so that their anomalies close once corrected.
    fn check<'a>(
        &'a self,
        tx: &'a mut dyn ModuleTx,
        context: &'a CheckContext<'a>,
    ) -> BoxFuture<'a, Result<Findings, StoreError>> {
        let _ = (tx, context);
        Box::pin(std::future::ready(Ok(Findings::default())))
    }

    /// The edges of the module's [`Module::edges`] that start or end at `object`, valid at
    /// the dates of `read` (REQ-008-11). None by default. The core filters them by the
    /// rights of the caller on both ends.
    fn edges<'a>(
        &'a self,
        read: &'a mut dyn ModuleRead,
        object: Uuid,
    ) -> BoxFuture<'a, Result<Vec<Edge>, StoreError>> {
        let _ = (read, object);
        Box::pin(std::future::ready(Ok(Vec::new())))
    }

    /// The values of the module's [`Module::measures`] for the changeset of `context`
    /// (local names), read after the sections of its differential; an approval policy
    /// holds the changeset on them (REQ-016-7). Nothing by default: a measure not given
    /// counts as zero.
    fn measure<'a>(
        &'a self,
        tx: &'a mut dyn ModuleTx,
        context: &'a CheckContext<'a>,
    ) -> BoxFuture<'a, Result<Vec<(&'static str, u64)>, StoreError>> {
        let _ = (tx, context);
        Box::pin(std::future::ready(Ok(Vec::new())))
    }

    /// Runs the background task `name` of the module's [`Module::jobs`], in one
    /// transaction of its own, committed when it ends well (REQ-016-16). Nothing by
    /// default.
    fn run_job<'a>(
        &'a self,
        name: &'a str,
        tx: &'a mut dyn ModuleTx,
    ) -> BoxFuture<'a, Result<(), StoreError>> {
        let _ = (name, tx);
        Box::pin(std::future::ready(Ok(())))
    }

    /// Runs the background task `name` and returns what its rules found (ADR 0026,
    /// amendment 4): the warnings become anomalies and the `(object, rule)` evaluated anew
    /// close the anomalies no warning raises again, journaled by kartenn in a changeset of
    /// its own (author kartenn), as the re-evaluation of a source of expected values
    /// (REQ-003-24). By default, [`Self::run_job`] and nothing found.
    fn run_job_findings<'a>(
        &'a self,
        name: &'a str,
        tx: &'a mut dyn ModuleTx,
    ) -> BoxFuture<'a, Result<Findings, StoreError>> {
        Box::pin(async move { self.run_job(name, tx).await.map(|()| Findings::default()) })
    }

    /// The permission whose holder validates `plan`, when the module requires a review of
    /// it: the changeset becomes a proposal even if its author may write it, and only an
    /// identity that holds this permission on the object validates it (REQ-016-10, 11,
    /// G12). `None` by default.
    fn review(&self, plan: &ModulePlan) -> Option<&'static str> {
        let _ = plan;
        None
    }

    /// A pending changeset holding operations of the module ends without being applied
    /// (rejected, withdrawn, expired), in the transaction that ends it: an error rolls the
    /// ending back (REQ-016-9). Nothing by default.
    fn ended<'a>(
        &'a self,
        tx: &'a mut dyn ModuleTx,
        context: &'a EndContext<'a>,
    ) -> BoxFuture<'a, Result<(), StoreError>> {
        let _ = (tx, context);
        Box::pin(std::future::ready(Ok(())))
    }
}

/// A business module compiled in the build (ADR 0026).
pub trait Module: Send + Sync + fmt::Debug + 'static {
    /// Its name, `[a-z][a-z0-9_]*`, the namespace of everything it declares.
    fn name(&self) -> &'static str;
    /// The version of its crate.
    fn version(&self) -> &'static str;
    /// The modules it needs.
    fn depends_on(&self) -> &'static [&'static str] {
        &[]
    }
    /// Its types of objects.
    fn object_types(&self) -> &'static [ObjectType] {
        &[]
    }
    /// Its property declarations (003/P12).
    fn declarations(&self) -> Option<ModuleDeclarations> {
        None
    }
    /// Its permissions.
    fn permissions(&self) -> &'static [&'static Permission] {
        &[]
    }
    /// The permissions its object types are read and edited under, when not
    /// `<type>.read` and `<type>.edit` (several types may share one).
    fn type_permissions(&self) -> &'static [kartenn_core::authz::TypePermissions] {
        &[]
    }
    /// Its validation rules.
    fn rules(&self) -> &'static [&'static Rule] {
        &[]
    }
    /// The French and English labels of its codes (spec 018, R8 b; ADR 0026, amendment
    /// of 018): its permissions, rules, kinds and states, served by the dictionary
    /// `GET /v1/labels`. A code without labels shows as its key.
    fn labels(&self) -> Vec<kartenn_core::labels::CodeLabel> {
        Vec::new()
    }
    /// Its versioned tables whose rows own labels (a column `key` and a column `labels`,
    /// spec 018, Q-018-6): each a space of the dictionary, read under a permission held
    /// on any scope (R24).
    fn label_tables(&self) -> &'static [LabelTable] {
        &[]
    }
    /// Its references.
    fn references(&self) -> &'static [Reference] {
        &[]
    }
    /// Its edges of the topology graph.
    fn edges(&self) -> &'static [EdgeType] {
        &[]
    }
    /// Its migrations, in version order.
    fn migrations(&self) -> &'static [ModuleMigration] {
        &[]
    }
    /// The operations it accepts in a changeset.
    fn operations(&self) -> &'static [OperationType] {
        &[]
    }
    /// The names of its operations a validator may join to the validation of a proposal
    /// (REQ-016-11, operations « de revue »); none by default.
    fn review_operations(&self) -> &'static [&'static str] {
        &[]
    }
    /// The permission each listed operation requires on the object it writes, in place of
    /// `<type>.edit` (ADR 0026, amendment 3): `(operation, permission)`, the permission
    /// one of [`Self::permissions`]. It is checked on the same objects, under the same
    /// scopes, as `<type>.edit` would be, and is its own application (spec 024,
    /// [`kartenn_core::authz::verb_permission`]); an operation not listed requires
    /// `<type>.edit`. None by default.
    fn operation_permissions(&self) -> &'static [(&'static str, &'static str)] {
        &[]
    }
    /// Its handler in the write pipeline.
    fn handler(&self) -> Option<Arc<dyn ModuleHandler>> {
        None
    }
    /// Its background tasks, run by [`ModuleHandler::run_job`] (REQ-016-16).
    fn jobs(&self) -> &'static [JobSpec] {
        &[]
    }
    /// The names of the events it emits by [`ModuleTx::emit`], `[a-z][a-z0-9_]*`, typed
    /// `org.kartenn.<module>.<name>` (REQ-016-15).
    fn events(&self) -> &'static [&'static str] {
        &[]
    }
    /// The names of its measures, `[a-z][a-z0-9_]*`, that an approval policy may name as
    /// `<module>.<measure>` in its triggers (REQ-016-7).
    fn measures(&self) -> &'static [&'static str] {
        &[]
    }
    /// The approval policies it installs on its first deployment (REQ-016-8).
    fn default_policies(&self) -> &'static [DefaultPolicy] {
        &[]
    }
    /// What it counts for the home page, `GET /v1/stats` (spec 024, REQ-024-14). None by
    /// default: the core still counts the objects of its types.
    fn stats(&self) -> Option<Arc<dyn StatsContributor>> {
        None
    }
}

/// The rights a module counts under (spec 024, REQ-024-15).
pub struct StatsScope<'a> {
    /// The caller, for what it reads as itself (the flows it requested).
    pub identity: Option<Uuid>,
    /// The argument of `kartenn_scopes` (migration 0033), the predicate of the lists;
    /// `None` reads everything.
    pub scope: Option<&'a serde_json::Value>,
    /// Whether the caller reads some object of a qualified type: a counter of a type it
    /// reads nowhere is left out.
    pub reads: &'a (dyn Fn(&str) -> bool + Sync),
}

impl fmt::Debug for StatsScope<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("StatsScope")
            .field("identity", &self.identity)
            .field("scope", &self.scope)
            .finish_non_exhaustive()
    }
}

/// What a module counts (spec 024, REQ-024-14): each figure filtered by the read rights of
/// the caller, with the predicate of its lists.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Contribution {
    /// Objects of its types by qualified type, in place of the count the core makes from
    /// their record (`module_object`): for a type its lists read beyond the predicate of
    /// the core (the flows of their requester).
    pub objects: Vec<(&'static str, u64)>,
    /// Its own counters by name, `[a-z][a-z0-9_]*`, served under the name of the module.
    pub counters: Vec<(&'static str, u64)>,
}

/// The port through which a module adds its counters to `GET /v1/stats` (spec 024,
/// REQ-024-14), as it adds its sections to a differential.
pub trait StatsContributor: Send + Sync + fmt::Debug {
    /// Its counters at the dates of `read`, under the rights of `scope`.
    fn contribute<'a>(
        &'a self,
        read: &'a mut dyn ModuleRead,
        scope: &'a StatsScope<'a>,
    ) -> BoxFuture<'a, Result<Contribution, StoreError>>;
}

/// Why the modules of a build cannot run together: the server stops (REQ-008-1).
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum RegistryError {
    /// A name outside `[a-z][a-z0-9_]*`.
    #[error("module `{0}`: invalid name, expected [a-z][a-z0-9_]*")]
    InvalidName(String),
    /// The name of the core.
    #[error("module `core`: the name is reserved for the core")]
    ReservedName,
    /// Two modules of the same name.
    #[error("module `{0}` is registered twice")]
    Duplicate(String),
    /// A dependency that is not in the build.
    #[error("module `{module}` depends on `{dependency}`, which is not in the build")]
    MissingDependency {
        /// The module.
        module: String,
        /// Its missing dependency.
        dependency: String,
    },
    /// Dependencies that loop.
    #[error("modules {0:?} depend on each other in a cycle")]
    Cycle(Vec<String>),
    /// A type of object outside the namespace of its module.
    #[error("module `{module}`: type `{kind}` must be named `{module}.<type>`")]
    ForeignKind {
        /// The module.
        module: String,
        /// The type.
        kind: String,
    },
    /// A permission or a rule outside the namespace of its module, or taken already.
    #[error("module `{module}`: `{name}` must be named `{module}.<…>` and be new")]
    ForeignName {
        /// The module.
        module: String,
        /// The permission or the rule.
        name: String,
    },
    /// A background task with an invalid or repeated name.
    #[error("module `{module}`: background task `{name}` must be named [a-z][a-z0-9_]* once")]
    InvalidJob {
        /// The module.
        module: String,
        /// The task.
        name: String,
    },
    /// An event with an invalid or repeated name.
    #[error("module `{module}`: event `{name}` must be named [a-z][a-z0-9_]* once")]
    InvalidEvent {
        /// The module.
        module: String,
        /// The event.
        name: String,
    },
    /// A measure with an invalid or repeated name, or a default policy on a measure the
    /// module does not declare.
    #[error("module `{module}`: measure `{name}` must be named [a-z][a-z0-9_]* once and declared")]
    InvalidMeasure {
        /// The module.
        module: String,
        /// The measure.
        name: String,
    },
    /// An operation or a type that requires a permission its module does not declare, or
    /// that names an operation or a type the module does not have (ADR 0026, amendment 3).
    #[error(
        "module `{module}`: `{operation}` requires `{permission}`, which the module does not declare"
    )]
    UnknownPermission {
        /// The module.
        module: String,
        /// The operation, or the qualified type.
        operation: String,
        /// The permission it requires.
        permission: String,
    },
    /// A REST collection declared twice, or taken from the core.
    #[error("collection `/v1/{collection}` of module `{module}` is already served by `{owner}`")]
    DuplicateCollection {
        /// The collection.
        collection: String,
        /// The module that declares it last.
        module: String,
        /// The module, or `core`, that has it.
        owner: String,
    },
}

/// The modules of the build, checked and in dependency order (ADR 0026, clause 2).
#[derive(Debug, Clone, Default)]
pub struct ModuleRegistry {
    modules: Vec<Arc<dyn Module>>,
}

/// The background tasks, the events and the measures of `module` have valid names, each
/// once, and its default policies name its own measures (REQ-016-7, 8, 15, 16).
fn check_jobs_and_events(module: &dyn Module) -> Result<(), RegistryError> {
    let mut jobs = BTreeSet::new();
    for job in module.jobs() {
        if !valid_name(job.name) || !jobs.insert(job.name) {
            return Err(RegistryError::InvalidJob {
                module: module.name().to_owned(),
                name: job.name.to_owned(),
            });
        }
    }
    let mut events = BTreeSet::new();
    for event in module.events() {
        if !valid_name(event) || !events.insert(*event) {
            return Err(RegistryError::InvalidEvent {
                module: module.name().to_owned(),
                name: (*event).to_owned(),
            });
        }
    }
    let invalid_measure = |name: &str| RegistryError::InvalidMeasure {
        module: module.name().to_owned(),
        name: name.to_owned(),
    };
    let mut measures = BTreeSet::new();
    for measure in module.measures() {
        if !valid_name(measure) || !measures.insert(*measure) {
            return Err(invalid_measure(measure));
        }
    }
    for policy in module.default_policies() {
        if let Some((measure, _)) = policy.triggers.iter().find(|(m, _)| !measures.contains(m)) {
            return Err(invalid_measure(measure));
        }
    }
    Ok(())
}

/// Each permission `module` puts in place of `<type>.read` or `<type>.edit` is one of its
/// own: for one of its types ([`Module::type_permissions`]), or for one of its operations,
/// each operation listed once ([`Module::operation_permissions`]) (ADR 0026, amendment 3).
fn check_operation_permissions(module: &dyn Module) -> Result<(), RegistryError> {
    let declared = |name: &str| module.permissions().iter().any(|p| p.name == name);
    for ty in module.type_permissions() {
        let own_type = module.object_types().iter().any(|t| t.kind == ty.kind);
        if let Some(permission) = [ty.read, ty.edit]
            .into_iter()
            .find(|p| !own_type || !declared(p))
        {
            return Err(RegistryError::UnknownPermission {
                module: module.name().to_owned(),
                operation: ty.kind.to_owned(),
                permission: permission.to_owned(),
            });
        }
    }
    let mut seen = BTreeSet::new();
    for (operation, permission) in module.operation_permissions() {
        let known = module.operations().iter().any(|o| o.name == *operation)
            && module.permissions().iter().any(|p| p.name == *permission)
            && seen.insert(*operation);
        if !known {
            return Err(RegistryError::UnknownPermission {
                module: module.name().to_owned(),
                operation: (*operation).to_owned(),
                permission: (*permission).to_owned(),
            });
        }
    }
    Ok(())
}

fn valid_name(name: &str) -> bool {
    let mut chars = name.chars();
    chars.next().is_some_and(|c| c.is_ascii_lowercase())
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

impl ModuleRegistry {
    /// Checks `modules` and orders them so that each comes after its dependencies; ties
    /// keep the order given.
    ///
    /// # Errors
    ///
    /// [`RegistryError`] for an invalid, reserved or repeated name, a missing dependency,
    /// a cycle, a type outside the namespace of its module or a collection served twice.
    pub fn new(modules: Vec<Arc<dyn Module>>) -> Result<Self, RegistryError> {
        let mut by_name: BTreeMap<&str, usize> = BTreeMap::new();
        for (i, module) in modules.iter().enumerate() {
            let name = module.name();
            if name == CORE {
                return Err(RegistryError::ReservedName);
            }
            if !valid_name(name) {
                return Err(RegistryError::InvalidName(name.to_owned()));
            }
            if by_name.insert(name, i).is_some() {
                return Err(RegistryError::Duplicate(name.to_owned()));
            }
        }
        let mut collections: BTreeMap<&str, &str> =
            CORE_COLLECTIONS.iter().map(|c| (*c, CORE)).collect();
        for module in &modules {
            for dependency in module.depends_on() {
                if !by_name.contains_key(dependency) {
                    return Err(RegistryError::MissingDependency {
                        module: module.name().to_owned(),
                        dependency: (*dependency).to_owned(),
                    });
                }
            }
            let prefix = format!("{}.", module.name());
            let names = module
                .permissions()
                .iter()
                .map(|p| p.name)
                .chain(module.rules().iter().map(|r| r.id));
            for name in names {
                let taken = kartenn_core::authz::PERMISSIONS
                    .iter()
                    .any(|p| p.name == name)
                    || kartenn_core::rules::all_rules().any(|r| r.id == name);
                if !name.starts_with(&prefix) || taken {
                    return Err(RegistryError::ForeignName {
                        module: module.name().to_owned(),
                        name: name.to_owned(),
                    });
                }
            }
            check_jobs_and_events(module.as_ref())?;
            check_operation_permissions(module.as_ref())?;
            for ty in module.object_types() {
                if ty
                    .kind
                    .split_once('.')
                    .is_none_or(|(m, t)| m != module.name() || t.is_empty())
                {
                    return Err(RegistryError::ForeignKind {
                        module: module.name().to_owned(),
                        kind: ty.kind.to_owned(),
                    });
                }
                if let Some(owner) = collections.insert(ty.collection, module.name()) {
                    return Err(RegistryError::DuplicateCollection {
                        collection: ty.collection.to_owned(),
                        module: module.name().to_owned(),
                        owner: owner.to_owned(),
                    });
                }
            }
        }
        // Kahn: repeatedly take the first module whose dependencies are all placed.
        let mut placed: BTreeSet<&str> = BTreeSet::new();
        let mut order = Vec::with_capacity(modules.len());
        let mut rest = modules;
        while !rest.is_empty() {
            let Some(next) = rest
                .iter()
                .position(|m| m.depends_on().iter().all(|d| placed.contains(d)))
            else {
                let mut cycle: Vec<String> = rest.iter().map(|m| m.name().to_owned()).collect();
                cycle.sort();
                return Err(RegistryError::Cycle(cycle));
            };
            let module = rest.remove(next);
            placed.insert(module.name());
            order.push(module);
        }
        // The catalogues of the build gain the permissions and rules of its modules
        // (REQ-008-8): role entries, scopes and journaled violations see them.
        for module in &order {
            kartenn_core::authz::register(module.permissions());
            kartenn_core::authz::register_types(module.type_permissions());
            kartenn_core::rules::register(module.rules());
        }
        Ok(Self { modules: order })
    }

    /// A registry without modules, the build of the core alone.
    #[must_use]
    pub fn empty() -> &'static Self {
        static EMPTY: LazyLock<ModuleRegistry> = LazyLock::new(ModuleRegistry::default);
        &EMPTY
    }

    /// The modules, each after its dependencies.
    pub fn modules(&self) -> impl Iterator<Item = &Arc<dyn Module>> {
        self.modules.iter()
    }

    /// The module of this name.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&Arc<dyn Module>> {
        self.modules.iter().find(|m| m.name() == name)
    }

    /// Whether there is no module.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.modules.is_empty()
    }

    /// The labels of the codes of every module, in registry order (spec 018).
    #[must_use]
    pub fn labels(&self) -> Vec<kartenn_core::labels::CodeLabel> {
        self.modules.iter().flat_map(|m| m.labels()).collect()
    }

    /// The tables of every module whose rows own labels (spec 018).
    pub fn label_tables(&self) -> impl Iterator<Item = &'static LabelTable> + '_ {
        self.modules.iter().flat_map(|m| m.label_tables().iter())
    }

    /// The types of objects of every module, in registry order.
    pub fn object_types(&self) -> impl Iterator<Item = &'static ObjectType> + '_ {
        self.modules.iter().flat_map(|m| m.object_types().iter())
    }

    /// The permissions of every module, in registry order.
    #[must_use]
    pub fn permissions(&self) -> Vec<&'static Permission> {
        self.modules
            .iter()
            .flat_map(|m| m.permissions().iter().copied())
            .collect()
    }

    /// The type of objects of this qualified name, if a module of the build declares it.
    #[must_use]
    pub fn object_type(&self, kind: &str) -> Option<&'static ObjectType> {
        self.object_types().find(|t| t.kind == kind)
    }

    /// The permission the operation `name` of `module` requires in place of `<type>.edit`,
    /// if the module declares one (ADR 0026, amendment 3).
    #[must_use]
    pub fn operation_permission(&self, module: &str, name: &str) -> Option<&'static str> {
        self.get(module)?
            .operation_permissions()
            .iter()
            .find(|(o, _)| *o == name)
            .map(|(_, p)| *p)
    }

    /// The operation `name` of `module`, with its handler, if the build has both.
    #[must_use]
    pub fn operation(
        &self,
        module: &str,
        name: &str,
    ) -> Option<(&'static OperationType, Arc<dyn ModuleHandler>)> {
        let found = self.get(module)?;
        let operation = found.operations().iter().find(|o| o.name == name)?;
        Some((operation, found.handler()?))
    }

    /// The background tasks of every module, qualified (`<module>.<task>`), with their
    /// period, in registry order (REQ-016-16).
    #[must_use]
    pub fn jobs(&self) -> Vec<(String, std::time::Duration)> {
        self.modules
            .iter()
            .flat_map(|m| {
                m.jobs()
                    .iter()
                    .map(|j| (format!("{}.{}", m.name(), j.name), j.period))
            })
            .collect()
    }

    /// Whether a module of the build declares the measure `measure` (`<module>.<name>`).
    #[must_use]
    pub fn declares_measure(&self, measure: &str) -> bool {
        measure.split_once('.').is_some_and(|(module, name)| {
            self.get(module)
                .is_some_and(|m| m.measures().contains(&name))
        })
    }

    /// The default policies of every module, with their module, in registry order
    /// (REQ-016-8).
    #[must_use]
    pub fn default_policies(&self) -> Vec<(&'static str, &'static DefaultPolicy)> {
        self.modules
            .iter()
            .flat_map(|m| m.default_policies().iter().map(|p| (m.name(), p)))
            .collect()
    }

    /// Whether a module of the build declares the event `event` (`<module>.<name>`).
    #[must_use]
    pub fn declares_event(&self, event: &str) -> bool {
        event.split_once('.').is_some_and(|(module, name)| {
            self.get(module).is_some_and(|m| m.events().contains(&name))
        })
    }

    /// The handler of the module that declares the type `kind`, if any.
    #[must_use]
    pub fn handler_of(&self, kind: ModuleKind) -> Option<Arc<dyn ModuleHandler>> {
        let found = self.get(kind.module())?;
        found
            .object_types()
            .iter()
            .any(|t| t.kind == kind.as_str())
            .then(|| found.handler())
            .flatten()
    }
}

/// Runs the background task `name` (`<module>.<task>`) of a module of `store`'s build, in
/// one transaction committed when the task ends well (REQ-016-16). `false` when no module
/// of the build declares the task: the scheduler then knows it is not a module's.
///
/// # Errors
///
/// [`StoreError`] of the task or of the transaction; the transaction is rolled back.
pub async fn run_job<S: crate::port::Store>(store: &S, name: &str) -> Result<bool, StoreError> {
    use crate::port::WriteTx as _;
    let Some((module, task)) = name.split_once('.') else {
        return Ok(false);
    };
    let Some(found) = store.modules().get(module) else {
        return Ok(false);
    };
    if !found.jobs().iter().any(|j| j.name == task) {
        return Ok(false);
    }
    let Some(handler) = found.handler() else {
        return Ok(false);
    };
    let mut tx = store.begin().await?;
    let outcome = match handler.run_job_findings(task, tx.module_tx()).await {
        // The events it emitted, with the task (REQ-016-15).
        Ok(found) if found.violations.is_empty() && found.reevaluated.is_empty() => {
            tx.journal_job(name).await
        }
        // Its findings, in a changeset of kartenn that opens and closes the anomalies
        // (amendment 4); the events go with it, under the key of the task.
        Ok(found) => {
            let request = crate::ChangesetRequest {
                author: crate::declarations::SYSTEM_AUTHOR,
                motive: format!("background task {name}"),
                period: None,
                operations: Vec::new(),
                simulate: false,
                idempotency_key: None,
                apply_own: true,
            };
            let objects: Vec<Uuid> = found
                .reevaluated
                .iter()
                .map(|(o, _)| *o)
                .chain(
                    found
                        .violations
                        .iter()
                        .flat_map(|v| v.subjects.iter().copied()),
                )
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect();
            let keys = [name.replacen('.', ":", 1)];
            let journal = crate::port::Journal {
                id: Uuid::now_v7(),
                request: &request,
                state: "applied",
                kind: "direct",
                validator: None,
                pending: None,
                violations: &found.violations,
                domains: &[],
                keys: &keys,
                objects: &objects,
                reevaluated: &found.reevaluated,
                diff: &[],
                cascaded: &[],
                audit: &[],
                on_behalf_of: None,
                draft: None,
            };
            tx.journal_applied(&journal).await
        }
        Err(e) => Err(e),
    };
    match outcome {
        Ok(()) => tx.commit().await.map(|()| true),
        Err(e) => {
            tx.rollback().await?;
            Err(e)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug)]
    struct Fake {
        name: &'static str,
        depends_on: &'static [&'static str],
        types: &'static [ObjectType],
    }

    impl Module for Fake {
        fn name(&self) -> &'static str {
            self.name
        }
        fn version(&self) -> &'static str {
            "0.0.0"
        }
        fn depends_on(&self) -> &'static [&'static str] {
            self.depends_on
        }
        fn object_types(&self) -> &'static [ObjectType] {
            self.types
        }
    }

    fn fake(name: &'static str, depends_on: &'static [&'static str]) -> Arc<dyn Module> {
        Arc::new(Fake {
            name,
            depends_on,
            types: &[],
        })
    }

    const fn ty(kind: &'static str, collection: &'static str) -> ObjectType {
        ObjectType {
            kind,
            collection,
            parent: None,
            owner_required: false,
            carries_site: false,
            domain_anchored: false,
        }
    }

    fn names(registry: &ModuleRegistry) -> Vec<&'static str> {
        registry.modules().map(|m| m.name()).collect()
    }

    #[test]
    fn registry_orders_dependencies_first() {
        let registry = ModuleRegistry::new(vec![
            fake("flow", &["policy"]),
            fake("policy", &[]),
            fake("vlan", &[]),
        ])
        .unwrap();
        assert_eq!(names(&registry), ["policy", "flow", "vlan"]);
        assert!(registry.get("flow").is_some());
        assert!(ModuleRegistry::empty().is_empty());
    }

    #[test]
    fn registry_refuses_duplicate() {
        assert_eq!(
            ModuleRegistry::new(vec![fake("vlan", &[]), fake("vlan", &[])]).unwrap_err(),
            RegistryError::Duplicate("vlan".into())
        );
    }

    #[test]
    fn registry_refuses_core_name() {
        assert_eq!(
            ModuleRegistry::new(vec![fake("core", &[])]).unwrap_err(),
            RegistryError::ReservedName
        );
        for wrong in ["Vlan", "1vlan", "vlan-x", ""] {
            assert_eq!(
                ModuleRegistry::new(vec![fake(wrong, &[])]).unwrap_err(),
                RegistryError::InvalidName(wrong.into())
            );
        }
    }

    #[test]
    fn registry_refuses_missing_dependency() {
        let error = ModuleRegistry::new(vec![fake("machines", &["vlan"])]).unwrap_err();
        assert_eq!(
            error,
            RegistryError::MissingDependency {
                module: "machines".into(),
                dependency: "vlan".into()
            }
        );
        let message = error.to_string();
        assert!(
            message.contains("machines") && message.contains("vlan"),
            "{message}"
        );
    }

    #[test]
    fn registry_refuses_a_repeated_job() {
        #[derive(Debug)]
        struct Jobs;
        impl Module for Jobs {
            fn name(&self) -> &'static str {
                "jobs"
            }
            fn version(&self) -> &'static str {
                "0.0.0"
            }
            fn jobs(&self) -> &'static [JobSpec] {
                const PERIOD: std::time::Duration = std::time::Duration::from_secs(60);
                &[
                    JobSpec {
                        name: "expiry",
                        period: PERIOD,
                    },
                    JobSpec {
                        name: "expiry",
                        period: PERIOD,
                    },
                ]
            }
        }
        assert_eq!(
            ModuleRegistry::new(vec![Arc::new(Jobs)]).map(|_| ()),
            Err(RegistryError::InvalidJob {
                module: "jobs".into(),
                name: "expiry".into()
            })
        );
    }

    #[test]
    fn registry_refuses_cycle() {
        assert_eq!(
            ModuleRegistry::new(vec![fake("a", &["b"]), fake("b", &["a"]), fake("c", &[])])
                .unwrap_err(),
            RegistryError::Cycle(vec!["a".into(), "b".into()])
        );
    }

    #[test]
    fn registry_refuses_duplicate_collection() {
        static VLANS: &[ObjectType] = &[ty("vlan.vlan", "vlans")];
        static ALSO_VLANS: &[ObjectType] = &[ty("other.vlan", "vlans")];
        static PREFIXES: &[ObjectType] = &[ty("other.prefix", "prefixes")];
        let module = |name, types| -> Arc<dyn Module> {
            Arc::new(Fake {
                name,
                depends_on: &[],
                types,
            })
        };
        assert_eq!(
            ModuleRegistry::new(vec![module("vlan", VLANS), module("other", ALSO_VLANS)])
                .unwrap_err(),
            RegistryError::DuplicateCollection {
                collection: "vlans".into(),
                module: "other".into(),
                owner: "vlan".into()
            }
        );
        assert_eq!(
            ModuleRegistry::new(vec![module("other", PREFIXES)]).unwrap_err(),
            RegistryError::DuplicateCollection {
                collection: "prefixes".into(),
                module: "other".into(),
                owner: "core".into()
            }
        );
        assert_eq!(
            ModuleRegistry::new(vec![module("other", VLANS)]).unwrap_err(),
            RegistryError::ForeignKind {
                module: "other".into(),
                kind: "vlan.vlan".into()
            }
        );
    }
}
