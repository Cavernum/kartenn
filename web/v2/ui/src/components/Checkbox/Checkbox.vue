<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

<script setup lang="ts">
// A checkbox (design « Inventaire », line 8; APG Checkbox, two and three states): a
// native input, Space toggles it. Three states: the model is `true`, `false` or `"mixed"`
// (some rows of a list selected), set on the input's `indeterminate`, which the browser
// exposes as `aria-checked="mixed"`; a mixed box checks when toggled (all the rows), the
// host then says what it is. The label wraps the box, so that the row is the target
// (24 px at least, WCAG 2.5.8, a hidden label included); the help is outside the name,
// linked by `aria-describedby`. Disabled leaves the tab order (a native control).
import { computed, useId } from "vue";
import Icon from "../Icon/Icon.vue";
import type { ControlSize } from "../layout.ts";

const props = withDefaults(
  defineProps<{
    label: string;
    /** The label stays for assistive technologies only (the box of a row). */
    hideLabel?: boolean;
    hint?: string | undefined;
    disabled?: boolean;
    size?: ControlSize;
  }>(),
  { hint: undefined, size: "md" },
);
const model = defineModel<boolean | "mixed">({ default: false });

const id = useId();
const hintId = `${id}-hint`;
const mark = computed(() =>
  model.value === "mixed" ? "minus" : model.value ? "check" : undefined,
);

function onChange(event: Event): void {
  model.value = (event.target as HTMLInputElement).checked;
}
</script>

<template>
  <div class="k-checkbox" :data-size="props.size">
    <label class="k-checkbox__hit">
      <span class="k-checkbox__box">
        <input
          :id="id"
          class="k-checkbox__input"
          type="checkbox"
          :checked="model === true"
          :indeterminate="model === 'mixed'"
          :disabled="props.disabled"
          :aria-describedby="props.hint ? hintId : undefined"
          @change="onChange"
        />
        <Icon v-if="mark" :name="mark" class="k-checkbox__mark" />
      </span>
      <span class="k-checkbox__label" :class="{ 'k-visually-hidden': props.hideLabel }">{{
        props.label
      }}</span>
    </label>
    <p v-if="props.hint" :id="hintId" class="k-checkbox__hint">{{ props.hint }}</p>
  </div>
</template>

<style>
@layer components {
  .k-checkbox {
    display: grid;
    grid-template-columns: minmax(0, 1fr);
    justify-items: start;
    min-inline-size: 0;
  }

  /* The target: the box and its label, a control's height, never under 24 × 24 px. */
  .k-checkbox__hit {
    display: inline-flex;
    gap: var(--k-space-2);
    align-items: center;
    justify-content: center;
    min-inline-size: var(--k-control-height-sm);
    min-block-size: var(--k-control-height-md);
    max-inline-size: 100%;
    color: var(--k-text);
    cursor: default;
  }

  .k-checkbox[data-size="sm"] .k-checkbox__hit {
    min-block-size: var(--k-control-height-sm);
  }

  .k-checkbox[data-size="lg"] .k-checkbox__hit {
    min-block-size: var(--k-control-height-lg);
  }

  .k-checkbox__box {
    display: grid;
    flex: none;
    place-items: center;
  }

  .k-checkbox__input,
  .k-checkbox__mark {
    grid-area: 1 / 1;
  }

  .k-checkbox__input {
    inline-size: var(--k-icon-size);
    block-size: var(--k-icon-size);
    margin: 0;
    appearance: none;
    background-color: var(--k-surface);
    border: var(--k-border-width) solid var(--k-border-control);
    border-radius: var(--k-radius-sm);
    transition-property: background-color, border-color;
    transition-duration: var(--k-duration-fade);
    transition-timing-function: var(--k-ease-standard);
  }

  .k-checkbox__hit:hover .k-checkbox__input:not(:disabled) {
    border-color: var(--k-text-muted);
  }

  .k-checkbox__input:checked,
  .k-checkbox__input:indeterminate {
    background-color: var(--k-accent);
    border-color: var(--k-accent);
  }

  .k-checkbox__hit:hover .k-checkbox__input:is(:checked, :indeterminate):not(:disabled) {
    background-color: var(--k-accent-hover);
    border-color: var(--k-accent-hover);
  }

  .k-checkbox__input:focus-visible {
    outline: var(--k-focus-width) solid var(--k-focus-ring);
    outline-offset: var(--k-focus-offset);
  }

  .k-checkbox__input:disabled {
    background-color: var(--k-surface-sunken);
    border-color: var(--k-border-strong);
  }

  /* The tick or the dash, drawn over the box; a click goes through to the input. */
  .k-checkbox__mark {
    color: var(--k-accent-fg);
    pointer-events: none;
    stroke-width: calc(var(--k-icon-stroke) * 2);
  }

  .k-checkbox__input:disabled + .k-checkbox__mark {
    color: var(--k-text-muted);
  }

  .k-checkbox__label {
    min-inline-size: 0;
    font-size: var(--k-text-md);
    line-height: var(--k-text-md-line);
  }

  .k-checkbox__hit:has(:disabled) {
    color: var(--k-text-muted);
  }

  /* The help under the label, aligned with its text. */
  .k-checkbox__hint {
    padding-inline-start: calc(var(--k-icon-size) + var(--k-space-2));
    font-size: var(--k-text-sm);
    line-height: var(--k-text-sm-line);
    color: var(--k-text-muted);
  }
}
</style>
