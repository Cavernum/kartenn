-- Drafts kept by the server (spec 024, REQ-024-1 to 7a; tasks 024-5, 6): a private set of
-- operations of a person, which applies nothing. Not a datum of the repository: no
-- bitemporal history; the version is the entity tag of `If-Match` (006/H19).
CREATE TABLE draft (
    id          uuid PRIMARY KEY,
    author_id   uuid NOT NULL REFERENCES person (id),
    title       text NOT NULL CHECK (title <> '' AND char_length(title) <= 200),
    -- `{summary, description}`, both optional: the motive of the changeset submitted.
    reason      jsonb NOT NULL DEFAULT '{}' CHECK (jsonb_typeof(reason) = 'object'),
    -- `Operation`s of the contract, checked by its deserialiser on every write.
    operations  jsonb NOT NULL DEFAULT '[]' CHECK (jsonb_typeof(operations) = 'array'),
    version     bigint NOT NULL DEFAULT 1 CHECK (version >= 1),
    created_at  timestamptz NOT NULL DEFAULT now(),
    updated_at  timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX draft_by_author ON draft (author_id, updated_at DESC);
-- The retention (REQ-024-7): the drafts inactive the longest.
CREATE INDEX draft_by_update ON draft (updated_at);
GRANT SELECT, INSERT, UPDATE, DELETE ON draft TO kartenn_app;

-- The draft a changeset was submitted from (REQ-024-5, M3): no reference, the draft is
-- deleted once the changeset is created.
ALTER TABLE changeset ADD COLUMN draft_id uuid;

-- The deletion of the drafts inactive for 90 days, daily (REQ-024-7).
INSERT INTO job (name, due_at, period) VALUES ('draft-retention', now(), interval '1 day');
