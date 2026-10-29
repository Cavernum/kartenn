<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

<script setup lang="ts">
// A switch (design « Inventaire », line 9; APG Switch): a binary setting that takes effect
// at once, never a choice of a form to submit (a `Checkbox` then). A button of role
// `switch`, `aria-checked` true or false; Space and Enter toggle it. Its label stays the
// same whichever the state (the state is `aria-checked`); on, the thumb goes to the end
// and the track fills (place and colour). The label wraps it, so that the row is the
// target (24 px at least, WCAG 2.5.8) and names it (`aria-labelledby`); the help is
// outside the name, linked by `aria-describedby`.
import { useId } from "vue";
import type { ControlSize } from "../layout.ts";

const props = withDefaults(
  defineProps<{
    label: string;
    /** The label stays for assistive technologies only. */
    hideLabel?: boolean;
    hint?: string | undefined;
    disabled?: boolean;
    size?: ControlSize;
  }>(),
  { hint: undefined, size: "md" },
);
const model = defineModel<boolean>({ default: false });

const id = useId();
const labelId = `${id}-label`;
const hintId = `${id}-hint`;
</script>

<template>
  <div class="k-switch" :data-size="props.size">
    <label class="k-switch__hit">
      <button
        :id="id"
        class="k-switch__control"
        type="button"
        role="switch"
        :aria-checked="model ? 'true' : 'false'"
        :aria-labelledby="labelId"
        :aria-describedby="props.hint ? hintId : undefined"
        :disabled="props.disabled"
        @click="model = !model"
      >
        <span class="k-switch__thumb"></span>
      </button>
      <span
        :id="labelId"
        class="k-switch__label"
        :class="{ 'k-visually-hidden': props.hideLabel }"
        >{{ props.label }}</span
      >
    </label>
    <p v-if="props.hint" :id="hintId" class="k-switch__hint">{{ props.hint }}</p>
  </div>
</template>

<style>
@layer components {
  .k-switch {
    --k-switch-inline: var(--k-space-8);
    --k-switch-block: var(--k-space-5);
    --k-switch-inset: var(--k-space-0-5);

    display: grid;
    grid-template-columns: minmax(0, 1fr);
    justify-items: start;
    min-inline-size: 0;
  }

  /* The target: the switch and its label, a control's height. */
  .k-switch__hit {
    display: inline-flex;
    gap: var(--k-space-2);
    align-items: center;
    min-block-size: var(--k-control-height-md);
    max-inline-size: 100%;
    color: var(--k-text);
  }

  .k-switch[data-size="sm"] .k-switch__hit {
    min-block-size: var(--k-control-height-sm);
  }

  .k-switch[data-size="lg"] .k-switch__hit {
    min-block-size: var(--k-control-height-lg);
  }

  .k-switch__control {
    display: flex;
    flex: none;
    align-items: center;
    inline-size: var(--k-switch-inline);
    block-size: var(--k-switch-block);
    padding: 0 var(--k-switch-inset);
    margin: 0;
    background-color: var(--k-surface);
    border: var(--k-border-width) solid var(--k-border-control);
    border-radius: var(--k-radius-full);
    transition-property: background-color, border-color;
    transition-duration: var(--k-duration-fade);
    transition-timing-function: var(--k-ease-standard);
  }

  /* As high as the track inside its border and inset; on, it travels the length of the
   * track less its height. */
  .k-switch__thumb {
    inline-size: calc(
      var(--k-switch-block) - var(--k-switch-inset) * 2 - var(--k-border-width) * 2
    );
    block-size: calc(var(--k-switch-block) - var(--k-switch-inset) * 2 - var(--k-border-width) * 2);
    background-color: var(--k-border-control);
    border-radius: var(--k-radius-full);
    transition-property: translate, background-color;
    transition-duration: var(--k-duration-short);
    transition-timing-function: var(--k-ease-standard);
  }

  .k-switch__hit:hover .k-switch__control:not(:disabled) {
    border-color: var(--k-text-muted);
  }

  .k-switch__control[aria-checked="true"] {
    background-color: var(--k-accent);
    border-color: var(--k-accent);
  }

  .k-switch__hit:hover .k-switch__control[aria-checked="true"]:not(:disabled) {
    background-color: var(--k-accent-hover);
    border-color: var(--k-accent-hover);
  }

  .k-switch__control[aria-checked="true"] .k-switch__thumb {
    translate: calc(var(--k-switch-inline) - var(--k-switch-block)) 0;
    background-color: var(--k-accent-fg);
  }

  .k-switch__control[aria-checked="true"]:dir(rtl) .k-switch__thumb {
    translate: calc(var(--k-switch-block) - var(--k-switch-inline)) 0;
  }

  .k-switch__control:focus-visible {
    outline: var(--k-focus-width) solid var(--k-focus-ring);
    outline-offset: var(--k-focus-offset);
  }

  .k-switch__control:disabled {
    background-color: var(--k-surface-sunken);
    border-color: var(--k-border-strong);
  }

  .k-switch__control:disabled .k-switch__thumb {
    background-color: var(--k-border-strong);
  }

  .k-switch__label {
    min-inline-size: 0;
    font-size: var(--k-text-md);
    line-height: var(--k-text-md-line);
  }

  .k-switch__hit:has(:disabled) {
    color: var(--k-text-muted);
  }

  .k-switch__hint {
    padding-inline-start: calc(var(--k-switch-inline) + var(--k-space-2));
    font-size: var(--k-text-sm);
    line-height: var(--k-text-sm-line);
    color: var(--k-text-muted);
  }
}
</style>
