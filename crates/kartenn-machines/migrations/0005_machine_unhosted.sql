-- The machines without a host (`/v1/machines?host:exists=false`; spec 021, A28; task
-- 021-18f): the first level of the hosting tree, a page read in the order of the list
-- (name, identifier) from this index alone, however few the machines without a host. The
-- member of a cluster is not hosted by it: `member_cluster_id` stays out of the predicate.

CREATE INDEX machine_unhosted ON machine (name, id)
    WHERE host_machine_id IS NULL AND host_cluster_id IS NULL;
