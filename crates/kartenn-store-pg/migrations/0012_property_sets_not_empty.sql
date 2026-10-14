-- REQ-003-9, P11: a set value has at least one element; no value is a cleared assignment.
ALTER TABLE property_assignment ADD CONSTRAINT property_assignment_set_not_empty CHECK (
    coalesce(cardinality(v_uuids), 1) > 0 AND coalesce(cardinality(v_texts), 1) > 0
    AND coalesce(cardinality(v_ints), 1) > 0 AND coalesce(cardinality(v_dates), 1) > 0);
