<script setup lang="ts">
// The networks that carry a VLAN (026, REQ-008-21; maquette E12, fiche d'un VLAN): each a
// link to its record, read page by page at the dates of the page, under the rights of the
// reader — a network one may not read is neither listed nor counted, which the tab says.
// Three states.
import { reads, type components } from "@kartenn/sdk";
import { DataGrid, EmptyState, Stack, type GridColumn, type GridWindow } from "@kartenn/ui";
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import { useInfiniteQuery } from "../../data/query.ts";
import KLink from "../../prefetch/KLink.vue";
import { useNextPage, type ListWindow } from "../../prefetch/nextPage.ts";
import NextPageAlert from "../../problems/NextPageAlert.vue";
import ProblemState from "../../problems/ProblemState.vue";
import { useTime } from "../../time/index.ts";
import { pageOfHref } from "../collections.ts";
import { rowsOf } from "../data.ts";

type Network = components["schemas"]["VlanNetwork"];

const props = defineProps<{ id: string; name: string }>();
const { t } = useI18n();
const time = useTime();

const networks = useInfiniteQuery(() =>
  reads.list(
    "/v1/vlans/{id}/prefixes",
    null,
    { validAt: time.value.validAt, asOf: time.value.asOf },
    { id: props.id },
  ),
);
const view = ref<ListWindow>({ end: 0, size: 0 });
useNextPage(networks, view);
const onWindow = (window: GridWindow) => (view.value = { end: window.end, size: window.size });

const rows = computed(
  () => rowsOf(networks.data.value) as unknown as (Network & Record<string, unknown>)[],
);
const columns = computed<GridColumn[]>(() => [
  { id: "prefix", label: t("views.vlan.network"), width: "fill", mono: true },
]);
</script>

<template>
  <Stack gap="2">
    <NextPageAlert
      v-if="networks.isFetchNextPageError.value"
      :error="networks.error.value"
      :retry="networks.fetchNextPage"
    />
    <ProblemState
      v-if="networks.isError.value && !networks.isFetchNextPageError.value"
      :title="t('views.vlan.networksError')"
      :error="networks.error.value"
      :retry="networks.refetch"
    />
    <EmptyState
      v-else-if="networks.isSuccess.value && rows.length === 0"
      :title="t('views.vlan.noNetworks')"
    >
      {{ t("views.vlan.noNetworksText") }}
    </EmptyState>
    <DataGrid
      v-else
      :label="t('views.vlan.networksOf', { name: props.name })"
      :columns="columns"
      :rows="rows"
      row-key="id"
      :loading="networks.isPending.value"
      :loading-rows="3"
      @window="onWindow"
    >
      <template #cell="{ row, value }">
        <KLink :to="pageOfHref(row.href) ?? `/prefixes/${String(row.id)}`">{{ value }}</KLink>
      </template>
    </DataGrid>
    <p class="k-vlan-note">{{ t("views.vlan.readableOnly") }}</p>
  </Stack>
</template>

<style>
@layer components {
  .k-vlan-note {
    color: var(--k-text-muted);
  }
}
</style>
