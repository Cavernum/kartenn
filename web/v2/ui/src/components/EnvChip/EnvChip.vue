<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

<script setup lang="ts">
// An environment (design « Inventaire », line 23; REQ-020-8, 21): its dot, of a hue of the
// closed palette, and its label, never the dot alone; without a label, its key in fixed
// pitch. The hue comes from the key (`envHue`), unless the host gives one. Like
// `KeyLabel`, the chip is a button that copies its key, shown in its bubble and said in
// its description; `plain` for a chip inside a control, its key then shown beside its
// label (REQ-020-21: never behind nothing). In forced colours the dot keeps
// its ring (its border, of its own hue otherwise): the backgrounds are the system's.
import { computed, ref, useId } from "vue";
import { useCopy } from "../internal/copy.ts";
import KeyBubble from "../internal/KeyBubble.vue";
import { envHue, type EnvHue } from "./env.ts";

const props = defineProps<{
  value: string;
  label?: string | null;
  hue?: EnvHue | undefined;
  plain?: boolean;
}>();

const describedId = useId();
const bubble = ref<InstanceType<typeof KeyBubble> | null>(null);
const copy = useCopy({
  text: () => props.value,
  selection: () => bubble.value?.keyNode(),
  atRest: () => true,
  // The key already shown (no label): the bubble only repeats the button, Escape goes on.
  passAtRest: () => !props.label,
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
  <span class="k-env-chip-host">
    <component
      :is="props.plain ? 'span' : 'button'"
      class="k-env-chip-frame"
      v-bind="control"
      v-on="props.plain ? {} : copy.listeners"
    >
      <span class="k-env-chip" :data-hue="props.hue ?? envHue(props.value)">
        <span class="k-env-chip__dot" aria-hidden="true"></span>
        <span v-if="props.label" class="k-env-chip__label">{{ props.label }}</span>
        <code v-else class="k-env-chip__label">{{ props.value }}</code>
        <!-- Plain, no bubble: the key beside its label. -->
        <code v-if="props.plain && props.label" class="k-env-chip__key">{{ props.value }}</code>
      </span>
    </component>
    <KeyBubble
      v-if="!props.plain"
      ref="bubble"
      :value="props.value"
      :label="props.label"
      :copy="copy"
      :described-id="describedId"
    />
  </span>
</template>

<style>
@layer components {
  .k-env-chip-host,
  .k-env-chip-frame {
    display: inline-flex;
    flex: none;
  }

  /* The button around the pill: a target of 24 px (WCAG 2.5.8) that takes no more room
   * than the pill of 20 px (its padding given back by a negative margin). */
  button.k-env-chip-frame {
    padding: calc(var(--k-space-px) * 2) 0;
    margin: calc(var(--k-space-px) * -2) 0;
    font: inherit;
    cursor: copy;
    background: none;
    border: 0;
    border-radius: var(--k-radius-full);
  }

  /* The ring in the frame, around the pill: within the 24 px of the target, so a row of
   * 28 px holds it whole (the row after paints over what passes its border). */
  button.k-env-chip-frame:focus-visible {
    outline-offset: calc(var(--k-focus-width) * -1);
  }

  .k-env-chip {
    --k-env-chip-hue: var(--k-env-1);

    display: inline-flex;
    flex: none;
    gap: var(--k-space-1);
    align-items: center;
    min-block-size: var(--k-space-5);
    padding-inline: var(--k-space-2);
    font-family: var(--k-font-sans);
    font-size: var(--k-text-sm);
    line-height: var(--k-text-sm-line);
    font-weight: var(--k-weight-regular);
    color: var(--k-text);
    white-space: nowrap;
    background-color: var(--k-surface);
    border: var(--k-border-width) solid var(--k-border-strong);
    border-radius: var(--k-radius-full);
  }

  button.k-env-chip-frame > .k-env-chip {
    transition-property: background-color, border-color;
    transition-duration: var(--k-duration-fade);
    transition-timing-function: var(--k-ease-standard);
  }

  button.k-env-chip-frame:hover > .k-env-chip {
    background-color: var(--k-hover);
    border-color: var(--k-border-control);
  }

  .k-env-chip[data-hue="2"] {
    --k-env-chip-hue: var(--k-env-2);
  }

  .k-env-chip[data-hue="3"] {
    --k-env-chip-hue: var(--k-env-3);
  }

  .k-env-chip[data-hue="4"] {
    --k-env-chip-hue: var(--k-env-4);
  }

  .k-env-chip[data-hue="5"] {
    --k-env-chip-hue: var(--k-env-5);
  }

  .k-env-chip[data-hue="6"] {
    --k-env-chip-hue: var(--k-env-6);
  }

  .k-env-chip__dot {
    box-sizing: border-box;
    flex: none;
    inline-size: var(--k-env-dot-size);
    block-size: var(--k-env-dot-size);
    background-color: var(--k-env-chip-hue);
    border: var(--k-border-width) solid var(--k-env-chip-hue);
    border-radius: var(--k-radius-full);
  }

  .k-env-chip code.k-env-chip__label,
  .k-env-chip__key {
    font-size: var(--k-text-sm);
    line-height: var(--k-text-sm-line);
  }

  .k-env-chip__key {
    color: var(--k-text-muted);
  }
}
</style>
