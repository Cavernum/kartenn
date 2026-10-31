<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

<script setup lang="ts">
// A key and its label (design « Inventaire », line 23; REQ-020-21): the label in the
// language of the user; without one, the key in fixed pitch, never an empty string.
// `showKey` puts the key beside the label (lists where the key is what one types),
// `stacked` under it (a narrow column: neither is cut). The key is always at hand: the
// pastille is a button that copies it (a click, Enter, Space), its bubble shows it under
// the pointer and the keyboard focus, and its description says it (« Clé : prod ») with
// how to copy it; the outcome is said by a `status` region and shown in the bubble
// (`useCopy`, `KeyBubble`). Its name is what it shows. `plain`: text, no button, for a
// pastille inside a control (a button, a link) or a grid whose rows activate; the key is
// then always shown beside the label (it is no longer behind a bubble, REQ-020-21).
import { computed, ref, useId } from "vue";
import { useCopy } from "../internal/copy.ts";
import KeyBubble from "../internal/KeyBubble.vue";

const props = defineProps<{
  value: string;
  label?: string | null;
  showKey?: boolean;
  stacked?: boolean;
  plain?: boolean;
}>();

const describedId = useId();
const bubble = ref<InstanceType<typeof KeyBubble> | null>(null);
/** The label, cut by the width: whole in the bubble (never an ellipsis with no way to read). */
const shown = ref<HTMLElement | null>(null);
const isCut = () => !!shown.value && shown.value.scrollWidth > shown.value.clientWidth;
const copy = useCopy({
  text: () => props.value,
  selection: () => bubble.value?.keyNode(),
  atRest: () => true,
  // The key already shown (beside, under, or alone): the bubble only repeats the button,
  // Escape goes on to the layer around it once it has closed it.
  passAtRest: () => !props.label || !!props.showKey || !!props.stacked,
});
const control = computed(() =>
  props.plain
    ? {}
    : {
        type: "button",
        "data-outcome": copy.outcome.value ?? undefined,
        "aria-describedby": describedId,
      },
);
</script>

<template>
  <span class="k-key-label" :data-state="props.label ? 'labelled' : 'key-only'">
    <component
      :is="props.plain ? 'span' : 'button'"
      class="k-key-label__text"
      :data-stacked="props.stacked && props.label ? 'true' : undefined"
      v-bind="control"
      v-on="props.plain ? {} : copy.listeners"
    >
      <template v-if="props.label">
        <span ref="shown" class="k-key-label__label">{{ props.label }}</span>
        <code v-if="props.showKey || props.stacked || props.plain" class="k-key-label__key">{{
          props.value
        }}</code>
      </template>
      <code v-else class="k-key-label__key" data-alone>{{ props.value }}</code>
    </component>
    <KeyBubble
      v-if="!props.plain"
      ref="bubble"
      :value="props.value"
      :label="props.label"
      :copy="copy"
      :described-id="describedId"
      :cut="isCut"
    />
  </span>
</template>

<style>
@layer components {
  .k-key-label {
    display: inline-flex;
    min-inline-size: 0;
    max-inline-size: 100%;
  }

  .k-key-label__text {
    display: inline-flex;
    gap: var(--k-space-2);
    align-items: baseline;
    min-inline-size: 0;
    max-inline-size: 100%;
  }

  /* The key under the label. */
  .k-key-label__text[data-stacked] {
    flex-direction: column;
    gap: 0;
    align-items: start;
  }

  /* The text it shows, as a button: no box at rest; under the pointer and the focus, a
   * dotted underline says there is more (its key), the cursor that a click copies. A
   * target of 24 px (WCAG 2.5.8) that takes no more room than its line: the height and
   * the width it adds (room for the focus ring off the glyphs) are given back by negative
   * margins (`--k-size-line`, its own line). */
  button.k-key-label__text {
    align-items: center;
    /* Its root is narrower by the inline margins given back: the room they take is its. */
    min-inline-size: min(var(--k-space-6), calc(100% + var(--k-space-1) * 2));
    max-inline-size: calc(100% + var(--k-space-1) * 2);
    min-block-size: var(--k-space-6);
    padding: 0 var(--k-space-1);
    /* Never positive: a line taller than the target is left as it is. */
    margin: calc((min(var(--k-size-line), var(--k-space-6)) - var(--k-space-6)) / 2)
      calc(var(--k-space-1) * -1);
    font: inherit;
    color: inherit;
    text-align: start;
    cursor: copy;
    background: none;
    border: 0;
    border-radius: var(--k-radius-sm);
  }

  /* The ring on the line it gives back, inside its box: whole even where a cell clips its
   * text to its line (`DataGrid`, WCAG 2.4.7). */
  button.k-key-label__text:focus-visible {
    outline-offset: calc(
      (min(var(--k-size-line), var(--k-space-6)) - var(--k-space-6)) / 2 - var(--k-focus-width)
    );
  }

  button.k-key-label__text[data-stacked] {
    align-items: start;
    margin-block: 0;
  }

  .k-key-label__text > * {
    text-decoration-line: underline;
    text-decoration-style: dotted;
    text-decoration-color: transparent;
    text-underline-offset: var(--k-space-1);
    transition-property: text-decoration-color;
    transition-duration: var(--k-duration-fade);
    transition-timing-function: var(--k-ease-standard);
  }

  button.k-key-label__text:hover > *,
  button.k-key-label__text:focus-visible > * {
    text-decoration-color: currentcolor;
  }

  /* Short of room, the key beside the label gives way first, then the label; both whole
   * in the bubble. */
  .k-key-label__label,
  .k-key-label__key {
    max-inline-size: 100%;
    min-inline-size: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .k-key-label__key {
    flex-shrink: 1000;
    color: var(--k-text-muted);
  }

  .k-key-label__key[data-alone] {
    color: var(--k-text);
  }
}
</style>
