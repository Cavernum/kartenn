//! Test databases cloned from a migrated template (ADR 0028).
//!
//! `#[sqlx::test(migrator = …)]` creates an empty database for each test and replays every
//! migration in it (about 0.5 s). Here the migrations run once, in a template named by
//! their fingerprint (`_kt_tpl_<empreinte>`), and each test receives a clone of it
//! (`CREATE DATABASE … TEMPLATE … STRATEGY = WAL_LOG`, about 0.1 s).
//!
//! - [`pg_test`] is the attribute that replaces `#[sqlx::test(migrator = …)]`; it calls
//!   [`run`].
//! - The template is built once per fingerprint for the whole cluster, under an advisory
//!   lock, in `_kt_build_<empreinte>`, then renamed (atomic) and frozen
//!   (`ALLOW_CONNECTIONS false`, `IS_TEMPLATE true`): a clone never sees a half-migrated
//!   template.
//! - The database of a test that succeeds is dropped; that of a failed test stays for
//!   examination, as with sqlx. [`clean`] (binary `kartenn-testing-clean`) drops what
//!   remains and the templates unused for a day; it must run while no test runs.
//!
//! The cluster is that of `DATABASE_URL`; its database holds the bookkeeping table
//! `_kartenn_test.templates`.

use std::collections::HashSet;
use std::future::Future;
use std::str::FromStr;
use std::sync::Mutex;
use std::time::Duration;

use sha2::{Digest, Sha256};
use sqlx::migrate::Migrator;
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use sqlx::{AssertSqlSafe, Connection, Executor, PgConnection, PgPool};

pub use kartenn_testing_macros::pg_test;

/// Prefix of the database of a test.
pub const TEST_PREFIX: &str = "_kt_test_";
/// Prefix of a template.
pub const TEMPLATE_PREFIX: &str = "_kt_tpl_";
/// Prefix of a template being built.
pub const BUILD_PREFIX: &str = "_kt_build_";
/// Prefix of the databases of `#[sqlx::test]`, which [`clean`] drops as well.
const SQLX_PREFIX: &str = "_sqlx_test_";
/// Advisory lock of the bookkeeping schema (`kt_tests` in ASCII).
const SCHEMA_LOCK: i64 = 0x6b74_5f74_6573_7473;

/// What a test returns: `()` or a `Result`, as for `#[sqlx::test]`.
pub trait Outcome {
    fn is_success(&self) -> bool;
}

impl Outcome for () {
    fn is_success(&self) -> bool {
        true
    }
}

impl<T, E> Outcome for Result<T, E> {
    fn is_success(&self) -> bool {
        self.is_ok()
    }
}

/// Runs `test` on a clone of the template of `migrator`, on a current-thread Tokio runtime
/// (that of `#[sqlx::test]`). The clone is dropped when the test succeeds.
///
/// # Panics
/// Without `DATABASE_URL`, or when the cluster refuses the template or the clone.
pub fn run<F, Fut>(migrator: &'static Migrator, test_path: &'static str, test: F) -> Fut::Output
where
    F: FnOnce(PgPool) -> Fut,
    Fut: Future,
    Fut::Output: Outcome,
{
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("failed to start the Tokio runtime of the test")
        .block_on(async move {
            let master = master_options();
            let mut conn = PgConnection::connect_with(&master)
                .await
                .expect("failed to connect to DATABASE_URL");
            let template = ensure_template(&mut conn, migrator)
                .await
                .expect("failed to build the template database");
            let name = database_name(test_path);
            clone(&mut conn, &template, &name)
                .await
                .expect("failed to clone the template database");
            let pool = PgPoolOptions::new()
                // As sqlx: no single test takes every connection.
                .max_connections(5)
                .idle_timeout(Some(Duration::from_secs(1)))
                .connect_with(master.clone().database(&name))
                .await
                .expect("failed to connect the test pool");
            let outcome = test(pool.clone()).await;
            if tokio::time::timeout(Duration::from_secs(10), pool.close())
                .await
                .is_err()
            {
                eprintln!("test {test_path} held onto its pool after exiting");
            }
            if outcome.is_success()
                && let Err(error) = drop_database(&mut conn, &name).await
            {
                eprintln!("failed to drop the test database {name}: {error}");
            }
            let _ = conn.close().await;
            outcome
        })
}

/// The options of `DATABASE_URL`.
///
/// # Panics
/// Without `DATABASE_URL` or when it does not parse.
#[must_use]
pub fn master_options() -> PgConnectOptions {
    let url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    PgConnectOptions::from_str(&url).expect("DATABASE_URL does not parse")
}

/// The fingerprint of a migrator: SHA-256 of its (version, checksum) pairs in version
/// order, 16 hexadecimal characters. A migration added or changed changes it.
#[must_use]
pub fn fingerprint(migrator: &Migrator) -> String {
    let mut migrations: Vec<_> = migrator.iter().collect();
    migrations.sort_by_key(|m| (m.version, m.migration_type.is_down_migration()));
    let mut hasher = Sha256::new();
    for m in migrations {
        hasher.update(m.version.to_be_bytes());
        hasher.update([u8::from(m.migration_type.is_down_migration())]);
        hasher.update((m.checksum.len() as u64).to_be_bytes());
        hasher.update(&m.checksum[..]);
    }
    hex(&hasher.finalize()[..8])
}

/// The name of the database of a test: its path hashed, 63 bytes as with sqlx.
#[must_use]
pub fn database_name(test_path: &str) -> String {
    let hash = Sha256::digest(test_path.as_bytes());
    let name = format!("{TEST_PREFIX}{}", hex(&hash[..27]));
    debug_assert_eq!(name.len(), 63);
    name
}

/// The templates checked by this process: the lock is taken once per fingerprint.
static READY: Mutex<Option<HashSet<String>>> = Mutex::new(None);

/// The template of `migrator`, built if no process has built it yet.
///
/// # Errors
/// Those of the cluster or of the migrations.
///
/// # Panics
/// When a test thread panicked while holding the process cache.
pub async fn ensure_template(
    conn: &mut PgConnection,
    migrator: &Migrator,
) -> Result<String, sqlx::Error> {
    let print = fingerprint(migrator);
    let template = format!("{TEMPLATE_PREFIX}{print}");
    let known = READY
        .lock()
        .expect("poisoned")
        .as_ref()
        .is_some_and(|set| set.contains(&template));
    if !known {
        build_template(conn, migrator).await?;
        READY
            .lock()
            .expect("poisoned")
            .get_or_insert_with(HashSet::new)
            .insert(template.clone());
    }
    Ok(template)
}

/// Builds the template of `migrator` unless it exists, under the advisory lock of its
/// fingerprint, and records its use. Several processes may call it at once: one builds,
/// the others wait and find it. Not cached: [`ensure_template`] is.
///
/// # Errors
/// Those of the cluster or of the migrations.
///
/// # Panics
/// Never: the fingerprint is 16 hexadecimal characters.
pub async fn build_template(
    conn: &mut PgConnection,
    migrator: &Migrator,
) -> Result<String, sqlx::Error> {
    let print = fingerprint(migrator);
    let template = format!("{TEMPLATE_PREFIX}{print}");
    let build = format!("{BUILD_PREFIX}{print}");
    // The 64 bits of the fingerprint, as the signed key of the advisory lock.
    let key = i64::from_be_bytes(
        u64::from_str_radix(&print, 16)
            .expect("hexadecimal")
            .to_be_bytes(),
    );
    bookkeeping(conn).await?;
    sqlx::query("SELECT pg_advisory_lock($1)")
        .bind(key)
        .execute(&mut *conn)
        .await?;
    // Released below whatever happens: the lock outlives this connection's transactions.
    let locked = async {
        if !exists(conn, &template).await? {
            drop_database(conn, &build).await?;
            exec(conn, format!(r#"CREATE DATABASE "{build}""#)).await?;
            let options = master_options().database(&build);
            let mut target = PgConnection::connect_with(&options).await?;
            migrator.run(&mut target).await?;
            target.close().await?;
            // The server process of the closed connection may still be exiting.
            retry(
                conn,
                &format!(r#"ALTER DATABASE "{build}" RENAME TO "{template}""#),
            )
            .await?;
            exec(
                conn,
                format!(r#"ALTER DATABASE "{template}" WITH ALLOW_CONNECTIONS false"#),
            )
            .await?;
            exec(
                conn,
                format!(r#"ALTER DATABASE "{template}" WITH IS_TEMPLATE true"#),
            )
            .await?;
        }
        sqlx::query(
            "INSERT INTO _kartenn_test.templates (name, last_used) VALUES ($1, now())
             ON CONFLICT (name) DO UPDATE SET last_used = now()",
        )
        .bind(&template)
        .execute(&mut *conn)
        .await?;
        Ok::<_, sqlx::Error>(())
    }
    .await;
    sqlx::query("SELECT pg_advisory_unlock($1)")
        .bind(key)
        .execute(&mut *conn)
        .await?;
    locked.map(|()| template)
}

/// Drops `name` if it exists, then clones `template` into it.
///
/// # Errors
/// Those of the cluster.
pub async fn clone(conn: &mut PgConnection, template: &str, name: &str) -> Result<(), sqlx::Error> {
    drop_database(conn, name).await?;
    exec(
        conn,
        format!(r#"CREATE DATABASE "{name}" TEMPLATE "{template}" STRATEGY = WAL_LOG"#),
    )
    .await
}

/// Drops `name` if it exists, then creates it empty: for a test that applies the
/// migrations itself, part of them first (a migration that moves data).
///
/// # Errors
/// Those of the cluster.
pub async fn create_empty(conn: &mut PgConnection, name: &str) -> Result<(), sqlx::Error> {
    drop_database(conn, name).await?;
    exec(conn, format!(r#"CREATE DATABASE "{name}""#)).await
}

/// What [`clean`] dropped.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Cleaned {
    /// Test databases (`_kt_test_`, `_sqlx_test_`) and abandoned builds.
    pub databases: usize,
    /// Templates unused for longer than the given age, or unknown to the bookkeeping.
    pub templates: usize,
}

/// Drops the test databases left by failed tests, the abandoned builds, and the
/// templates unused for longer than `max_age`. To run while no test runs: it would drop
/// their databases (`k-check` holds the exclusive PostgreSQL lock).
///
/// # Errors
/// Those of the cluster.
pub async fn clean(conn: &mut PgConnection, max_age: Duration) -> Result<Cleaned, sqlx::Error> {
    bookkeeping(conn).await?;
    let names: Vec<String> = sqlx::query_scalar("SELECT datname::text FROM pg_database")
        .fetch_all(&mut *conn)
        .await?;
    let stale: Vec<String> = sqlx::query_scalar(
        "SELECT name FROM _kartenn_test.templates
         WHERE last_used < now() - make_interval(secs => $1)",
    )
    .bind(max_age.as_secs_f64())
    .fetch_all(&mut *conn)
    .await?;
    let recorded: Vec<String> = sqlx::query_scalar("SELECT name FROM _kartenn_test.templates")
        .fetch_all(&mut *conn)
        .await?;
    let mut cleaned = Cleaned::default();
    for name in names {
        if [TEST_PREFIX, SQLX_PREFIX, BUILD_PREFIX]
            .iter()
            .any(|prefix| name.starts_with(prefix))
        {
            drop_database(conn, &name).await?;
            cleaned.databases += 1;
        } else if name.starts_with(TEMPLATE_PREFIX)
            && (stale.contains(&name) || !recorded.contains(&name))
        {
            exec(
                conn,
                format!(r#"ALTER DATABASE "{name}" WITH IS_TEMPLATE false"#),
            )
            .await?;
            drop_database(conn, &name).await?;
            sqlx::query("DELETE FROM _kartenn_test.templates WHERE name = $1")
                .bind(&name)
                .execute(&mut *conn)
                .await?;
            cleaned.templates += 1;
        }
    }
    // sqlx keeps its own list: forget the databases dropped above.
    let sqlx_list: Option<String> =
        sqlx::query_scalar("SELECT to_regclass('_sqlx_test.databases')::text")
            .fetch_one(&mut *conn)
            .await?;
    if sqlx_list.is_some() {
        conn.execute(
            "DELETE FROM _sqlx_test.databases d
             WHERE NOT EXISTS (SELECT 1 FROM pg_database p WHERE p.datname = d.db_name)",
        )
        .await?;
    }
    Ok(cleaned)
}

/// The bookkeeping table of the templates, in the database of `DATABASE_URL`.
async fn bookkeeping(conn: &mut PgConnection) -> Result<(), sqlx::Error> {
    // One implicit transaction: concurrent `CREATE … IF NOT EXISTS` may otherwise collide.
    exec(
        conn,
        format!(
            "SELECT pg_advisory_xact_lock({SCHEMA_LOCK});
             CREATE SCHEMA IF NOT EXISTS _kartenn_test;
             CREATE TABLE IF NOT EXISTS _kartenn_test.templates (
                 name text PRIMARY KEY,
                 last_used timestamptz NOT NULL
             );"
        ),
    )
    .await
}

async fn exists(conn: &mut PgConnection, name: &str) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM pg_database WHERE datname = $1)")
        .bind(name)
        .fetch_one(&mut *conn)
        .await
}

/// Drops `name`, ending the sessions left on it. `FORCE` must be allowed to end every
/// backend of the database: an autovacuum worker busy on a fresh clone belongs to no test
/// role, and the drop is then refused (42501) until it is done, a matter of moments.
///
/// # Errors
///
/// The error of the drop, past 10 s of refusals for another role's backend.
pub async fn drop_database(conn: &mut PgConnection, name: &str) -> Result<(), sqlx::Error> {
    let mut attempts = 0;
    loop {
        match exec(
            conn,
            format!(r#"DROP DATABASE IF EXISTS "{name}" WITH (FORCE)"#),
        )
        .await
        {
            Ok(()) => return Ok(()),
            // 42501: insufficient_privilege, the backend of another role.
            Err(sqlx::Error::Database(e))
                if e.code().as_deref() == Some("42501") && attempts < 100 =>
            {
                attempts += 1;
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
            Err(e) => return Err(e),
        }
    }
}

/// Runs a statement built here: the names it quotes are hexadecimal or from
/// `pg_database`, never from input.
async fn exec(conn: &mut PgConnection, sql: String) -> Result<(), sqlx::Error> {
    conn.execute(AssertSqlSafe(sql)).await?;
    Ok(())
}

/// Runs `sql` again for up to 5 s while the database is still in use.
async fn retry(conn: &mut PgConnection, sql: &str) -> Result<(), sqlx::Error> {
    let mut attempts = 0;
    loop {
        match exec(conn, sql.to_owned()).await {
            Ok(()) => return Ok(()),
            // 55006: object_in_use.
            Err(sqlx::Error::Database(e))
                if e.code().as_deref() == Some("55006") && attempts < 50 =>
            {
                attempts += 1;
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
            Err(e) => return Err(e),
        }
    }
}

/// The URI of a request of a test: a write (`POST`, `PUT`, `PATCH`, `DELETE`) asks to
/// apply its changeset (`apply=own`, spec 024, REQ-024-13) unless it names `apply`
/// itself; the tests write as authors who apply their own changesets when they may.
#[must_use]
pub fn applied_by_author(method: &str, uri: &str) -> String {
    let write = matches!(
        method.to_ascii_uppercase().as_str(),
        "POST" | "PUT" | "PATCH" | "DELETE"
    );
    let query = uri.split_once('?').map_or("", |(_, q)| q);
    if !write
        || query
            .split('&')
            .any(|p| p == "apply" || p.starts_with("apply="))
    {
        return uri.to_owned();
    }
    let separator = if uri.contains('?') { '&' } else { '?' };
    format!("{uri}{separator}apply=own")
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    bytes.iter().fold(String::new(), |mut s, b| {
        let _ = write!(s, "{b:02x}");
        s
    })
}
