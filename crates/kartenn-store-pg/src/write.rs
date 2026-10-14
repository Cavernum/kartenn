//! PostgreSQL implementation of the write port of the pipeline (spec 004, ADR 0015).

use kartenn_core::addr::{Addr, Prefix};
use kartenn_core::family::ALL;
use kartenn_core::objects::{Address, AddressUsage, Nature, PrefixObject};
use kartenn_core::period::Period;
use kartenn_core::rules::{self, Rule, Violation};
use kartenn_service::port::{Journal, Replay, Store, StoreError, WriteError, WriteTx};
use kartenn_service::{ChangesetRequest, ObjectKind, Operation};
use sha2::{Digest, Sha256};
use sqlx::types::Uuid;
use sqlx::types::time::OffsetDateTime;
use sqlx::{PgPool, Postgres, Transaction};

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
        "addressing_domain_slug_unique" | "entity_slug_unique" | "site_slug_unique" => {
            &rules::SLUG_DUPLICATE
        }
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
}

impl PgStore {
    /// A store on a pool.
    #[must_use]
    pub const fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

/// A write transaction.
#[derive(Debug)]
pub struct PgTx {
    tx: Transaction<'static, Postgres>,
}

impl PgTx {
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
}

type Binds = Vec<Bind>;

impl Store for PgStore {
    type Tx = PgTx;

    async fn begin(&self) -> Result<PgTx, StoreError> {
        Ok(PgTx {
            tx: self.pool.begin().await.map_err(store_err)?,
        })
    }

    async fn journal_refused(&self, journal: &Journal<'_>) -> Result<(), StoreError> {
        let mut tx = self.pool.begin().await.map_err(store_err)?;
        write_journal(&mut tx, journal).await?;
        tx.commit().await.map_err(store_err)
    }
}

fn table(kind: ObjectKind) -> &'static str {
    match kind {
        ObjectKind::Domain => "addressing_domain",
        ObjectKind::Prefix => "prefix",
        ObjectKind::Address => "address",
    }
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
        "INSERT INTO changeset (id, author_id, motive, state, kind, default_period, retroactive, applied_at, violations, anomalies)
         VALUES ($1, $2, $3, $4, 'direct', tstzrange(coalesce($5::timestamptz, now()), $6::timestamptz),
                 coalesce($5::timestamptz < now(), false), CASE WHEN $4 = 'applied' THEN now() END, $7, $8)",
    )
    .bind(journal.id)
    .bind(journal.request.author)
    .bind(&journal.request.motive)
    .bind(journal.state)
    .bind(start)
    .bind(end)
    .bind(violations_json(&errors))
    .bind(violations_json(&warnings))
    .execute(&mut **tx)
    .await
    .map_err(store_err)?;
    if let Some(key) = &journal.request.idempotency_key {
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
    }
    for (ordinal, op) in journal.request.operations.iter().enumerate() {
        let payload = serde_json::to_value(op).map_err(store_err)?;
        sqlx::query(
            "INSERT INTO changeset_operation (changeset_id, ordinal, payload) VALUES ($1, $2, $3)",
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
            ObjectKind::Address => "SELECT domain_id FROM address WHERE id = $1 LIMIT 1",
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
            "INSERT INTO prefix (id, domain_id, prefix, nature, point_to_point, public, registry_version, valid_period, changeset_id)
             VALUES ($1, $2, $3::cidr, $4, $5, $6, $7, {}, $10)",
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
            "SELECT DISTINCT ON (id) id, prefix::text, point_to_point FROM prefix
             WHERE domain_id = $1 AND nature = 'network' AND prefix {op} $2::cidr
               AND valid_period && {}",
            period_sql(3, 4)
        );
        let rows: Vec<(Uuid, String, bool)> = sqlx::query_as(sqlx::AssertSqlSafe(sql))
            .bind(domain)
            .bind(prefix.to_string())
            .bind(a)
            .bind(b)
            .fetch_all(&mut *self.tx)
            .await
            .map_err(store_err)?;
        rows.into_iter()
            .map(|(id, text, point_to_point)| {
                let value = Prefix::parse_any(&ALL, &text).map_err(store_err)?;
                Ok(PrefixObject {
                    id,
                    domain,
                    value,
                    nature: Nature::Network,
                    point_to_point,
                })
            })
            .collect()
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
    ) -> Result<Vec<(Uuid, Uuid)>, StoreError> {
        let (a, b) = bounds(period)?;
        let sql = format!(
            "SELECT DISTINCT id, domain_id FROM prefix
             WHERE public AND nature = 'network' AND domain_id <> $1 AND prefix && $2::cidr
               AND valid_period && {}",
            period_sql(3, 4)
        );
        sqlx::query_as(sqlx::AssertSqlSafe(sql))
            .bind(domain)
            .bind(prefix.to_string())
            .bind(a)
            .bind(b)
            .fetch_all(&mut *self.tx)
            .await
            .map_err(store_err)
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
            .map(|(id, usage)| {
                let usage = match usage.as_str() {
                    "host" => AddressUsage::Host,
                    "gateway" => AddressUsage::Gateway,
                    "vip" => AddressUsage::Vip,
                    "anycast" => AddressUsage::Anycast,
                    "reserved" => AddressUsage::Reserved,
                    other => return Err(store_err(format!("unknown usage {other}"))),
                };
                Ok((id, usage))
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

    async fn journal_applied(&mut self, journal: &Journal<'_>) -> Result<(), StoreError> {
        write_journal(&mut self.tx, journal).await
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
