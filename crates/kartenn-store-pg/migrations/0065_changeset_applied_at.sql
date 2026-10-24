-- The changesets applied by day over the last 30 days (spec 024, REQ-024-14; task 024-7):
-- `GET /v1/stats` reads them by date of application, which no index of 0003 or 0038
-- orders.
CREATE INDEX changeset_applied_at ON changeset (applied_at) WHERE state = 'applied';
