-- Approval policies on the measures of modules (spec 016, REQ-016-7, 8): their triggers,
-- `[measure, threshold]` pairs whose measure a module of the build declares; their origin
-- (`admin` for a policy written by changeset, `module` for one a module installed by
-- default); and the default policies already installed, never installed again.

ALTER TABLE approval_policy
    ADD COLUMN module_triggers jsonb NOT NULL DEFAULT '[]',
    ADD COLUMN origin text NOT NULL DEFAULT 'admin' CHECK (origin IN ('admin', 'module'));

CREATE TABLE module_default_policy (
    module       text NOT NULL,
    name         text NOT NULL,
    installed_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (module, name)
);
GRANT SELECT, INSERT ON module_default_policy TO kartenn_app;
