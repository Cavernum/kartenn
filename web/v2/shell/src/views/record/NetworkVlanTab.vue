<script setup lang="ts">
// The VLAN tab of a network (026, REQ-026-6, REQ-008-17; design 021 « VLAN, écrans du
// cœur » : onglet VLAN d'un préfixe): the VLAN its property `core.vlan` names, its group
// and status, read at the dates of the page; « Retirer le VLAN » adds the removal of the
// property to the draft, once (a removal already there is said, not added twice). A
// property the reader may not read is said masked, never « no VLAN », and offers no write.
// A container carries none (`core.vlan.prefix_not_network`). A network that carries none
// (maquette E12): « Créer un VLAN pour ce réseau » opens the edit panel on a VLAN set on it
// in the same changeset (021/A13); « Poser un VLAN existant » adds the setting of the
// property to the draft, a VLAN chosen among the suggestions of `/v1/vlans`. Three states.
import { drafts, reads, type components, type Draft, type Entry } from "@kartenn/sdk";
import { addToActiveDraft } from "@kartenn/sdk/host";
import {
  AttributeList,
  Button,
  CodeBadge,
  Cluster,
  Combobox,
  EmptyState,
  Heading,
  InlineAlert,
  Skeleton,
  Stack,
  StatusBadge,
  injectToasts,
  type ListOption,
} from "@kartenn/ui";
import { computed, nextTick, onScopeDispose, ref, useTemplateRef, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useQuery } from "../../data/query.ts";
import ProblemState from "../../problems/ProblemState.vue";
import { useSession } from "../../rights/index.ts";
import { refusalWords, useTime, useWrites } from "../../time/index.ts";
import { nameOf, type Item } from "../collections.ts";
import { useDeclarationsOf } from "../data.ts";
import { editRight } from "../edit/fields.ts";
import { NETWORK_VLAN_EDITS } from "../edit/vlan.ts";
import { whyWords, type Refusal } from "../edit/inline.ts";
import { openEdit } from "../edit/panel.ts";
import RefusalAlert from "../edit/RefusalAlert.vue";
import { cacheSuggest } from "../filters.ts";
import {
  isNetworkVlanAssignment,
  isNetworkVlanRemoval,
  networkVlanOperation,
  VLAN_PROPERTY,
  VLAN_STATES,
  vlanOfProperties,
  type Resolved,
} from "../parts/vlan.ts";
import ObjectName from "./ObjectName.vue";

type Vlan = components["schemas"]["Vlan"];

const props = defineProps<{ id: string; name: string; nature: unknown }>();
const { t, te } = useI18n();
const time = useTime();
const session = useSession();
const toasts = injectToasts();
const dated = () => ({ validAt: time.value.validAt, asOf: time.value.asOf });

const network = computed(() => props.nature === "network");
const resolved = useQuery(() => ({
  ...reads.sub<Resolved>("prefix", props.id, "properties", null, dated()),
  enabled: network.value,
}));
const state = computed(() =>
  resolved.data.value ? vlanOfProperties(resolved.data.value.data) : null,
);
const vlanId = computed(() => (state.value?.kind === "vlan" ? state.value.id : null));
const vlan = useQuery(() => ({
  ...reads.object<Vlan>("vlan", vlanId.value ?? "-", dated()),
  enabled: vlanId.value !== null,
}));
const data = computed(() => vlan.data.value?.data);
const vlanName = computed(() => (data.value ? nameOf(data.value as unknown as Item) : null));
const statusWords = (status: string) => {
  const key = `views.value.status.${status}`;
  return te(key) ? t(key) : status;
};
const attributes = computed(() => [
  { key: "vlan", label: t("nav.vlans") },
  { key: "group", label: t("views.field.group") },
  { key: "status", label: t("views.field.status") },
  { key: "property", label: t("views.vlan.property") },
]);

// The declaration of `core.vlan`, which the operation names by its identifier: read by its
// key alone.
const declared = useDeclarationsOf(() => [VLAN_PROPERTY]);
const property = computed(() => (declared.of(VLAN_PROPERTY) as { id?: string } | undefined)?.id);
// The active draft: a removal already there is not added again.
const active = ref<string | null>(drafts.active());
onScopeDispose(drafts.onActive((id) => (active.value = id)));
const activeDraft = useQuery(() => ({
  ...drafts.get(active.value ?? "-"),
  enabled: active.value !== null,
}));
const draftOperations = computed(
  () => (activeDraft.data.value as Entry<Draft> | undefined)?.data.operations ?? [],
);
const drafted = computed(() =>
  draftOperations.value.some((op) => isNetworkVlanRemoval(op, props.id, property.value)),
);
const settingDrafted = computed(() =>
  draftOperations.value.some((op) => isNetworkVlanAssignment(op, props.id, property.value)),
);

const writes = useWrites();
/**
 * Why the date or the gate of writes refuses a write of the property now, if they do: off
 * the present (`valid_at` alone included: its operation goes to the draft undated, it would
 * apply at once), then under 1024 px wide (REQ-021-29).
 */
const gateWhy = computed(() =>
  time.value.present ? refusalWords(writes.value, t) : t("edit.why.past"),
);
/**
 * Why a write of the property is refused, if it is: the rights (`extra`, those of the VLAN
 * created too), the date and the width, the declaration unread (the server stays the judge).
 */
function writeBlocked(extra: readonly string[] = []): { forbidden?: string; reason?: string } {
  const missing = [...extra, ...NETWORK_VLAN_EDITS].find((p) => !session.value.rights.has(p));
  if (missing) return { forbidden: missing };
  if (gateWhy.value) return { reason: gateWhy.value };
  if (!property.value) {
    return {
      reason: declared.failed.value
        ? t("views.vlan.declarationFailed", { property: VLAN_PROPERTY })
        : t("views.vlan.declarationPending", { property: VLAN_PROPERTY }),
    };
  }
  return {};
}
// « Retirer le VLAN »: refused too once the removal is in the draft.
const removeBlocked = computed(() => {
  const blocked = writeBlocked();
  if (Object.keys(blocked).length > 0) return blocked;
  return drafted.value ? { reason: t("views.vlan.removalDrafted") } : {};
});
// « Créer un VLAN pour ce réseau », « Poser le VLAN »: refused too once a setting is drafted.
const setBlocked = (extra: readonly string[] = []) => {
  const blocked = writeBlocked(extra);
  if (Object.keys(blocked).length > 0) return blocked;
  return settingDrafted.value ? { reason: t("views.vlan.settingDrafted") } : {};
};
const createBlocked = computed(() => setBlocked([editRight("core.vlan")]));
const poseBlocked = computed(() => setBlocked());
const refusal = ref<Refusal | null>(null);
const busy = ref(false);

/** The edit panel on a VLAN created for this network, set on it in the same changeset. */
function createForNetwork(): void {
  if (!property.value || Object.keys(createBlocked.value).length > 0) return;
  openEdit({
    mode: "create",
    collection: "vlans",
    network: { id: props.id, name: props.name, property: property.value },
  });
}

// « Poser un VLAN existant »: a VLAN chosen among the suggestions (`q`: the start of its VID
// or of its name, only the VLAN the reader may read), its setting to the draft.
const setText = ref("");
const setValue = ref<string | undefined>(undefined);
const setError = ref<string | undefined>(undefined);
const setForm = useTemplateRef<HTMLFormElement>("setForm");
// A VLAN taken from the list: the error that asked for one goes.
watch(setValue, (value) => {
  if (value !== undefined) setError.value = undefined;
});
const vlanNames = new Map<string, string>();
async function suggestVlans(query: string, signal?: AbortSignal): Promise<ListOption[]> {
  const q = query.trim();
  const items = await cacheSuggest(
    "/v1/vlans",
    q === "" ? {} : { q },
    signal ?? new AbortController().signal,
  );
  return items.map((item) => {
    const label = nameOf(item as Item);
    vlanNames.set(String(item.id), label);
    return { value: String(item.id), label, mono: true };
  });
}
/** Why « Poser le VLAN » is refused now, in words (the permission missing first), if so. */
const poseWhy = computed(() => {
  const blocked = poseBlocked.value;
  if (blocked.forbidden) return whyWords({ kind: "forbidden", permission: blocked.forbidden }, t);
  return blocked.reason ?? null;
});
/**
 * The refusal said in the form when Enter sends it anyway (the window narrowed since «
 * Poser le VLAN » was drawn, say): the VLAN typed kept. The reason of now is shown, gone
 * once open; each Enter refused counts a turn, the alert inserted anew and read again.
 */
const refusedTurn = ref(0);
watch(poseWhy, (why) => {
  if (!why) refusedTurn.value = 0;
});
async function pose(): Promise<void> {
  if (busy.value) return;
  // The gate read again as it sends, not only as the button was drawn.
  if (poseWhy.value || !property.value) {
    if (poseWhy.value) refusedTurn.value += 1;
    return;
  }
  refusal.value = null;
  // A text typed and no VLAN taken from the list: nothing is guessed.
  if (setValue.value === undefined) {
    setError.value = t("views.vlan.setChoose");
    // The focus to the field refused, its error read with it (as the edit panel does).
    await nextTick();
    setForm.value?.querySelector<HTMLElement>("[aria-invalid='true']")?.focus();
    return;
  }
  setError.value = undefined;
  const vlan = setValue.value;
  busy.value = true;
  const outcome = await addToActiveDraft([networkVlanOperation(props.id, property.value, vlan)]);
  busy.value = false;
  if (outcome.kind === "saved") {
    toasts?.show({
      tone: "ok",
      title: t("edit.drafted", {
        what: t("views.vlan.setting", {
          vlan: vlanNames.get(vlan) ?? setText.value,
          network: props.name,
        }),
      }),
    });
    setText.value = "";
    setValue.value = undefined;
  } else refusal.value = outcome as Refusal;
}
async function remove(): Promise<void> {
  if (busy.value || !property.value || Object.keys(removeBlocked.value).length > 0) return;
  busy.value = true;
  refusal.value = null;
  const outcome = await addToActiveDraft([networkVlanOperation(props.id, property.value, null)]);
  busy.value = false;
  if (outcome.kind === "saved") {
    toasts?.show({
      tone: "ok",
      title: t("edit.drafted", {
        what: t("views.vlan.removal", { vlan: vlanName.value ?? "", network: props.name }),
      }),
    });
  } else refusal.value = outcome as Refusal;
}
</script>

<template>
  <InlineAlert v-if="!network" tone="info" :title="t('views.vlan.notNetwork')">
    {{ t("views.vlan.notNetworkText") }}
    <CodeBadge code="core.vlan.prefix_not_network" />
  </InlineAlert>
  <ProblemState
    v-else-if="resolved.isError.value"
    :title="t('views.record.propertiesError')"
    :error="resolved.error.value"
    :retry="resolved.refetch"
  />
  <ProblemState
    v-else-if="vlan.isError.value"
    :title="t('views.vlan.vlanError')"
    :error="vlan.error.value"
    :retry="vlan.refetch"
  />
  <Skeleton
    v-else-if="state === null || (vlanId !== null && vlan.isPending.value)"
    variant="text"
    :rows="3"
  />
  <InlineAlert
    v-else-if="state.kind === 'masked'"
    tone="info"
    icon="lock"
    :title="t('views.vlan.masked')"
  >
    {{ t("views.record.explanationMasked") }}
    <CodeBadge :code="`prop.${VLAN_PROPERTY}.read`" />
  </InlineAlert>
  <Stack v-else-if="state.kind === 'none'" gap="4">
    <EmptyState :title="t('views.vlan.noVlan')">
      {{ t("views.vlan.noVlanText") }}
      <template #action>
        <Button icon="plus" v-bind="createBlocked" @click="createForNetwork">{{
          t("views.vlan.createForNetwork")
        }}</Button>
      </template>
    </EmptyState>
    <form
      ref="setForm"
      class="k-network-vlan__set"
      :aria-labelledby="`k-network-vlan-set-${props.id}`"
      novalidate
      @submit.prevent="pose"
    >
      <Stack gap="2">
        <Heading :id="`k-network-vlan-set-${props.id}`" :level="2">
          {{ t("views.vlan.setTitle") }}
        </Heading>
        <Combobox
          v-model="setText"
          v-model:value="setValue"
          class="k-network-vlan__field"
          :label="t('views.vlan.setLabel')"
          :hint="t('views.vlan.setHint')"
          :error="setError"
          :suggest="suggestVlans"
          mono
        />
        <Cluster gap="2">
          <!-- Refused, it is no longer the form's submit button: Enter in the field then
               sends the form, which says the refusal instead of nothing. -->
          <Button
            icon="vlan"
            :type="Object.keys(poseBlocked).length > 0 ? 'button' : 'submit'"
            v-bind="poseBlocked"
            :loading="busy"
            >{{ t("views.vlan.set") }}</Button
          >
        </Cluster>
        <InlineAlert
          v-if="refusedTurn > 0 && poseWhy"
          :key="refusedTurn"
          tone="warn"
          icon="lock"
          :title="poseWhy"
        />
        <RefusalAlert v-if="refusal" :refusal="refusal" />
      </Stack>
    </form>
  </Stack>
  <Stack v-else gap="3">
    <p>{{ t("views.vlan.carries", { vlan: vlanName }) }}</p>
    <AttributeList :items="attributes">
      <template #vlan><ObjectName v-if="vlanId" :id="vlanId" kind="vlan" /></template>
      <template #group>
        <ObjectName v-if="data" :id="data.group.id" kind="vlan_group" />
      </template>
      <template #status>
        <StatusBadge
          v-if="data && VLAN_STATES[data.status]"
          :state="VLAN_STATES[data.status]!"
          :label="statusWords(data.status)"
        />
      </template>
      <template #property><CodeBadge :code="VLAN_PROPERTY" /></template>
    </AttributeList>
    <Cluster gap="2">
      <Button icon="trash" v-bind="removeBlocked" :loading="busy" @click="remove">{{
        t("views.vlan.remove")
      }}</Button>
    </Cluster>
    <RefusalAlert v-if="refusal" :refusal="refusal" />
  </Stack>
</template>

<style>
@layer components {
  .k-network-vlan__field {
    max-inline-size: var(--k-size-field);
  }
}
</style>
