-- Resolution of properties along the addressing tree (spec 003, task 003-8; ADR 0016, P9).

-- Every object of the addressing tree with its domain and the value that places it in the
-- tree: the prefix, the first address of a range, the address; null for the domain (the
-- root). A range hangs off its network, never off another range (P3).
CREATE VIEW addressing_object AS
    SELECT id, domain_id, prefix::inet AS value, 'prefix' AS kind, valid_period FROM prefix
    UNION ALL
    SELECT id, domain_id, lower(range), 'range', valid_period FROM ip_range
    UNION ALL
    SELECT id, domain_id, address, 'address', valid_period FROM address
    UNION ALL
    SELECT id, id, NULL::inet, 'domain', valid_period FROM addressing_domain;
GRANT SELECT ON addressing_object TO kartenn_app;

-- Removes the assignment of a property from an object over [from_time, to_time) (null
-- `to_time`: unbounded), keeping the parts of the versions outside it as new versions of
-- `changeset`. The service writes the new assignment, if any, afterwards.
CREATE FUNCTION public.kartenn_unassign(
    target uuid, property uuid, from_time timestamptz, to_time timestamptz, changeset uuid)
RETURNS integer
LANGUAGE plpgsql SET search_path = pg_catalog, public AS $$
DECLARE
    portion tstzrange := tstzrange(from_time, to_time);
    r property_assignment;
    piece property_assignment;
    touched integer := 0;
BEGIN
    FOR r IN SELECT * FROM property_assignment
             WHERE object_id = target AND property_id = property AND valid_period && portion
    LOOP
        DELETE FROM property_assignment
        WHERE object_id = target AND property_id = property AND valid_period = r.valid_period;
        touched := touched + 1;
        IF lower(r.valid_period) < from_time THEN
            piece := r;
            piece.valid_period := tstzrange(lower(r.valid_period), from_time);
            piece.changeset_id := changeset;
            INSERT INTO property_assignment SELECT piece.*;
        END IF;
        IF to_time IS NOT NULL AND (upper_inf(r.valid_period) OR upper(r.valid_period) > to_time) THEN
            piece := r;
            piece.valid_period := tstzrange(to_time, upper(r.valid_period));
            piece.changeset_id := changeset;
            INSERT INTO property_assignment SELECT piece.*;
        END IF;
    END LOOP;
    RETURN touched;
END $$;
GRANT EXECUTE ON FUNCTION public.kartenn_unassign(uuid, uuid, timestamptz, timestamptz, uuid) TO kartenn_app;
