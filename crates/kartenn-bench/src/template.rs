//! The prepared database kept as a template of the bench instance (ADR 0041, decision 2).
//!
//! A bench prepares its database (the dataset, its history, the modules and their
//! data…), then measures. With `--template`, which the `bench-*` scripts pass on the bench
//! instance only, the prepared database becomes the template
//! `kartenn_bench_tpl_<bench>_<fingerprint>` and the run measures on a copy of it,
//! `CREATE DATABASE … TEMPLATE … STRATEGY FILE_COPY`, its files cloned where the file
//! system can (`file_copy_method = clone`, PostgreSQL 18). The next run with the same
//! fingerprint copies the template instead of preparing; a measure « de préparation »
//! prepares again, and replaces the template.
//!
//! The fingerprint covers the migrations of the core and of the modules, the sources of
//! the generator and of the preparation of the bench, the sources of the workspace crates
//! it goes through and `Cargo.lock`, the seed and the scale: exactness first, a template
//! is made again as soon as one of them changes. Three templates at most are kept per
//! bench, the oldest dropped; only databases named as the templates of the bench are ever
//! dropped.
//!
//! The preparation ends with `VACUUM (ANALYZE)`, so that its visibility map and its
//! statistics are in the template; the copy is read before the measures (`pg_prewarm`
//! where the database has it, else a sequential read of its tables), so that they start
//! warm as on the database just prepared. A template accepts no connection
//! (`ALLOW_CONNECTIONS false`), and it is renamed and copied only when no client is
//! connected to it. What a bench needs from its preparation (identifiers…) is read back
//! from the copy, in the run that prepared as in the next ones, and the lines of the
//! report the preparation wrote are kept in the comment of the template: two runs of one
//! fingerprint measure, and report, the same way.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::str::FromStr;
use std::time::{Duration, Instant};

use anyhow::{Context, bail};
use kartenn_service::module::Module;
use sqlx::postgres::{PgConnectOptions, PgConnection};
use sqlx::{AssertSqlSafe, Connection, PgPool};
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

use crate::measures::Selection;
use crate::props::progress;

/// The start of the name of every template.
pub const PREFIX: &str = "kartenn_bench_tpl_";
/// Templates kept per bench.
pub const KEPT: usize = 3;
/// The start of the name of a database a run may copy a template into.
pub const TARGET_PREFIX: &str = "kartenn_bench";
/// The form of a template, in its fingerprint: raised when the templates already made no
/// longer fit (2: `pg_prewarm` in the prepared database, from `template1`).
const FORMAT: u64 = 2;
/// How long the connections of a closed pool may take to end.
const LINGER: Duration = Duration::from_secs(10);

/// A source of the preparation: its file in `src/`, and its text.
type Source = (&'static str, &'static str);

macro_rules! sources {
    ($($file:literal),* $(,)?) => {
        &[$(($file, include_str!($file))),*]
    };
}

/// The sources of the dataset of `bench-core` and of its preparation, shared by the
/// benches that start from it; this module too, which makes the template.
const CORE: &[Source] = sources!["lib.rs", "template.rs", "props.rs", "core_budgets.rs"];
const PROPS: &[Source] = sources!["lib.rs", "template.rs", "props.rs"];
const RBAC: &[Source] = sources!["rbac.rs"];
const HTTP: &[Source] = sources!["rbac.rs", "http.rs"];
const POLICY: &[Source] = sources!["vlan_budgets.rs", "policy_budgets.rs"];
const FLOW: &[Source] = sources!["vlan_budgets.rs", "rbac.rs", "flow_budgets.rs"];
const EVENTS: &[Source] = sources!["template.rs", "rbac.rs", "events_budgets.rs"];

/// The sources of the preparation of `bench`.
///
/// # Panics
///
/// On a bench that has no preparation listed here (a test checks every bench).
pub fn sources(bench: &str) -> Vec<Source> {
    let own: &[&[Source]] = match bench {
        "props" => &[PROPS],
        "core" | "telemetry" => &[CORE],
        "rbac" => &[CORE, RBAC],
        "http" => &[CORE, HTTP],
        "policy" => &[CORE, POLICY],
        "flow" => &[CORE, FLOW],
        "events" => &[EVENTS],
        other => panic!("no sources listed for the preparation of the bench `{other}`"),
    };
    own.iter().flat_map(|s| s.iter().copied()).collect()
}

/// The hashes of the workspace crates and of `Cargo.lock`, by `build.rs`.
mod built {
    include!(concat!(env!("OUT_DIR"), "/crates.rs"));
}

/// The workspace crates the preparation of `bench` goes through, as its sources name them;
/// their own workspace dependencies come with them ([`crate_hashes`]). A test checks that
/// every workspace crate the sources of a preparation name is there.
///
/// # Panics
///
/// On a bench that has no preparation listed here.
pub fn crates(bench: &str) -> &'static [&'static str] {
    const CORE: &[&str] = &["kartenn-core", "kartenn-service", "kartenn-store-pg"];
    const HTTP: &[&str] = &[
        "kartenn-auth",
        "kartenn-core",
        "kartenn-rest",
        "kartenn-server",
        "kartenn-service",
        "kartenn-store-pg",
    ];
    const MODULES: &[&str] = &[
        "kartenn-auth",
        "kartenn-core",
        "kartenn-flow",
        "kartenn-machines",
        "kartenn-policy",
        "kartenn-rest",
        "kartenn-service",
        "kartenn-store-pg",
    ];
    const EVENTS: &[&str] = &[
        "kartenn-auth",
        "kartenn-core",
        "kartenn-rest",
        "kartenn-service",
        "kartenn-store-pg",
    ];
    match bench {
        "props" | "core" | "telemetry" | "rbac" => CORE,
        "events" => EVENTS,
        "http" => HTTP,
        "policy" | "flow" => MODULES,
        other => panic!("no crates listed for the preparation of the bench `{other}`"),
    }
}

/// `direct` and their workspace dependencies, transitively, sorted: each name and the hash
/// of its sources.
///
/// # Panics
///
/// On a crate `kartenn-bench` does not depend on.
pub fn crate_hashes(direct: &[&str]) -> Vec<(&'static str, u64)> {
    let mut todo: Vec<&str> = direct.to_vec();
    let mut out = BTreeMap::new();
    while let Some(name) = todo.pop() {
        let (name, hash, dependencies) = built::CRATES
            .iter()
            .find(|(n, _, _)| *n == name)
            .unwrap_or_else(|| panic!("`{name}` is no workspace dependency of kartenn-bench"));
        if out.insert(*name, *hash).is_none() {
            todo.extend(dependencies.iter().copied());
        }
    }
    out.into_iter().collect()
}

/// A migration the fingerprint covers: the core or a module, its version, its checksum
/// (core) or its SQL (module).
pub type Migration = (&'static str, i64, &'static [u8]);

/// The migrations of the core and of every module a bench deploys.
pub fn migrations() -> Vec<Migration> {
    let mut out: Vec<Migration> = kartenn_store_pg::MIGRATOR
        .iter()
        .map(|m| ("core", m.version, &*m.checksum))
        .collect();
    let modules: [&'static dyn Module; 3] = [
        &kartenn_machines::Machines,
        &kartenn_policy::Policy,
        &kartenn_flow::Flows,
    ];
    for module in modules {
        out.extend(
            module
                .migrations()
                .iter()
                .map(|m| (module.name(), m.version, m.sql.as_bytes())),
        );
    }
    out
}

/// What a prepared database is made from, beyond the bench, its seed and its scale.
#[derive(Debug, Clone)]
pub struct Inputs {
    /// The migrations of the core and of the modules.
    pub migrations: Vec<Migration>,
    /// The sources of the preparation in `kartenn-bench`.
    pub sources: Vec<Source>,
    /// The workspace crates it goes through, and the hashes of their sources.
    pub crates: Vec<(&'static str, u64)>,
    /// The hash of `Cargo.lock`: the versions of the other crates.
    pub lock: u64,
}

impl Inputs {
    /// Those of the preparation of `bench` in this build.
    pub fn of(bench: &str) -> Self {
        Self {
            migrations: migrations(),
            sources: sources(bench),
            crates: crate_hashes(crates(bench)),
            lock: built::CARGO_LOCK,
        }
    }
}

/// 64-bit FNV-1a: the same value from one build, toolchain or machine to the next, which
/// the hasher of the standard library does not promise.
struct Fnv(u64);

impl Fnv {
    const fn new() -> Self {
        Self(0xcbf2_9ce4_8422_2325)
    }

    fn bytes(&mut self, bytes: &[u8]) {
        for b in bytes {
            self.0 ^= u64::from(*b);
            self.0 = self.0.wrapping_mul(0x0100_0000_01b3);
        }
    }

    /// A field, its length first: two lists of fields never give the same stream.
    fn field(&mut self, bytes: &[u8]) {
        self.bytes(&(bytes.len() as u64).to_le_bytes());
        self.bytes(bytes);
    }
}

/// The fingerprint of a prepared database: the bench, its seed and scale, and what it is
/// made from.
pub fn fingerprint(bench: &str, seed: u64, scale: &str, inputs: &Inputs) -> u64 {
    let mut h = Fnv::new();
    h.field(&FORMAT.to_le_bytes());
    h.field(bench.as_bytes());
    h.field(&seed.to_le_bytes());
    h.field(scale.as_bytes());
    h.field(&(inputs.migrations.len() as u64).to_le_bytes());
    for (owner, version, body) in &inputs.migrations {
        h.field(owner.as_bytes());
        h.field(&version.to_le_bytes());
        h.field(body);
    }
    h.field(&(inputs.sources.len() as u64).to_le_bytes());
    for (file, text) in &inputs.sources {
        h.field(file.as_bytes());
        h.field(text.as_bytes());
    }
    h.field(&(inputs.crates.len() as u64).to_le_bytes());
    for (name, hash) in &inputs.crates {
        h.field(name.as_bytes());
        h.field(&hash.to_le_bytes());
    }
    h.field(&inputs.lock.to_le_bytes());
    h.0
}

/// The name of the template of `bench` with this fingerprint.
pub fn name(bench: &str, fingerprint: u64) -> String {
    format!("{PREFIX}{bench}_{fingerprint:016x}")
}

/// Whether `datname` is named as a template of `bench`: the prefix, the bench, then 16
/// lowercase hexadecimal digits, nothing more.
pub fn is_template_of(bench: &str, datname: &str) -> bool {
    datname
        .strip_prefix(PREFIX)
        .and_then(|rest| rest.strip_prefix(bench))
        .and_then(|rest| rest.strip_prefix('_'))
        .is_some_and(|hex| {
            hex.len() == 16
                && hex
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        })
}

/// The templates to drop among `names`, oldest first: all but the [`KEPT`] newest.
pub fn surplus(names: &[String]) -> &[String] {
    &names[..names.len().saturating_sub(KEPT)]
}

/// The comment of a template: its description on the first line, then the lines of the
/// report its preparation wrote.
fn comment(description: &str, report: &str) -> String {
    format!("{description}\n{report}")
}

/// The lines of the report kept in the comment of a template.
fn report_of(comment: &str) -> &str {
    comment.split_once('\n').map_or("", |(_, report)| report)
}

/// How a run gets its prepared database.
#[derive(Debug, Clone)]
pub struct Reuse {
    /// Templates kept and copied (`--template`).
    enabled: bool,
    bench: &'static str,
    /// The template of this run.
    name: String,
    /// A measure « de préparation » runs: the run prepares whatever the templates.
    forced: bool,
    /// For the comment of the template.
    description: String,
}

impl Reuse {
    /// No template: the run prepares its database and measures on it (the dataset of
    /// `bench-web`, another instance than that of the benches).
    pub fn off() -> Self {
        Self {
            enabled: false,
            bench: "",
            name: String::new(),
            forced: true,
            description: String::new(),
        }
    }

    /// The template of `selection`'s bench for `seed` at `scale` (a description of the
    /// volumes), used when `enabled`.
    pub fn new(enabled: bool, selection: &Selection, seed: u64, scale: &str) -> Self {
        let bench = selection.bench();
        let fingerprint = fingerprint(bench, seed, scale, &Inputs::of(bench));
        Self {
            enabled,
            bench,
            name: name(bench, fingerprint),
            forced: selection.prepares(),
            description: format!("kartenn-bench {bench}, seed {seed}, {scale}"),
        }
    }

    /// The name of the template of the run.
    pub fn name(&self) -> &str {
        &self.name
    }
}

/// The database of a run, prepared. `write` prepares the empty database at `database`,
/// unless the template of `reuse` exists (and no measure « de préparation » runs); with
/// templates, the prepared database becomes the template and the run measures on a warmed
/// copy. Returns the pool of the database to measure, the lines of the report (those of
/// the preparation, kept in the template), and what `write` returned (`None` when the
/// template was copied).
///
/// # Errors
///
/// Database failures, a database of the run not named `kartenn_bench…` under
/// `--template`, or a client connected to the prepared database or to a template when it
/// is renamed or copied.
pub(crate) async fn prepared<T>(
    database: &str,
    reuse: &Reuse,
    write: impl AsyncFnOnce(&PgPool, &mut String) -> anyhow::Result<T>,
) -> anyhow::Result<(PgPool, String, Option<T>)> {
    let mut report = String::new();
    if !reuse.enabled {
        let pool = connect(database).await?;
        let written = write(&pool, &mut report).await?;
        vacuum(&pool).await?;
        return Ok((pool, report, Some(written)));
    }
    let target = Target::of(database)?;
    if !target.name.starts_with(TARGET_PREFIX) {
        bail!(
            "--template copies templates into a database named `{TARGET_PREFIX}…` only, not \
             `{}`",
            target.name
        );
    }
    let mut admin = target.admin().await?;
    let (copied, written) = if !reuse.forced && exists(&mut admin, &reuse.name).await? {
        let kept = template_report(&mut admin, &reuse.name).await?;
        report.push_str(&kept);
        let copied = copy(&mut admin, &reuse.name, &target.name).await?;
        admin.close().await?;
        (copied, None)
    } else {
        admin.close().await?;
        let pool = connect(database).await?;
        let written = write(&pool, &mut report).await?;
        vacuum(&pool).await?;
        pool.close().await;
        let mut admin = target.admin().await?;
        register(&mut admin, &target.name, reuse, &report).await?;
        let copied = copy(&mut admin, &reuse.name, &target.name).await?;
        prune(&mut admin, reuse).await?;
        admin.close().await?;
        progress(&format!("template {} registered", reuse.name));
        (copied, Some(written))
    };
    let pool = connect(database).await?;
    let warmed = warm(&pool).await?;
    progress(&format!("prepared database: {copied}; {warmed}"));
    writeln!(report, "INFO prepared database: {copied}; {warmed}")?;
    Ok((pool, report, written))
}

/// A pool on the database at `database`.
pub(crate) async fn connect(database: &str) -> anyhow::Result<PgPool> {
    PgPool::connect(database)
        .await
        .with_context(|| format!("connecting to {database}"))
}

/// The end of every preparation: the visibility map and the statistics of what it wrote.
async fn vacuum(pool: &PgPool) -> anyhow::Result<()> {
    let started = Instant::now();
    sqlx::raw_sql("VACUUM (ANALYZE)").execute(pool).await?;
    progress(&format!(
        "vacuum of the preparation: {:?}",
        started.elapsed()
    ));
    Ok(())
}

/// Reads the copy before the measures, as warm as the database just prepared: every
/// relation with `pg_prewarm` where the database has the extension (the largest first, so
/// that the smallest stay in the buffers), else every table sequentially. How, and how
/// long.
async fn warm(pool: &PgPool) -> anyhow::Result<String> {
    let started = Instant::now();
    let prewarm: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM pg_extension WHERE extname = 'pg_prewarm')",
    )
    .fetch_one(pool)
    .await?;
    let how = if prewarm {
        sqlx::raw_sql(
            "SELECT pg_prewarm(c.oid) FROM pg_class c
               JOIN pg_namespace n ON n.oid = c.relnamespace
              WHERE c.relkind IN ('r', 'i', 't', 'm') AND c.relpersistence = 'p'
                AND n.nspname NOT IN ('pg_catalog', 'information_schema')
                AND pg_has_role(c.relowner, 'USAGE')
              ORDER BY pg_relation_size(c.oid) DESC",
        )
        .execute(pool)
        .await?;
        "pg_prewarm"
    } else {
        let tables: Vec<String> = sqlx::query_scalar(
            "SELECT format('%I.%I', n.nspname, c.relname) FROM pg_class c
               JOIN pg_namespace n ON n.oid = c.relnamespace
              WHERE c.relkind IN ('r', 'm') AND c.relpersistence = 'p'
                AND n.nspname NOT IN ('pg_catalog', 'information_schema')
              ORDER BY pg_relation_size(c.oid) DESC",
        )
        .fetch_all(pool)
        .await?;
        for table in tables {
            sqlx::raw_sql(AssertSqlSafe(format!("SELECT count(*) FROM ONLY {table}")))
                .execute(pool)
                .await?;
        }
        "tables read (pg_prewarm absent)"
    };
    Ok(format!("warmed by {how} in {:.1?}", started.elapsed()))
}

/// The database of the run, and the way to its server.
struct Target {
    options: PgConnectOptions,
    name: String,
}

impl Target {
    fn of(database: &str) -> anyhow::Result<Self> {
        let options = PgConnectOptions::from_str(database)
            .with_context(|| format!("the URL of the bench, {database}"))?;
        let name = options
            .get_database()
            .with_context(|| format!("{database} names no database"))?
            .to_owned();
        Ok(Self { options, name })
    }

    /// A connection to the maintenance database `postgres` of the same server.
    async fn admin(&self) -> anyhow::Result<PgConnection> {
        PgConnection::connect_with(&self.options.clone().database("postgres"))
            .await
            .context("connecting to the database postgres of the bench instance")
    }
}

fn ident(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}

async fn exists(admin: &mut PgConnection, datname: &str) -> anyhow::Result<bool> {
    Ok(
        sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM pg_database WHERE datname = $1)")
            .bind(datname)
            .fetch_one(&mut *admin)
            .await?,
    )
}

/// The lines of the report kept in the comment of `template`.
async fn template_report(admin: &mut PgConnection, template: &str) -> anyhow::Result<String> {
    let comment: Option<String> = sqlx::query_scalar(
        "SELECT shobj_description(oid, 'pg_database') FROM pg_database WHERE datname = $1",
    )
    .bind(template)
    .fetch_one(&mut *admin)
    .await?;
    Ok(report_of(comment.as_deref().unwrap_or_default()).to_owned())
}

/// Fails, naming them, when clients stay connected to `datname` beyond [`LINGER`]: the
/// connections of a pool just closed may take a moment to end, and the server's own
/// processes (autovacuum…) are left to the server, which waits for them or stops them.
async fn unused(admin: &mut PgConnection, datname: &str, what: &str) -> anyhow::Result<()> {
    let started = Instant::now();
    loop {
        let open: Vec<(i32, Option<String>)> = sqlx::query_as(
            "SELECT pid, usename::text FROM pg_stat_activity
              WHERE datname = $1 AND backend_type = 'client backend'
                AND pid <> pg_backend_pid()
              ORDER BY pid",
        )
        .bind(datname)
        .fetch_all(&mut *admin)
        .await?;
        if open.is_empty() {
            return Ok(());
        }
        if started.elapsed() >= LINGER {
            let list: Vec<String> = open
                .iter()
                .map(|(pid, user)| format!("{pid} ({})", user.as_deref().unwrap_or("?")))
                .collect();
            bail!(
                "{what} {datname} has {} client connection(s) open (pid {}) after {LINGER:?}: \
                 a template is renamed and copied with no client connected",
                open.len(),
                list.join(", ")
            );
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
}

/// The prepared database `target` becomes the template of `reuse`, replacing one of the
/// same name (a preparation forced by a measure « de préparation »); its comment keeps
/// the lines of the `report` of the preparation.
async fn register(
    admin: &mut PgConnection,
    target: &str,
    reuse: &Reuse,
    report: &str,
) -> anyhow::Result<()> {
    unused(admin, target, "the prepared database").await?;
    if exists(admin, &reuse.name).await? {
        unused(admin, &reuse.name, "the template").await?;
        sqlx::raw_sql(AssertSqlSafe(format!(
            "DROP DATABASE {}",
            ident(&reuse.name)
        )))
        .execute(&mut *admin)
        .await?;
    }
    let prepared_at = OffsetDateTime::now_utc().format(&Rfc3339)?;
    let comment = comment(
        &format!("{}, prepared {prepared_at}", reuse.description),
        report,
    )
    .replace('\'', "''");
    let template = ident(&reuse.name);
    sqlx::raw_sql(AssertSqlSafe(format!(
        "ALTER DATABASE {} RENAME TO {template};
         ALTER DATABASE {template} WITH ALLOW_CONNECTIONS false;
         COMMENT ON DATABASE {template} IS '{comment}'",
        ident(target)
    )))
    .execute(&mut *admin)
    .await?;
    Ok(())
}

/// `target` recreated as a copy of `template`; how, how long and how large.
async fn copy(admin: &mut PgConnection, template: &str, target: &str) -> anyhow::Result<String> {
    unused(admin, target, "the database of the run").await?;
    sqlx::raw_sql(AssertSqlSafe(format!(
        "DROP DATABASE IF EXISTS {}",
        ident(target)
    )))
    .execute(&mut *admin)
    .await?;
    unused(admin, template, "the template").await?;
    // PostgreSQL 18 clones the files where the file system can (copy_file_range); a
    // server that does not know the setting copies them.
    let method = match sqlx::raw_sql("SET file_copy_method = clone")
        .execute(&mut *admin)
        .await
    {
        Ok(_) => "clone",
        Err(e) => {
            progress(&format!(
                "file_copy_method = clone refused ({e}): the files are copied"
            ));
            "copy"
        }
    };
    let started = Instant::now();
    sqlx::raw_sql(AssertSqlSafe(format!(
        "CREATE DATABASE {} TEMPLATE {} STRATEGY FILE_COPY",
        ident(target),
        ident(template)
    )))
    .execute(&mut *admin)
    .await
    .with_context(|| format!("copying the template {template}"))?;
    let took = started.elapsed();
    let size: i64 = sqlx::query_scalar("SELECT pg_database_size($1)")
        .bind(target)
        .fetch_one(&mut *admin)
        .await?;
    Ok(format!(
        "copy of {template} in {took:.1?} (FILE_COPY, file_copy_method = {method}), {} MiB",
        size / (1 << 20)
    ))
}

/// Drops the templates of the bench beyond the [`KEPT`] newest; one that cannot be
/// dropped is reported and kept.
async fn prune(admin: &mut PgConnection, reuse: &Reuse) -> anyhow::Result<()> {
    let names: Vec<String> = sqlx::query_scalar(
        "SELECT datname::text FROM pg_database WHERE starts_with(datname, $1) ORDER BY oid",
    )
    .bind(format!("{PREFIX}{}_", reuse.bench))
    .fetch_all(&mut *admin)
    .await?;
    let names: Vec<String> = names
        .into_iter()
        .filter(|n| is_template_of(reuse.bench, n))
        .collect();
    for old in surplus(&names) {
        if *old == reuse.name {
            continue;
        }
        let dropped = match unused(admin, old, "the old template").await {
            Ok(()) => sqlx::raw_sql(AssertSqlSafe(format!("DROP DATABASE {}", ident(old))))
                .execute(&mut *admin)
                .await
                .map(|_| ())
                .map_err(anyhow::Error::from),
            Err(e) => Err(e),
        };
        match dropped {
            Ok(()) => progress(&format!("template {old} dropped ({KEPT} kept per bench)")),
            Err(e) => progress(&format!("template {old} kept: {e:#}")),
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::measures::BENCHES;

    fn inputs() -> Inputs {
        Inputs::of("core")
    }

    #[test]
    fn every_bench_lists_the_sources_of_its_preparation() {
        for bench in BENCHES {
            let sources = sources(bench.name);
            assert!(
                sources
                    .iter()
                    .any(|(file, text)| *file != "template.rs"
                        && text.contains("template::prepared(")),
                "{}: no source listed for its preparation makes the prepared database",
                bench.name
            );
        }
    }

    /// The workspace crates a text names (`kartenn_<crate>::`), `kartenn-bench` aside.
    fn named_crates(text: &str) -> Vec<String> {
        let mut out = Vec::new();
        let mut rest = text;
        while let Some(at) = rest.find("kartenn_") {
            let tail = &rest[at..];
            let end = tail
                .find(|c: char| !(c.is_ascii_lowercase() || c == '_'))
                .unwrap_or(tail.len());
            if tail[end..].starts_with("::") && &tail[..end] != "kartenn_bench" {
                out.push(tail[..end].replace('_', "-"));
            }
            rest = &tail[end..];
        }
        out.sort();
        out.dedup();
        out
    }

    #[test]
    fn every_crate_a_preparation_names_is_in_its_fingerprint() {
        assert_eq!(
            named_crates("use kartenn_store_pg::x; kartenn_bench::y; \"kartenn_bench_tpl_\""),
            ["kartenn-store-pg"]
        );
        for bench in BENCHES {
            let covered: Vec<&str> = crate_hashes(crates(bench.name))
                .into_iter()
                .map(|(name, _)| name)
                .collect();
            // This module names the modules for their migrations only, which the
            // fingerprint covers on their own.
            for (file, text) in sources(bench.name)
                .into_iter()
                .filter(|(file, _)| *file != "template.rs")
            {
                for named in named_crates(text) {
                    assert!(
                        covered.contains(&named.as_str()),
                        "{}: {file} names {named}, absent from its crates",
                        bench.name
                    );
                }
            }
        }
        let core: Vec<&str> = crate_hashes(crates("core"))
            .into_iter()
            .map(|(name, _)| name)
            .collect();
        assert!(
            core.contains(&"kartenn-workspace-hack"),
            "transitively: {core:?}"
        );
        assert!(
            !core.contains(&"kartenn-testing"),
            "no development dependency: {core:?}"
        );
    }

    #[test]
    fn a_migration_added_changes_the_fingerprint() {
        let base = inputs();
        let before = fingerprint("core", 42, "Reduced", &base);
        assert_eq!(before, fingerprint("core", 42, "Reduced", &inputs()));
        let mut more = base.clone();
        more.migrations
            .push(("core", i64::MAX, b"checksum of a new migration"));
        assert_ne!(before, fingerprint("core", 42, "Reduced", &more));
        let mut changed = base;
        changed.migrations[0].2 = b"another checksum";
        assert_ne!(before, fingerprint("core", 42, "Reduced", &changed));
    }

    #[test]
    fn a_crate_or_the_lock_changed_changes_the_fingerprint() {
        let base = inputs();
        let before = fingerprint("core", 42, "Reduced", &base);
        let mut crate_changed = base.clone();
        crate_changed.crates[0].1 ^= 1;
        assert_ne!(before, fingerprint("core", 42, "Reduced", &crate_changed));
        let mut lock_changed = base;
        lock_changed.lock ^= 1;
        assert_ne!(before, fingerprint("core", 42, "Reduced", &lock_changed));
    }

    #[test]
    fn seed_scale_bench_and_sources_change_the_fingerprint() {
        let base = inputs();
        let before = fingerprint("core", 42, "Reduced", &base);
        assert_ne!(before, fingerprint("core", 43, "Reduced", &base));
        assert_ne!(before, fingerprint("core", 42, "Full", &base));
        assert_ne!(before, fingerprint("telemetry", 42, "Reduced", &base));
        let mut edited = base;
        edited.sources[0].1 = "an edited generator";
        assert_ne!(before, fingerprint("core", 42, "Reduced", &edited));
        // Fields are delimited: moving a byte from one to the next changes the value.
        let empty = Inputs {
            migrations: Vec::new(),
            sources: Vec::new(),
            crates: Vec::new(),
            lock: 0,
        };
        assert_ne!(
            fingerprint("ab", 1, "c", &empty),
            fingerprint("a", 1, "bc", &empty)
        );
    }

    #[test]
    fn only_names_of_templates_of_the_bench_are_templates() {
        let name = name("core", 0x0123_4567_89ab_cdef);
        assert_eq!(name, "kartenn_bench_tpl_core_0123456789abcdef");
        assert!(name.len() <= 63, "a PostgreSQL identifier");
        assert!(is_template_of("core", &name));
        assert!(name.starts_with(TARGET_PREFIX));
        for other in [
            "kartenn_bench",
            "kartenn_bench_policy",
            "kartenn_bench_tpl_core_0123456789abcde",
            "kartenn_bench_tpl_core_0123456789abcdef0",
            "kartenn_bench_tpl_core_0123456789ABCDEF",
            "kartenn_bench_tpl_core_x_0123456789abcdef",
            "kartenn_bench_tpl_coreX0123456789abcdef",
            "kartenn_bench_tpl_rbac_0123456789abcdef",
            "postgres",
        ] {
            assert!(!is_template_of("core", other), "{other}");
        }
        let longest = BENCHES
            .iter()
            .map(|b| super::name(b.name, u64::MAX).len())
            .max();
        assert!(longest.is_some_and(|l| l <= 63));
    }

    #[test]
    fn the_three_newest_are_kept() {
        let names: Vec<String> = (0..5).map(|i| format!("t{i}")).collect();
        assert_eq!(surplus(&names), &names[..2]);
        assert!(surplus(&names[..3]).is_empty());
        assert!(surplus(&[]).is_empty());
    }

    #[test]
    fn the_comment_keeps_the_report() {
        let report = "INFO checkpoint before the measures: true\nINFO policy dataset: 3\n";
        let comment = comment("kartenn-bench core, seed 42, prepared now", report);
        assert_eq!(report_of(&comment), report);
        assert_eq!(report_of("a comment of another time"), "");
    }
}
