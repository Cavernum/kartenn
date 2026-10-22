-- B-tree indexes on the flow of the derived tables and on the identifier of the flows
-- (task 011-10): the primary keys `WITHOUT OVERLAPS` are GiST indexes, slow to probe for a
-- batch of flows (`flow_id = ANY(...)`), which the re-evaluation reads, locks, ends and
-- removes by lots of 512 flows (design 011 « Réévaluation », steps 2 and 3).

CREATE INDEX flow_by_id ON flow (id);
CREATE INDEX flow_endpoint_by_flow ON flow_endpoint_address (flow_id);
CREATE INDEX flow_evaluation_by_flow ON flow_evaluation (flow_id);
CREATE INDEX flow_summary_by_flow ON flow_summary (flow_id);
