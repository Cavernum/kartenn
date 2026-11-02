<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

<script setup lang="ts">
// The record of an interface (task 18cc; design 021 « Plugin Machines »): its attributes
// (machine, MAC, 802.1Q mode, state), the addresses attached to it, read from the card of
// its machine, and the change of its 802.1Q mode, added to the draft (W10) at the present
// only (W9). Each read through the shared cache at the dates of the context.
import { ApiProblem, reads } from "@kartenn/sdk";
import {
  DataGrid,
  EmptyState,
  ErrorState,
  Grid,
  Select,
  Skeleton,
  Stack,
  StatusBadge,
  type GridColumn,
  type ListOption,
} from "@kartenn/ui";
import { computed, nextTick, ref, shallowRef, useTemplateRef, watch } from "vue";
import CitedName from "./CitedName.vue";
import { useList, useNotified, useRead } from "./data.ts";
import { hrefOf, PAGES } from "./dom.ts";
import DraftForm from "./DraftForm.vue";
import { useHosted, usePresent, useTexts } from "./host.ts";
import { modeOf, Names, type Interface, type MachineCard, type Vlan } from "./reads.ts";
import RetryAlert from "./RetryAlert.vue";

const props = defineProps<{ id: string }>();

const t = useTexts();
const { context, queryClient } = useHosted();
const present = usePresent();
const names = useNotified((changed, live) => new Names(changed, live));
/** The identifiers of the headings of the sections (one record per element). */
const ids = { addresses: "k-machines-iface-addresses", mode: "k-machines-iface-mode" };

const read = useRead<Interface, never>(
  () => reads.object<Interface>("machines.interface", props.id) as never,
);
const iface = computed(() => read.value.data);
const cardRead = () =>
  iface.value ? reads.object<MachineCard>("machines.machine", iface.value.machine.id) : null;
const card = useRead<MachineCard, never>(() => cardRead() as never);

/** A read again from the server, its failure gone (« Réessayer »). */
function retry(options: { queryKey: readonly unknown[] } | null): void {
  if (options) void queryClient.refetchQueries({ queryKey: options.queryKey, exact: true });
}
const problemOf = (error: unknown) => (error instanceof ApiProblem ? error : undefined);

// ----------------------------------------------------------------------------- addresses

const addressColumns = computed<GridColumn[]>(() => [
  { id: "value", label: t("tab.address"), width: "fill", mono: true },
  { id: "primary", label: t("tab.primary"), width: "sm" },
]);
/** Its addresses, from the card of its machine. */
const addresses = computed(() =>
  (card.value.data?.interfaces.find((i) => i.id === props.id)?.addresses ?? []).map((a) => ({
    id: a.id,
    value: a.value,
    primary: a.primary,
  })),
);
const addressPage = (id: string) => {
  void context.value.time;
  return hrefOf(`/addresses/${encodeURIComponent(id)}`);
};

// ---------------------------------------------------------------------------------- mode

const vlans = useList<Vlan, never>(() => (present() ? (reads.list("/v1/vlans") as never) : null));
/** The VLAN it may be in access on: the first page, by VID. */
const vlanOptions = computed(() =>
  [...(vlans.state.value.items ?? [])]
    .sort((a, b) => a.vid - b.vid)
    .map((v): ListOption => ({ value: v.id, label: `${v.vid} · ${v.name}`, mono: true })),
);
const modes = computed(() =>
  (["none", "access"] as const).map((m): ListOption => ({ value: m, label: t(`mode.${m}`) })),
);
const mode = ref<string | undefined>();
const access = ref<string | undefined>();
type Errors = Partial<Record<"mode" | "access", string | undefined>>;
const errors = shallowRef<Errors>({});
// The VLAN is asked in the mode access only: its error goes with the mode changed.
watch(mode, () => (errors.value = {}));
/** A trunk is changed through the API only: the field says so while it is not replaced. */
const trunk = computed(() => iface.value?.vlan.mode === "trunk");
watch(access, () => (errors.value = { ...errors.value, access: undefined }));
// The mode it has, to change from, once read; a trunk is not offered here: none chosen.
watch(
  iface,
  (now, before) => {
    if (!now || before) return;
    mode.value = now.vlan.mode === "trunk" ? undefined : now.vlan.mode;
    access.value = now.vlan.access?.id ?? undefined;
  },
  { immediate: true },
);

// The field of the VLAN is mounted again as its options come (`SelectValue`, skill
// kartenn-ui « Pièges de Reka UI »): the focus it held goes to the new one.
const accessField = useTemplateRef<HTMLElement>("accessField");
watch(
  () => vlanOptions.value.length,
  async () => {
    const field = accessField.value;
    if (!field?.contains(document.activeElement)) return;
    await nextTick();
    field.querySelector<HTMLElement>("[role='combobox']")?.focus();
  },
  { flush: "pre" },
);

function operation() {
  const required = t("form.required");
  const next: Errors = {};
  if (!mode.value) next.mode = required;
  else if (mode.value === "access" && !access.value) next.access = required;
  errors.value = next;
  if (next.mode || next.access) return null;
  const vlan =
    mode.value === "access" ? { mode: "access", access: { id: access.value } } : { mode: "none" };
  return { op: "machines.update_interface", id: { id: props.id }, vlan } as never;
}
/** The violations of the server on the fields: said there; whether one was. */
function violations(problem: ApiProblem): boolean {
  const next: Errors = {};
  for (const v of problem.violations) {
    const pointer = v.pointer ?? "";
    const field = pointer.includes("/access")
      ? "access"
      : pointer.includes("/vlan")
        ? "mode"
        : null;
    if (field && !next[field]) next[field] = `${v.detail} (${v.rule})`;
  }
  errors.value = next;
  return Object.keys(next).length > 0;
}
</script>

<template>
  <Stack gap="6">
    <Stack gap="2">
      <h1 tabindex="-1" :class="{ 'k-mono': !!iface }">
        {{ iface ? iface.name : t("interfaces.record") }}
      </h1>
      <ErrorState
        v-if="!iface && read.error"
        :title="t('interfaces.failedOne')"
        :problem="problemOf(read.error)"
        :retry-label="t('retry')"
        @retry="retry(reads.object('machines.interface', props.id))"
      >
        {{ problemOf(read.error) ? "" : t("network") }}
      </ErrorState>
    </Stack>

    <Skeleton v-if="!iface && !read.error" variant="text" :rows="3" />
    <Grid v-else-if="iface" as="dl" columns="auto" gap="4">
      <div>
        <dt>{{ t("interfaces.machine") }}</dt>
        <dd>
          <CitedName
            :id="iface.machine.id"
            :names="names"
            type="machines.machine"
            :page="PAGES.machine(iface.machine.id)"
          />
        </dd>
      </div>
      <div>
        <dt>{{ t("interfaces.mac") }}</dt>
        <dd>
          <span v-if="iface.mac" class="k-mono">{{ iface.mac }}</span>
          <span v-else
            ><span aria-hidden="true">—</span
            ><span class="k-visually-hidden">{{ t("none") }}</span></span
          >
        </dd>
      </div>
      <div>
        <dt>{{ t("interfaces.mode") }}</dt>
        <dd>{{ modeOf(iface, names()) }}</dd>
      </div>
      <div>
        <dt>{{ t("interfaces.state") }}</dt>
        <dd>
          <StatusBadge
            :state="iface.enabled ? 'ok' : 'neutral'"
            :label="t(iface.enabled ? 'interfaces.enabled' : 'interfaces.disabled')"
          />
        </dd>
      </div>
    </Grid>

    <section v-if="iface" :aria-labelledby="ids.addresses">
      <Stack gap="4">
        <h2 :id="ids.addresses">{{ t("interfaces.addresses") }}</h2>
        <ErrorState
          v-if="card.error"
          :title="t('interfaces.failedAddresses')"
          :problem="problemOf(card.error)"
          :retry-label="t('retry')"
          @retry="retry(cardRead())"
        >
          {{ problemOf(card.error) ? "" : t("network") }}
        </ErrorState>
        <EmptyState
          v-else-if="card.data && addresses.length === 0"
          :title="t('interfaces.noAddress')"
        >
          {{ t("interfaces.noAddressText") }}
        </EmptyState>
        <DataGrid
          v-else
          :label="t('interfaces.addresses')"
          :columns="addressColumns"
          :rows="addresses"
          row-key="id"
          :loading="!card.data"
          :loading-rows="2"
          :row-count="card.data ? addresses.length : -1"
        >
          <template #cell="{ row, column }">
            <a v-if="column.id === 'value'" :href="addressPage(row.id)" class="k-mono">{{
              row.value
            }}</a>
            <template v-else>{{ t(row.primary ? "tab.yes" : "tab.no") }}</template>
          </template>
        </DataGrid>
      </Stack>
    </section>

    <section v-if="present() && iface" :aria-labelledby="ids.mode">
      <Stack gap="4">
        <h2 :id="ids.mode">{{ t("interfaces.setMode") }}</h2>
        <DraftForm
          permission="machines.interface.edit"
          :operation="operation"
          :violations="violations"
        >
          <Select
            v-model="mode"
            :label="t('interfaces.mode')"
            :options="modes"
            :hint="trunk ? t('interfaces.trunkHint') : undefined"
            :error="errors.mode"
            required
          />
          <!-- The VLAN offered could not be read: said, read again by « Réessayer ». -->
          <RetryAlert
            v-if="mode === 'access' && vlans.state.value.error"
            :title="t('interfaces.failedVlans')"
            :error="vlans.state.value.error"
            @retry="vlans.retry()"
          />
          <div v-if="mode === 'access'" ref="accessField">
            <Select
              :key="vlanOptions.length"
              v-model="access"
              :label="t('interfaces.access')"
              :options="vlanOptions"
              :placeholder="vlans.state.value.pending ? t('loading') : t('interfaces.chooseVlan')"
              :hint="vlans.state.value.more ? t('interfaces.firstVlans') : undefined"
              :readonly="vlanOptions.length === 0"
              :error="errors.access"
              mono
              required
            />
          </div>
        </DraftForm>
      </Stack>
    </section>
  </Stack>
</template>
