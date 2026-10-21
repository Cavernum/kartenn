-- The origins of a batch of objects (grouped read, REQ-016-1): the primary key is GiST,
-- whose `= ANY` over hundreds of objects reads by bitmap; a btree by object reads them
-- by equality. Partial on a condition always true but stated by no filter: only a query
-- that names its objects (`object_id = ANY(…)` implies it) may use it, so the filters by
-- property keep `property_origin_by_origin` instead of a scan of this one in object order.
CREATE INDEX property_origin_by_object ON property_origin
    USING btree (object_id) INCLUDE (property_id, origin_object_id, valid_period)
    WHERE object_id IS NOT NULL;
