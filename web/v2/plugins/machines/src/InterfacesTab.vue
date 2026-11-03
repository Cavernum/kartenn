<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

<script setup lang="ts">
// The « Interfaces » tab `interfaces-of` (task 18cc), on the record of an address, of a
// network and of a VLAN: the interfaces that hold the address, or that carry the VLAN; for a
// network, each address of the network held by an interface, with its interface and its
// machine, in one list (`/v1/interfaces?network=`, each interface with its addresses of the
// network, task 18e). No heading of its own: the tab of the shell names its panel, the grid
// its rows. Next pages read as the window nears the end, never again after a page refused.
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
import CitedName from "./CitedName.vue";
import { useList, useNotified } from "./data.ts";
import { arrival, followLink, hrefOf, PAGES } from "./dom.ts";
import { useHosted, useTexts } from "./host.ts";
import InterfaceGrid from "./InterfaceGrid.vue";
import PageRoot from "./PageRoot.vue";
import { Names, type Interface } from "./reads.ts";
import RetryAlert from "./RetryAlert.vue";

const props = defineProps<{ object: ObjectRef | null; row: unknown }>();

/** The rows left before the end of the window when the next page is read. */
const AHEAD = 20;

const t = useTexts();
const { context, element } = useHosted();
const names = useNotified((changed, live) => new Names(changed, live));
const problemOf = (error: unknown) => (error instanceof ApiProblem ? error : undefined);

const network = computed(() => (props.object?.type === "prefix" ? props.object.id : null));

/** The interfaces of the address, of the VLAN or of the network. */
const interfaces = useList<Interface, never>(() => {
  const object = props.object;
  if (!object) return null;
  const filter =
    object.type === "prefix"
      ? { network: object.id }
      : object.type === "vlan"
        ? { vlan: object.id }
        : { address: object.id };
  return reads.list("/v1/interfaces", filter) as never;
});
const listed = computed(() => interfaces.state.value);

/** The next page once the window reaches the end of its `n` rows; never after a refusal. */
function ahead(w: GridWindow, n: number) {
  const s = listed.value;
  if (w.end < n - AHEAD || !s.more || s.fetching || s.error || s.pages >= MAX_PAGES) return;
  interfaces.next();
}

/** Each address of the network held by an interface, with its interface and its machine. */
const networkRows = computed(() =>
  (listed.value.items ?? []).flatMap((iface) =>
    (iface.addresses ?? []).map((a) => ({
      id: `${iface.id}/${a.id}`,
      iface: iface.id,
      address: a.id,
      name: iface.name,
      machine: iface.machine.id,
      value: a.value,
      primary: a.primary,
    })),
  ),
);
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

const rowCount = computed(() =>
  network.value ? networkRows.value.length : (listed.value.items?.length ?? 0),
);
const capped = computed(() => listed.value.more && listed.value.pages >= MAX_PAGES);
</script>

<template>
  <PageRoot :heading="false">
    <Stack gap="4">
      <ErrorState
        v-if="listed.items === undefined && listed.error"
        :title="t('tab.failed')"
        :problem="problemOf(listed.error)"
        :retry-label="t('retry')"
        @retry="interfaces.retry()"
      >
        {{ problemOf(listed.error) ? "" : t("network") }}
      </ErrorState>
      <EmptyState
        v-else-if="listed.items !== undefined && rowCount === 0 && !listed.more"
        :title="t(network ? 'tab.noneNetwork' : 'tab.none')"
      >
        {{ t("tab.noneText") }}
      </EmptyState>
      <template v-else>
        <RetryAlert
          v-if="listed.error"
          :title="t('tab.failed')"
          :error="listed.error"
          @retry="interfaces.retry()"
        />
        <InlineAlert v-if="capped" tone="info" :title="t('tab.first', { n: rowCount })" />
        <DataGrid
          v-if="network"
          :label="t('tab.title')"
          :columns="networkColumns"
          :rows="networkRows"
          row-key="id"
          :loading="listed.items === undefined"
          :loading-rows="3"
          :row-count="listed.items === undefined || listed.more ? -1 : networkRows.length"
          @window="(w) => ahead(w, networkRows.length)"
          @activate="onActivate"
        >
          <template #cell="{ row: line, column }">
            <a
              v-if="column.id === 'name'"
              :href="link(PAGES.interface(line.iface))"
              class="k-mono"
              >{{ line.name }}</a
            >
            <CitedName
              v-else-if="column.id === 'machine'"
              :id="line.machine"
              :names="names"
              type="machines.machine"
              :page="PAGES.machine(line.machine)"
            />
            <a
              v-else-if="column.id === 'value'"
              :href="link(`/addresses/${encodeURIComponent(line.address)}`)"
              class="k-mono"
              >{{ line.value }}</a
            >
            <template v-else>{{ t(line.primary ? "tab.yes" : "tab.no") }}</template>
          </template>
        </DataGrid>
        <InterfaceGrid
          v-else
          :label="t('tab.title')"
          :interfaces="listed.items ?? []"
          :names="names"
          :loading="listed.items === undefined"
          :more="listed.more"
          @window="(w) => ahead(w, listed.items?.length ?? 0)"
        />
      </template>
    </Stack>
  </PageRoot>
</template>
