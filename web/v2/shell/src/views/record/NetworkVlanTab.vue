<script setup lang="ts">
// The VLAN tab of a network (026, REQ-026-6, REQ-008-17; design 021 « VLAN, écrans du
// cœur » : onglet VLAN d'un préfixe): the VLAN its property `core.vlan` names, its group
// and status, read at the dates of the page; « Retirer le VLAN » adds the removal of the
// property to the draft, once (a removal already there is said, not added twice). A
// property the reader may not read is said masked, never « no VLAN », and offers no write.
// A container carries none (`core.vlan.prefix_not_network`). Creating a VLAN for the
// network waits for an arbitration (task 021-17). Three states.
import { drafts, reads, type components, type Draft, type Entry } from "@kartenn/sdk";
import { addToActiveDraft } from "@kartenn/sdk/host";
import {
  Button,
  CodeBadge,
  Cluster,
  EmptyState,
  InlineAlert,
  Skeleton,
  Stack,
  StatusBadge,
  injectToasts,
} from "@kartenn/ui";
import { computed, onScopeDispose, ref } from "vue";
import { useI18n } from "vue-i18n";
import { useQuery } from "../../data/query.ts";
import ProblemState from "../../problems/ProblemState.vue";
import { useSession } from "../../rights/index.ts";
import { useTime } from "../../time/index.ts";
import { nameOf, type Item } from "../collections.ts";
import { useDeclarations } from "../data.ts";
import { editRight, propertyRight } from "../edit/fields.ts";
import type { Refusal } from "../edit/inline.ts";
import RefusalAlert from "../edit/RefusalAlert.vue";
import AttributeList from "../parts/AttributeList.vue";
import {
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

// The declaration of `core.vlan`, which the operation names by its identifier.
const declared = useDeclarations();
const property = computed(
  () =>
    (
      declared.declarations.value.find((d) => d.key === VLAN_PROPERTY) as
        { id?: string } | undefined
    )?.id,
);
// The active draft: a removal already there is not added again.
const active = ref<string | null>(drafts.active());
onScopeDispose(drafts.onActive((id) => (active.value = id)));
const activeDraft = useQuery(() => ({
  ...drafts.get(active.value ?? "-"),
  enabled: active.value !== null,
}));
const drafted = computed(() =>
  ((activeDraft.data.value as Entry<Draft> | undefined)?.data.operations ?? []).some((op) =>
    isNetworkVlanRemoval(op, props.id, property.value),
  ),
);

// Why « Retirer le VLAN » is refused, if it is: the rights, the date, the declaration unread,
// the removal already in the draft (the server stays the judge).
const removeBlocked = computed(() => {
  const missing = [editRight("core.prefix"), propertyRight(VLAN_PROPERTY)].find(
    (p) => !session.value.rights.has(p),
  );
  if (missing) return { forbidden: missing };
  if (!time.value.present) return { reason: t("edit.why.past") };
  if (!property.value) {
    return {
      reason: declared.failed.value
        ? t("views.vlan.declarationFailed", { property: VLAN_PROPERTY })
        : t("views.vlan.declarationPending", { property: VLAN_PROPERTY }),
    };
  }
  if (drafted.value) return { reason: t("views.vlan.removalDrafted") };
  return {};
});
const refusal = ref<Refusal | null>(null);
const busy = ref(false);
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
  <EmptyState v-else-if="state.kind === 'none'" :title="t('views.vlan.noVlan')">
    {{ t("views.vlan.noVlanText") }}
  </EmptyState>
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
