<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

<script setup lang="ts">
// A labelled native text field (design « Inventaire », line 4), in a `FormField`: label,
// then error and help linked by `aria-describedby` (error first), the error said in a
// live region when it comes, `aria-invalid` with it; read-only, disabled, pending
// (`aria-busy`, indicator, the word in the description); fixed-pitch variant for
// addresses, prefixes and identifiers. Heights of the controls (REQ-020-17).
import { computed, useId } from "vue";
import type { IconName } from "../../icons/index.ts";
import { t } from "../../i18n.ts";
import FormField from "../FormField/FormField.vue";
import Icon from "../Icon/Icon.vue";
import type { ControlSize } from "../layout.ts";

const props = withDefaults(
  defineProps<{
    label: string;
    /** The label stays for assistive technologies only (a cell editor, a search box). */
    hideLabel?: boolean;
    type?: "text" | "search" | "email" | "url" | "password" | "tel";
    size?: ControlSize;
    hint?: string | undefined;
    /** The error: sets `aria-invalid` and is read with the field. */
    error?: string | undefined;
    placeholder?: string | undefined;
    icon?: IconName | undefined;
    mono?: boolean;
    required?: boolean;
    readonly?: boolean;
    disabled?: boolean;
    /** A check or a save under way. */
    pending?: boolean;
    autocomplete?: string | undefined;
  }>(),
  {
    type: "text",
    size: "md",
    hint: undefined,
    error: undefined,
    placeholder: undefined,
    icon: undefined,
    autocomplete: undefined,
  },
);
const model = defineModel<string>({ default: "" });

// The ids of the error and the help are those of the `FormField` (`<id>-error`,
// `<id>-hint`); pending goes between them, said in words too: under reduced motion the
// indicator no longer turns.
const id = useId();
const hintId = `${id}-hint`;
const errorId = `${id}-error`;
const pendingId = `${id}-pending`;
const describedBy = computed(
  () =>
    [props.error ? errorId : null, props.pending ? pendingId : null, props.hint ? hintId : null]
      .filter(Boolean)
      .join(" ") || undefined,
);
</script>

<template>
  <FormField
    :id="id"
    class="k-text-field"
    :label="props.label"
    :hide-label="props.hideLabel"
    :hint="props.hint"
    :error="props.error"
    :required="props.required"
  >
    <div
      class="k-text-field__control"
      :data-size="props.size"
      :data-invalid="props.error ? 'true' : undefined"
    >
      <Icon v-if="props.icon" :name="props.icon" class="k-text-field__icon" />
      <input
        :id="id"
        v-model="model"
        class="k-text-field__input"
        :class="{ 'k-mono': props.mono }"
        :type="props.type"
        :placeholder="props.placeholder"
        :required="props.required"
        :readonly="props.readonly"
        :disabled="props.disabled"
        :autocomplete="props.autocomplete"
        :aria-invalid="props.error ? 'true' : undefined"
        :aria-describedby="describedBy"
        :aria-busy="props.pending ? 'true' : undefined"
        spellcheck="false"
      />
      <Icon v-if="props.pending" name="loader-circle" class="k-spinner k-text-field__icon" />
      <span v-if="props.pending" :id="pendingId" class="k-visually-hidden">{{ t("loading") }}</span>
    </div>
  </FormField>
</template>

<style>
@layer components {
  .k-text-field__control {
    display: flex;
    gap: var(--k-space-1);
    align-items: center;
    min-block-size: var(--k-control-height-md);
    padding-inline: var(--k-control-padding-inline-md);
    color: var(--k-text);
    background-color: var(--k-surface);
    border: var(--k-border-width) solid var(--k-border-control);
    border-radius: var(--k-radius-md);
    transition-property: border-color;
    transition-duration: var(--k-duration-fade);
    transition-timing-function: var(--k-ease-standard);
  }

  .k-text-field__control[data-size="sm"] {
    min-block-size: var(--k-control-height-sm);
    padding-inline: var(--k-control-padding-inline-sm);
  }

  .k-text-field__control[data-size="lg"] {
    min-block-size: var(--k-control-height-lg);
    padding-inline: var(--k-control-padding-inline-lg);
  }

  .k-text-field__control:hover {
    border-color: var(--k-text-muted);
  }

  /* The ring is on the frame, not on the bare input. */
  .k-text-field__control:focus-within {
    outline: var(--k-focus-width) solid var(--k-focus-ring);
    outline-offset: var(--k-focus-offset);
  }

  .k-text-field__control[data-invalid] {
    border-color: var(--k-state-ko-fg);
  }

  .k-text-field__control:has(:disabled) {
    color: var(--k-text-muted);
    background-color: var(--k-surface-sunken);
    border-color: var(--k-border-strong);
  }

  .k-text-field__control:has(:read-only:not(:disabled)) {
    background-color: var(--k-surface-sunken);
    border-style: dashed;
  }

  .k-text-field__input {
    flex: 1;
    align-self: stretch;
    min-inline-size: 0;
    padding: 0;
    font: inherit;
    color: inherit;
    background: none;
    border: 0;
    outline: 0;
  }

  .k-text-field__input.k-mono {
    font-family: var(--k-font-mono);
    font-size: var(--k-text-mono);
  }

  .k-text-field__input::placeholder {
    color: var(--k-text-muted);
    opacity: 1;
  }

  .k-text-field__icon {
    color: var(--k-text-muted);
  }
}
</style>
