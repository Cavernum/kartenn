<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

<script setup lang="ts">
// The « Interfaces » tab `interfaces-of` (task 18cc), on the record of an address, of a
// network and of a VLAN: the interfaces that hold the address, or that carry the VLAN; for a
// network, the interfaces of the machines that hold one of its addresses, with the address
// (the machines of `?network=`, then the addresses of each). No heading of its own: the tab
// of the shell names its panel, the grid its rows. Next pages read as the window nears the
// end, never again after a page refused.
import { ApiProblem, MAX_PAGES, reads, type ObjectRef } from "@kartenn/sdk";
import {
  DataGrid,
  EmptyState,
  ErrorState,
  InlineAlert,
  Stack,
  type CellRef,
  type GridColumn,
  type GridWindow,
} from "@kartenn/ui";
import { computed } from "vue";
import { useList, useNotified, useRead, useReads, type List } from "./data.ts";
import { arrival, followLink, hrefOf, PAGES } from "./dom.ts";
import { useHosted, useTexts } from "./host.ts";
import InterfaceGrid from "./InterfaceGrid.vue";
import PageRoot from "./PageRoot.vue";
import { Names, within, type Interface, type Machine, type MachineAddress } from "./reads.ts";
import RetryAlert from "./RetryAlert.vue";

const props = defineProps<{ object: ObjectRef | null; row: unknown }>();

/** The rows left before the end of the window when the next page is read. */
const AHEAD = 20;

const t = useTexts();
const { context, element, queryClient } = useHosted();
const names = useNotified((changed, live) => new Names(changed, live));
const problemOf = (error: unknown) => (error instanceof ApiProblem ? error : undefined);
/** A read again from the server, its failure gone (« Réessayer »). */
const refetch = (options: { queryKey: readonly unknown[] }) =>
  void queryClient.refetchQueries({ queryKey: options.queryKey, exact: true });

const network = computed(() => (props.object?.type === "prefix" ? props.object.id : null));

/** The next page of a list once the window reaches its end; never after a page refused. */
function ahead(list: { state: { value: List<unknown> }; next(): void }, w: GridWindow, n: number) {
  const s = list.state.value;
  if (w.end < n - AHEAD || !s.more || s.fetching || s.error || s.pages >= MAX_PAGES) return;
  list.next();
}

// ------------------------------------------------------------------- an address, a VLAN

const interfaces = useList<Interface, never>(() => {
  const object = props.object;
  if (!object || object.type === "prefix") return null;
  const filter = object.type === "vlan" ? { vlan: object.id } : { address: object.id };
  return reads.list("/v1/interfaces", filter) as never;
});
const listed = computed(() => interfaces.state.value);

// ------------------------------------------------------------------------------ a network

type Prefix = { value: string };
type Addresses = { items: MachineAddress[] };
const prefixRead = () => (network.value ? reads.object<Prefix>("prefix", network.value) : null);
const prefix = useRead<Prefix, never>(() => prefixRead() as never);
const machines = useList<Machine, never>(() =>
  network.value ? (reads.list("/v1/machines", { network: network.value }) as never) : null,
);
const addressReads = () =>
  (machines.state.value.items ?? []).map((m) =>
    reads.sub<Addresses>("machines.machine", m.id, "addresses"),
  );
const held = useReads<Addresses, never>(() => addressReads() as never);

/** Each address of the network held by a machine, with its interface. */
const networkRows = computed(() => {
  const value = prefix.value.data?.value;
  if (value === undefined) return [];
  const list = machines.state.value.items ?? [];
  return list.flatMap((machine, i) =>
    (held.value[i]?.data?.items ?? [])
      .filter((a) => within(a.value, value))
      .map((a) => ({
        id: `${machine.id}/${a.address.id}`,
        iface: a.interface.id,
        address: a.address.id,
        name: a.interface_name,
        machine: machine.id,
        machineName: machine.name,
        value: a.value,
        primary: a.primary,
      })),
  );
});
const networkPending = computed(
  () => prefix.value.pending || machines.state.value.pending || held.value.some((r) => r.pending),
);
const networkError = computed(
  () =>
    machines.state.value.error ??
    prefix.value.error ??
    held.value.find((r) => r.error)?.error ??
    null,
);
/** « Réessayer »: each read of the network that failed. */
function retryNetwork(): void {
  if (machines.state.value.error) machines.retry();
  const options = prefixRead();
  if (prefix.value.error && options) refetch(options);
  addressReads().forEach((options, i) => {
    if (held.value[i]?.error) refetch(options);
  });
}
const networkColumns = computed<GridColumn[]>(() => [
  { id: "name", label: t("interfaces.name"), width: "md", mono: true },
  { id: "machine", label: t("interfaces.machine"), width: "fill" },
  { id: "value", label: t("tab.address"), width: "md", mono: true },
  { id: "primary", label: t("tab.primary"), width: "sm", priority: 2 },
]);
const link = (page: string) => {
  void context.value.time;
  return hrefOf(page);
};
/** Enter on a cell without link: the record of the interface of its row, as in the others. */
function onActivate(cell: CellRef): void {
  const row = networkRows.value.find((r) => r.id === cell.row);
  if (!row) return;
  arrival.byLink = true;
  followLink(element, link(PAGES.interface(row.iface)));
}

const capped = computed(() => {
  const s = network.value ? machines.state.value : listed.value;
  return s.more && s.pages >= MAX_PAGES;
});
</script>

<template>
  <PageRoot :heading="false">
    <Stack gap="4">
      <template v-if="network">
        <ErrorState
          v-if="networkRows.length === 0 && networkError"
          :title="t('tab.failed')"
          :problem="problemOf(networkError)"
          :retry-label="t('retry')"
          @retry="retryNetwork"
        >
          {{ problemOf(networkError) ? "" : t("network") }}
        </ErrorState>
        <EmptyState
          v-else-if="networkRows.length === 0 && !networkPending"
          :title="t('tab.noneNetwork')"
        >
          {{ t("tab.noneText") }}
        </EmptyState>
        <template v-else>
          <RetryAlert
            v-if="networkError"
            :title="t('tab.failed')"
            :error="networkError"
            @retry="retryNetwork"
          />
          <InlineAlert
            v-if="capped"
            tone="info"
            :title="t('tab.first', { n: networkRows.length })"
          />
          <DataGrid
            :label="t('tab.title')"
            :columns="networkColumns"
            :rows="networkRows"
            row-key="id"
            :loading="networkRows.length === 0"
            :loading-rows="3"
            :row-count="networkPending || machines.state.value.more ? -1 : networkRows.length"
            @window="(w) => ahead(machines, w, networkRows.length)"
            @activate="onActivate"
          >
            <template #cell="{ row: line, column }">
              <a
                v-if="column.id === 'name'"
                :href="link(PAGES.interface(line.iface))"
                class="k-mono"
                >{{ line.name }}</a
              >
              <a v-else-if="column.id === 'machine'" :href="link(PAGES.machine(line.machine))">{{
                line.machineName
              }}</a>
              <a
                v-else-if="column.id === 'value'"
                :href="link(`/addresses/${encodeURIComponent(line.address)}`)"
                class="k-mono"
                >{{ line.value }}</a
              >
              <template v-else>{{ t(line.primary ? "tab.yes" : "tab.no") }}</template>
            </template>
          </DataGrid>
        </template>
      </template>
      <template v-else>
        <ErrorState
          v-if="listed.items === undefined && listed.error"
          :title="t('tab.failed')"
          :problem="problemOf(listed.error)"
          :retry-label="t('retry')"
          @retry="interfaces.retry()"
        >
          {{ problemOf(listed.error) ? "" : t("network") }}
        </ErrorState>
        <EmptyState v-else-if="listed.items?.length === 0" :title="t('tab.none')">
          {{ t("tab.noneText") }}
        </EmptyState>
        <template v-else>
          <RetryAlert
            v-if="listed.error"
            :title="t('tab.failed')"
            :error="listed.error"
            @retry="interfaces.retry()"
          />
          <InlineAlert
            v-if="capped"
            tone="info"
            :title="t('tab.first', { n: listed.items?.length ?? 0 })"
          />
          <InterfaceGrid
            :label="t('tab.title')"
            :interfaces="listed.items ?? []"
            :names="names"
            :loading="listed.items === undefined"
            :more="listed.more"
            @window="(w) => ahead(interfaces, w, listed.items?.length ?? 0)"
          />
        </template>
      </template>
    </Stack>
  </PageRoot>
</template>
