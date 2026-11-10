-- The pod ranges of a node as the connector reads them (`spec.podCIDRs`; REQ-019-13,
-- Q-019-14; task 019-3c), versioned with the node: `range_ids` cites the networks of the
-- core created for those of them inside a pod container of the cluster, and the rule
-- `kube.node.range_outside` warns of the others, which get no network.

CALL public.kartenn_add_versioned_column('node', 'pod_cidrs cidr[] NOT NULL DEFAULT ''{}''');

-- The node citing a network, asked whenever a prefix of the core closes (reference
-- `kube.node` → `prefix`, refused while cited).
CREATE INDEX node_ranges ON node USING GIN (range_ids);
