<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

<script setup lang="ts">
// A labelled native text field (design « Inventaire », line 4): label, help and error
// linked by `aria-describedby`, `aria-invalid` with the error; read-only, disabled,
// pending (`aria-busy`, indicator); fixed-pitch variant for addresses, prefixes and
// identifiers. Heights of the controls (REQ-020-17).
import { computed, useId } from "vue";
import type { IconName } from "../../icons/index.ts";
import { t } from "../../i18n.ts";
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

const id = useId();
const hintId = `${id}-hint`;
const errorId = `${id}-error`;
const pendingId = `${id}-pending`;
// Pending is said in words too: under reduced motion the indicator no longer turns.
const describedBy = computed(
  () =>
    [props.error ? errorId : null, props.pending ? pendingId : null, props.hint ? hintId : null]
      .filter(Boolean)
      .join(" ") || undefined,
);
</script>

<template>
  <div
    class="k-text-field"
    :data-size="props.size"
    :data-invalid="props.error ? 'true' : undefined"
  >
    <label :for="id" class="k-text-field__label" :class="{ 'k-visually-hidden': props.hideLabel }">
      {{ props.label
      }}<span v-if="props.required" class="k-text-field__required">
        <span aria-hidden="true">*</span
        ><span class="k-visually-hidden">({{ t("required") }})</span></span
      >
    </label>
    <div class="k-text-field__control">
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
    <p v-if="props.error" :id="errorId" class="k-text-field__error">
      <Icon name="circle-x" />{{ props.error }}
    </p>
    <p v-if="props.hint" :id="hintId" class="k-text-field__hint">{{ props.hint }}</p>
  </div>
</template>

<style>
@layer components {
  .k-text-field {
    display: grid;
    gap: var(--k-space-1);
    min-inline-size: 0;
  }

  .k-text-field__label {
    font-size: var(--k-text-sm);
    line-height: var(--k-text-sm-line);
    font-weight: var(--k-weight-medium);
    color: var(--k-text);
  }

  .k-text-field__required {
    margin-inline-start: var(--k-space-0-5);
    color: var(--k-state-ko-fg);
  }

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

  .k-text-field[data-size="sm"] .k-text-field__control {
    min-block-size: var(--k-control-height-sm);
    padding-inline: var(--k-control-padding-inline-sm);
  }

  .k-text-field[data-size="lg"] .k-text-field__control {
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

  .k-text-field[data-invalid] .k-text-field__control {
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

  .k-text-field__hint,
  .k-text-field__error {
    display: flex;
    gap: var(--k-space-1);
    align-items: center;
    font-size: var(--k-text-sm);
    line-height: var(--k-text-sm-line);
    color: var(--k-text-muted);
  }

  .k-text-field__error {
    color: var(--k-state-ko-fg);
  }
}
</style>
