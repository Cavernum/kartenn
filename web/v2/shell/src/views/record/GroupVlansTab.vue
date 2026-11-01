<script setup lang="ts">
// The VLAN of a group (026, REQ-008-19; maquette E12, un groupe): its VLAN by VID, name
// and status, each a link to its record, read page by page at the dates of the page;
// three states. « Créer un VLAN » opens the edit panel on the group (`c` of the lists).
import { reads, type components } from "@kartenn/sdk";
import {
  Button,
  DataGrid,
  EmptyState,
  StatusBadge,
  type GridColumn,
  type GridWindow,
} from "@kartenn/ui";
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import { useInfiniteQuery } from "../../data/query.ts";
import KLink from "../../prefetch/KLink.vue";
import { useNextPage, type ListWindow } from "../../prefetch/nextPage.ts";
import NextPageAlert from "../../problems/NextPageAlert.vue";
import ProblemState from "../../problems/ProblemState.vue";
import { useTime } from "../../time/index.ts";
import { rowsOf } from "../data.ts";
import { VLAN_STATES } from "../parts/vlan.ts";

type Vlan = components["schemas"]["Vlan"];

const props = defineProps<{
  id: string;
  name: string;
  /** Why « Créer un VLAN » is refused, if it is (the permission, or the date). */
  blocked: { forbidden?: string; reason?: string };
}>();
const emit = defineEmits<{ create: [] }>();
const { t, te } = useI18n();
const time = useTime();

const vlans = useInfiniteQuery(() =>
  reads.list(
    "/v1/vlans",
    { group: props.id },
    { validAt: time.value.validAt, asOf: time.value.asOf },
  ),
);
const view = ref<ListWindow>({ end: 0, size: 0 });
useNextPage(vlans, view);
const onWindow = (window: GridWindow) => (view.value = { end: window.end, size: window.size });

const rows = computed(
  () => rowsOf(vlans.data.value) as unknown as (Vlan & Record<string, unknown>)[],
);
const columns = computed<GridColumn[]>(() => [
  { id: "vid", label: t("views.field.vid"), width: "sm", mono: true },
  { id: "name", label: t("views.field.name"), width: "fill", mono: true },
  { id: "status", label: t("views.field.status"), width: "md" },
]);
const statusWords = (status: string) => {
  const key = `views.value.status.${status}`;
  return te(key) ? t(key) : status;
};
</script>

<template>
  <NextPageAlert
    v-if="vlans.isFetchNextPageError.value"
    :error="vlans.error.value"
    :retry="vlans.fetchNextPage"
  />
  <ProblemState
    v-if="vlans.isError.value && !vlans.isFetchNextPageError.value"
    :title="t('views.vlan.vlansError')"
    :error="vlans.error.value"
    :retry="vlans.refetch"
  />
  <EmptyState
    v-else-if="vlans.isSuccess.value && rows.length === 0"
    :title="t('views.vlan.noVlans')"
  >
    {{ t("views.vlan.noVlansText") }}
    <template #action>
      <Button icon="plus" v-bind="props.blocked" @click="emit('create')">{{
        t("views.vlan.createVlan")
      }}</Button>
    </template>
  </EmptyState>
  <DataGrid
    v-else
    :label="t('views.vlan.vlansOf', { name: props.name })"
    :columns="columns"
    :rows="rows"
    row-key="id"
    :loading="vlans.isPending.value"
    :loading-rows="4"
    stack
    @window="onWindow"
  >
    <template #cell="{ row, column, value }">
      <KLink v-if="column.id === 'vid'" :to="`/vlans/${String(row.id)}`">{{ value }}</KLink>
      <StatusBadge
        v-else-if="column.id === 'status' && VLAN_STATES[String(value)]"
        :state="VLAN_STATES[String(value)]!"
        :label="statusWords(String(value))"
      />
      <template v-else>{{ value }}</template>
    </template>
  </DataGrid>
</template>
