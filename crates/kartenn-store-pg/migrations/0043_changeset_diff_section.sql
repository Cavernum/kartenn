-- Sections of the differential contributed by the modules (spec 008, REQ-008-26; ADR 0026,
-- clause 13): typed rows, an object with its value before and after, over a period (NULL:
-- the changeset's). Recorded in the transaction of the changeset, stored with its journal
-- like `changeset_diff`, paginated by `seq` within a section.

CREATE TABLE changeset_diff_section (
    changeset_id uuid NOT NULL REFERENCES changeset (id),
    section      text NOT NULL CHECK (section ~ '^[a-z][a-z0-9_]*\.[a-z0-9_.-]+$'),
    seq          bigint GENERATED ALWAYS AS IDENTITY,
    object_id    uuid NOT NULL,
    kind         text NOT NULL,
    before       jsonb,
    after        jsonb,
    period       tstzrange CHECK (period IS NULL OR NOT isempty(period))
);
CREATE INDEX changeset_diff_section_page ON changeset_diff_section (changeset_id, section, seq);
CREATE INDEX changeset_diff_section_by_object ON changeset_diff_section (object_id);
GRANT SELECT, INSERT ON changeset_diff_section TO kartenn_app;
