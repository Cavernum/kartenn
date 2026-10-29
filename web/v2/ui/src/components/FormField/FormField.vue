<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

<script setup lang="ts">
// The frame of a labelled field (design « Inventaire », line 4): a label for the control,
// then its error and its help, which the control takes by `aria-describedby` (error
// first). The control is the host's (a native input, later a list of choices or a date):
// the default slot receives its `id`, its description, the id of the label (a control
// named by `aria-labelledby`) and whether it is invalid. `group`: the control is a group
// of controls that have their own labels (checkboxes, switches); the root is then a
// `group`, named by the label and described by the error and the help.
// The error is said when it comes: it lies in a polite live region that is there, empty,
// before any error (a region inserted with its text is not announced, relecture du
// 2026-10-26); the description gives it again when the control takes the focus.
// No gap on the root: an empty live region takes no room, the margins of what shows
// space the parts.
import { computed, useId } from "vue";
import { t } from "../../i18n.ts";
import Icon from "../Icon/Icon.vue";

const props = withDefaults(
  defineProps<{
    label: string;
    /** The label stays for assistive technologies only (a cell editor, a search box). */
    hideLabel?: boolean;
    hint?: string | undefined;
    /** The error: the control is invalid, and the error is said then read with it. */
    error?: string | undefined;
    required?: boolean;
    /** A group of controls with their own labels: the root is a named `group`. */
    group?: boolean;
    /** The id of the control (a fresh one by default). */
    id?: string | undefined;
  }>(),
  { hint: undefined, error: undefined, id: undefined },
);

defineSlots<{
  default(slot: {
    /** The id the control takes (the label points to it). */
    id: string;
    /** The id of the label, for a control named by `aria-labelledby`. */
    labelId: string;
    /** Its `aria-describedby`: the error, then the help; undefined without either. */
    describedBy: string | undefined;
    invalid: boolean;
    required: boolean;
  }): unknown;
}>();

const ownId = useId();
const id = computed(() => props.id ?? ownId);
const labelId = computed(() => `${id.value}-label`);
const describedBy = computed(
  () =>
    [props.error ? `${id.value}-error` : null, props.hint ? `${id.value}-hint` : null]
      .filter(Boolean)
      .join(" ") || undefined,
);
const control = computed(() => ({
  id: id.value,
  labelId: labelId.value,
  describedBy: describedBy.value,
  invalid: Boolean(props.error),
  required: props.required,
}));
</script>

<template>
  <div
    class="k-form-field"
    :role="props.group ? 'group' : undefined"
    :aria-labelledby="props.group ? labelId : undefined"
    :aria-describedby="props.group ? describedBy : undefined"
  >
    <component
      :is="props.group ? 'span' : 'label'"
      :id="labelId"
      :for="props.group ? undefined : id"
      class="k-form-field__label"
      :class="{ 'k-visually-hidden': props.hideLabel }"
    >
      {{ props.label
      }}<span v-if="props.required" class="k-form-field__required">
        <span aria-hidden="true">*</span
        ><span class="k-visually-hidden">({{ t("required") }})</span></span
      >
    </component>
    <slot v-bind="control" />
    <div class="k-form-field__live" aria-live="polite">
      <p v-if="props.error" :id="`${id}-error`" class="k-form-field__error">
        <Icon name="circle-x" />{{ props.error }}
      </p>
    </div>
    <p v-if="props.hint" :id="`${id}-hint`" class="k-form-field__hint">{{ props.hint }}</p>
  </div>
</template>

<style>
@layer components {
  /* One elastic column: the field takes the width it is given, never the intrinsic width
   * of its control (an editor in a grid cell stays in its cell). */
  .k-form-field {
    display: grid;
    grid-template-columns: minmax(0, 1fr);
    min-inline-size: 0;
  }

  .k-form-field__label {
    margin-block-end: var(--k-space-1);
    font-size: var(--k-text-sm);
    line-height: var(--k-text-sm-line);
    font-weight: var(--k-weight-medium);
    color: var(--k-text);
  }

  .k-form-field__required {
    margin-inline-start: var(--k-space-0-5);
    color: var(--k-state-ko-fg);
  }

  .k-form-field__hint,
  .k-form-field__error {
    display: flex;
    gap: var(--k-space-1);
    align-items: center;
    margin-block-start: var(--k-space-1);
    font-size: var(--k-text-sm);
    line-height: var(--k-text-sm-line);
    color: var(--k-text-muted);
  }

  .k-form-field__error {
    color: var(--k-state-ko-fg);
  }
}
</style>
