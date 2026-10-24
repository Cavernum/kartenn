<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

<script setup lang="ts">
// The draft counter of the status bar (APG Button; design « Inventaire », line 27; spec
// 021, « Cadre »): « Brouillon (n) », which opens and closes the draft panel
// (`aria-expanded`, `aria-controls`); muted at zero.
import { t } from "../../i18n.ts";
import Icon from "../Icon/Icon.vue";

const props = withDefaults(
  defineProps<{ count: number; expanded?: boolean; controls?: string | undefined }>(),
  { controls: undefined },
);
const emit = defineEmits<{ toggle: [] }>();
</script>

<template>
  <button
    type="button"
    class="k-draft-counter"
    :data-state="props.count === 0 ? 'empty' : 'pending'"
    :aria-expanded="props.expanded"
    :aria-controls="props.controls"
    @click="emit('toggle')"
  >
    <Icon name="draft" />
    <span>{{ t("draft") }}</span>
    <span class="k-draft-counter__count"
      ><span class="k-visually-hidden">(</span>{{ props.count
      }}<span class="k-visually-hidden">)</span></span
    >
  </button>
</template>

<style>
@layer components {
  .k-draft-counter {
    display: inline-flex;
    gap: var(--k-space-1);
    align-items: center;
    min-block-size: var(--k-control-height-md);
    padding-inline: var(--k-control-padding-inline-md) var(--k-control-padding-inline-sm);
    font: inherit;
    font-weight: var(--k-weight-medium);
    color: var(--k-text);
    cursor: pointer;
    background-color: var(--k-surface);
    border: var(--k-border-width) solid var(--k-border-control);
    border-radius: var(--k-radius-md);
    transition-property: background-color;
    transition-duration: var(--k-duration-short);
    transition-timing-function: var(--k-ease-standard);
  }

  .k-draft-counter:hover,
  .k-draft-counter[aria-expanded="true"] {
    background-color: var(--k-hover);
  }

  .k-draft-counter__count {
    min-inline-size: var(--k-space-5);
    padding-inline: var(--k-space-1);
    font-size: var(--k-text-sm);
    line-height: var(--k-text-sm-line);
    font-variant-numeric: tabular-nums;
    color: var(--k-accent-fg);
    text-align: center;
    background-color: var(--k-accent);
    border-radius: var(--k-radius-full);
  }

  .k-draft-counter[data-state="empty"] {
    color: var(--k-text-muted);
  }

  .k-draft-counter[data-state="empty"] .k-draft-counter__count {
    color: var(--k-text-muted);
    background-color: var(--k-surface-sunken);
  }
}
</style>
