//! Storage port of the write pipeline (hexagonal architecture, ADR 0001).

use std::future::Future;

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
    /// (REQ-002-32), with their domain.
    fn public_networks_overlapping(
        &mut self,
        domain: Uuid,
        prefix: Prefix,
        period: Option<Period>,
    ) -> impl Future<Output = Result<Vec<(Uuid, Uuid)>, StoreError>> + Send;

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
