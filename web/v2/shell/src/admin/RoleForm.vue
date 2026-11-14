<script setup lang="ts">
// A role created, changed or duplicated (`set_role`, 005, REQ-005-17, 25; spec 018, R20):
// its key (fixed once created), its label in the language of the interface (the others
// kept), its permissions by their served name; to the draft. Left open as the window
// narrows under 1024 px (the gate of writes), it keeps its input and sends nothing.
import { Button, Dialog, InlineAlert, Stack, TextField } from "@kartenn/ui";
import { computed, ref, useId, watch } from "vue";
import { useI18n } from "vue-i18n";
import RefusalAlert from "../views/edit/RefusalAlert.vue";
import { useAdminDraft } from "./draft.ts";
import { entriesOf, ROLE_KEY, withLabel, type RoleItem } from "./model.ts";

const props = defineProps<{
  mode: "create" | "edit" | "copy";
  /** The role changed or duplicated, as the draft would leave it. */
  role?: RoleItem | undefined;
  /** The keys already taken (a creation and a copy refuse them). */
  taken: readonly string[];
}>();
const open = defineModel<boolean>("open", { default: false });
const { t, locale } = useI18n();
const draft = useAdminDraft();
/** The form, which the button of the actions submits (Enter in a field too). */
const formId = useId();

const key = ref("");
const label = ref("");
const entries = ref("");
const tried = ref(false);
watch(open, (now) => {
  if (!now) return;
  key.value = props.mode === "edit" ? (props.role?.key ?? "") : "";
  label.value = props.mode === "create" ? "" : (props.role?.labels[locale.value] ?? "");
  entries.value = props.mode === "create" ? "" : (props.role?.entries.join(", ") ?? "");
  tried.value = false;
  draft.refusal.value = null;
});

const keyError = computed(() => {
  if (props.mode === "edit" || !tried.value) return undefined;
  const text = key.value.trim();
  if (!text) return t("admin.required", { field: t("admin.field.key") });
  if (!ROLE_KEY.test(text)) return t("admin.roleKeyInvalid");
  return props.taken.includes(text) ? t("admin.keyTaken") : undefined;
});
const title = computed(() =>
  props.mode === "create"
    ? t("admin.roles.createTitle")
    : t(props.mode === "edit" ? "admin.roles.editTitle" : "admin.roles.copyTitle", {
        role: props.role?.key ?? "",
      }),
);

/** Why nothing is sent now: the window narrowed (or the period moved) since it opened. */
const held = computed(() => draft.gate.why.value);

async function submit(): Promise<void> {
  // Refused by the gate, read when sending: said in the dialog, the input kept.
  if (held.value) return;
  tried.value = true;
  if (keyError.value) return;
  const roleKey = props.mode === "edit" ? props.role!.key : key.value.trim();
  const before = props.mode === "edit" ? (props.role?.labels ?? {}) : {};
  const labels = withLabel(before, locale.value, label.value);
  const what = t(props.mode === "edit" ? "admin.roles.modified" : "admin.roles.created", {
    role: roleKey,
  });
  const done = await draft.writeRole(
    props.mode === "edit" ? (props.role ?? null) : null,
    roleKey,
    labels,
    entriesOf(entries.value),
    what,
  );
  if (done) open.value = false;
}
</script>

<template>
  <Dialog v-model:open="open" :title="title" size="md">
    <form :id="formId" novalidate @submit.prevent="submit">
      <Stack gap="3">
        <TextField
          v-model="key"
          :label="t('admin.field.key')"
          :hint="t('admin.groups.keyHint')"
          :error="keyError"
          :readonly="props.mode === 'edit'"
          mono
          required
          autocomplete="off"
        />
        <TextField
          v-model="label"
          :label="t('admin.field.label', { lang: locale })"
          autocomplete="off"
        />
        <TextField
          v-model="entries"
          :label="t('admin.roles.entries')"
          :hint="t('admin.roles.entriesHint')"
          mono
          autocomplete="off"
        />
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
