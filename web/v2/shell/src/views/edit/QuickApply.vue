<script setup lang="ts">
// The motive asked in line for an edit applied now (REQ-021-15, 16; design 021 « Édition en
// place »): Ctrl+Enter in a cell opens this line at the foot of its list, the value typed
// and a generic motive prefilled, the focus on the motive. Enter (or Ctrl+Enter) applies,
// Escape or « Annuler » gives up; the focus then goes back to the cell. A refusal stays
// here with the input: after a 412, the value read again is said beside it and applying
// again writes on the new version.
import { Button, Cluster, Select, TextField, type ListOption } from "@kartenn/ui";
import { computed, nextTick, onMounted, ref, useTemplateRef, watch } from "vue";
import { useI18n } from "vue-i18n";
import type { QuickAsk } from "./inline.ts";
import RefusalAlert from "./RefusalAlert.vue";

const props = defineProps<{
  ask: QuickAsk;
  words: (field: string, key: string) => string | null;
}>();
const emit = defineEmits<{ confirm: [motive: string, value: string]; cancel: [] }>();
const { t } = useI18n();

const fieldLabel = computed(() => t(`views.field.${props.ask.field.name}`));
const title = computed(() => t("edit.applyNow", { field: fieldLabel.value, name: props.ask.name }));
const value = ref(props.ask.value);
const motive = ref(t("edit.motiveQuick", { field: fieldLabel.value, name: props.ask.name }));
const missing = ref<string | undefined>();
watch(
  () => props.ask.row + props.ask.column,
  () => (value.value = props.ask.value),
);
const options = computed<ListOption[]>(() =>
  (props.ask.field.options ?? []).map((key) => ({
    value: key,
    label: props.words(props.ask.field.name, key) ?? key,
    detail: key,
  })),
);
const changed = computed(() =>
  props.ask.current === null
    ? []
    : [{ label: fieldLabel.value, current: props.ask.current, input: value.value }],
);

function submit(): void {
  if (props.ask.busy) return;
  if (motive.value.trim() === "") {
    missing.value = t("edit.motiveMissing");
    return;
  }
  missing.value = undefined;
  emit("confirm", motive.value.trim(), value.value);
}
function onKeydown(event: KeyboardEvent): void {
  if (event.defaultPrevented) return;
  if (event.key === "Escape") {
    event.preventDefault();
    emit("cancel");
  } else if (event.key === "Enter" && (event.ctrlKey || event.metaKey)) {
    event.preventDefault();
    submit();
  }
}

const root = useTemplateRef<HTMLFormElement>("root");
/** The focus to the motive, its text chosen; after the grid gave it back to the cell. */
function focusMotive(): void {
  if (root.value?.contains(document.activeElement)) return;
  const input = root.value?.querySelector<HTMLInputElement>(".k-quick-apply__motive input");
  input?.focus();
  input?.select();
}
onMounted(async () => {
  await nextTick();
  focusMotive();
  requestAnimationFrame(focusMotive);
});
</script>

<template>
  <form
    ref="root"
    class="k-quick-apply"
    :aria-label="title"
    novalidate
    @submit.prevent="submit"
    @keydown="onKeydown"
  >
    <p class="k-quick-apply__title">{{ title }}</p>
    <Cluster gap="2" align="end" class="k-quick-apply__fields">
      <Select
        v-if="props.ask.field.kind === 'choice'"
        v-model="value"
        :label="fieldLabel"
        :options="options"
        size="sm"
      />
      <TextField
        v-else
        v-model="value"
        :label="fieldLabel"
        :mono="props.ask.field.mono === true"
        size="sm"
        :hint="
          props.ask.current !== null
            ? t('edit.currentValue', { value: props.ask.current })
            : undefined
        "
      />
      <TextField
        v-model="motive"
        class="k-quick-apply__motive"
        :label="t('edit.motive')"
        :error="missing"
        size="sm"
        required
      />
      <Button
        type="submit"
        variant="primary"
        size="sm"
        :loading="props.ask.busy"
        :shortcut="['Control', 'Enter']"
        >{{
          props.ask.refusal?.kind === "conflict" ? t("edit.applyAgain") : t("edit.apply")
        }}</Button
      >
      <Button variant="ghost" size="sm" :shortcut="['Escape']" @click="emit('cancel')">{{
        t("views.cancel")
      }}</Button>
    </Cluster>
    <RefusalAlert
      v-if="props.ask.refusal"
      :key="props.ask.refusal.kind + (props.ask.current ?? '')"
      :refusal="props.ask.refusal"
      :changed="changed"
    />
  </form>
</template>

<style>
@layer components {
  .k-quick-apply {
    display: grid;
    grid-template-columns: minmax(0, 1fr);
    gap: var(--k-space-2);
    inline-size: 100%;
  }

  .k-quick-apply__title {
    font-weight: var(--k-weight-semibold);
  }

  .k-quick-apply__motive {
    flex: 1 1 var(--k-size-col-lg);
    min-inline-size: 0;
  }
}
</style>
