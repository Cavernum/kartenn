//! PostgreSQL implementation of the write port of the pipeline (spec 004, ADR 0015).

use kartenn_core::addr::{Addr, Prefix};
use kartenn_core::family::ALL;
use kartenn_core::objects::{Address, AddressUsage, IpRange, Nature, PrefixObject, RangeUsage};
use kartenn_core::period::Period;
use kartenn_core::properties::{Declaration, ObligationScope};
use kartenn_core::rules::{self, Rule, Violation};
use kartenn_service::port::{
    ApprovalPolicy, Change, DiffCount, DiffItem, ImportRecord, Journal, ObjectView,
    PendingChangeset, PropertyFacts, Replay, Store, StoreError, UndoFacts, WriteError, WriteTx,
};

use crate::properties::{
    ASSIGNMENT_COLUMNS, AssignmentRow, DECLARATION_COLUMNS, DeclarationRow, ValueColumns,
    decode_assignment, decode_declaration, encode_type, encode_value, ref_kind, referenced,
};
use kartenn_core::inheritance::Assigned;
use kartenn_core::values::Value;
use kartenn_service::{ChangesetRequest, ObjectKind, Operation};
use sha2::{Digest, Sha256};
use sqlx::types::Uuid;
use sqlx::types::time::OffsetDateTime;
use sqlx::{PgPool, Postgres, Transaction};
use std::collections::HashMap;

/// CloudEvents `source` of the events of this instance (to be made configurable with the
/// server configuration).
pub(crate) const EVENT_SOURCE: &str = "urn:kartenn:instance";

/// Advisory lock key of public space (ADR 0015).
const PUBLIC_SPACE_LOCK: i64 = 0x6b61_7274_656e_6e50; // "kartennP"

fn store_err(e: impl std::fmt::Display) -> StoreError {
    StoreError(e.to_string())
}

/// The rule a database constraint backs, by constraint name.
fn constraint_rule(name: &str) -> Option<&'static Rule> {
    Some(match name {
        "prefix_unique_per_domain" => &rules::PREFIX_DUPLICATE,
        "prefix_p2p_network" => &rules::PREFIX_P2P_LENGTH,
        "address_unique_per_domain" => &rules::ADDRESS_DUPLICATE,
        "address_public_unique" => &rules::PUBLIC_OVERLAP,
        "address_is_host" => &rules::ADDR_SYNTAX,
        "ip_range_disjoint" => &rules::RANGE_OVERLAP,
        "ip_range_bounds" => &rules::RANGE_BOUNDS,
        "addressing_domain_slug_unique" | "entity_slug_unique" | "site_slug_unique" => {
            &rules::SLUG_DUPLICATE
        }
        "property_key_unique" => &rules::PROP_DUPLICATE_KEY,
        "property_enum_key_unique" => &rules::PROP_ENUM_VALUE_INVALID,
        "external_id_pkey" => &rules::EXTERNAL_ID_DUPLICATE,
        _ => return None,
    })
}

fn write_err(e: sqlx::Error, subject: Uuid) -> WriteError {
    if let Some(db) = e.as_database_error()
        && let Some(rule) = db.constraint().and_then(constraint_rule)
    {
        return WriteError::Rule(
            Violation::new(rule, db.message().to_owned()).with_subjects([subject]),
        );
    }
    WriteError::Store(store_err(e))
}

fn instant(i: kartenn_core::period::Instant) -> Result<OffsetDateTime, StoreError> {
    OffsetDateTime::from_unix_timestamp_nanos(i128::from(i.0) * 1_000).map_err(store_err)
}

/// Bounds of a period to bind: `None` start means transaction time.
fn bounds(
    period: Option<Period>,
) -> Result<(Option<OffsetDateTime>, Option<OffsetDateTime>), StoreError> {
    match period {
        None => Ok((None, None)),
        Some(p) => Ok((Some(instant(p.start())?), p.end().map(instant).transpose()?)),
    }
}

/// SQL of a period from bound parameters `$a`, `$b`: transaction time when `$a` is null.
fn period_sql(a: u8, b: u8) -> String {
    format!("tstzrange(coalesce(${a}::timestamptz, now()), ${b}::timestamptz)")
}

/// Stable digest of a request, to recognise a resubmission under the same idempotency key.
fn request_hash(request: &ChangesetRequest) -> Result<Vec<u8>, StoreError> {
    let bytes = serde_json::to_vec(request).map_err(store_err)?;
    Ok(Sha256::digest(&bytes).to_vec())
}

/// PostgreSQL store.
#[derive(Debug, Clone)]
pub struct PgStore {
    pool: PgPool,
    down_from: usize,
}

impl PgStore {
    /// A store on a pool.
    #[must_use]
    pub const fn new(pool: PgPool) -> Self {
        Self {
            pool,
            down_from: crate::origins::DOWN_FROM,
        }
    }

    /// The number of targets from which origins are recomputed going down from the
    /// objects that carry an assignment rather than up from each target (both give the
    /// same origins; tests use it to cover either way).
    #[must_use]
    pub const fn with_down_from(mut self, targets: usize) -> Self {
        self.down_from = targets;
        self
    }
}

/// A write transaction.
#[derive(Debug)]
pub struct PgTx {
    pub(crate) tx: Transaction<'static, Postgres>,
    pub(crate) down_from: usize,
}

impl PgTx {
    /// Inserts a version of a declaration from `start` (transaction time if `None`) to
    /// `end`.
    async fn write_declaration(
        &mut self,
        id: Uuid,
        d: &Declaration,
        start: Option<OffsetDateTime>,
        end: Bind,
        changeset: Uuid,
    ) -> Result<(), WriteError> {
        let (value_type, params) = encode_type(&d.spec);
        let sql = format!(
            "INSERT INTO property_declaration
                 (id, key, value_type, type_params, cardinality, carriers, inherit_tree, valid_period, changeset_id,
                  expected_source, expected_mode)
             VALUES ($1, $2, $3, $4::jsonb, $5, $6, $7, {}, $10, $11, $12)",
            period_sql(8, 9)
        );
        let binds = vec![
            Bind::Uuid(id),
            Bind::Text(d.key.to_string()),
            Bind::Text(value_type),
            Bind::Text(params),
            Bind::Text(d.cardinality.as_str().to_owned()),
            Bind::Texts(d.carriers.iter().cloned().collect()),
            Bind::OptText(d.inherit.as_ref().map(|t| t.as_str().to_owned())),
            Bind::Time(start),
            end,
            Bind::Uuid(changeset),
            Bind::OptText(d.expected.as_ref().map(|e| e.source.clone())),
            Bind::OptText(d.expected.as_ref().map(|e| e.mode.as_str().to_owned())),
        ];
        self.guarded(sqlx::AssertSqlSafe(sql), binds, id)
            .await
            .map(|_| ())
    }

    /// Runs one write under a savepoint, so that a refused write keeps the transaction.
    async fn guarded(
        &mut self,
        sql: sqlx::AssertSqlSafe<String>,
        binds: Binds,
        subject: Uuid,
    ) -> Result<u64, WriteError> {
        sqlx::raw_sql("SAVEPOINT write")
            .execute(&mut *self.tx)
            .await
            .map_err(store_err)?;
        let mut q = sqlx::query(sql);
        for b in binds {
            q = match b {
                Bind::Uuid(v) => q.bind(v),
                Bind::Text(v) => q.bind(v),
                Bind::Bool(v) => q.bind(v),
                Bind::Time(v) => q.bind(v),
                Bind::OptText(v) => q.bind(v),
                Bind::Texts(v) => q.bind(v),
                Bind::OptTexts(v) => q.bind(v),
                Bind::OptUuid(v) => q.bind(v),
                Bind::Uuids(v) => q.bind(v),
                Bind::OptInt(v) => q.bind(v),
                Bind::Ints(v) => q.bind(v),
                Bind::OptBool(v) => q.bind(v),
            };
        }
        match q.execute(&mut *self.tx).await {
            Ok(done) => {
                sqlx::raw_sql("RELEASE SAVEPOINT write")
                    .execute(&mut *self.tx)
                    .await
                    .map_err(store_err)?;
                Ok(done.rows_affected())
            }
            Err(e) => {
                sqlx::raw_sql("ROLLBACK TO SAVEPOINT write")
                    .execute(&mut *self.tx)
                    .await
                    .map_err(store_err)?;
                Err(write_err(e, subject))
            }
        }
    }
}

enum Bind {
    Uuid(Uuid),
    Text(String),
    Bool(bool),
    Time(Option<OffsetDateTime>),
    OptText(Option<String>),
    Texts(Vec<String>),
    OptTexts(Option<Vec<String>>),
    OptUuid(Option<Uuid>),
    Uuids(Option<Vec<Uuid>>),
    OptInt(Option<i64>),
    Ints(Option<Vec<i64>>),
    OptBool(Option<bool>),
}

type Binds = Vec<Bind>;

impl Store for PgStore {
    type Tx = PgTx;

    async fn begin(&self) -> Result<PgTx, StoreError> {
        Ok(PgTx {
            tx: self.pool.begin().await.map_err(store_err)?,
            down_from: self.down_from,
        })
    }

    async fn journal_refused(&self, journal: &Journal<'_>) -> Result<(), StoreError> {
        let mut tx = self.pool.begin().await.map_err(store_err)?;
        write_journal(&mut tx, journal).await?;
        tx.commit().await.map_err(store_err)
    }

    async fn pending(&self, id: Uuid) -> Result<Option<PendingChangeset>, StoreError> {
        crate::pending::find(&self.pool, id).await
    }

    async fn close_pending(
        &self,
        id: Uuid,
        state: &'static str,
        by: Uuid,
        rejection: Option<&str>,
    ) -> Result<bool, StoreError> {
        crate::pending::close(&self.pool, id, state, by, rejection).await
    }

    async fn expire_pending(&self) -> Result<Vec<Uuid>, StoreError> {
        crate::pending::expire(&self.pool).await
    }

    async fn pending_proposals(&self) -> Result<Vec<PendingChangeset>, StoreError> {
        crate::pending::proposals(&self.pool).await
    }

    async fn undo_facts(&self, changeset: Uuid) -> Result<Option<UndoFacts>, StoreError> {
        crate::undo::facts(&self.pool, changeset).await
    }

    async fn create_import(&self, record: &ImportRecord) -> Result<(), StoreError> {
        crate::imports::create(&self.pool, record).await
    }

    async fn import(&self, id: Uuid) -> Result<Option<ImportRecord>, StoreError> {
        crate::imports::find(&self.pool, id).await
    }

    async fn set_batch(
        &self,
        import: Uuid,
        index: i32,
        state: &str,
        changeset: Option<Uuid>,
    ) -> Result<(), StoreError> {
        crate::imports::set_batch(&self.pool, import, index, state, changeset).await
    }

    async fn set_import(
        &self,
        import: Uuid,
        state: &str,
        report: &serde_json::Value,
    ) -> Result<(), StoreError> {
        crate::imports::set_import(&self.pool, import, state, report).await
    }

    async fn external_object(
        &self,
        source: &str,
        external_id: &str,
    ) -> Result<Option<Uuid>, StoreError> {
        crate::imports::external(&self.pool, source, external_id).await
    }
}

fn table(kind: ObjectKind) -> &'static str {
    match kind {
        ObjectKind::Domain => "addressing_domain",
        ObjectKind::Prefix => "prefix",
        ObjectKind::Range => "ip_range",
        ObjectKind::Address => "address",
        ObjectKind::Property => "property_declaration",
        ObjectKind::EnumValue => "property_enum_value",
        ObjectKind::Entity => "entity",
        ObjectKind::Site => "site",
    }
}

/// Row of a network: id, domain, value, point-to-point, anycast.
type NetworkRow = (Uuid, Uuid, String, bool, bool);

fn network(
    (id, domain, text, point_to_point, anycast): NetworkRow,
) -> Result<PrefixObject, StoreError> {
    Ok(PrefixObject {
        id,
        domain,
        value: Prefix::parse_any(&ALL, &text).map_err(store_err)?,
        nature: Nature::Network,
        point_to_point,
        anycast,
    })
}

fn address_usage(text: &str) -> Result<AddressUsage, StoreError> {
    Ok(match text {
        "host" => AddressUsage::Host,
        "gateway" => AddressUsage::Gateway,
        "vip" => AddressUsage::Vip,
        "anycast" => AddressUsage::Anycast,
        "reserved" => AddressUsage::Reserved,
        other => return Err(store_err(format!("unknown address usage {other}"))),
    })
}

fn range_usage(text: &str) -> Result<RangeUsage, StoreError> {
    Ok(match text {
        "dhcp" => RangeUsage::Dhcp,
        "reserved" => RangeUsage::Reserved,
        "static" => RangeUsage::Static,
        other => return Err(store_err(format!("unknown range usage {other}"))),
    })
}

fn addr(text: &str) -> Result<Addr, StoreError> {
    Addr::parse_any(&ALL, text).map_err(store_err)
}

fn violations_json(violations: &[Violation]) -> serde_json::Value {
    serde_json::Value::Array(
        violations
            .iter()
            .map(|v| {
                serde_json::json!({
                    "rule": v.rule.id,
                    "severity": v.rule.severity.to_string(),
                    "message": v.message,
                    "subjects": v.subjects,
                })
            })
            .collect(),
    )
}

async fn write_journal(
    tx: &mut Transaction<'static, Postgres>,
    journal: &Journal<'_>,
) -> Result<(), StoreError> {
    let (start, end) = match journal.request.period {
        None => (None, None),
        Some(p) => (
            Some(instant(kartenn_core::period::Instant(p.start))?),
            p.end
                .map(|e| instant(kartenn_core::period::Instant(e)))
                .transpose()?,
        ),
    };
    let (errors, warnings): (Vec<_>, Vec<_>) = journal
        .violations
        .iter()
        .cloned()
        .partition(|v| v.rule.severity == rules::Severity::Error);
    sqlx::query(
        "INSERT INTO changeset (id, author_id, motive, state, kind, default_period, retroactive, applied_at,
                                violations, anomalies, diff_summary, validator_id, expires_at, policy_id,
                                required_rights, request)
         VALUES ($1, $2, $3, $4, $10, tstzrange(coalesce($5::timestamptz, now()), $6::timestamptz),
                 coalesce($16::timestamptz < now(), false), CASE WHEN $4 = 'applied' THEN now() END, $7, $8, $9,
                 $11, CASE WHEN $4 = 'pending' THEN now() + make_interval(days => $15) END, $12, $13, $14)
         ON CONFLICT (id) DO UPDATE SET state = EXCLUDED.state, validator_id = EXCLUDED.validator_id,
             applied_at = EXCLUDED.applied_at, violations = EXCLUDED.violations,
             anomalies = EXCLUDED.anomalies, diff_summary = EXCLUDED.diff_summary",
    )
    .bind(journal.id)
    .bind(journal.request.author)
    .bind(&journal.request.motive)
    .bind(journal.state)
    .bind(start)
    .bind(end)
    .bind(violations_json(&errors))
    .bind(violations_json(&warnings))
    .bind(serde_json::to_value(journal.diff).map_err(store_err)?)
    .bind(journal.kind)
    .bind(journal.validator)
    .bind(journal.pending.and_then(|p| p.policy))
    .bind(
        journal
            .pending
            .map(serde_json::to_value)
            .transpose()
            .map_err(store_err)?,
    )
    .bind(
        journal
            .pending
            .map(|_| serde_json::to_value(journal.request))
            .transpose()
            .map_err(store_err)?,
    )
    .bind(crate::pending::PENDING_DAYS)
    .bind(earliest(journal.request)?)
    .execute(&mut **tx)
    .await
    .map_err(store_err)?;
    crate::pending::journal_event(
        tx,
        journal.id,
        journal.state,
        journal.request.author,
        journal.validator,
    )
    .await?;
    record_key(tx, journal).await?;
    if journal.state == "applied" {
        // REQ-004-36: one CloudEvents 1.0 event per applied changeset, in its transaction.
        sqlx::query(
            "INSERT INTO outbox (changeset_id, domain_ids, payload) VALUES ($1, $2, jsonb_build_object(
                'specversion', '1.0', 'id', $1::text, 'source', $3::text,
                'type', 'org.kartenn.changeset.applied', 'time', now(),
                'datacontenttype', 'application/json',
                'data', jsonb_strip_nulls(jsonb_build_object('changeset', $1, 'domains', $2, 'objects', $4,
                                                'author', $5, 'validator', $6))))",
        )
        .bind(journal.id)
        .bind(journal.domains)
        .bind(EVENT_SOURCE)
        .bind(journal.objects)
        .bind(journal.request.author)
        .bind(journal.validator)
        .execute(&mut **tx)
        .await
        .map_err(store_err)?;
    }
    record_operations(tx, journal).await
}

/// The earliest start among the changeset's period and its operations' own (REQ-004-2,
/// 27): before now, the changeset is retroactive.
fn earliest(request: &ChangesetRequest) -> Result<Option<OffsetDateTime>, StoreError> {
    request
        .period
        .iter()
        .copied()
        .chain(request.operations.iter().filter_map(Operation::own_period))
        .map(|p| p.start)
        .min()
        .map(|start| instant(kartenn_core::period::Instant(start)))
        .transpose()
}

/// The idempotency key of a submission, for 24 h (REQ-004-10).
async fn record_key(
    tx: &mut Transaction<'static, Postgres>,
    journal: &Journal<'_>,
) -> Result<(), StoreError> {
    let Some(key) = &journal.request.idempotency_key else {
        return Ok(());
    };
    sqlx::query(
        "INSERT INTO idempotency_key (identity_id, key, request_hash, changeset_id, expires_at)
             VALUES ($1, $2, $3, $4, now() + interval '24 hours')
             ON CONFLICT (identity_id, key) DO UPDATE
               SET request_hash = EXCLUDED.request_hash, changeset_id = EXCLUDED.changeset_id,
                   expires_at = EXCLUDED.expires_at
             WHERE idempotency_key.expires_at <= now()",
    )
    .bind(journal.request.author)
    .bind(key)
    .bind(request_hash(journal.request)?)
    .bind(journal.id)
    .execute(&mut **tx)
    .await
    .map_err(store_err)?;
    Ok(())
}

/// The operations of a changeset, once (a validated changeset keeps those of its
/// submission).
async fn record_operations(
    tx: &mut Transaction<'static, Postgres>,
    journal: &Journal<'_>,
) -> Result<(), StoreError> {
    for (ordinal, op) in journal.request.operations.iter().enumerate() {
        let payload = serde_json::to_value(op).map_err(store_err)?;
        sqlx::query(
            "INSERT INTO changeset_operation (changeset_id, ordinal, payload) VALUES ($1, $2, $3)
             ON CONFLICT DO NOTHING",
        )
        .bind(journal.id)
        .bind(i32::try_from(ordinal).map_err(store_err)?)
        .bind(payload)
        .execute(&mut **tx)
        .await
        .map_err(store_err)?;
    }
    Ok(())
}

impl WriteTx for PgTx {
    async fn lock(&mut self, domains: &[Uuid], public_space: bool) -> Result<(), StoreError> {
        let mut keys: Vec<i64> = domains
            .iter()
            .map(|d| d.as_u64_pair().0.cast_signed())
            .collect();
        if public_space {
            keys.push(PUBLIC_SPACE_LOCK);
        }
        keys.sort_unstable();
        keys.dedup();
        for key in keys {
            sqlx::query("SELECT pg_advisory_xact_lock($1)")
                .bind(key)
                .execute(&mut *self.tx)
                .await
                .map_err(store_err)?;
        }
        Ok(())
    }

    async fn replay(&mut self, request: &ChangesetRequest) -> Result<Replay, StoreError> {
        let Some(key) = &request.idempotency_key else {
            return Ok(Replay::Fresh);
        };
        sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1::text || '/' || $2, 0))")
            .bind(request.author)
            .bind(key)
            .execute(&mut *self.tx)
            .await
            .map_err(store_err)?;
        let found: Option<(Uuid, Vec<u8>, String)> = sqlx::query_as(
            "SELECT k.changeset_id, k.request_hash, c.state FROM idempotency_key k
             JOIN changeset c ON c.id = k.changeset_id
             WHERE k.identity_id = $1 AND k.key = $2 AND k.expires_at > now()",
        )
        .bind(request.author)
        .bind(key)
        .fetch_optional(&mut *self.tx)
        .await
        .map_err(store_err)?;
        Ok(match found {
            None => Replay::Fresh,
            Some((changeset, hash, state)) if hash == request_hash(request)? => {
                Replay::Same { changeset, state }
            }
            Some(_) => Replay::Conflict,
        })
    }

    async fn current_changeset(
        &mut self,
        kind: ObjectKind,
        id: Uuid,
        period: Option<Period>,
    ) -> Result<Option<Uuid>, StoreError> {
        let (at, _) = bounds(period)?;
        let sql = format!(
            "SELECT changeset_id FROM {} WHERE id = $1 AND valid_period @> coalesce($2::timestamptz, now())",
            table(kind)
        );
        sqlx::query_scalar(sqlx::AssertSqlSafe(sql))
            .bind(id)
            .bind(at)
            .fetch_optional(&mut *self.tx)
            .await
            .map_err(store_err)
    }

    async fn domain_of(&mut self, kind: ObjectKind, id: Uuid) -> Result<Option<Uuid>, StoreError> {
        let sql = match kind {
            ObjectKind::Domain => return Ok(Some(id)),
            ObjectKind::Prefix => "SELECT domain_id FROM prefix WHERE id = $1 LIMIT 1",
            ObjectKind::Range => "SELECT domain_id FROM ip_range WHERE id = $1 LIMIT 1",
            ObjectKind::Address => "SELECT domain_id FROM address WHERE id = $1 LIMIT 1",
            ObjectKind::Property
            | ObjectKind::EnumValue
            | ObjectKind::Entity
            | ObjectKind::Site => return Ok(None),
        };
        sqlx::query_scalar(sql)
            .bind(id)
            .fetch_optional(&mut *self.tx)
            .await
            .map_err(store_err)
    }

    async fn insert_domain(
        &mut self,
        id: Uuid,
        slug: &str,
        period: Option<Period>,
        changeset: Uuid,
    ) -> Result<(), WriteError> {
        let (a, b) = bounds(period)?;
        let sql = format!(
            "INSERT INTO addressing_domain (id, slug, valid_period, changeset_id) VALUES ($1, $2, {}, $5)",
            period_sql(3, 4)
        );
        let binds = vec![
            Bind::Uuid(id),
            Bind::Text(slug.to_owned()),
            Bind::Time(a),
            Bind::Time(b),
            Bind::Uuid(changeset),
        ];
        self.guarded(sqlx::AssertSqlSafe(sql), binds, id)
            .await
            .map(|_| ())
    }

    async fn insert_prefix(
        &mut self,
        p: &PrefixObject,
        public: bool,
        period: Option<Period>,
        changeset: Uuid,
    ) -> Result<(), WriteError> {
        let (a, b) = bounds(period)?;
        let sql = format!(
            "INSERT INTO prefix (id, domain_id, prefix, nature, point_to_point, public, registry_version, valid_period, changeset_id, anycast)
             VALUES ($1, $2, $3::cidr, $4, $5, $6, $7, {}, $10, $11)",
            period_sql(8, 9)
        );
        let binds = vec![
            Bind::Uuid(p.id),
            Bind::Uuid(p.domain),
            Bind::Text(p.value.to_string()),
            Bind::Text(p.nature.as_str().to_owned()),
            Bind::Bool(p.point_to_point),
            Bind::Bool(public),
            Bind::Text(kartenn_core::registry::UPDATED.to_owned()),
            Bind::Time(a),
            Bind::Time(b),
            Bind::Uuid(changeset),
            Bind::Bool(p.anycast),
        ];
        self.guarded(sqlx::AssertSqlSafe(sql), binds, p.id)
            .await
            .map(|_| ())
    }

    async fn insert_address(
        &mut self,
        addr: &Address,
        public: bool,
        period: Option<Period>,
        changeset: Uuid,
    ) -> Result<(), WriteError> {
        let (a, b) = bounds(period)?;
        let sql = format!(
            "INSERT INTO address (id, domain_id, address, usage, public, registry_version, valid_period, changeset_id)
             VALUES ($1, $2, $3::inet, $4, $5, $6, {}, $9)",
            period_sql(7, 8)
        );
        let binds = vec![
            Bind::Uuid(addr.id),
            Bind::Uuid(addr.domain),
            Bind::Text(addr.value.to_string()),
            Bind::Text(addr.usage.as_str().to_owned()),
            Bind::Bool(public),
            Bind::Text(kartenn_core::registry::UPDATED.to_owned()),
            Bind::Time(a),
            Bind::Time(b),
            Bind::Uuid(changeset),
        ];
        self.guarded(sqlx::AssertSqlSafe(sql), binds, addr.id)
            .await
            .map(|_| ())
    }

    async fn close(
        &mut self,
        kind: ObjectKind,
        id: Uuid,
        period: Option<Period>,
        changeset: Uuid,
    ) -> Result<(), WriteError> {
        let (at, _) = bounds(period)?;
        let t = table(kind);
        let shortened = self
            .guarded(
                sqlx::AssertSqlSafe(format!(
                    "UPDATE {t} SET valid_period = tstzrange(lower(valid_period), coalesce($2::timestamptz, now())),
                                    changeset_id = $3
                     WHERE id = $1 AND valid_period @> coalesce($2::timestamptz, now())
                       AND lower(valid_period) < coalesce($2::timestamptz, now())"
                )),
                vec![Bind::Uuid(id), Bind::Time(at), Bind::Uuid(changeset)],
                id,
            )
            .await?;
        let removed = self
            .guarded(
                sqlx::AssertSqlSafe(format!(
                    "DELETE FROM {t} WHERE id = $1 AND lower(valid_period) >= coalesce($2::timestamptz, now())"
                )),
                vec![Bind::Uuid(id), Bind::Time(at)],
                id,
            )
            .await?;
        if shortened + removed == 0 {
            Err(WriteError::NoVersion(id))
        } else {
            Ok(())
        }
    }

    async fn modify(
        &mut self,
        kind: ObjectKind,
        id: Uuid,
        change: &Change,
        period: Option<Period>,
        changeset: Uuid,
    ) -> Result<(), WriteError> {
        let (a, b) = bounds(period)?;
        let mut patch = serde_json::Map::new();
        match change {
            Change::Prefix {
                value,
                nature,
                point_to_point,
                anycast,
            } => {
                if let Some((value, public)) = value {
                    patch.insert("prefix".into(), value.to_string().into());
                    patch.insert("public".into(), (*public).into());
                    patch.insert(
                        "registry_version".into(),
                        kartenn_core::registry::UPDATED.into(),
                    );
                }
                if let Some(nature) = nature {
                    patch.insert("nature".into(), nature.as_str().into());
                }
                if let Some(p) = point_to_point {
                    patch.insert("point_to_point".into(), (*p).into());
                }
                if let Some(p) = anycast {
                    patch.insert("anycast".into(), (*p).into());
                }
            }
            Change::Address(usage) => {
                patch.insert("usage".into(), usage.as_str().into());
            }
            Change::Range(usage) => {
                patch.insert("usage".into(), usage.as_str().into());
            }
            Change::Parent(parent) => {
                patch.insert(
                    "parent_id".into(),
                    parent.map_or(serde_json::Value::Null, |p| p.to_string().into()),
                );
            }
        }
        // `kartenn_portion` returns the number of versions over the portion.
        let touched = self
            .guarded(
                sqlx::AssertSqlSafe(
                    "SELECT 1 WHERE kartenn_portion($1::regclass, $2, coalesce($3::timestamptz, now()), $4, $5::jsonb, $6) > 0"
                        .to_owned(),
                ),
                vec![
                    Bind::Text(table(kind).to_owned()),
                    Bind::Uuid(id),
                    Bind::Time(a),
                    Bind::Time(b),
                    Bind::Text(serde_json::Value::Object(patch).to_string()),
                    Bind::Uuid(changeset),
                ],
                id,
            )
            .await?;
        if touched == 0 {
            Err(WriteError::NoVersion(id))
        } else {
            Ok(())
        }
    }

    async fn restore(
        &mut self,
        kind: ObjectKind,
        id: Uuid,
        property: Option<Uuid>,
        before: Uuid,
        period: Option<Period>,
        changeset: Uuid,
    ) -> Result<(), WriteError> {
        let (a, b) = bounds(period)?;
        let at: Option<OffsetDateTime> = sqlx::query_scalar(
            "SELECT applied_at FROM changeset WHERE id = $1 AND state = 'applied'",
        )
        .bind(before)
        .fetch_optional(&mut *self.tx)
        .await
        .map_err(store_err)?
        .flatten();
        let Some(at) = at else {
            return Err(WriteError::Rule(
                Violation::new(
                    &rules::CHANGESET_NO_VERSION,
                    format!("changeset {before} is not applied: nothing to restore"),
                )
                .with_subjects([before]),
            ));
        };
        let (target, key) = match property {
            None => (table(kind), serde_json::json!({ "id": id })),
            Some(p) => (
                "property_assignment",
                serde_json::json!({ "object_id": id, "property_id": p }),
            ),
        };
        self.guarded(
            sqlx::AssertSqlSafe(
                "SELECT kartenn_restore($1::regclass, $2::jsonb, $3, coalesce($4::timestamptz, now()), $5, $6)"
                    .to_owned(),
            ),
            vec![
                Bind::Text(target.to_owned()),
                Bind::Text(key.to_string()),
                Bind::Time(Some(at)),
                Bind::Time(a),
                Bind::Time(b),
                Bind::Uuid(changeset),
            ],
            id,
        )
        .await
        .map(|_| ())
    }

    async fn dependents(
        &mut self,
        kind: ObjectKind,
        id: Uuid,
        period: Option<Period>,
    ) -> Result<Vec<(ObjectKind, Uuid)>, StoreError> {
        let (a, _) = bounds(period)?;
        let w = "tstzrange(coalesce($2::timestamptz, now()), NULL)";
        let sql = match kind {
            ObjectKind::Prefix => format!(
                "WITH p AS (SELECT DISTINCT domain_id, prefix FROM prefix
                            WHERE id = $1 AND nature = 'network' AND valid_period && {w})
                 SELECT DISTINCT 'range', r.id FROM ip_range r, p
                 WHERE r.domain_id = p.domain_id AND lower(r.range) <<= p.prefix AND r.valid_period && {w}
                 UNION
                 SELECT DISTINCT 'address', x.id FROM address x, p
                 WHERE x.domain_id = p.domain_id AND x.address <<= p.prefix AND x.valid_period && {w}"
            ),
            ObjectKind::Domain => format!(
                "SELECT DISTINCT 'prefix', id FROM prefix WHERE domain_id = $1 AND valid_period && {w}
                 UNION SELECT DISTINCT 'range', id FROM ip_range WHERE domain_id = $1 AND valid_period && {w}
                 UNION SELECT DISTINCT 'address', id FROM address WHERE domain_id = $1 AND valid_period && {w}"
            ),
            ObjectKind::Entity | ObjectKind::Site => format!(
                "SELECT DISTINCT '{}', id FROM {} WHERE parent_id = $1 AND valid_period && {w}",
                kind.name(),
                table(kind)
            ),
            ObjectKind::Range
            | ObjectKind::Address
            | ObjectKind::Property
            | ObjectKind::EnumValue => return Ok(Vec::new()),
        };
        let rows: Vec<(String, Uuid)> = sqlx::query_as(sqlx::AssertSqlSafe(sql))
            .bind(id)
            .bind(a)
            .fetch_all(&mut *self.tx)
            .await
            .map_err(store_err)?;
        rows.into_iter()
            .map(|(k, id)| Ok((object_kind(&k)?, id)))
            .collect()
    }

    async fn referencing(
        &mut self,
        target: Uuid,
        period: Option<Period>,
    ) -> Result<Vec<(Uuid, ObjectKind, Uuid)>, StoreError> {
        let (a, _) = bounds(period)?;
        let rows: Vec<(Uuid, String, Uuid)> = sqlx::query_as(
            "SELECT DISTINCT a.object_id, a.object_type, a.property_id FROM property_assignment a
             JOIN property_declaration d ON d.id = a.property_id AND d.value_type = 'ref'
             WHERE a.kind = 'value' AND (a.v_uuid = $1 OR $1 = ANY(a.v_uuids))
               AND a.valid_period && tstzrange(coalesce($2::timestamptz, now()), NULL)",
        )
        .bind(target)
        .bind(a)
        .fetch_all(&mut *self.tx)
        .await
        .map_err(store_err)?;
        rows.into_iter()
            .map(|(o, k, p)| Ok((o, object_kind(&k)?, p)))
            .collect()
    }

    async fn bind_external(
        &mut self,
        object: Uuid,
        kind: ObjectKind,
        source: &str,
        external_id: &str,
    ) -> Result<(), WriteError> {
        self.guarded(
            sqlx::AssertSqlSafe(
                "INSERT INTO external_id (source, external_id, object_id, object_type) VALUES ($1, $2, $3, $4)"
                    .to_owned(),
            ),
            vec![
                Bind::Text(source.to_owned()),
                Bind::Text(external_id.to_owned()),
                Bind::Uuid(object),
                Bind::Text(kind.name().to_owned()),
            ],
            object,
        )
        .await
        .map(|_| ())
    }

    async fn insert_anchor(
        &mut self,
        kind: ObjectKind,
        id: Uuid,
        slug: &str,
        parent: Option<Uuid>,
        period: Option<Period>,
        changeset: Uuid,
    ) -> Result<(), WriteError> {
        let (a, b) = bounds(period)?;
        let sql = format!(
            "INSERT INTO {} (id, slug, parent_id, valid_period, changeset_id) VALUES ($1, $2, $3, {}, $6)",
            table(kind),
            period_sql(4, 5)
        );
        let binds = vec![
            Bind::Uuid(id),
            Bind::Text(slug.to_owned()),
            Bind::OptUuid(parent),
            Bind::Time(a),
            Bind::Time(b),
            Bind::Uuid(changeset),
        ];
        self.guarded(sqlx::AssertSqlSafe(sql), binds, id)
            .await
            .map(|_| ())
    }

    async fn missing_parents(
        &mut self,
        kind: ObjectKind,
        id: Uuid,
        period: Option<Period>,
    ) -> Result<Vec<Uuid>, StoreError> {
        let (a, b) = bounds(period)?;
        let t = table(kind);
        let sql = format!(
            "SELECT DISTINCT c.parent_id FROM {t} c
             WHERE c.id = $1 AND c.parent_id IS NOT NULL AND c.valid_period && {w}
               AND NOT coalesce((SELECT range_agg(p.valid_period) FROM {t} p WHERE p.id = c.parent_id)
                                @> (c.valid_period * {w}), false)",
            w = period_sql(2, 3)
        );
        sqlx::query_scalar(sqlx::AssertSqlSafe(sql))
            .bind(id)
            .bind(a)
            .bind(b)
            .fetch_all(&mut *self.tx)
            .await
            .map_err(store_err)
    }

    async fn in_cycle(
        &mut self,
        kind: ObjectKind,
        id: Uuid,
        period: Option<Period>,
    ) -> Result<bool, StoreError> {
        let (a, b) = bounds(period)?;
        let t = table(kind);
        // Every parent link over the period, whatever its instant: conservative.
        let sql = format!(
            "WITH RECURSIVE up(id) AS (
                 SELECT parent_id FROM {t} WHERE id = $1 AND parent_id IS NOT NULL AND valid_period && {w}
                 UNION
                 SELECT x.parent_id FROM {t} x JOIN up ON x.id = up.id
                 WHERE x.parent_id IS NOT NULL AND x.valid_period && {w})
             SELECT EXISTS (SELECT 1 FROM up WHERE id = $1)",
            w = period_sql(2, 3)
        );
        sqlx::query_scalar(sqlx::AssertSqlSafe(sql))
            .bind(id)
            .bind(a)
            .bind(b)
            .fetch_one(&mut *self.tx)
            .await
            .map_err(store_err)
    }

    async fn children(
        &mut self,
        kind: ObjectKind,
        id: Uuid,
        period: Option<Period>,
    ) -> Result<Vec<Uuid>, StoreError> {
        let (a, _) = bounds(period)?;
        let sql = format!(
            "SELECT DISTINCT id FROM {} WHERE parent_id = $1
               AND valid_period && tstzrange(coalesce($2::timestamptz, now()), NULL) ORDER BY id",
            table(kind)
        );
        sqlx::query_scalar(sqlx::AssertSqlSafe(sql))
            .bind(id)
            .bind(a)
            .fetch_all(&mut *self.tx)
            .await
            .map_err(store_err)
    }

    async fn containers_carrying(
        &mut self,
        key: &str,
        prefixes: &[Uuid],
        period: Option<Period>,
    ) -> Result<Vec<Uuid>, StoreError> {
        let (a, b) = bounds(period)?;
        let sql = format!(
            "SELECT DISTINCT p.id FROM prefix p
             JOIN property_assignment a ON a.object_id = p.id AND a.valid_period && p.valid_period
             JOIN property_declaration d ON d.id = a.property_id AND d.key = $1
             WHERE p.id = ANY($2) AND p.nature = 'container'
               AND (p.valid_period * a.valid_period) && {} ORDER BY p.id",
            period_sql(3, 4)
        );
        sqlx::query_scalar(sqlx::AssertSqlSafe(sql))
            .bind(key)
            .bind(prefixes)
            .bind(a)
            .bind(b)
            .fetch_all(&mut *self.tx)
            .await
            .map_err(store_err)
    }

    async fn prefix_versions(
        &mut self,
        id: Uuid,
        period: Option<Period>,
    ) -> Result<Vec<(PrefixObject, bool)>, StoreError> {
        let (a, b) = bounds(period)?;
        let sql = format!(
            "SELECT domain_id, prefix::text, nature, point_to_point, anycast, public FROM prefix
             WHERE id = $1 AND valid_period && {} ORDER BY lower(valid_period)",
            period_sql(2, 3)
        );
        let rows: Vec<(Uuid, String, String, bool, bool, bool)> =
            sqlx::query_as(sqlx::AssertSqlSafe(sql))
                .bind(id)
                .bind(a)
                .bind(b)
                .fetch_all(&mut *self.tx)
                .await
                .map_err(store_err)?;
        rows.into_iter()
            .map(|(domain, text, nature, point_to_point, anycast, public)| {
                let nature = match nature.as_str() {
                    "container" => Nature::Container,
                    "network" => Nature::Network,
                    other => return Err(store_err(format!("unknown nature {other}"))),
                };
                Ok((
                    PrefixObject {
                        id,
                        domain,
                        value: Prefix::parse_any(&ALL, &text).map_err(store_err)?,
                        nature,
                        point_to_point,
                        anycast,
                    },
                    public,
                ))
            })
            .collect()
    }

    async fn address_versions(
        &mut self,
        id: Uuid,
        period: Option<Period>,
    ) -> Result<Vec<(Address, bool)>, StoreError> {
        let (a, b) = bounds(period)?;
        let sql = format!(
            "SELECT domain_id, host(address), usage, public FROM address
             WHERE id = $1 AND valid_period && {} ORDER BY lower(valid_period)",
            period_sql(2, 3)
        );
        let rows: Vec<(Uuid, String, String, bool)> = sqlx::query_as(sqlx::AssertSqlSafe(sql))
            .bind(id)
            .bind(a)
            .bind(b)
            .fetch_all(&mut *self.tx)
            .await
            .map_err(store_err)?;
        rows.into_iter()
            .map(|(domain, value, usage, public)| {
                Ok((
                    Address {
                        id,
                        domain,
                        value: addr(&value)?,
                        usage: address_usage(&usage)?,
                    },
                    public,
                ))
            })
            .collect()
    }

    async fn range_versions(
        &mut self,
        id: Uuid,
        period: Option<Period>,
    ) -> Result<Vec<IpRange>, StoreError> {
        let (a, b) = bounds(period)?;
        let sql = format!(
            "SELECT domain_id, host(lower(range)), host(upper(range)), usage FROM ip_range
             WHERE id = $1 AND valid_period && {} ORDER BY lower(valid_period)",
            period_sql(2, 3)
        );
        let rows: Vec<(Uuid, String, String, String)> = sqlx::query_as(sqlx::AssertSqlSafe(sql))
            .bind(id)
            .bind(a)
            .bind(b)
            .fetch_all(&mut *self.tx)
            .await
            .map_err(store_err)?;
        rows.into_iter()
            .map(|(domain, first, last, usage)| {
                Ok(IpRange {
                    id,
                    domain,
                    first: addr(&first)?,
                    last: addr(&last)?,
                    usage: range_usage(&usage)?,
                })
            })
            .collect()
    }

    async fn networks_containing(
        &mut self,
        domain: Uuid,
        prefix: Prefix,
        include_equal: bool,
        period: Option<Period>,
    ) -> Result<Vec<PrefixObject>, StoreError> {
        let (a, b) = bounds(period)?;
        let op = if include_equal { ">>=" } else { ">>" };
        let sql = format!(
            "SELECT DISTINCT ON (id) id, domain_id, prefix::text, point_to_point, anycast FROM prefix
             WHERE domain_id = $1 AND nature = 'network' AND prefix {op} $2::cidr
               AND valid_period && {}",
            period_sql(3, 4)
        );
        let rows: Vec<NetworkRow> = sqlx::query_as(sqlx::AssertSqlSafe(sql))
            .bind(domain)
            .bind(prefix.to_string())
            .bind(a)
            .bind(b)
            .fetch_all(&mut *self.tx)
            .await
            .map_err(store_err)?;
        rows.into_iter().map(network).collect()
    }

    async fn prefixes_inside(
        &mut self,
        domain: Uuid,
        prefix: Prefix,
        period: Option<Period>,
    ) -> Result<Vec<Uuid>, StoreError> {
        let (a, b) = bounds(period)?;
        let sql = format!(
            "SELECT DISTINCT id FROM prefix WHERE domain_id = $1 AND prefix << $2::cidr AND valid_period && {}",
            period_sql(3, 4)
        );
        sqlx::query_scalar(sqlx::AssertSqlSafe(sql))
            .bind(domain)
            .bind(prefix.to_string())
            .bind(a)
            .bind(b)
            .fetch_all(&mut *self.tx)
            .await
            .map_err(store_err)
    }

    async fn address_covered(&mut self, address: Uuid) -> Result<bool, StoreError> {
        // Every version of the address must be covered by the union of the periods of the
        // networks of its domain that contain it (multirange containment).
        sqlx::query_scalar(
            "SELECT coalesce(bool_and(coalesce((
                 SELECT range_agg(n.valid_period) FROM prefix n
                 WHERE n.domain_id = a.domain_id AND n.nature = 'network' AND n.prefix >>= a.address
               ), '{}'::tstzmultirange) @> a.valid_period), true)
             FROM address a WHERE a.id = $1",
        )
        .bind(address)
        .fetch_one(&mut *self.tx)
        .await
        .map_err(store_err)
    }

    async fn addresses_inside(
        &mut self,
        domain: Uuid,
        prefix: Prefix,
        period: Option<Period>,
    ) -> Result<Vec<Uuid>, StoreError> {
        let (a, b) = bounds(period)?;
        let sql = format!(
            "SELECT DISTINCT id FROM address WHERE domain_id = $1 AND address <<= $2::cidr AND valid_period && {}",
            period_sql(3, 4)
        );
        sqlx::query_scalar(sqlx::AssertSqlSafe(sql))
            .bind(domain)
            .bind(prefix.to_string())
            .bind(a)
            .bind(b)
            .fetch_all(&mut *self.tx)
            .await
            .map_err(store_err)
    }

    async fn public_networks_overlapping(
        &mut self,
        domain: Uuid,
        prefix: Prefix,
        period: Option<Period>,
    ) -> Result<Vec<PrefixObject>, StoreError> {
        let (a, b) = bounds(period)?;
        let sql = format!(
            "SELECT DISTINCT ON (id) id, domain_id, prefix::text, point_to_point, anycast FROM prefix
             WHERE public AND nature = 'network' AND domain_id <> $1 AND prefix && $2::cidr
               AND valid_period && {}",
            period_sql(3, 4)
        );
        let rows: Vec<NetworkRow> = sqlx::query_as(sqlx::AssertSqlSafe(sql))
            .bind(domain)
            .bind(prefix.to_string())
            .bind(a)
            .bind(b)
            .fetch_all(&mut *self.tx)
            .await
            .map_err(store_err)?;
        rows.into_iter().map(network).collect()
    }

    async fn public_addresses_equal(
        &mut self,
        domain: Uuid,
        address: Addr,
        period: Option<Period>,
    ) -> Result<Vec<(Uuid, AddressUsage)>, StoreError> {
        let (a, b) = bounds(period)?;
        let sql = format!(
            "SELECT DISTINCT id, usage FROM address
             WHERE public AND domain_id <> $1 AND address = $2::inet AND valid_period && {}",
            period_sql(3, 4)
        );
        let rows: Vec<(Uuid, String)> = sqlx::query_as(sqlx::AssertSqlSafe(sql))
            .bind(domain)
            .bind(address.to_string())
            .bind(a)
            .bind(b)
            .fetch_all(&mut *self.tx)
            .await
            .map_err(store_err)?;
        rows.into_iter()
            .map(|(id, usage)| Ok((id, address_usage(&usage)?)))
            .collect()
    }

    async fn insert_range(
        &mut self,
        r: &IpRange,
        public: bool,
        period: Option<Period>,
        changeset: Uuid,
    ) -> Result<(), WriteError> {
        let (a, b) = bounds(period)?;
        let sql = format!(
            "INSERT INTO ip_range (id, domain_id, range, usage, public, registry_version, valid_period, changeset_id)
             VALUES ($1, $2, inetrange($3::inet, $4::inet, '[]'), $5, $6, $7, {}, $10)",
            period_sql(8, 9)
        );
        let binds = vec![
            Bind::Uuid(r.id),
            Bind::Uuid(r.domain),
            Bind::Text(r.first.to_string()),
            Bind::Text(r.last.to_string()),
            Bind::Text(r.usage.as_str().to_owned()),
            Bind::Bool(public),
            Bind::Text(kartenn_core::registry::UPDATED.to_owned()),
            Bind::Time(a),
            Bind::Time(b),
            Bind::Uuid(changeset),
        ];
        self.guarded(sqlx::AssertSqlSafe(sql), binds, r.id)
            .await
            .map(|_| ())
    }

    async fn range_covered(&mut self, range: Uuid) -> Result<bool, StoreError> {
        // As for addresses (multirange containment); networks never nest, so the networks
        // holding both bounds at an instant are a single one.
        sqlx::query_scalar(
            "SELECT coalesce(bool_and(coalesce((
                 SELECT range_agg(n.valid_period) FROM prefix n
                 WHERE n.domain_id = r.domain_id AND n.nature = 'network'
                   AND n.prefix >>= lower(r.range) AND n.prefix >>= upper(r.range)
               ), '{}'::tstzmultirange) @> r.valid_period), true)
             FROM ip_range r WHERE r.id = $1",
        )
        .bind(range)
        .fetch_one(&mut *self.tx)
        .await
        .map_err(store_err)
    }

    async fn ranges_overlapping(
        &mut self,
        domain: Uuid,
        first: Addr,
        last: Addr,
        period: Option<Period>,
    ) -> Result<Vec<IpRange>, StoreError> {
        let (a, b) = bounds(period)?;
        let sql = format!(
            "SELECT DISTINCT ON (id) id, host(lower(range)), host(upper(range)), usage FROM ip_range
             WHERE domain_id = $1 AND range && inetrange($2::inet, $3::inet, '[]')
               AND valid_period && {}",
            period_sql(4, 5)
        );
        let rows: Vec<(Uuid, String, String, String)> = sqlx::query_as(sqlx::AssertSqlSafe(sql))
            .bind(domain)
            .bind(first.to_string())
            .bind(last.to_string())
            .bind(a)
            .bind(b)
            .fetch_all(&mut *self.tx)
            .await
            .map_err(store_err)?;
        rows.into_iter()
            .map(|(id, first, last, usage)| {
                Ok(IpRange {
                    id,
                    domain,
                    first: addr(&first)?,
                    last: addr(&last)?,
                    usage: range_usage(&usage)?,
                })
            })
            .collect()
    }

    async fn ranges_inside(
        &mut self,
        domain: Uuid,
        prefix: Prefix,
        period: Option<Period>,
    ) -> Result<Vec<Uuid>, StoreError> {
        let (a, b) = bounds(period)?;
        let sql = format!(
            "SELECT DISTINCT id FROM ip_range
             WHERE domain_id = $1 AND lower(range) <<= $2::cidr AND upper(range) <<= $2::cidr
               AND valid_period && {}",
            period_sql(3, 4)
        );
        sqlx::query_scalar(sqlx::AssertSqlSafe(sql))
            .bind(domain)
            .bind(prefix.to_string())
            .bind(a)
            .bind(b)
            .fetch_all(&mut *self.tx)
            .await
            .map_err(store_err)
    }

    async fn addresses_between(
        &mut self,
        domain: Uuid,
        first: Addr,
        last: Addr,
        period: Option<Period>,
    ) -> Result<Vec<Address>, StoreError> {
        let (a, b) = bounds(period)?;
        let sql = format!(
            "SELECT DISTINCT ON (id) id, host(address), usage FROM address
             WHERE domain_id = $1 AND address BETWEEN $2::inet AND $3::inet
               AND family(address) = family($2::inet) AND valid_period && {}",
            period_sql(4, 5)
        );
        let rows: Vec<(Uuid, String, String)> = sqlx::query_as(sqlx::AssertSqlSafe(sql))
            .bind(domain)
            .bind(first.to_string())
            .bind(last.to_string())
            .bind(a)
            .bind(b)
            .fetch_all(&mut *self.tx)
            .await
            .map_err(store_err)?;
        rows.into_iter()
            .map(|(id, value, usage)| {
                Ok(Address {
                    id,
                    domain,
                    value: addr(&value)?,
                    usage: address_usage(&usage)?,
                })
            })
            .collect()
    }

    async fn prefix_of(&mut self, id: Uuid) -> Result<Option<(Uuid, Prefix)>, StoreError> {
        let row: Option<(Uuid, String)> = sqlx::query_as(
            "SELECT domain_id, prefix::text FROM prefix WHERE id = $1
             UNION ALL SELECT domain_id, prefix::text FROM prefix_history WHERE id = $1 LIMIT 1",
        )
        .bind(id)
        .fetch_optional(&mut *self.tx)
        .await
        .map_err(store_err)?;
        row.map(|(domain, text)| Ok((domain, Prefix::parse_any(&ALL, &text).map_err(store_err)?)))
            .transpose()
    }

    async fn insert_declaration(
        &mut self,
        id: Uuid,
        declaration: &Declaration,
        period: Option<Period>,
        changeset: Uuid,
    ) -> Result<(), WriteError> {
        let (a, b) = bounds(period)?;
        self.write_declaration(id, declaration, a, Bind::Time(b), changeset)
            .await?;
        Ok(self
            .set_obligations(id, &declaration.obligations, a, b, changeset)
            .await?)
    }

    async fn declaration(
        &mut self,
        id: Uuid,
        period: Option<Period>,
    ) -> Result<Option<Declaration>, StoreError> {
        let (at, _) = bounds(period)?;
        let row: Option<DeclarationRow> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
            "SELECT {DECLARATION_COLUMNS} FROM property_declaration
             WHERE id = $1 AND valid_period @> coalesce($2::timestamptz, now())"
        )))
        .bind(id)
        .bind(at)
        .fetch_optional(&mut *self.tx)
        .await
        .map_err(store_err)?;
        let Some(mut declaration) = row.map(decode_declaration).transpose()? else {
            return Ok(None);
        };
        declaration.obligations = self.obligations_at(id, at).await?;
        Ok(Some(declaration))
    }

    async fn replace_declaration(
        &mut self,
        id: Uuid,
        declaration: &Declaration,
        period: Option<Period>,
        changeset: Uuid,
    ) -> Result<(), WriteError> {
        let (at, _) = bounds(period)?;
        let version: Option<(bool, Option<OffsetDateTime>)> = sqlx::query_as(
            "SELECT lower(valid_period) = coalesce($2::timestamptz, now()), upper(valid_period)
             FROM property_declaration WHERE id = $1 AND valid_period @> coalesce($2::timestamptz, now())",
        )
        .bind(id)
        .bind(at)
        .fetch_optional(&mut *self.tx)
        .await
        .map_err(store_err)?;
        let Some((starts_here, end)) = version else {
            return Err(WriteError::NoVersion(id));
        };
        // A version starting at the same instant is replaced; an older one is shortened
        // and followed by the new version up to its former end.
        let shorten = if starts_here {
            "DELETE FROM property_declaration
             WHERE id = $1 AND lower(valid_period) = coalesce($2::timestamptz, now())"
        } else {
            "UPDATE property_declaration
             SET valid_period = tstzrange(lower(valid_period), coalesce($2::timestamptz, now())),
                 changeset_id = $3
             WHERE id = $1 AND valid_period @> coalesce($2::timestamptz, now())"
        };
        let mut binds = vec![Bind::Uuid(id), Bind::Time(at)];
        if !starts_here {
            binds.push(Bind::Uuid(changeset));
        }
        self.guarded(sqlx::AssertSqlSafe(shorten.to_owned()), binds, id)
            .await?;
        self.write_declaration(id, declaration, at, Bind::Time(end), changeset)
            .await?;
        Ok(self
            .set_obligations(id, &declaration.obligations, at, end, changeset)
            .await?)
    }

    async fn objects_without_value(
        &mut self,
        property: Uuid,
        carriers: &[String],
        heritable: bool,
        scope: &ObligationScope,
        candidates: Option<&[Uuid]>,
        period: Option<Period>,
    ) -> Result<Vec<Uuid>, StoreError> {
        let (a, _) = bounds(period)?;
        self.without_value(property, carriers, heritable, scope, candidates, a)
            .await
    }

    async fn dangling_references(&mut self, target: Uuid) -> Result<Vec<Uuid>, StoreError> {
        sqlx::query_scalar(
            "WITH life AS (
                 SELECT range_agg(valid_period) AS m FROM (
                     SELECT valid_period FROM addressing_object WHERE id = $1
                     UNION ALL SELECT valid_period FROM entity WHERE id = $1
                     UNION ALL SELECT valid_period FROM site WHERE id = $1) x)
             SELECT DISTINCT a.object_id FROM property_assignment a
             JOIN property_declaration d ON d.id = a.property_id AND d.value_type = 'ref'
             WHERE a.kind = 'value' AND (a.v_uuid = $1 OR $1 = ANY(a.v_uuids))
               AND NOT coalesce((SELECT m FROM life) @> a.valid_period, false)
             ORDER BY 1",
        )
        .bind(target)
        .fetch_all(&mut *self.tx)
        .await
        .map_err(store_err)
    }

    async fn obligated(
        &mut self,
        period: Option<Period>,
    ) -> Result<Vec<(Uuid, Declaration)>, StoreError> {
        let (a, _) = bounds(period)?;
        let ids: Vec<Uuid> = sqlx::query_scalar(
            "SELECT DISTINCT property_id FROM property_obligation
             WHERE valid_period @> coalesce($1::timestamptz, now())",
        )
        .bind(a)
        .fetch_all(&mut *self.tx)
        .await
        .map_err(store_err)?;
        let mut out = Vec::with_capacity(ids.len());
        for id in ids {
            if let Some(d) = self.declaration(id, period).await? {
                out.push((id, d));
            }
        }
        Ok(out)
    }

    async fn declarations_in(
        &mut self,
        namespace: &str,
    ) -> Result<Vec<(Uuid, Declaration, Vec<String>)>, StoreError> {
        let ids: Vec<Uuid> = sqlx::query_scalar(
            "SELECT id FROM property_declaration
             WHERE split_part(key, '.', 1) = $1 AND valid_period @> now() ORDER BY key",
        )
        .bind(namespace)
        .fetch_all(&mut *self.tx)
        .await
        .map_err(store_err)?;
        let mut out = Vec::with_capacity(ids.len());
        for id in ids {
            let Some(declaration) = self.declaration(id, None).await? else {
                continue;
            };
            let keys: Vec<String> = sqlx::query_scalar(
                "SELECT key FROM property_enum_value
                 WHERE property_id = $1 AND valid_period @> now() ORDER BY key",
            )
            .bind(id)
            .fetch_all(&mut *self.tx)
            .await
            .map_err(store_err)?;
            out.push((id, declaration, keys));
        }
        Ok(out)
    }

    async fn insert_enum_value(
        &mut self,
        id: Uuid,
        property: Uuid,
        key: &str,
        period: Option<Period>,
        changeset: Uuid,
    ) -> Result<(), WriteError> {
        let (a, b) = bounds(period)?;
        let sql = format!(
            "INSERT INTO property_enum_value (id, property_id, key, valid_period, changeset_id)
             VALUES ($1, $2, $3, {}, $6)",
            period_sql(4, 5)
        );
        let binds = vec![
            Bind::Uuid(id),
            Bind::Uuid(property),
            Bind::Text(key.to_owned()),
            Bind::Time(a),
            Bind::Time(b),
            Bind::Uuid(changeset),
        ];
        self.guarded(sqlx::AssertSqlSafe(sql), binds, id)
            .await
            .map(|_| ())
    }

    async fn close_enum_values_of(
        &mut self,
        property: Uuid,
        period: Option<Period>,
        changeset: Uuid,
    ) -> Result<(), StoreError> {
        let (at, _) = bounds(period)?;
        sqlx::query(
            "UPDATE property_enum_value
             SET valid_period = tstzrange(lower(valid_period), coalesce($2::timestamptz, now())),
                 changeset_id = $3
             WHERE property_id = $1 AND valid_period @> coalesce($2::timestamptz, now())
               AND lower(valid_period) < coalesce($2::timestamptz, now())",
        )
        .bind(property)
        .bind(at)
        .bind(changeset)
        .execute(&mut *self.tx)
        .await
        .map_err(store_err)?;
        sqlx::query(
            "DELETE FROM property_enum_value
             WHERE property_id = $1 AND lower(valid_period) >= coalesce($2::timestamptz, now())",
        )
        .bind(property)
        .bind(at)
        .execute(&mut *self.tx)
        .await
        .map_err(store_err)?;
        Ok(())
    }

    async fn property_facts(
        &mut self,
        property: Uuid,
        type_id: &str,
        period: Option<Period>,
    ) -> Result<PropertyFacts, StoreError> {
        let (a, b) = bounds(period)?;
        let rows: Vec<AssignmentRow> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
            "SELECT DISTINCT ON (object_id) {ASSIGNMENT_COLUMNS} FROM property_assignment
             WHERE property_id = $1 AND valid_period && {}",
            period_sql(2, 3)
        )))
        .bind(property)
        .bind(a)
        .bind(b)
        .fetch_all(&mut *self.tx)
        .await
        .map_err(store_err)?;
        let assignments = rows
            .into_iter()
            .map(|r| decode_assignment(type_id, r))
            .collect::<Result<Vec<_>, _>>()?;
        let inheriting: Vec<Uuid> = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
            "SELECT DISTINCT object_id FROM property_origin
             WHERE property_id = $1 AND origin_object_id IS NOT NULL
               AND origin_object_id <> object_id AND valid_period && {}",
            period_sql(2, 3)
        )))
        .bind(property)
        .bind(a)
        .bind(b)
        .fetch_all(&mut *self.tx)
        .await
        .map_err(store_err)?;
        let enum_values: Vec<(String, Uuid)> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
            "SELECT DISTINCT ON (key) key, id FROM property_enum_value
             WHERE property_id = $1 AND valid_period && {}",
            period_sql(2, 3)
        )))
        .bind(property)
        .bind(a)
        .bind(b)
        .fetch_all(&mut *self.tx)
        .await
        .map_err(store_err)?;
        let kinds: Vec<(Uuid, String)> = sqlx::query_as(
            "SELECT id, kind FROM (
                 SELECT id, 'entity' AS kind FROM entity UNION ALL SELECT id, 'site' FROM site
                 UNION ALL SELECT id, 'domain' FROM addressing_domain
                 UNION ALL SELECT id, 'prefix' FROM prefix UNION ALL SELECT id, 'range' FROM ip_range
                 UNION ALL SELECT id, 'address' FROM address) o
             WHERE id = ANY($1)",
        )
        .bind(referenced(&assignments))
        .fetch_all(&mut *self.tx)
        .await
        .map_err(store_err)?;
        Ok(PropertyFacts {
            assignments,
            inheriting,
            enum_values: enum_values.into_iter().collect(),
            ref_kinds: kinds
                .into_iter()
                .filter_map(|(id, kind)| ref_kind(&kind).map(|k| (id, k)))
                .collect(),
        })
    }

    async fn objects_assigned(
        &mut self,
        property: Uuid,
        period: Option<Period>,
    ) -> Result<Vec<Uuid>, StoreError> {
        let (a, b) = bounds(period)?;
        sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
            "SELECT DISTINCT object_id FROM property_assignment
             WHERE property_id = $1 AND valid_period && {}",
            period_sql(2, 3)
        )))
        .bind(property)
        .bind(a)
        .bind(b)
        .fetch_all(&mut *self.tx)
        .await
        .map_err(store_err)
    }

    async fn objects_carrying(
        &mut self,
        enum_value: Uuid,
        period: Option<Period>,
    ) -> Result<Vec<Uuid>, StoreError> {
        let (a, b) = bounds(period)?;
        sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
            "SELECT DISTINCT object_id FROM property_assignment
             WHERE (v_uuid = $1 OR $1 = ANY(v_uuids)) AND valid_period && {}",
            period_sql(2, 3)
        )))
        .bind(enum_value)
        .bind(a)
        .bind(b)
        .fetch_all(&mut *self.tx)
        .await
        .map_err(store_err)
    }

    async fn object_kind_covering(
        &mut self,
        id: Uuid,
        period: Option<Period>,
    ) -> Result<Option<&'static str>, StoreError> {
        let (a, b) = bounds(period)?;
        let kind: Option<String> = sqlx::query_scalar(
            "SELECT kind FROM (
                 SELECT id, kind, valid_period FROM addressing_object
                 UNION ALL SELECT id, 'entity', valid_period FROM entity
                 UNION ALL SELECT id, 'site', valid_period FROM site) o
             WHERE id = $1 GROUP BY kind
             HAVING range_agg(valid_period) @> tstzrange(coalesce($2::timestamptz, now()), $3::timestamptz)",
        )
        .bind(id)
        .bind(a)
        .bind(b)
        .fetch_optional(&mut *self.tx)
        .await
        .map_err(store_err)?;
        Ok(kind.as_deref().and_then(ref_kind))
    }

    async fn enum_values_covering(
        &mut self,
        property: Uuid,
        keys: &[String],
        period: Option<Period>,
    ) -> Result<HashMap<String, Uuid>, StoreError> {
        let (a, b) = bounds(period)?;
        let rows: Vec<(String, Uuid)> = sqlx::query_as(
            "SELECT key, id FROM property_enum_value
             WHERE property_id = $1 AND key = ANY($2) GROUP BY key, id
             HAVING range_agg(valid_period) @> tstzrange(coalesce($3::timestamptz, now()), $4::timestamptz)",
        )
        .bind(property)
        .bind(keys)
        .bind(a)
        .bind(b)
        .fetch_all(&mut *self.tx)
        .await
        .map_err(store_err)?;
        Ok(rows.into_iter().collect())
    }

    async fn assign(
        &mut self,
        object: Uuid,
        object_type: &str,
        property: Uuid,
        assigned: &Assigned,
        provenance: &str,
        period: Option<Period>,
        changeset: Uuid,
    ) -> Result<(), WriteError> {
        let (a, b) = bounds(period)?;
        let (kind, c) = match assigned {
            Assigned::Cleared => ("cleared", ValueColumns::default()),
            Assigned::Value(v) => (
                "value",
                encode_value(v).map_err(|e| {
                    WriteError::Rule(
                        Violation::new(&rules::PROP_INVALID_VALUE, e).with_subjects([object]),
                    )
                })?,
            ),
        };
        self.unassign(object, Some(property), period, changeset)
            .await?;
        let sql = format!(
            "INSERT INTO property_assignment (object_id, object_type, property_id, kind,
                 v_uuid, v_uuids, v_text, v_texts, v_int, v_ints, v_bool, v_date, v_dates,
                 provenance, valid_period, changeset_id)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12::date, $13::date[],
                 $17, {}, $16)",
            period_sql(14, 15)
        );
        let binds = vec![
            Bind::Uuid(object),
            Bind::Text(object_type.to_owned()),
            Bind::Uuid(property),
            Bind::Text(kind.to_owned()),
            Bind::OptUuid(c.uuid),
            Bind::Uuids(c.uuids),
            Bind::OptText(c.text),
            Bind::OptTexts(c.texts),
            Bind::OptInt(c.int),
            Bind::Ints(c.ints),
            Bind::OptBool(c.boolean),
            Bind::OptText(c.date),
            Bind::OptTexts(c.dates),
            Bind::Time(a),
            Bind::Time(b),
            Bind::Uuid(changeset),
            Bind::Text(provenance.to_owned()),
        ];
        self.guarded(sqlx::AssertSqlSafe(sql), binds, object)
            .await
            .map(|_| ())
    }

    async fn object_view(
        &mut self,
        object: Uuid,
        period: Option<Period>,
    ) -> Result<Option<ObjectView>, StoreError> {
        let (a, _) = bounds(period)?;
        self.view_of(object, a).await
    }

    async fn resolved_value(
        &mut self,
        object: Uuid,
        property: Uuid,
        declaration: &Declaration,
        period: Option<Period>,
    ) -> Result<Option<Value>, StoreError> {
        let (a, _) = bounds(period)?;
        self.resolved_at(object, property, declaration, a).await
    }

    async fn source_version(&mut self, source: &str) -> Result<Option<String>, StoreError> {
        sqlx::query_scalar("SELECT version FROM expected_source_state WHERE source = $1")
            .bind(source)
            .fetch_optional(&mut *self.tx)
            .await
            .map_err(store_err)
    }

    async fn set_source_version(&mut self, source: &str, version: &str) -> Result<(), StoreError> {
        sqlx::query(
            "INSERT INTO expected_source_state (source, version) VALUES ($1, $2)
             ON CONFLICT (source) DO UPDATE SET version = EXCLUDED.version, evaluated_at = now()",
        )
        .bind(source)
        .bind(version)
        .execute(&mut *self.tx)
        .await
        .map_err(store_err)?;
        Ok(())
    }

    async fn with_expected(
        &mut self,
        period: Option<Period>,
    ) -> Result<Vec<(Uuid, Declaration)>, StoreError> {
        let (a, _) = bounds(period)?;
        let ids: Vec<Uuid> = sqlx::query_scalar(
            "SELECT id FROM property_declaration
             WHERE expected_source IS NOT NULL AND valid_period @> coalesce($1::timestamptz, now())",
        )
        .bind(a)
        .fetch_all(&mut *self.tx)
        .await
        .map_err(store_err)?;
        let mut out = Vec::with_capacity(ids.len());
        for id in ids {
            if let Some(d) = self.declaration(id, period).await? {
                out.push((id, d));
            }
        }
        Ok(out)
    }

    async fn unassign(
        &mut self,
        object: Uuid,
        property: Option<Uuid>,
        period: Option<Period>,
        changeset: Uuid,
    ) -> Result<(), StoreError> {
        let (a, b) = bounds(period)?;
        // Without a property, the object is closed: every assignment ends at the start.
        let (properties, end) = match property {
            Some(p) => (vec![p], b),
            None => (
                sqlx::query_scalar(
                    "SELECT DISTINCT property_id FROM property_assignment WHERE object_id = $1",
                )
                .bind(object)
                .fetch_all(&mut *self.tx)
                .await
                .map_err(store_err)?,
                None,
            ),
        };
        for p in properties {
            sqlx::query(
                "SELECT kartenn_unassign($1, $2, coalesce($3::timestamptz, now()), $4::timestamptz, $5)",
            )
            .bind(object)
            .bind(p)
            .bind(a)
            .bind(end)
            .bind(changeset)
            .execute(&mut *self.tx)
            .await
            .map_err(store_err)?;
        }
        Ok(())
    }

    async fn addressing_heritable(
        &mut self,
        period: Option<Period>,
    ) -> Result<Vec<(Uuid, Vec<String>)>, StoreError> {
        let (a, _) = bounds(period)?;
        sqlx::query_as(
            "SELECT id, carriers FROM property_declaration
             WHERE inherit_tree = 'addressing' AND valid_period @> coalesce($1::timestamptz, now())",
        )
        .bind(a)
        .fetch_all(&mut *self.tx)
        .await
        .map_err(store_err)
    }

    async fn subtree(
        &mut self,
        object: Uuid,
        period: Option<Period>,
    ) -> Result<Vec<Uuid>, StoreError> {
        let (a, _) = bounds(period)?;
        sqlx::query_scalar(
            "WITH w AS (SELECT tstzrange(coalesce($2::timestamptz, now()), NULL) AS r),
                  o AS (SELECT DISTINCT domain_id, value, kind FROM addressing_object, w
                        WHERE id = $1 AND valid_period && w.r)
             -- three branches rather than an OR, so that each uses its index
             SELECT x.id FROM addressing_object x, w WHERE x.id = $1 AND x.valid_period && w.r
             UNION
             SELECT x.id FROM addressing_object x, o, w
             WHERE o.kind = 'domain' AND x.domain_id = o.domain_id AND x.valid_period && w.r
             UNION
             SELECT x.id FROM addressing_object x, o, w
             WHERE o.kind = 'prefix' AND x.domain_id = o.domain_id AND x.kind <> 'domain'
               AND x.value <<= o.value AND x.valid_period && w.r",
        )
        .bind(object)
        .bind(a)
        .fetch_all(&mut *self.tx)
        .await
        .map_err(store_err)
    }

    async fn objects_of_kinds(
        &mut self,
        kinds: &[String],
        period: Option<Period>,
    ) -> Result<Vec<Uuid>, StoreError> {
        let (a, _) = bounds(period)?;
        sqlx::query_scalar(
            "SELECT DISTINCT id FROM addressing_object
             WHERE kind = ANY($1) AND valid_period && tstzrange(coalesce($2::timestamptz, now()), NULL)",
        )
        .bind(kinds)
        .bind(a)
        .fetch_all(&mut *self.tx)
        .await
        .map_err(store_err)
    }

    async fn inheritors(
        &mut self,
        origin: Uuid,
        period: Option<Period>,
    ) -> Result<Vec<(Uuid, Uuid)>, StoreError> {
        let (a, _) = bounds(period)?;
        sqlx::query_as(
            "SELECT DISTINCT property_id, object_id FROM property_origin
             WHERE origin_object_id = $1
               AND valid_period && tstzrange(coalesce($2::timestamptz, now()), NULL)",
        )
        .bind(origin)
        .bind(a)
        .fetch_all(&mut *self.tx)
        .await
        .map_err(store_err)
    }

    async fn assignment_presence_changed(
        &mut self,
        object: Uuid,
        property: Uuid,
        period: Option<Period>,
    ) -> Result<bool, StoreError> {
        let (a, _) = bounds(period)?;
        // Before: the versions the transaction did not write, current or replaced by it.
        sqlx::query_scalar(
            "WITH w AS (SELECT tstzrange(coalesce($3::timestamptz, now()), NULL) AS r),
                  b AS (SELECT valid_period FROM property_assignment
                        WHERE object_id = $1 AND property_id = $2 AND sys_start < now()
                        UNION ALL
                        SELECT valid_period FROM property_assignment_history
                        WHERE object_id = $1 AND property_id = $2 AND sys_start < now()
                          AND sys_end = now())
             SELECT coalesce((SELECT range_agg(b.valid_period * w.r) FROM b, w
                              WHERE b.valid_period && w.r), '{}')
                    IS DISTINCT FROM
                    coalesce((SELECT range_agg(a.valid_period * w.r) FROM property_assignment a, w
                              WHERE a.object_id = $1 AND a.property_id = $2
                                AND a.valid_period && w.r), '{}')",
        )
        .bind(object)
        .bind(property)
        .bind(a)
        .fetch_one(&mut *self.tx)
        .await
        .map_err(store_err)
    }

    async fn heirs(
        &mut self,
        property: Uuid,
        origin: Uuid,
        period: Option<Period>,
    ) -> Result<Vec<Uuid>, StoreError> {
        let (a, _) = bounds(period)?;
        sqlx::query_scalar(
            "SELECT DISTINCT object_id FROM property_origin
             WHERE property_id = $1 AND origin_object_id = $2
               AND valid_period && tstzrange(coalesce($3::timestamptz, now()), NULL)",
        )
        .bind(property)
        .bind(origin)
        .bind(a)
        .fetch_all(&mut *self.tx)
        .await
        .map_err(store_err)
    }

    async fn refresh_origins(
        &mut self,
        property: Uuid,
        carriers: &[String],
        targets: &[Uuid],
        period: Option<Period>,
        changeset: Uuid,
    ) -> Result<usize, StoreError> {
        let (a, _) = bounds(period)?;
        self.refresh_origins_from(property, carriers, targets, a, changeset)
            .await
    }

    async fn drop_origins(
        &mut self,
        property: Uuid,
        period: Option<Period>,
        changeset: Uuid,
    ) -> Result<(), StoreError> {
        let (a, _) = bounds(period)?;
        self.drop_origins_from(property, a, changeset).await
    }

    async fn record_diff(
        &mut self,
        property: Uuid,
        objects: &[Uuid],
        heritable: bool,
        excluded: &[Uuid],
        period: Option<Period>,
    ) -> Result<(), StoreError> {
        let (a, _) = bounds(period)?;
        self.record_diff_from(property, objects, heritable, excluded, a)
            .await
    }

    async fn record_value_diff(
        &mut self,
        property: Uuid,
        origins: &[Uuid],
        excluded: &[Uuid],
        period: Option<Period>,
    ) -> Result<(), StoreError> {
        let (a, _) = bounds(period)?;
        self.record_value_diff_from(property, origins, excluded, a)
            .await
    }

    async fn diff_summary(&mut self) -> Result<Vec<DiffCount>, StoreError> {
        self.diff_counts().await
    }

    async fn diff_objects(&mut self, property: &str) -> Result<Vec<Uuid>, StoreError> {
        self.diff_objects_of(property).await
    }

    async fn diff_sample(&mut self, limit: i64) -> Result<Vec<DiffItem>, StoreError> {
        self.diff_first(limit).await
    }

    async fn lock_changeset(&mut self, id: Uuid) -> Result<Option<String>, StoreError> {
        crate::pending::lock(&mut self.tx, id).await
    }

    async fn approval_policies(&mut self) -> Result<Vec<ApprovalPolicy>, StoreError> {
        crate::pending::policies(&mut self.tx).await
    }

    async fn policy_named(&mut self, name: &str) -> Result<Option<Uuid>, StoreError> {
        crate::pending::policy_named(&mut self.tx, name).await
    }

    async fn set_policy(&mut self, policy: &ApprovalPolicy) -> Result<(), WriteError> {
        crate::pending::set_policy(&mut self.tx, policy).await
    }

    async fn journal_applied(&mut self, journal: &Journal<'_>) -> Result<(), StoreError> {
        write_journal(&mut self.tx, journal).await?;
        self.store_diff(journal.id).await?;
        crate::anomalies::record(
            &mut self.tx,
            journal.id,
            journal.reevaluated,
            journal.violations,
        )
        .await
        .map_err(store_err)
    }

    async fn commit(self) -> Result<(), StoreError> {
        self.tx.commit().await.map_err(store_err)
    }

    async fn rollback(self) -> Result<(), StoreError> {
        self.tx.rollback().await.map_err(store_err)
    }
}

/// Operations journaled with a changeset can be read back for replay (pending changesets).
///
/// # Errors
///
/// Database or decoding errors.
pub async fn operations_of(pool: &PgPool, changeset: Uuid) -> Result<Vec<Operation>, StoreError> {
    let payloads: Vec<serde_json::Value> = sqlx::query_scalar(
        "SELECT payload FROM changeset_operation WHERE changeset_id = $1 ORDER BY ordinal",
    )
    .bind(changeset)
    .fetch_all(pool)
    .await
    .map_err(store_err)?;
    payloads
        .into_iter()
        .map(|p| serde_json::from_value(p).map_err(store_err))
        .collect()
}

/// The kind of an object from its stored name.
fn object_kind(name: &str) -> Result<ObjectKind, StoreError> {
    Ok(match name {
        "domain" => ObjectKind::Domain,
        "prefix" => ObjectKind::Prefix,
        "range" => ObjectKind::Range,
        "address" => ObjectKind::Address,
        "entity" => ObjectKind::Entity,
        "site" => ObjectKind::Site,
        other => return Err(store_err(format!("unknown object kind {other}"))),
    })
}
