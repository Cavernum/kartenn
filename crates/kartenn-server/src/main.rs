//! kartenn server binary: composition, configuration, observability.
//!
//! `kartenn-server migrate <database-url>`: the migration job (D9), run once per
//! deployment before the instances, with the owner of the schema: migrations, history
//! partitions ahead of time, declarations of the core (004-25, 003/P12).

use anyhow::{Context, bail};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.as_slice() {
        [command, database] if command == "migrate" => {
            let pool = sqlx::PgPool::connect(database)
                .await
                .with_context(|| format!("connecting to {database}"))?;
            kartenn_store_pg::maintenance::deploy(&pool).await?;
            println!("kartenn {}: deployed", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        _ => bail!("usage: kartenn-server migrate <database-url>"),
    }
}
