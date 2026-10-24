<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

<script setup lang="ts">
// A button (APG Button; design « Inventaire », line 2): four variants, three sizes shared
// with the fields (REQ-020-17), an optional leading icon and shortcut. Busy, it keeps the
// focus (`aria-disabled`, `aria-busy`) and ignores activation; disabled, it leaves the
// tab order (native `disabled`).
import { computed } from "vue";
import Icon from "../Icon/Icon.vue";
import Kbd from "../Kbd/Kbd.vue";
import type { ButtonProps } from "./button.ts";
import { ariaKeyshortcuts } from "./shortcut.ts";

const props = withDefaults(defineProps<ButtonProps>(), {
  variant: "secondary",
  size: "md",
  type: "button",
  pressed: undefined,
});
const emit = defineEmits<{ click: [event: MouseEvent] }>();

const keyshortcuts = computed(() =>
  props.shortcut ? ariaKeyshortcuts(props.shortcut) : undefined,
);

function onClick(event: MouseEvent): void {
  if (props.loading || props.disabled) return;
  emit("click", event);
}
</script>

<template>
  <button
    class="k-button"
    :type="props.type"
    :data-variant="props.variant"
    :data-size="props.size"
    :disabled="props.disabled"
    :aria-disabled="props.loading ? 'true' : undefined"
    :aria-busy="props.loading ? 'true' : undefined"
    :aria-pressed="props.pressed"
    :aria-keyshortcuts="keyshortcuts"
    @click="onClick"
  >
    <Icon v-if="props.loading" class="k-spinner" name="loader-circle" />
    <Icon v-else-if="props.icon" :name="props.icon" />
    <span class="k-button__label"><slot /></span>
    <Kbd v-if="props.shortcut" :keys="props.shortcut" aria-hidden="true" />
  </button>
</template>

<style>
@layer components {
  .k-button {
    --k-button-bg: var(--k-surface);
    --k-button-fg: var(--k-text);
    --k-button-border: var(--k-border-control);
    --k-button-hover: var(--k-hover);

    display: inline-flex;
    flex: none;
    gap: var(--k-space-2);
    align-items: center;
    justify-content: center;
    min-block-size: var(--k-control-height-md);
    padding-inline: var(--k-control-padding-inline-md);
    font: inherit;
    font-weight: var(--k-weight-medium);
    color: var(--k-button-fg);
    white-space: nowrap;
    cursor: pointer;
    background-color: var(--k-button-bg);
    border: var(--k-border-width) solid var(--k-button-border);
    border-radius: var(--k-radius-md);
    transition-property: background-color, border-color, color;
    transition-duration: var(--k-duration-short);
    transition-timing-function: var(--k-ease-standard);
  }

  .k-button[data-size="sm"] {
    gap: var(--k-space-1);
    min-block-size: var(--k-control-height-sm);
    padding-inline: var(--k-control-padding-inline-sm);
    font-size: var(--k-text-sm);
  }

  .k-button[data-size="lg"] {
    min-block-size: var(--k-control-height-lg);
    padding-inline: var(--k-control-padding-inline-lg);
  }

  .k-button[data-variant="primary"] {
    --k-button-bg: var(--k-accent);
    --k-button-fg: var(--k-accent-fg);
    --k-button-border: var(--k-accent);
    --k-button-hover: var(--k-accent-hover);
  }

  .k-button[data-variant="danger"] {
    --k-button-bg: var(--k-danger);
    --k-button-fg: var(--k-danger-fg);
    --k-button-border: var(--k-danger);
    --k-button-hover: var(--k-danger-hover);
  }

  .k-button[data-variant="ghost"] {
    background-color: transparent;
    border-color: transparent;
  }

  .k-button:hover:not(:disabled, [aria-disabled="true"]) {
    background-color: var(--k-button-hover);
  }

  .k-button:active:not(:disabled, [aria-disabled="true"]),
  .k-button[aria-pressed="true"] {
    --k-button-hover: var(--k-active);

    background-color: var(--k-active);
  }

  .k-button[data-variant="primary"]:active:not(:disabled, [aria-disabled="true"]),
  .k-button[data-variant="danger"]:active:not(:disabled, [aria-disabled="true"]) {
    background-color: var(--k-button-hover);
  }

  .k-button:disabled {
    color: var(--k-text-muted);
    cursor: not-allowed;
    background-color: var(--k-surface-sunken);
    border-color: var(--k-border-strong);
  }

  .k-button[aria-busy="true"] {
    cursor: progress;
  }

  .k-button__label {
    display: inline-flex;
    gap: var(--k-space-1);
    align-items: center;
  }

  .k-button__label:empty {
    display: none;
  }

  .k-button .k-kbd {
    margin-inline-start: var(--k-space-1);
  }

  /* On a filled button, keycaps take its colours. */
  .k-button[data-variant="primary"] .k-kbd__key,
  .k-button[data-variant="danger"] .k-kbd__key {
    color: inherit;
    background-color: transparent;
    border-color: currentcolor;
  }
}
</style>
