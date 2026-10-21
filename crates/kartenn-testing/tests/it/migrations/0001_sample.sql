-- A table readable by the application role and nothing more: a clone keeps the grants
-- of its template (INV-11, ADR 0028 décision 7).
CREATE TABLE sample (n integer NOT NULL);
INSERT INTO sample (n) VALUES (1), (2), (3);
GRANT SELECT ON sample TO kartenn_app;
