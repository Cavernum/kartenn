<script setup lang="ts">
// The creation of a group, or the change of its label (`set_group`, spec 025, REQ-025-1):
// its key (fixed once created) and its label in the language of the interface, the others
// kept; to the draft, like every write of the administration. Left open as the window
// narrows under 1024 px (the gate of writes), it keeps its input and sends nothing.
import { Button, Dialog, InlineAlert, Stack, TextField } from "@kartenn/ui";
import { computed, ref, useId, watch } from "vue";
import { useI18n } from "vue-i18n";
import RefusalAlert from "../views/edit/RefusalAlert.vue";
import { useAdminDraft } from "./draft.ts";
import { GROUP_KEY, withLabel, type GroupItem } from "./model.ts";

const props = defineProps<{
  /** The group changed; none: a creation. */
  group?: GroupItem | undefined;
  /** The keys already taken (a creation refuses them). */
  taken?: readonly string[];
}>();
const open = defineModel<boolean>("open", { default: false });
const { t, locale } = useI18n();
const draft = useAdminDraft();
/** The form, which the button of the actions submits (Enter in a field too). */
const formId = useId();

const key = ref("");
const label = ref("");
const tried = ref(false);
watch(open, (now) => {
  if (!now) return;
  key.value = props.group?.key ?? "";
  label.value = props.group?.labels[locale.value] ?? "";
  tried.value = false;
  draft.refusal.value = null;
});

const keyError = computed(() => {
  if (props.group || !tried.value) return undefined;
  const text = key.value.trim();
  if (!text) return t("admin.required", { field: t("admin.field.key") });
  if (!GROUP_KEY.test(text)) return t("admin.keyInvalid");
  return props.taken?.includes(text) ? t("admin.keyTaken") : undefined;
});

/** Why nothing is sent now: the window narrowed (or the period moved) since it opened. */
const held = computed(() => draft.gate.why.value);

async function submit(): Promise<void> {
  // Refused by the gate, read when sending: said in the dialog, the input kept.
  if (held.value) return;
  tried.value = true;
  if (keyError.value) return;
  const groupKey = props.group?.key ?? key.value.trim();
  const labels = withLabel(props.group?.labels ?? {}, locale.value, label.value);
  const what = t(props.group ? "admin.groups.modified" : "admin.groups.created", {
    group: groupKey,
  });
  if (await draft.add([{ op: "set_group", key: groupKey, labels }], what)) open.value = false;
}
</script>

<template>
  <Dialog
    v-model:open="open"
    :title="
      props.group
        ? t('admin.groups.editTitle', { group: props.group.key })
        : t('admin.groups.createTitle')
    "
    size="sm"
  >
    <form :id="formId" novalidate @submit.prevent="submit">
      <Stack gap="3">
        <TextField
          v-model="key"
          :label="t('admin.field.key')"
          :hint="t('admin.groups.keyHint')"
          :error="keyError"
          :readonly="!!props.group"
          mono
          required
          autocomplete="off"
        />
        <TextField
          v-model="label"
          :label="t('admin.field.label', { lang: locale })"
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
