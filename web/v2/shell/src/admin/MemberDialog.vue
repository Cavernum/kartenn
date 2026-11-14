<script setup lang="ts">
// « Ajouter un membre » (spec 025, REQ-025-2, 7; maquette E7): an identity, chosen among
// those the server lists (name, kind, issuer), or a group of an IdP, `émetteur / groupe`,
// as its tokens name it; never another group of kartenn (G2). To the draft (`add_member`).
// Left open as the window narrows under 1024 px, it keeps its input and sends nothing.
import {
  Button,
  Combobox,
  Dialog,
  InlineAlert,
  Select,
  Stack,
  TextField,
  filterOptions,
  type ListOption,
} from "@kartenn/ui";
import { computed, ref, useId, watch } from "vue";
import { useI18n } from "vue-i18n";
import RefusalAlert from "../views/edit/RefusalAlert.vue";
import { useAdminDraft } from "./draft.ts";
import { memberName, type IdentityItem, type MemberRow } from "./model.ts";

const props = defineProps<{
  group: string;
  identities: readonly IdentityItem[];
  /** The members shown now (an identity or a group already there is refused). */
  members: readonly MemberRow[];
}>();
const open = defineModel<boolean>("open", { default: false });
const { t } = useI18n();
const draft = useAdminDraft();
/** The form, which the button of the actions submits (Enter in a field too). */
const formId = useId();

/** `identity` or `idp_group`, as the Select gives it. */
const kind = ref<string | undefined>("identity");
const text = ref("");
const identity = ref<string | undefined>(undefined);
const issuer = ref("");
const groupName = ref("");
const tried = ref(false);
watch(open, (now) => {
  if (!now) return;
  kind.value = "identity";
  text.value = "";
  identity.value = undefined;
  issuer.value = "";
  groupName.value = "";
  tried.value = false;
  draft.refusal.value = null;
});

const kinds = computed<ListOption[]>(() => [
  { value: "identity", label: t("admin.members.identity") },
  { value: "idp_group", label: t("admin.members.idpGroup") },
]);
const options = computed<ListOption[]>(() =>
  props.identities.map((i) => ({
    value: i.id,
    label: i.name,
    detail: [t(`admin.members.kind.${i.kind}`), i.issuer].filter(Boolean).join(" · "),
  })),
);
const suggest = async (query: string) => filterOptions(options.value, query);

const present = (row: Pick<MemberRow, "kind" | "identity" | "issuer" | "group">) =>
  props.members.some(
    (m) =>
      m.pending !== "removed" &&
      m.kind === row.kind &&
      (row.kind === "identity"
        ? m.identity === row.identity
        : m.issuer === row.issuer && m.group === row.group),
  );
const errors = computed(() => {
  if (!tried.value) return {};
  const required = (field: string) => t("admin.required", { field });
  if (kind.value === "identity") {
    if (!identity.value) return { identity: required(t("admin.members.identity")) };
    return present({ kind: "identity", identity: identity.value, issuer: null, group: null })
      ? { identity: t("admin.members.already") }
      : {};
  }
  const out: Record<string, string> = {};
  if (!issuer.value.trim()) out.issuer = required(t("admin.members.issuer"));
  if (!groupName.value.trim()) out.group = required(t("admin.members.groupName"));
  if (
    !out.issuer &&
    !out.group &&
    present({
      kind: "idp_group",
      identity: null,
      issuer: issuer.value.trim(),
      group: groupName.value.trim(),
    })
  ) {
    out.group = t("admin.members.already");
  }
  return out;
});

/** Why nothing is sent now: the window narrowed (or the period moved) since it opened. */
const held = computed(() => draft.gate.why.value);

async function submit(): Promise<void> {
  // Refused by the gate, read when sending: said in the dialog, the input kept.
  if (held.value) return;
  tried.value = true;
  if (Object.keys(errors.value).length > 0) return;
  const member =
    kind.value === "identity"
      ? { kind: "identity" as const, id: identity.value! }
      : { kind: "idp_group" as const, issuer: issuer.value.trim(), group: groupName.value.trim() };
  const name =
    member.kind === "identity"
      ? (props.identities.find((i) => i.id === member.id)?.name ?? member.id)
      : memberName({ ...member, name: null, identity: null });
  const what = t("admin.members.added", { name, group: props.group });
  if (await draft.add([{ op: "add_member", group: props.group, member }], what)) {
    open.value = false;
  }
}
</script>

<template>
  <Dialog v-model:open="open" :title="t('admin.members.dialog', { group: props.group })" size="md">
    <form :id="formId" novalidate @submit.prevent="submit">
      <Stack gap="3">
        <Select v-model="kind" :label="t('admin.members.sort')" :options="kinds" required />
        <Combobox
          v-if="kind === 'identity'"
          v-model="text"
          v-model:value="identity"
          :label="t('admin.members.identity')"
          :hint="t('admin.members.identityHint')"
          :error="errors.identity"
          :suggest="suggest"
          required
        />
        <template v-else>
          <TextField
            v-model="issuer"
            :label="t('admin.members.issuer')"
            :error="errors.issuer"
            mono
            required
            autocomplete="off"
          />
          <TextField
            v-model="groupName"
            :label="t('admin.members.groupName')"
            :error="errors.group"
            mono
            required
            autocomplete="off"
          />
        </template>
        <InlineAlert v-if="held" tone="warn" icon="lock" :title="held" />
        <RefusalAlert v-if="draft.refusal.value" :refusal="draft.refusal.value" :note="null" />
      </Stack>
    </form>
    <template #actions>
      <Button variant="ghost" @click="open = false">{{ t("admin.cancel") }}</Button>
      <Button
        variant="primary"
        icon="draft"
        type="submit"
        :form="formId"
        :loading="draft.busy.value"
        v-bind="held ? { reason: held } : {}"
        >{{ t("admin.toDraft") }}</Button
      >
    </template>
  </Dialog>
</template>
