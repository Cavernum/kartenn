-- Readiness compares the latest applied migration with the binary's (REQ-006-36,
-- REQ-015-11). sqlx creates _sqlx_migrations for the owner alone: under the instance role
-- the query failed and /health/ready stayed at 503. Read only (INV-11): the application
-- never records nor alters a migration.

GRANT SELECT ON _sqlx_migrations TO kartenn_app;
