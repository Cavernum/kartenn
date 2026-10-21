//! Tests of the template engine and of `#[pg_test]` (ADR 0028), against the cluster of
//! `DATABASE_URL`. One binary for the crate (ADR 0027).

mod attribute;
mod engine;

/// The migrations of `tests/it/migrations`, for the tests of the attribute.
pub(crate) static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("tests/it/migrations");
