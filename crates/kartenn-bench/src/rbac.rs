//! Cost of access control on the budgets of the core (spec 005, REQ-005-29; task 005-16):
//! the dataset of `bench-core`, and a person of an issuer with 50 groups and 1 000
//! bindings (20 on domains, the others on prefix subtrees and on the identity). Each
//! budget is measured twice on the same draws, alternately: without access control (the
//! system), then with the decision and the filtering of the person; each p95 with access
//! control must hold its budget, the overhead is reported (005/K15).

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

pub(crate) const ISSUER: &str = "https://idp.bench.invalid";
pub(crate) const GROUPS: usize = 50;
const BINDINGS: usize = 1_000;
/// Bindings on the identity itself, the others on its groups.
const OWN: usize = 10;

/// One budget measured both ways.
struct Pair {
    name: &'static str,
    /// The budget of the p95 (INV-30 to 34, as in `bench-core`; INV-33 relative to the
    /// current read).
    limit: Duration,
    plain: Vec<Duration>,
    controlled: Vec<Duration>,
}

impl Pair {
    const fn new(name: &'static str, limit: Duration) -> Self {
        Self {
            name,
            limit,
            plain: Vec::new(),
            controlled: Vec::new(),
        }
    }
}

/// Binds the role `$2` on the scope `$3`, object `$4`, to the identity `$1` through a
/// group of kartenn of its own, from a day ago (spec 025: a binding serves a group).
pub(crate) const BIND_IDENTITY: &str = "WITH g AS (
        INSERT INTO rbac_group (id, key, labels, valid_period, changeset_id)
        VALUES (gen_random_uuid(), 'b-' || replace(gen_random_uuid()::text, '-', ''),
                '{\"fr\": \"b\"}', tstzrange(now() - interval '1 day', NULL), gen_random_uuid())
        RETURNING id),
     m AS (
        INSERT INTO group_member (id, group_id, member_kind, identity_id, valid_period, changeset_id)
        SELECT gen_random_uuid(), g.id, 'identity', $1::uuid,
               tstzrange(now() - interval '1 day', NULL), gen_random_uuid() FROM g)
    INSERT INTO binding (id, group_id, role_id, scope_kind, scope_id, valid_period, changeset_id)
    SELECT gen_random_uuid(), g.id, $2::uuid, $3::text, $4::uuid,
           tstzrange(now() - interval '1 day', NULL), gen_random_uuid() FROM g";

/// A group of kartenn `key` over [`SINCE`], whose only member is the identity `identity`
/// or else the group `idp_group` of [`ISSUER`]; its identifier.
pub(crate) async fn group(
    pool: &PgPool,
    key: &str,
    identity: Option<Uuid>,
    idp_group: Option<&str>,
) -> anyhow::Result<Uuid> {
    let id = Uuid::now_v7();
    let nil = Uuid::nil();
    sqlx::query(
        "INSERT INTO rbac_group (id, key, labels, valid_period, changeset_id)
         VALUES ($1, $2, jsonb_build_object('fr', $2::text), $3::tstzrange, $4)",
    )
    .bind(id)
    .bind(key)
    .bind(SINCE)
    .bind(nil)
    .execute(pool)
    .await?;
    sqlx::query(
        "INSERT INTO group_member (id, group_id, member_kind, identity_id, issuer, group_name,
                                   valid_period, changeset_id)
         VALUES ($1, $2, CASE WHEN $3::uuid IS NULL THEN 'idp_group' ELSE 'identity' END, $3,
                 CASE WHEN $3::uuid IS NULL THEN $4::text END, $5, $6::tstzrange, $7)",
    )
    .bind(Uuid::now_v7())
    .bind(id)
    .bind(identity)
    .bind(ISSUER)
    .bind(idp_group)
    .bind(SINCE)
    .bind(nil)
    .execute(pool)
    .await?;
    Ok(id)
}

/// The role, the global attachment of the issuer and the bindings of `person`.
pub(crate) async fn bind(
    pool: &PgPool,
    data: &Dataset,
    person: Uuid,
    seed: u64,
) -> anyhow::Result<()> {
    let role = Uuid::now_v7();
    let nil = Uuid::nil();
    sqlx::query(
        "INSERT INTO role (id, key, labels, valid_period, changeset_id)
         VALUES ($1, 'bench_operator', jsonb_build_object('fr', 'Bench operator'), $2::tstzrange, $3)",
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
        "core.address.edit",
        "core.address.apply_own",
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
    // The person's own group, and a group of kartenn for each group of the issuer.
    let own = group(pool, "bench-own", Some(person), None).await?;
    let mut groups = Vec::with_capacity(GROUPS);
    for g in 0..GROUPS {
        let name = format!("g{g:02}");
        groups.push(group(pool, &format!("bench-{name}"), None, Some(&name)).await?);
    }
    for (group, scope_kind, scope) in rows {
        sqlx::query(
            "INSERT INTO binding (id, group_id, role_id, scope_kind, scope_id, valid_period,
                                  changeset_id)
             VALUES ($1, $2, $3, $4, $5, $6::tstzrange, $7)",
        )
        .bind(Uuid::now_v7())
        .bind(group.map_or(own, |g| groups[g]))
        .bind(role)
        .bind(scope_kind)
        .bind(scope)
        .bind(SINCE)
        .bind(nil)
        .execute(pool)
        .await?;
    }
    sqlx::raw_sql(
        "ANALYZE binding, rbac_group, group_member, role, role_permission, issuer_attachment",
    )
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
        apply_own: true,
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

/// Loads, binds and measures; returns the report and whether every budget holds with
/// access control.
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
        "SELECT count(*) FROM binding WHERE group_id IN (
             SELECT group_id FROM group_member WHERE identity_id = $1 OR group_name LIKE 'g%')",
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
    let mut read = Pair::new("read an object", Duration::from_millis(50));
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

    // Reading an object as known in the past (INV-33): the same decision, on now.
    let mut dated = Pair::new(
        "read an object as known in the past",
        Duration::from_millis(50),
    );
    let from = OffsetDateTime::from_unix_timestamp(1_696_118_400)?;
    for _ in 0..200 {
        let id = data.addresses[rng.random_range(0..data.addresses.len())].id;
        let as_of = Some(from + Duration::from_hours(24) * rng.random_range(0..700));
        let t = Instant::now();
        object(&pool, id, now, as_of)
            .await?
            .context("a past version is missing")?;
        dated.plain.push(t.elapsed());
        let t = Instant::now();
        object(&pool, id, now, as_of)
            .await?
            .context("a past version is missing")?;
        authorizer
            .authorize_many(&subject, "core.address.read", &[id])
            .await
            .map_err(|e| anyhow!(e.0))?;
        dated.controlled.push(t.elapsed());
    }

    // Filtered pages of 100: the scope of the person, then the filtered page.
    let mut page = Pair::new("filtered page of 100", Duration::from_millis(150));
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
    let mut subtree = Pair::new("subtree and occupancy", Duration::from_millis(200));
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
    let mut changeset = Pair::new("unitary changeset", Duration::from_millis(200));
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

    // INV-33: twice the current read, with access control as well (floor 5 ms).
    dated.limit = (p95(read.controlled.clone()) * 2).max(Duration::from_millis(5));
    let mut ok = true;
    for pair in [read, dated, page, subtree, changeset] {
        let plain = p95(pair.plain);
        let controlled = p95(pair.controlled);
        let overhead = controlled.as_secs_f64() / plain.as_secs_f64().max(1e-9) - 1.0;
        let pass = controlled <= pair.limit;
        ok &= pass;
        writeln!(
            report,
            "{} {}, p95: {controlled:?} with access control, budget {:?} ({plain:?} without, {:+.1} %)",
            if pass { "PASS" } else { "FAIL" },
            pair.name,
            pair.limit,
            overhead * 100.0,
        )?;
    }
    Ok((report, ok))
}
