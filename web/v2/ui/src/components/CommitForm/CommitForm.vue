<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

<script setup lang="ts">
// The motive of a submission, in the manner of a commit (design « Inventaire », line 31;
// REQ-020-37; 024/M9): a summary on one line and an optional description, both labelled.
// Ctrl+Enter (or ⌘+Enter) in either field submits: `submit` is emitted, unless the summary
// is required and empty, in which case the error is said under it and the focus goes
// back to it. The host runs the action chosen beside it (`SplitButton`, which shows the
// shortcut). Read only (`readonly`, or a `reason` alone: nothing is written under `as_of`;
// 024/M14), both fields stay in the tab order, `readonly`, and say why: the reason under
// the summary and in the description of each field; « Ajouter une description » is
// refused with it; nothing is submitted.
import { computed, nextTick, ref, useId } from "vue";
import { t } from "../../i18n.ts";
import Button from "../Button/Button.vue";
import FormField from "../FormField/FormField.vue";
import TextField from "../TextField/TextField.vue";

const props = withDefaults(
  defineProps<{
    summaryLabel?: string | undefined;
    descriptionLabel?: string | undefined;
    /** The summary is required (the motive of a changeset). */
    required?: boolean;
    /** An error from the host, under the summary. */
    error?: string | undefined;
    /** Out of the tab order (native `disabled`), without a reason. */
    disabled?: boolean;
    /** Read only: the fields stay focusable, their text can be read and copied. */
    readonly?: boolean;
    /** Why the motive cannot be written (a past date read): read only, and said. */
    reason?: string | undefined;
    /**
     * An empty description stays behind a button « Ajouter une description » until asked
     * for (a form in a short footer: the panel keeps its height for the draft).
     */
    collapsible?: boolean;
  }>(),
  { summaryLabel: undefined, descriptionLabel: undefined, error: undefined, reason: undefined },
);
const summary = defineModel<string>("summary", { default: "" });
const description = defineModel<string>("description", { default: "" });
const emit = defineEmits<{ submit: [] }>();

const id = useId();
const form = ref<HTMLElement | null>(null);
/** Read only: asked for, or refused with a reason (a reason alone refuses, 024/M14). */
const locked = computed(() => props.readonly || !!props.reason);
const reasonId = `${id}-reason`;
/** A submission was refused for an empty summary: say it until the summary is filled. */
const missing = ref(false);
const shownError = computed(
  () => props.error ?? (missing.value && !summary.value.trim() ? t("summaryMissing") : undefined),
);

/** The description field shows: not collapsible, asked for, or holding text. */
const opened = ref(false);
const descriptionShown = computed(
  () => !props.collapsible || opened.value || description.value !== "",
);
async function openDescription(): Promise<void> {
  opened.value = true;
  await nextTick();
  form.value?.querySelector<HTMLTextAreaElement>("textarea")?.focus();
}

function focusSummary(): void {
  form.value?.querySelector<HTMLInputElement>("input")?.focus();
}

/** Submits if the summary allows it; the host calls it from its action button too. */
function submit(): boolean {
  if (props.disabled || locked.value) return false;
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
      :hint="props.reason"
      :readonly="locked"
      :disabled="props.disabled"
      autocomplete="off"
    />
    <span v-if="!descriptionShown">
      <Button
        variant="ghost"
        size="sm"
        icon="plus"
        :disabled="props.disabled || locked"
        :reason="props.reason"
        @click="openDescription"
        >{{ t("descriptionAdd") }}</Button
      >
    </span>
    <FormField
      v-else
      :id="`${id}-description`"
      v-slot="{ id: descriptionId }"
      :label="props.descriptionLabel ?? t('descriptionOptional')"
    >
      <textarea
        :id="descriptionId"
        v-model="description"
        class="k-commit-form__description"
        rows="3"
        :readonly="locked"
        :disabled="props.disabled"
        :aria-describedby="props.reason ? reasonId : undefined"
        spellcheck="true"
      ></textarea>
      <!-- Said under the summary already: read with this field too, not shown twice. -->
      <span v-if="props.reason" :id="reasonId" class="k-visually-hidden">{{ props.reason }}</span>
    </FormField>
  </div>
</template>

<style>
@layer components {
  .k-commit-form {
    display: grid;
    gap: var(--k-space-2);
    min-inline-size: 0;
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
    transition-duration: var(--k-duration-fade);
    transition-timing-function: var(--k-ease-standard);
  }

  .k-commit-form__description:hover {
    border-color: var(--k-text-muted);
  }

  .k-commit-form__description:focus-visible {
    outline: var(--k-focus-width) solid var(--k-focus-ring);
    outline-offset: var(--k-focus-offset);
  }

  /* As a read-only `TextField` (shared.css): the look wins over hovering. */
  .k-commit-form__description:read-only:not(:disabled) {
    background-color: var(--k-surface-sunken);
    border-color: var(--k-border-control);
    border-style: dashed;
  }

  .k-commit-form__description:disabled {
    color: var(--k-text-muted);
    background-color: var(--k-surface-sunken);
    border-color: var(--k-border-strong);
  }
}
</style>
