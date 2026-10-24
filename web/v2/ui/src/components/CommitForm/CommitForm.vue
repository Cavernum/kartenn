<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

<script setup lang="ts">
// The motive of a submission, in the manner of a commit (design « Inventaire », line 31;
// REQ-020-37; 024/M9): a summary on one line and an optional description, both labelled.
// Ctrl+Enter (or ⌘+Enter) in either field submits: `submit` is emitted, unless the summary
// is required and empty, in which case the error is said under it and the focus goes
// back to it. The host runs the action chosen beside it (`SplitButton`, which shows the
// shortcut).
import { computed, ref, useId } from "vue";
import { t } from "../../i18n.ts";
import TextField from "../TextField/TextField.vue";

const props = withDefaults(
  defineProps<{
    summaryLabel?: string | undefined;
    descriptionLabel?: string | undefined;
    /** The summary is required (the motive of a changeset). */
    required?: boolean;
    /** An error from the host, under the summary. */
    error?: string | undefined;
    disabled?: boolean;
  }>(),
  { summaryLabel: undefined, descriptionLabel: undefined, error: undefined },
);
const summary = defineModel<string>("summary", { default: "" });
const description = defineModel<string>("description", { default: "" });
const emit = defineEmits<{ submit: [] }>();

const id = useId();
const form = ref<HTMLElement | null>(null);
/** A submission was refused for an empty summary: say it until the summary is filled. */
const missing = ref(false);
const shownError = computed(
  () => props.error ?? (missing.value && !summary.value.trim() ? t("summaryMissing") : undefined),
);

function focusSummary(): void {
  form.value?.querySelector<HTMLInputElement>("input")?.focus();
}

/** Submits if the summary allows it; the host calls it from its action button too. */
function submit(): boolean {
  if (props.disabled) return false;
  if (props.required && !summary.value.trim()) {
    missing.value = true;
    focusSummary();
    return false;
  }
  missing.value = false;
  emit("submit");
  return true;
}

function onKeydown(event: KeyboardEvent): void {
  if (event.key === "Enter" && (event.ctrlKey || event.metaKey)) {
    event.preventDefault();
    submit();
  }
}

defineExpose({ submit, focus: focusSummary });
</script>

<template>
  <div ref="form" class="k-commit-form" @keydown="onKeydown">
    <TextField
      v-model="summary"
      :label="props.summaryLabel ?? t('summary')"
      :required="props.required"
      :error="shownError"
      :disabled="props.disabled"
      autocomplete="off"
    />
    <div class="k-commit-form__field">
      <label :for="`${id}-description`" class="k-commit-form__label">{{
        props.descriptionLabel ?? t("descriptionOptional")
      }}</label>
      <textarea
        :id="`${id}-description`"
        v-model="description"
        class="k-commit-form__description"
        rows="3"
        :disabled="props.disabled"
        spellcheck="true"
      ></textarea>
    </div>
  </div>
</template>

<style>
@layer components {
  .k-commit-form {
    display: grid;
    gap: var(--k-space-2);
    min-inline-size: 0;
  }

  .k-commit-form__field {
    display: grid;
    gap: var(--k-space-1);
    min-inline-size: 0;
  }

  .k-commit-form__label {
    font-size: var(--k-text-sm);
    line-height: var(--k-text-sm-line);
    font-weight: var(--k-weight-medium);
    color: var(--k-text);
  }

  .k-commit-form__description {
    min-inline-size: 0;
    min-block-size: calc(var(--k-text-md-line) * 3);
    padding: var(--k-space-1) var(--k-control-padding-inline-md);
    font: inherit;
    color: var(--k-text);
    resize: vertical;
    background-color: var(--k-surface);
    border: var(--k-border-width) solid var(--k-border-control);
    border-radius: var(--k-radius-md);
    transition-property: border-color;
    transition-duration: var(--k-duration-short);
    transition-timing-function: var(--k-ease-standard);
  }

  .k-commit-form__description:hover {
    border-color: var(--k-text-muted);
  }

  .k-commit-form__description:focus-visible {
    outline: var(--k-focus-width) solid var(--k-focus-ring);
    outline-offset: var(--k-focus-offset);
  }

  .k-commit-form__description:disabled {
    color: var(--k-text-muted);
    background-color: var(--k-surface-sunken);
    border-color: var(--k-border-strong);
  }
}
</style>
