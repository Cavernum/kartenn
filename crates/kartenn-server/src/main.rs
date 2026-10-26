//! kartenn server binary: composition, configuration, observability.
//!
//! The URL of the database is the first argument, or, without it, the content of the file
//! `[database] url_file` of the configuration (spec 015, REQ-015-4), which keeps the URL
//! off the command line; `migrate` and `breakglass create` read that configuration with
//! `--config <file>`. `kartenn-server --version` prints the version.
//!
//! - `kartenn-server migrate <database-url>`: the migration job (D9), run once per
//!   deployment before the instances, with the owner of the schema: migrations, history
//!   partitions ahead of time, declarations of the core (004-25, 003/P12).
//! - `kartenn-server audit verify <database-url> [<public-key-hex>]`: verifies the chain of
//!   the audit seals, and their signatures with the public key (spec 012, REQ-012-14);
//!   exits with 2 when the journal is altered.
//! - `kartenn-server run <database-url> [<config-file>]`: an instance (technical logs,
//!   OpenTelemetry, sealing and export of the audit, the API with `[http]`), until SIGTERM
//!   or Ctrl-C (specs 012, 006); SIGHUP reads the TLS certificate again.
//! - `kartenn-server audit purge <database-url> [<config-file>]`: retention of the audit
//!   by criticality, with the owner of the schema (REQ-012-16), for a daily timer.
//! - `kartenn-server openapi [<path>]`: writes the OpenAPI contract of this build, its
//!   modules included, to the reference of the repository or to `path` (spec 006,
//!   REQ-006-34; spec 008, REQ-008-9).
//! - `kartenn-server breakglass create <database-url> <name> <hours> <key-file>`: a
//!   break-glass account valid `hours` (24 at most), with the owner of the schema (spec
//!   005, REQ-005-14), member of the delivered group `secours`, bound to the role
//!   `administrator`, until its end (K17, spec 025 G7: the bootstrap of a new instance,
//!   after `migrate`); prints its passphrase and TOTP URI once.

use std::path::Path;
use std::process::ExitCode;

use anyhow::{Context, bail};

/// 32 octets from 64 hexadecimal digits.
fn public_key(text: &str) -> anyhow::Result<[u8; 32]> {
    let bytes: Vec<u8> = (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(text.get(i..i + 2).unwrap_or("x"), 16))
        .collect::<Result<_, _>>()
        .context("the public key is hexadecimal")?;
    <[u8; 32]>::try_from(bytes.as_slice()).context("the public key has 32 octets")
}

/// Creates a break-glass account valid `hours` (1 to 24), its TOTP secret sealed under the
/// key ring of `key_file`; prints its passphrase and TOTP URI once (REQ-005-14).
async fn create_breakglass(
    pool: &sqlx::PgPool,
    name: &str,
    hours: &str,
    key_file: &str,
) -> anyhow::Result<()> {
    let hours: u64 = hours.parse().context("hours is a number")?;
    if !(1..=24).contains(&hours) {
        bail!("a break-glass account is valid from 1 to 24 hours");
    }
    let ring = kartenn_auth::keys::KeyRing::parse(
        &std::fs::read_to_string(key_file).with_context(|| format!("reading {key_file}"))?,
    )?;
    let account = kartenn_auth::breakglass::create(&ring, name);
    let valid_until = sqlx::types::time::OffsetDateTime::now_utc()
        + std::time::Duration::from_secs(hours * 3_600);
    let instance = kartenn_service::audit::Instance::new(env!("CARGO_PKG_VERSION"));
    kartenn_store_pg::breakglass::create(
        pool,
        &kartenn_store_pg::breakglass::Stored {
            name,
            password_hash: &account.password_hash,
            totp_sealed: &account.totp_sealed,
            totp_key_id: &account.totp_key_id,
            valid_until,
        },
        &instance,
    )
    .await
    .map_err(|e| anyhow::anyhow!(e.0))?;
    println!("break-glass account {name}, valid {hours} h; shown once:");
    println!("passphrase: {}", account.passphrase);
    println!("TOTP: {}", account.otpauth);
    Ok(())
}

/// Resolves at SIGTERM or Ctrl-C.
async fn shutdown() {
    #[cfg(unix)]
    {
        let mut term = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("installing the SIGTERM handler");
        tokio::select! {
            _ = term.recv() => {}
            _ = tokio::signal::ctrl_c() => {}
        }
    }
    #[cfg(not(unix))]
    let _ = tokio::signal::ctrl_c().await;
}

/// The pool of an instance, its transactions bounded by `bound`.
async fn connect_bounded(
    database: &str,
    bound: std::time::Duration,
) -> anyhow::Result<sqlx::PgPool> {
    let redacted = || kartenn_telemetry::capture::redact_credentials(database);
    let options = database
        .parse::<sqlx::postgres::PgConnectOptions>()
        .with_context(|| format!("parsing {}", redacted()))?;
    sqlx::PgPool::connect_with(kartenn_server::app::bounded(options, bound))
        .await
        .with_context(|| format!("connecting to {}", redacted()))
}

/// A database URL given as an argument, rather than a file name.
fn is_url(argument: &str) -> bool {
    argument.starts_with("postgres://") || argument.starts_with("postgresql://")
}

/// `[<database-url>] [<config-file>]`: the URL when the first is one, then the file.
fn url_and_file(rest: &[String]) -> Option<(Option<&str>, Option<&str>)> {
    match rest {
        [] => Some((None, None)),
        [one] if is_url(one) => Some((Some(one), None)),
        [one] => Some((None, Some(one))),
        [url, file] if is_url(url) => Some((Some(url), Some(file))),
        _ => None,
    }
}

/// `args` without `--config <file>`, and that file.
fn config_option(args: &[String]) -> anyhow::Result<(Vec<String>, Option<String>)> {
    let mut rest = Vec::new();
    let mut file = None;
    let mut args = args.iter();
    while let Some(argument) = args.next() {
        if argument == "--config" {
            let Some(path) = args.next() else {
                bail!("--config needs a file");
            };
            file = Some(path.clone());
        } else {
            rest.push(argument.clone());
        }
    }
    Ok((rest, file))
}

/// The configuration of `file` and the URL of the database: `argument` or `[database]
/// url_file` (spec 015, REQ-015-4).
fn configured(
    file: Option<&str>,
    argument: Option<&str>,
) -> anyhow::Result<(kartenn_server::config::Config, String)> {
    let config = kartenn_server::config::load(file.map(Path::new))?;
    config.i18n.install()?;
    let url = config.database_url(argument)?;
    Ok((config, url))
}

#[tokio::main]
#[allow(clippy::too_many_lines)] // one arm per subcommand
async fn main() -> anyhow::Result<ExitCode> {
    let (args, config_file) = config_option(&std::env::args().skip(1).collect::<Vec<_>>())?;
    let connect = |database: &str| {
        let database = database.to_owned();
        async move {
            sqlx::PgPool::connect(&database).await.with_context(|| {
                format!(
                    "connecting to {}",
                    kartenn_telemetry::capture::redact_credentials(&database)
                )
            })
        }
    };
    match args.as_slice() {
        [flag] if flag == "--version" || flag == "-V" => {
            println!("kartenn-server {}", env!("CARGO_PKG_VERSION"));
            Ok(ExitCode::SUCCESS)
        }
        [command, rest @ ..] if command == "migrate" && rest.len() <= 1 => {
            let (_, database) =
                configured(config_file.as_deref(), rest.first().map(String::as_str))?;
            let pool = connect(&database).await?;
            let modules = kartenn_server::modules::registry()?;
            let deployed = kartenn_store_pg::maintenance::deploy(&pool, &modules).await?;
            // The bindings reported on groups (spec 025, REQ-025-14): `compte-…` groups to
            // regroup by hand, bindings of whole issuers closed.
            for line in deployed
                .rbac_report
                .iter()
                .flat_map(kartenn_store_pg::rbac_migration::Report::lines)
            {
                println!("{line}");
            }
            // The data the modules change after the deployment (spec 011, F21).
            for line in kartenn_server::modules::migrate_data(&pool, &modules).await? {
                println!("{line}");
            }
            println!("kartenn {}: deployed", env!("CARGO_PKG_VERSION"));
            Ok(ExitCode::SUCCESS)
        }
        [command, rest @ ..] if command == "run" && config_file.is_none() => {
            let Some((argument, file)) = url_and_file(rest) else {
                bail!("usage: kartenn-server run [<database-url>] [<config-file>]");
            };
            let (config, database) = configured(file, argument)?;
            let pool = connect_bounded(&database, config.write_transaction_timeout()).await?;
            let report = kartenn_server::app::run(pool, &config, shutdown()).await?;
            if report.lost_technical > 0 {
                eprintln!("kartenn: {} technical records lost", report.lost_technical);
            }
            Ok(ExitCode::SUCCESS)
        }
        [audit, purge, rest @ ..]
            if audit == "audit" && purge == "purge" && config_file.is_none() =>
        {
            let Some((argument, file)) = url_and_file(rest) else {
                bail!("usage: kartenn-server audit purge [<database-url>] [<config-file>]");
            };
            let (config, database) = configured(file, argument)?;
            let pool = connect(&database).await?;
            let sinks: Vec<&str> = config
                .audit
                .sinks
                .iter()
                .map(kartenn_server::config::Sink::name)
                .collect();
            let done = kartenn_store_pg::audit_purge::purge(
                &pool,
                &config.audit.retention_days.retention(),
                &sinks,
            )
            .await
            .map_err(|e| anyhow::anyhow!(e.0))?;
            for month in &done {
                println!(
                    "{} {}: {}",
                    month.criticality,
                    month.month,
                    if month.dropped {
                        "purged"
                    } else {
                        "kept, not sealed or exported yet"
                    }
                );
            }
            Ok(ExitCode::SUCCESS)
        }
        [audit, verify, database, rest @ ..] if audit == "audit" && verify == "verify" => {
            let public = rest.first().map(|k| public_key(k)).transpose()?;
            let pool = connect(database).await?;
            let instance = kartenn_service::audit::Instance::new(env!("CARGO_PKG_VERSION"));
            let report = kartenn_store_pg::audit_verify::verify_and_record(
                &pool,
                public.as_ref(),
                &instance,
            )
            .await
            .map_err(|e| anyhow::anyhow!(e.0))?;
            println!(
                "{} seals, {} events checked, {} purged, {} late unsealed",
                report.seals, report.events, report.purged, report.late_unsealed
            );
            Ok(match report.broken {
                None => ExitCode::SUCCESS,
                Some(broken) => {
                    eprintln!("ALTERED at seal {}: {}", broken.seal, broken.reason);
                    ExitCode::from(2)
                }
            })
        }
        [command, rest @ ..] if command == "openapi" && rest.len() <= 1 => {
            let path = rest.first().map_or_else(
                || Path::new(env!("CARGO_MANIFEST_DIR")).join(kartenn_rest::openapi::REFERENCE),
                std::path::PathBuf::from,
            );
            let registry = kartenn_server::modules::registry()?;
            kartenn_rest::openapi::write_with(&path, &registry, kartenn_server::modules::routers())
                .with_context(|| format!("writing {}", path.display()))?;
            Ok(ExitCode::SUCCESS)
        }
        [bg, create, rest @ ..]
            if bg == "breakglass" && create == "create" && matches!(rest.len(), 3 | 4) =>
        {
            let (argument, [name, hours, key_file]) = match rest {
                [url, name, hours, key_file] => (Some(url.as_str()), [name, hours, key_file]),
                [name, hours, key_file] => (None, [name, hours, key_file]),
                _ => unreachable!("three or four arguments"),
            };
            let (_, database) = configured(config_file.as_deref(), argument)?;
            create_breakglass(&connect(&database).await?, name, hours, key_file).await?;
            Ok(ExitCode::SUCCESS)
        }
        _ => bail!(
            "usage: kartenn-server --version\n       kartenn-server migrate [<database-url>] [--config <file>]\n       kartenn-server run [<database-url>] [<config-file>]\n       kartenn-server audit verify <database-url> [<public-key-hex>]\n       kartenn-server audit purge [<database-url>] [<config-file>]\n       kartenn-server openapi [<path>]\n       kartenn-server breakglass create [<database-url>] <name> <hours> <key-file> [--config <file>]\n(without <database-url>, `[database] url_file` of the configuration names a file holding it)"
        ),
    }
}
