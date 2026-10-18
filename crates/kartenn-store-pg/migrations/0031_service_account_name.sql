-- The name of a service account is its Basic user (spec 005, REQ-005-12): unique among
-- the service accounts.
CREATE UNIQUE INDEX person_service_account_name ON person (display_name)
    WHERE kind = 'service_account';
