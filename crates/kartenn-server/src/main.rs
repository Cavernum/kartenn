//! kartenn server binary: composition, configuration, observability.
//!
//! - `kartenn-server migrate <database-url>`: the migration job (D9), run once per
//!   deployment before the instances, with the owner of the schema: migrations, history
//!   partitions ahead of time, declarations of the core (004-25, 003/P12).
//! - `kartenn-server audit verify <database-url> [<public-key-hex>]`: verifies the chain of
//!   the audit seals, and their signatures with the public key (spec 012, REQ-012-14);
//!   exits with 2 when the journal is altered.

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

#[tokio::main]
async fn main() -> anyhow::Result<ExitCode> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let connect = |database: &str| {
        let database = database.to_owned();
        async move {
            sqlx::PgPool::connect(&database)
                .await
                .with_context(|| format!("connecting to {database}"))
        }
    };
    match args.as_slice() {
        [command, database] if command == "migrate" => {
            let pool = connect(database).await?;
            kartenn_store_pg::maintenance::deploy(&pool).await?;
            println!("kartenn {}: deployed", env!("CARGO_PKG_VERSION"));
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
        _ => bail!(
            "usage: kartenn-server migrate <database-url>\n       kartenn-server audit verify <database-url> [<public-key-hex>]"
        ),
    }
}
