-- The date of the last login is not kept (spec 025, Q-025-1 « Reprise le 2026-10-25 »;
-- 005/K3 amended): it would track the activity of a person. Only the groups of its issuer
-- that login carried stay (`last_login_groups`), written at each login, erased by the
-- pseudonymisation. Dropping the column drops its column grant.

ALTER TABLE person DROP COLUMN last_login_at;
