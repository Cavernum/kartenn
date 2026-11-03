<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

<script setup lang="ts">
// The record of a machine (task 18a, in Vue by 18cb; design 021 « Plugin Machines », « Fiche
// et interfaces »), in sections: its attributes (kind, owner, site, host, cluster), the move
// of its host (`HostSection`), its interfaces with their 802.1Q mode, their addresses and
// their state (`StatusBadge`), the machines it hosts, and the creation of an interface,
// added to the draft (W10). Each read through the shared cache at the dates of the context;
// writes offered at the present only (W9).
import { ApiProblem, MAX_PAGES, queryClient, reads } from "@kartenn/sdk";
import {
  AttributeList,
  DataGrid,
  EmptyState,
  ErrorState,
  Heading,
  KeyLabel,
  NoValue,
  Skeleton,
  Stack,
  StatusBadge,
  TextField,
  type AttributeItem,
  type GridColumn,
  type GridWindow,
} from "@kartenn/ui";
import { computed, ref, shallowRef, watch, type ShallowRef } from "vue";
import CitedName from "./CitedName.vue";
import { useList, useNotified, useRead, type List } from "./data.ts";
import { hrefOf, PAGES } from "./dom.ts";
import DraftForm from "./DraftForm.vue";
import HostSection from "./HostSection.vue";
import { useHosted, usePresent, useTexts } from "./host.ts";
import { modeOf, Names, type Interface, type Machine, type MachineCard } from "./reads.ts";
import RetryAlert from "./RetryAlert.vue";

const props = defineProps<{ id: string }>();

const t = useTexts();
const { context } = useHosted();
const present = usePresent();
const names = useNotified((changed, live) => new Names(changed, live));
/** The identifiers of the headings of the sections (one record per element). */
const ids = {
  host: "k-machines-host",
  ifaces: "k-machines-ifaces",
  hosted: "k-machines-hosted",
  create: "k-machines-iface-create",
};

const READS = {
  card: () => reads.object<MachineCard>("machines.machine", props.id),
  interfaces: () => reads.list("/v1/interfaces", { machine: props.id }),
  hosted: () => reads.subPages<Machine>("machines.machine", props.id, "hosted"),
};
/** A read again from the server, its failure gone (« Réessayer »). */
function retry(read: keyof typeof READS): void {
  const { queryKey } = READS[read]() as { queryKey: readonly unknown[] };
  void queryClient.refetchQueries({ queryKey, exact: true });
}

const card = useRead<MachineCard, never>(() => READS.card() as never);
const interfaces = useList<Interface, never>(() => READS.interfaces() as never);
const hosted = useList<Machine, never>(() => READS.hosted() as never);

const machine = computed(() => card.value.data);
const problemOf = (error: unknown) => (error instanceof ApiProblem ? error : undefined);
const link = (page: string) => {
  void context.value.time;
  return hrefOf(page);
};
const host = computed(() => {
  const h = machine.value?.host;
  if (!h) return null;
  return "machine" in h
    ? { type: "machines.machine" as const, id: h.machine.id, page: PAGES.machine(h.machine.id) }
    : { type: "machines.cluster" as const, id: h.cluster.id, page: PAGES.cluster(h.cluster.id) };
});
/** Its attributes, each by its slot; one absent (no owner, no host…) said by the list. */
const attributes = computed<AttributeItem[]>(() => [
  { key: "kind", label: t("machines.kind") },
  { key: "owner", label: t("machines.owner") },
  { key: "site", label: t("machines.site") },
  { key: "host", label: t("machines.host") },
  { key: "memberOf", label: t("machines.memberOf") },
]);

// ---------------------------------------------------------------------------- interfaces

const ifaceColumns = computed<GridColumn[]>(() => [
  { id: "name", label: t("interfaces.name"), width: "md", mono: true },
  { id: "mac", label: t("interfaces.mac"), width: "md", mono: true },
  { id: "mode", label: t("interfaces.mode"), width: "md" },
  { id: "addresses", label: t("interfaces.addresses"), width: "fill", mono: true, priority: 2 },
  { id: "state", label: t("interfaces.state"), width: "sm" },
]);
const ifaceRows = computed(() => {
  // The addresses of each interface, from the card of the machine.
  const addresses = new Map(
    (machine.value?.interfaces ?? []).map((i) => [i.id, i.addresses.map((a) => a.value)]),
  );
  return (interfaces.state.value.items ?? []).map((i) => ({
    id: i.id,
    name: i.name,
    mac: i.mac ?? null,
    mode: modeOf(i, names()),
    addresses: addresses.get(i.id) ?? [],
    enabled: i.enabled,
  }));
});

const hostedColumns = computed<GridColumn[]>(() => [
  { id: "name", label: t("machines.name"), width: "fill" },
  { id: "kind", label: t("machines.kind"), width: "sm" },
]);
const hostedRows = computed(() =>
  (hosted.state.value.items ?? []).map((m) => ({ id: m.id, name: m.name, kind: m.kind })),
);

/** The next page of a list once the window of its grid reaches its last row. */
function ahead(list: { state: Readonly<ShallowRef<List<unknown>>>; next(): void }, w: GridWindow) {
  const s = list.state.value;
  if (w.end >= (s.items?.length ?? 0) - 1 && s.more && !s.fetching && s.pages < MAX_PAGES) {
    list.next();
  }
}

/** A list's failure before its first page; after it, above the rows kept. */
const firstFailure = (s: { items: unknown; error: unknown }) =>
  s.items === undefined ? s.error : null;
const laterFailure = (s: { items: unknown; error: unknown }) =>
  s.items !== undefined ? s.error : null;

// ----------------------------------------------------------------------------- creation

const name = ref("");
const mac = ref("");
/** What is said on each field, by the name of the member of the operation. */
type Errors = Partial<Record<"name" | "mac", string | undefined>>;
const errors = shallowRef<Errors>({});
watch(name, () => (errors.value = { ...errors.value, name: undefined }));
watch(mac, () => (errors.value = { ...errors.value, mac: undefined }));

function creation() {
  const trimmed = name.value.trim();
  if (!trimmed) {
    errors.value = { name: t("form.required") };
    return null;
  }
  errors.value = {};
  const value = mac.value.trim();
  return {
    op: "machines.create_interface",
    machine: { id: props.id },
    name: trimmed,
    ...(value ? { mac: value } : {}),
  } as never;
}
function violations(problem: ApiProblem): boolean {
  const next: Errors = {};
  for (const v of problem.violations) {
    const field = v.pointer?.split("/").pop();
    if ((field === "name" || field === "mac") && !next[field]) {
      next[field] = `${v.detail} (${v.rule})`;
    }
  }
  errors.value = next;
  return Object.keys(next).length > 0;
}
function added(): void {
  name.value = "";
  mac.value = "";
  errors.value = {};
}
</script>

<template>
  <Stack gap="6">
    <Stack gap="2">
      <Heading :level="1" tabindex="-1">{{
        machine ? machine.name : t("machines.record")
      }}</Heading>
      <ErrorState
        v-if="!machine && card.error"
        :title="t('machines.failedOne')"
        :problem="problemOf(card.error)"
        :retry-label="t('retry')"
        @retry="retry('card')"
      >
        {{ problemOf(card.error) ? "" : t("network") }}
      </ErrorState>
    </Stack>

    <Skeleton v-if="!machine && !card.error" variant="text" :rows="3" />
    <AttributeList v-else-if="machine" :items="attributes">
      <template #kind>
        <KeyLabel :value="machine.kind" :label="t(`kind.${machine.kind}`)" />
      </template>
      <template #owner>
        <CitedName v-if="machine.owner" :id="machine.owner.id" :names="names" type="core.entity" />
      </template>
      <template #site>
        <CitedName v-if="machine.site" :id="machine.site.id" :names="names" type="core.site" />
      </template>
      <template #host>
        <CitedName v-if="host" :id="host.id" :names="names" :type="host.type" :page="host.page" />
      </template>
      <template #memberOf>
        <CitedName
          v-if="machine.member_of"
          :id="machine.member_of.id"
          :names="names"
          type="machines.cluster"
          :page="PAGES.cluster(machine.member_of.id)"
        />
      </template>
    </AttributeList>

    <!-- Moved only once the machine is read: without it, nothing to judge the hosts by. -->
    <section v-if="present() && machine" :aria-labelledby="ids.host">
      <Stack gap="4">
        <Heading :id="ids.host" :level="2">{{ t("move.title") }}</Heading>
        <HostSection :id="props.id" :machine="machine" />
      </Stack>
    </section>

    <section :aria-labelledby="ids.ifaces">
      <Stack gap="4">
        <Heading :id="ids.ifaces" :level="2">{{ t("interfaces.title") }}</Heading>
        <ErrorState
          v-if="firstFailure(interfaces.state.value)"
          :title="t('interfaces.failed')"
          :problem="problemOf(interfaces.state.value.error)"
          :retry-label="t('retry')"
          @retry="retry('interfaces')"
        />
        <EmptyState
          v-else-if="interfaces.state.value.items?.length === 0"
          :title="t('interfaces.none')"
        >
          {{ t("interfaces.noneText") }}
        </EmptyState>
        <template v-else>
          <RetryAlert
            v-if="laterFailure(interfaces.state.value)"
            :title="t('interfaces.failed')"
            :error="interfaces.state.value.error"
            @retry="interfaces.retry()"
          />
          <DataGrid
            :label="t('interfaces.title')"
            :columns="ifaceColumns"
            :rows="ifaceRows"
            row-key="id"
            :loading="interfaces.state.value.items === undefined"
            :loading-rows="2"
            :row-count="interfaces.state.value.more ? -1 : ifaceRows.length"
            @window="(w) => ahead(interfaces, w)"
          >
            <template #cell="{ row, column }">
              <a v-if="column.id === 'name'" :href="link(PAGES.interface(row.id))" class="k-mono">{{
                row.name
              }}</a>
              <template v-else-if="column.id === 'mac' || column.id === 'addresses'">
                <template v-if="column.id === 'mac' ? row.mac : row.addresses.length">{{
                  column.id === "mac" ? row.mac : row.addresses.join(", ")
                }}</template>
                <NoValue v-else />
              </template>
              <StatusBadge
                v-else-if="column.id === 'state'"
                :state="row.enabled ? 'ok' : 'neutral'"
                :label="t(row.enabled ? 'interfaces.enabled' : 'interfaces.disabled')"
              />
              <template v-else>{{ row.mode }}</template>
            </template>
          </DataGrid>
        </template>
      </Stack>
    </section>

    <section :aria-labelledby="ids.hosted">
      <Stack gap="4">
        <Heading :id="ids.hosted" :level="2">{{ t("machines.hosted") }}</Heading>
        <ErrorState
          v-if="firstFailure(hosted.state.value)"
          :title="t('machines.failed')"
          :problem="problemOf(hosted.state.value.error)"
          :retry-label="t('retry')"
          @retry="retry('hosted')"
        />
        <EmptyState
          v-else-if="hosted.state.value.items?.length === 0"
          :title="t('machines.noneHosted')"
        >
          {{ t("machines.noneHostedText") }}
        </EmptyState>
        <template v-else>
          <RetryAlert
            v-if="laterFailure(hosted.state.value)"
            :title="t('machines.failed')"
            :error="hosted.state.value.error"
            @retry="hosted.retry()"
          />
          <DataGrid
            :label="t('machines.hosted')"
            :columns="hostedColumns"
            :rows="hostedRows"
            row-key="id"
            :loading="hosted.state.value.items === undefined"
            :loading-rows="2"
            :row-count="hosted.state.value.more ? -1 : hostedRows.length"
            @window="(w) => ahead(hosted, w)"
          >
            <template #cell="{ row, column }">
              <a v-if="column.id === 'name'" :href="link(PAGES.machine(row.id))">{{ row.name }}</a>
              <KeyLabel v-else :value="row.kind" :label="t(`kind.${row.kind}`)" plain />
            </template>
          </DataGrid>
        </template>
      </Stack>
    </section>

    <section v-if="present()" :aria-labelledby="ids.create">
      <Stack gap="4">
        <Heading :id="ids.create" :level="2">{{ t("interfaces.create") }}</Heading>
        <DraftForm
          permission="machines.interface.edit"
          :operation="creation"
          :violations="violations"
          @added="added"
        >
          <TextField
            v-model="name"
            :label="t('interfaces.name')"
            :error="errors.name"
            mono
            required
          />
          <TextField
            v-model="mac"
            :label="t('interfaces.mac')"
            :hint="t('interfaces.macHint')"
            :error="errors.mac"
            mono
          />
        </DraftForm>
      </Stack>
    </section>
  </Stack>
</template>
