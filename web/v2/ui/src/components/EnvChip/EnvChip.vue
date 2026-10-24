<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

<script setup lang="ts">
// An environment (design « Inventaire », line 23; REQ-020-8, 21): its dot, of a hue of the
// closed palette, and its label, never the dot alone; without a label, its key in fixed
// pitch.
const props = defineProps<{ value: string; label?: string | null; hue: 1 | 2 | 3 | 4 | 5 | 6 }>();
</script>

<template>
  <span class="k-env-chip" :data-hue="props.hue">
    <span class="k-env-chip__dot" aria-hidden="true"></span>
    <span v-if="props.label" class="k-env-chip__label">{{ props.label }}</span>
    <code v-else class="k-env-chip__label">{{ props.value }}</code>
  </span>
</template>

<style>
@layer components {
  .k-env-chip {
    --k-env-chip-hue: var(--k-env-1);

    display: inline-flex;
    flex: none;
    gap: var(--k-space-1);
    align-items: center;
    min-block-size: var(--k-space-5);
    padding-inline: var(--k-space-2);
    font-size: var(--k-text-sm);
    line-height: var(--k-text-sm-line);
    color: var(--k-text);
    white-space: nowrap;
    background-color: var(--k-surface);
    border: var(--k-border-width) solid var(--k-border-strong);
    border-radius: var(--k-radius-full);
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
    inline-size: var(--k-env-dot-size);
    block-size: var(--k-env-dot-size);
    background-color: var(--k-env-chip-hue);
    border-radius: var(--k-radius-full);
  }

  .k-env-chip code.k-env-chip__label {
    font-size: var(--k-text-sm);
    line-height: var(--k-text-sm-line);
  }
}
</style>
