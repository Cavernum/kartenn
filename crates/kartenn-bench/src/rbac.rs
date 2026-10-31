//! Cost of access control on the budgets of the core (spec 005, REQ-005-29; task 005-16):
//! the dataset of `bench-core`, and a person of an issuer with 50 groups in its token and
//! 1 000 bindings (one per domain, the others on prefix subtrees) through 155 of the 200
//! groups of kartenn (spec 025, REQ-025-15), mixed members of identities and groups of
//! the issuer; the decision is also measured for 50 groups of the issuer resolved to 0, 1
//! or 5 groups of kartenn. Each
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
use crate::measures::{Bench, Measure, Selection};
use crate::props::{SINCE, p95, progress};

const READ: &str = "rbac.read-object";
const READ_PAST: &str = "rbac.read-past";
const FILTERED_PAGE: &str = "rbac.filtered-page";
const SUBTREE: &str = "rbac.subtree";
const UNITARY: &str = "rbac.unitary-changeset";
const RESOLUTION: &str = "rbac.resolution";

/// The measures of `bench-rbac` (ADR 0041). The budget of the read in the past is twice
/// the current read with access control.
pub(crate) static BENCH: Bench = Bench {
    name: "rbac",
    measures: &[
        Measure::new(READ),
        Measure::new(READ_PAST).after(&[READ]),
        Measure::new(FILTERED_PAGE),
        Measure::new(SUBTREE),
        Measure::new(UNITARY),
        Measure::new(RESOLUTION),
    ],
};

pub(crate) const ISSUER: &str = "https://idp.bench.invalid";
/// The groups of the issuer in the token of the person (`g00` to `g49`).
pub(crate) const GROUPS: usize = 50;
/// The groups of kartenn (spec 025, REQ-025-15).
const KARTENN_GROUPS: usize = 200;
/// Groups of kartenn the person belongs to as an identity; those before them it belongs
/// to through one of its groups of the issuer, those after not at all.
const DIRECT: usize = 5;
/// Groups of kartenn the person does not belong to: their members and bindings are
/// noise for the resolution and the join.
const FOREIGN: usize = 45;
/// Groups of kartenn the person belongs to.
const HELD: usize = KARTENN_GROUPS - FOREIGN;
/// Bindings of the person, through its groups of kartenn.
const BINDINGS: usize = 1_000;
/// Bindings of each group the person does not belong to.
const FOREIGN_BINDINGS: usize = 2;
/// Other identities, members of the groups beside the person.
const OTHERS: usize = 100;
/// Groups of kartenn resolved from the 50 groups of the issuer of a token, for the
/// decision alone (the case of the group resolution).
const RESOLVED: [usize; 3] = [0, 1, 5];

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

/// A group of kartenn `key` over [`SINCE`], whose members are the identities `identities`
/// and the groups `idp_groups` of [`ISSUER`]; its identifier.
pub(crate) async fn group(
    pool: &PgPool,
    key: &str,
    identities: &[Uuid],
    idp_groups: &[String],
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
        "INSERT INTO group_member (id, group_id, member_kind, identity_id, valid_period,
                                   changeset_id)
         SELECT gen_random_uuid(), $1, 'identity', i, $3::tstzrange, $4
         FROM unnest($2::uuid[]) AS i",
    )
    .bind(id)
    .bind(identities)
    .bind(SINCE)
    .bind(nil)
    .execute(pool)
    .await?;
    sqlx::query(
        "INSERT INTO group_member (id, group_id, member_kind, issuer, group_name, valid_period,
                                   changeset_id)
         SELECT gen_random_uuid(), $1, 'idp_group', $2, n, $4::tstzrange, $5
         FROM unnest($3::text[]) AS n",
    )
    .bind(id)
    .bind(ISSUER)
    .bind(idp_groups)
    .bind(SINCE)
    .bind(nil)
    .execute(pool)
    .await?;
    Ok(id)
}

/// Binds `role` on `scope_kind` `scope` to the group of kartenn `group`, over [`SINCE`].
async fn binding(
    pool: &PgPool,
    group: Uuid,
    role: Uuid,
    scope_kind: &str,
    scope: Uuid,
) -> anyhow::Result<()> {
    sqlx::query(
        "INSERT INTO binding (id, group_id, role_id, scope_kind, scope_id, valid_period,
                              changeset_id)
         VALUES ($1, $2, $3, $4, $5, $6::tstzrange, $7)",
    )
    .bind(Uuid::now_v7())
    .bind(group)
    .bind(role)
    .bind(scope_kind)
    .bind(scope)
    .bind(SINCE)
    .bind(Uuid::nil())
    .execute(pool)
    .await?;
    Ok(())
}

/// The role, the global attachment of the issuer, the [`KARTENN_GROUPS`] groups of
/// kartenn and their bindings: [`BINDINGS`] reach `person`, whose token carries the
/// [`GROUPS`] groups `g00` to `g49`, through [`HELD`] groups (the first by one of its
/// groups of the issuer, the last [`DIRECT`] as an identity); [`FOREIGN`] groups hold
/// [`FOREIGN_BINDINGS`] each that do not. Every group also has members that are not the
/// person (another identity, a group of the issuer absent from its token). Returns the
/// role.
pub(crate) async fn bind(
    pool: &PgPool,
    data: &Dataset,
    person: Uuid,
    seed: u64,
) -> anyhow::Result<Uuid> {
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
        "core.address.approval.self",
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
    // The groups of kartenn: those of the person first, then the foreign ones; each with
    // another identity and a group of the issuer absent from the token.
    let others: Vec<Uuid> = (0..OTHERS).map(|_| Uuid::now_v7()).collect();
    let mut groups = Vec::with_capacity(KARTENN_GROUPS);
    for k in 0..KARTENN_GROUPS {
        let mut identities = vec![others[k % OTHERS]];
        let mut idp_groups = vec![format!("x{k:03}")];
        if k < HELD - DIRECT {
            idp_groups.push(format!("g{:02}", k % GROUPS));
        } else if k < HELD {
            identities.push(person);
        }
        groups.push(group(pool, &format!("bench-k{k:03}"), &identities, &idp_groups).await?);
    }
    // Every domain through a group, so that every draw is visible; prefix subtrees
    // drawn at random for the rest, as many lists for the predicate to go through.
    let mut rng = ChaCha8Rng::seed_from_u64(seed.wrapping_add(7));
    for (k, domain) in data.domains.iter().enumerate() {
        binding(pool, groups[k % HELD], role, "domain", domain.id).await?;
    }
    for k in data.domains.len()..BINDINGS {
        let prefix = data.prefixes[rng.random_range(0..data.prefixes.len())].id;
        binding(pool, groups[k % HELD], role, "prefix", prefix).await?;
    }
    for k in 0..FOREIGN * FOREIGN_BINDINGS {
        let prefix = data.prefixes[rng.random_range(0..data.prefixes.len())].id;
        binding(pool, groups[HELD + k % FOREIGN], role, "prefix", prefix).await?;
    }
    sqlx::raw_sql(
        "ANALYZE binding, rbac_group, group_member, role, role_permission, issuer_attachment",
    )
    .execute(pool)
    .await?;
    Ok(role)
}

/// For each count of [`RESOLVED`], a person whose token carries [`GROUPS`] groups of the
/// issuer, that many of them members of a group of kartenn of its own (the domains bound
/// to them in turn, so that every draw is visible unless there is none).
async fn resolution_subjects(
    pool: &PgPool,
    data: &Dataset,
    role: Uuid,
) -> anyhow::Result<Vec<(usize, Subject)>> {
    let mut subjects = Vec::with_capacity(RESOLVED.len());
    for n in RESOLVED {
        let names: Vec<String> = (0..GROUPS).map(|g| format!("r{n}-{g:02}")).collect();
        let mut groups = Vec::with_capacity(n);
        for (j, name) in names.iter().take(n).enumerate() {
            groups.push(
                group(
                    pool,
                    &format!("bench-r{n}-{j}"),
                    &[],
                    std::slice::from_ref(name),
                )
                .await?,
            );
        }
        if n > 0 {
            for (k, domain) in data.domains.iter().enumerate() {
                binding(pool, groups[k % n], role, "domain", domain.id).await?;
            }
        }
        subjects.push((
            n,
            Subject {
                id: Uuid::now_v7(),
                issuer: Some(ISSUER.into()),
                groups: names,
            },
        ));
    }
    sqlx::raw_sql("ANALYZE binding, rbac_group, group_member")
        .execute(pool)
        .await?;
    Ok(subjects)
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
        approval_self: true,
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

/// Loads, binds and measures what `selection` runs; returns the report and whether every
/// budget holds with access control.
///
/// # Errors
///
/// Database or pipeline failures, a draw the person cannot see.
#[allow(clippy::too_many_lines)] // one linear scenario, step by step
pub async fn run(
    database: &str,
    data: &Dataset,
    seed: u64,
    selection: &Selection,
) -> anyhow::Result<(String, bool)> {
    let started = Instant::now();
    let Prepared {
        pool,
        author,
        rack,
        mut report,
    } = prepare(database, data, seed).await?;
    let role = bind(&pool, data, author, seed).await?;
    let resolution = resolution_subjects(&pool, data, role).await?;
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
    let (bindings, held): (i64, i64) = sqlx::query_as(
        "WITH m AS (SELECT kartenn_member_of($1, $2, $3, now()) AS group_id)
         SELECT (SELECT count(*) FROM binding WHERE group_id IN (SELECT group_id FROM m)),
                (SELECT count(*) FROM m)",
    )
    .bind(author)
    .bind(ISSUER)
    .bind(&subject.groups)
    .fetch_one(&pool)
    .await?;
    let (all, groups, members): (i64, i64, i64) = sqlx::query_as(
        "SELECT (SELECT count(*) FROM binding), (SELECT count(*) FROM rbac_group),
                (SELECT count(*) FROM group_member)",
    )
    .fetch_one(&pool)
    .await?;
    writeln!(
        report,
        "INFO {bindings} bindings through {held} groups of kartenn for {GROUPS} groups of the \
         issuer in the token ({all} bindings, {groups} groups, {members} members in all), \
         scope of a read: {} octets",
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

    let mut pairs = Vec::new();
    // Reading an object: the read, then the decision on it.
    let mut current = None;
    if selection.runs(READ) {
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
        current = Some(p95(read.controlled.clone()));
        pairs.push(read);
    }

    // Reading an object as known in the past (INV-33): the same decision, on now; twice
    // the current read, with access control as well (floor 5 ms).
    if selection.runs(READ_PAST) {
        let current = current.context("the current read is a prerequisite")?;
        let mut dated = Pair::new(
            "read an object as known in the past",
            (current * 2).max(Duration::from_millis(5)),
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
        pairs.push(dated);
    }

    // Filtered pages of 100: the scope of the person, then the filtered page.
    if selection.runs(FILTERED_PAGE) {
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
            let visible =
                filter_in(&pool, rack, domain, &condition, now, None, 100, &scope).await?;
            page.controlled.push(t.elapsed());
            if visible != all {
                bail!("a filtered page differs for a person who sees every domain");
            }
        }
        writeln!(
            report,
            "INFO the read scope of the person, p95: {:?}",
            p95(scopes)
        )?;
        pairs.push(page);
    }

    // Subtree and occupancy.
    if selection.runs(SUBTREE) {
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
        pairs.push(subtree);
    }

    // Unitary changesets: a new address in a random network, alternately.
    if selection.runs(UNITARY) {
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
        pairs.push(changeset);
    }

    // The resolution of the groups (REQ-025-15): the decision on an address and the read
    // scope, for 50 groups of the issuer resolved to 0, 1 or 5 groups of kartenn, then
    // for the person.
    if selection.runs(RESOLUTION) {
        let mut resolved = Vec::with_capacity(RESOLVED.len() + 1);
        for (n, who) in resolution
            .iter()
            .map(|(n, s)| (*n, s))
            .chain([(usize::try_from(held)?, &subject)])
        {
            let mut decisions = Vec::with_capacity(200);
            let mut reads = Vec::with_capacity(200);
            for _ in 0..200 {
                let id = data.addresses[rng.random_range(0..data.addresses.len())].id;
                let t = Instant::now();
                let outside = authorizer
                    .authorize_many(who, "core.address.read", &[id])
                    .await
                    .map_err(|e| anyhow!(e.0))?;
                decisions.push(t.elapsed());
                if outside.is_empty() == (n == 0) {
                    bail!("the decision on {id} for {n} groups of kartenn is wrong");
                }
                let t = Instant::now();
                authorizer
                    .read_scope(who, now)
                    .await
                    .map_err(|e| anyhow!(e.0))?;
                reads.push(t.elapsed());
            }
            resolved.push((n, p95(decisions), p95(reads)));
        }
        for (n, decision, read) in resolved {
            writeln!(
                report,
                "INFO {GROUPS} groups of the issuer, {n} groups of kartenn: decision on an \
                 address p95 {decision:?}, read scope p95 {read:?}"
            )?;
        }
    }
    progress(&format!("measures: {:?}", started.elapsed()));

    let mut ok = true;
    for pair in pairs {
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
