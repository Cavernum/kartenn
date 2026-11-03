<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

<script setup lang="ts">
// The record of a cluster (`/x/machines/clusters/<id>`; task 18cd, the behaviour of the 18a
// kept): its owner and its site, then its machines — its members and those it hosts
// (`/v1/clusters/<id>/machines`) — in a grid, each with its link and its tie to the cluster.
// Each read through the shared cache at the dates of the context; a failure said with its
// codes and « Réessayer », never read again by itself.
import { ApiProblem, MAX_PAGES, queryClient, reads } from "@kartenn/sdk";
import {
  AttributeList,
  DataGrid,
  EmptyState,
  ErrorState,
  Heading,
  KeyLabel,
  Skeleton,
  Stack,
  type AttributeItem,
  type GridColumn,
  type GridWindow,
} from "@kartenn/ui";
import { computed } from "vue";
import CitedName from "./CitedName.vue";
import { useList, useNotified, useRead } from "./data.ts";
import { hrefOf, PAGES } from "./dom.ts";
import { useHosted, useTexts } from "./host.ts";
import NotSet from "./NotSet.vue";
import { Names, type Cluster, type Machine } from "./reads.ts";
import RetryAlert from "./RetryAlert.vue";

const props = defineProps<{ id: string }>();

const t = useTexts();
const { context } = useHosted();
const names = useNotified((changed, live) => new Names(changed, live));
/** The identifier of the heading of its machines (one record per element). */
const heading = "k-machines-cluster-machines";

const READS = {
  cluster: () => reads.object<Cluster>("machines.cluster", props.id),
  machines: () => reads.subPages<Machine>("machines.cluster", props.id, "machines"),
};
/** The cluster read again from the server, its failure gone (« Réessayer »). */
function retryCluster(): void {
  const { queryKey } = READS.cluster() as { queryKey: readonly unknown[] };
  void queryClient.refetchQueries({ queryKey, exact: true });
}

const read = useRead<Cluster, never>(() => READS.cluster() as never);
const machines = useList<Machine, never>(() => READS.machines() as never);
const cluster = computed(() => read.value.data);
const list = computed(() => machines.state.value);
const problemOf = (error: unknown) => (error instanceof ApiProblem ? error : undefined);
const link = (page: string) => {
  void context.value.time;
  return hrefOf(page);
};

/** Its attributes; a value absent said by the list (« non renseigné »). */
const attributes = computed<AttributeItem[]>(() => [
  { key: "owner", label: t("machines.owner") },
  { key: "site", label: t("machines.site") },
]);

const columns = computed<GridColumn[]>(() => [
  { id: "name", label: t("machines.name"), width: "fill" },
  { id: "kind", label: t("machines.kind"), width: "sm" },
  { id: "relation", label: t("clusters.relation"), width: "md" },
]);
const rows = computed(() =>
  (list.value.items ?? []).map((m) => {
    const member = m.member_of?.id === props.id;
    const hosted = !!m.host && "cluster" in m.host && m.host.cluster.id === props.id;
    const relation = [member && t("clusters.member"), hosted && t("clusters.hosted")]
      .filter(Boolean)
      .join(", ");
    return { id: m.id, name: m.name, kind: m.kind, relation };
  }),
);

/** The next page once the window of the grid reaches its last row. */
function ahead(w: GridWindow): void {
  const s = list.value;
  if (w.end >= (s.items?.length ?? 0) - 1 && s.more && !s.fetching && !s.error) {
    if (s.pages < MAX_PAGES) machines.next();
  }
}
</script>

<template>
  <Stack gap="6">
    <Stack gap="2">
      <Heading :level="1" tabindex="-1" :class="cluster ? 'k-mono' : undefined">
        {{ cluster ? cluster.slug : t("clusters.record") }}
      </Heading>
      <ErrorState
        v-if="!cluster && read.error"
        :title="t('clusters.failedOne')"
        :problem="problemOf(read.error)"
        :retry-label="t('retry')"
        @retry="retryCluster"
      >
        {{ problemOf(read.error) ? "" : t("network") }}
      </ErrorState>
    </Stack>

    <Skeleton v-if="!cluster && !read.error" variant="text" :rows="2" />
    <AttributeList v-else-if="cluster" :items="attributes">
      <template #owner>
        <CitedName v-if="cluster.owner" :id="cluster.owner.id" :names="names" type="core.entity" />
      </template>
      <template #site>
        <CitedName v-if="cluster.site" :id="cluster.site.id" :names="names" type="core.site" />
      </template>
    </AttributeList>

    <section :aria-labelledby="heading">
      <Stack gap="4">
        <Heading :id="heading" :level="2">{{ t("clusters.machines") }}</Heading>
        <ErrorState
          v-if="list.items === undefined && list.error"
          :title="t('machines.failed')"
          :problem="problemOf(list.error)"
          :retry-label="t('retry')"
          @retry="machines.retry()"
        />
        <EmptyState v-else-if="list.items?.length === 0" :title="t('clusters.noMachines')">
          {{ t("clusters.noMachinesText") }}
        </EmptyState>
        <template v-else>
          <RetryAlert
            v-if="list.error"
            :title="t('machines.failed')"
            :error="list.error"
            @retry="machines.retry()"
          />
          <DataGrid
            :label="t('clusters.machines')"
            :columns="columns"
            :rows="rows"
            row-key="id"
            :loading="list.items === undefined"
            :loading-rows="2"
            :row-count="list.more ? -1 : rows.length"
            @window="ahead"
          >
            <template #cell="{ row, column }">
              <a v-if="column.id === 'name'" :href="link(PAGES.machine(row.id))">{{ row.name }}</a>
              <KeyLabel
                v-else-if="column.id === 'kind'"
                :value="row.kind"
                :label="t(`kind.${row.kind}`)"
                plain
              />
              <template v-else-if="row.relation">{{ row.relation }}</template>
              <NotSet v-else />
            </template>
          </DataGrid>
        </template>
      </Stack>
    </section>
  </Stack>
</template>
