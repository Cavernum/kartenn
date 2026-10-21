//! `kartenn-testing-clean`: drops the test databases left by failed tests and the
//! templates unused for a day (ADR 0028). To run while no test runs, under the exclusive
//! PostgreSQL lock of `k-heavy --pg`.

use std::time::Duration;

use sqlx::{Connection, PgConnection};

/// A template unused for longer than this is dropped.
const MAX_AGE: Duration = Duration::from_hours(24);

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), sqlx::Error> {
    let mut conn = PgConnection::connect_with(&kartenn_testing::master_options()).await?;
    let cleaned = kartenn_testing::clean(&mut conn, MAX_AGE).await?;
    conn.close().await?;
    if cleaned != kartenn_testing::Cleaned::default() {
        eprintln!(
            "kartenn-testing-clean: {} test database(s), {} template(s) dropped",
            cleaned.databases, cleaned.templates
        );
    }
    Ok(())
}
