//! Cost of access control on the budgets of the core (spec 005, REQ-005-29; task 005-16):
//! the dataset of `bench-core`, and a person of an issuer with 50 groups and 1 000
//! bindings (20 on domains, the others on prefix subtrees and on the identity). Each
//! budget is measured twice on the same draws, alternately: without access control (the
//! system), then with the decision and the filtering of the person; the overhead on the
//! p95 must stay under 10 %.

use std::fmt::Write as _;
use std::time::{Duration, Instant};

use anyhow::{Context, anyhow, bail};
use kartenn_service::audit::RequestContext;
use kartenn_service::authz::{Authorizer, Subject, Unrestricted};
use kartenn_service::expected::Sources;
use kartenn_service::pipeline::MAX_OPERATIONS;
use kartenn_service::{ChangesetRequest, Operation, Outcome, submit_in};
use kartenn_store_pg::filter::{Condition, filter, filter_in};
use kartenn_store_pg::rbac::PgAuthorizer;
use kartenn_store_pg::read::object;
use kartenn_store_pg::tree::{children, children_in, occupancy};
use kartenn_store_pg::write::PgStore;
use rand::{RngExt, SeedableRng};
use rand_chacha::ChaCha8Rng;
use sqlx::PgPool;
use sqlx::types::Uuid;
use sqlx::types::time::OffsetDateTime;

use crate::Dataset;
use crate::core_budgets::{Prepared, new_address, prepare};
use crate::props::{SINCE, p95, progress};

const ISSUER: &str = "https://idp.bench.invalid";
const GROUPS: usize = 50;
const BINDINGS: usize = 1_000;
/// Bindings on the identity itself, the others on its groups.
const OWN: usize = 10;
/// Most overhead of access control on a p95 (REQ-005-29).
const OVERHEAD_MAX: f64 = 0.10;

/// One budget measured both ways.
struct Pair {
    name: &'static str,
    plain: Vec<Duration>,
    controlled: Vec<Duration>,
}

impl Pair {
    const fn new(name: &'static str) -> Self {
        Self {
            name,
            plain: Vec::new(),
            controlled: Vec::new(),
        }
    }
}

/// The role, the global attachment of the issuer and the bindings of `person`.
async fn bind(pool: &PgPool, data: &Dataset, person: Uuid, seed: u64) -> anyhow::Result<()> {
    let role = Uuid::now_v7();
    let nil = Uuid::nil();
    sqlx::query(
        "INSERT INTO role (id, key, label, valid_period, changeset_id)
         VALUES ($1, 'bench_operator', 'Bench operator', $2::tstzrange, $3)",
    )
    .bind(role)
    .bind(SINCE)
    .bind(nil)
    .execute(pool)
    .await?;
    for permission in [
        "core.domain.read",
        "core.prefix.read",
        "core.range.read",
        "core.address.read",
        "core.address.write",
    ] {
        sqlx::query(
            "INSERT INTO role_permission (role_id, permission, valid_period, changeset_id)
             VALUES ($1, $2, $3::tstzrange, $4)",
        )
        .bind(role)
        .bind(permission)
        .bind(SINCE)
        .bind(nil)
        .execute(pool)
        .await?;
    }
    sqlx::query(
        "INSERT INTO issuer_attachment (issuer, entity_id, valid_period, changeset_id)
         VALUES ($1, NULL, $2::tstzrange, $3)",
    )
    .bind(ISSUER)
    .bind(SINCE)
    .bind(nil)
    .execute(pool)
    .await?;
    // Every domain through a group, so that every draw is visible; prefix subtrees
    // drawn at random for the rest, as many lists for the predicate to go through.
    let mut rng = ChaCha8Rng::seed_from_u64(seed.wrapping_add(7));
    let mut rows = Vec::new();
    for (k, domain) in data.domains.iter().enumerate() {
        rows.push((Some(k % GROUPS), "domain", domain.id));
    }
    while rows.len() < BINDINGS {
        let prefix = data.prefixes[rng.random_range(0..data.prefixes.len())].id;
        let group = (rows.len() >= OWN + data.domains.len()).then_some(rows.len() % GROUPS);
        rows.push((group, "prefix", prefix));
    }
    for (group, scope_kind, scope) in rows {
        sqlx::query(
            "INSERT INTO binding (id, subject_kind, subject_id, issuer, group_name, role_id,
                                  scope_kind, scope_id, valid_period, changeset_id)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9::tstzrange, $10)",
        )
        .bind(Uuid::now_v7())
        .bind(if group.is_some() { "group" } else { "identity" })
        .bind(group.is_none().then_some(person))
        .bind(group.is_some().then_some(ISSUER))
        .bind(group.map(|g| format!("g{g:02}")))
        .bind(role)
        .bind(scope_kind)
        .bind(scope)
        .bind(SINCE)
        .bind(nil)
        .execute(pool)
        .await?;
    }
    sqlx::raw_sql("ANALYZE binding, role, role_permission, issuer_attachment")
        .execute(pool)
        .await?;
    Ok(())
}

/// A unitary changeset of `context`: its duration, or why it was not applied.
async fn unitary<A: Authorizer>(
    store: &PgStore,
    authorizer: &A,
    context: &RequestContext,
    author: Uuid,
    operation: Operation,
) -> anyhow::Result<Duration> {
    let request = ChangesetRequest {
        author,
        motive: "bench".into(),
        period: None,
        operations: vec![operation],
        simulate: false,
        idempotency_key: None,
    };
    let started = Instant::now();
    let outcome = submit_in(
        store,
        authorizer,
        &Sources::none(),
        context,
        &request,
        MAX_OPERATIONS,
    )
    .await?;
    let elapsed = started.elapsed();
    match outcome {
        Outcome::Applied { .. } => Ok(elapsed),
        other => bail!("changeset not applied: {other:?}"),
    }
}

/// Loads, binds and measures; returns the report and whether every overhead holds.
///
/// # Errors
///
/// Database or pipeline failures, a draw the person cannot see.
#[allow(clippy::too_many_lines)] // one linear scenario, step by step
pub async fn run(database: &str, data: &Dataset, seed: u64) -> anyhow::Result<(String, bool)> {
    let started = Instant::now();
    let Prepared {
        pool,
        author,
        rack,
        mut report,
    } = prepare(database, data, seed).await?;
    bind(&pool, data, author, seed).await?;
    progress(&format!("bindings: {:?}", started.elapsed()));
    let subject = Subject {
        id: author,
        issuer: Some(ISSUER.into()),
        groups: (0..GROUPS).map(|g| format!("g{g:02}")).collect(),
    };
    let person = RequestContext::of(&subject);
    let system = RequestContext::person(author);
    let authorizer = PgAuthorizer::new(pool.clone());
    let store = PgStore::new(pool.clone());
    let mut rng = ChaCha8Rng::seed_from_u64(seed.wrapping_add(1));
    let now = OffsetDateTime::now_utc();
    let scope = authorizer
        .read_scope(&subject, now)
        .await
        .map_err(|e| anyhow!(e.0))?;
    let bindings: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM binding WHERE subject_id = $1 OR group_name LIKE 'g%'",
    )
    .bind(author)
    .fetch_one(&pool)
    .await?;
    writeln!(
        report,
        "INFO {bindings} bindings, {GROUPS} groups, scope of a read: {} octets",
        scope.to_json().map_or(0, |j| j.to_string().len())
    )?;

    // Warm-up of the connections and statements, both ways.
    for _ in 0..20 {
        let id = data.addresses[rng.random_range(0..data.addresses.len())].id;
        object(&pool, id, now, None).await?;
        authorizer
            .authorize_many(&subject, "core.address.read", &[id])
            .await
            .map_err(|e| anyhow!(e.0))?;
    }

    // Reading an object: the read, then the decision on it.
    let mut read = Pair::new("read an object");
    for _ in 0..200 {
        let id = if rng.random_range(0..4) == 0 {
            data.prefixes[rng.random_range(0..data.prefixes.len())].id
        } else {
            data.addresses[rng.random_range(0..data.addresses.len())].id
        };
        let t = Instant::now();
        object(&pool, id, now, None)
            .await?
            .context("a loaded object is missing")?;
        read.plain.push(t.elapsed());
        let t = Instant::now();
        let found = object(&pool, id, now, None)
            .await?
            .context("a loaded object is missing")?;
        let outside = authorizer
            .authorize_many(&subject, &format!("core.{}.read", found.kind), &[id])
            .await
            .map_err(|e| anyhow!(e.0))?;
        read.controlled.push(t.elapsed());
        if !outside.is_empty() {
            bail!("the person cannot read {id}: the bindings miss a domain");
        }
    }

    // Filtered pages of 100: the scope of the person, then the filtered page.
    let mut page = Pair::new("filtered page of 100");
    let mut scopes = Vec::new();
    for _ in 0..100 {
        let domain = data.domains[rng.random_range(0..data.domains.len())].id;
        let low: i64 = rng.random_range(0..900_000);
        let condition =
            Condition::Between(Some(low.to_string()), Some((low + 100_000).to_string()));
        let t = Instant::now();
        let all = filter(&pool, rack, domain, &condition, now, None, 100).await?;
        page.plain.push(t.elapsed());
        let t = Instant::now();
        let scope = authorizer
            .read_scope(&subject, now)
            .await
            .map_err(|e| anyhow!(e.0))?;
        scopes.push(t.elapsed());
        let visible = filter_in(&pool, rack, domain, &condition, now, None, 100, &scope).await?;
        page.controlled.push(t.elapsed());
        if visible != all {
            bail!("a filtered page differs for a person who sees every domain");
        }
    }

    // Subtree and occupancy.
    let mut subtree = Pair::new("subtree and occupancy");
    for _ in 0..200 {
        let p = data.prefixes[rng.random_range(0..data.prefixes.len())].id;
        let t = Instant::now();
        children(&pool, p, now).await?;
        occupancy(&pool, p, now).await?;
        subtree.plain.push(t.elapsed());
        let t = Instant::now();
        let scope = authorizer
            .read_scope(&subject, now)
            .await
            .map_err(|e| anyhow!(e.0))?;
        children_in(&pool, p, now, &scope).await?;
        occupancy(&pool, p, now).await?;
        subtree.controlled.push(t.elapsed());
    }

    // Unitary changesets: a new address in a random network, alternately.
    let networks: Vec<(Uuid, Uuid)> = data
        .prefixes
        .iter()
        .filter(|p| p.nature == crate::Nature::Network)
        .map(|p| (p.id, data.domains[p.domain].id))
        .collect();
    let mut changeset = Pair::new("unitary changeset");
    for _ in 0..100 {
        let network = networks[rng.random_range(0..networks.len())];
        let operation = new_address(&pool, network).await?;
        changeset
            .plain
            .push(unitary(&store, &Unrestricted, &system, author, operation).await?);
        let network = networks[rng.random_range(0..networks.len())];
        let operation = new_address(&pool, network).await?;
        changeset
            .controlled
            .push(unitary(&store, &authorizer, &person, author, operation).await?);
    }
    progress(&format!("measures: {:?}", started.elapsed()));
    writeln!(
        report,
        "INFO the read scope of the person, p95: {:?}",
        p95(scopes)
    )?;

    let mut ok = true;
    for pair in [read, page, subtree, changeset] {
        let plain = p95(pair.plain);
        let controlled = p95(pair.controlled);
        let overhead = controlled.as_secs_f64() / plain.as_secs_f64().max(1e-9) - 1.0;
        let pass = overhead < OVERHEAD_MAX;
        ok &= pass;
        writeln!(
            report,
            "{} {}, p95: {plain:?} without, {controlled:?} with access control ({:+.1} %, at most {:.0} %)",
            if pass { "PASS" } else { "FAIL" },
            pair.name,
            overhead * 100.0,
            OVERHEAD_MAX * 100.0,
        )?;
    }
    Ok((report, ok))
}
