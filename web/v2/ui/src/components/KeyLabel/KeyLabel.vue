<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

<script setup lang="ts">
// A key and its label (design « Inventaire », line 23; REQ-020-21): the label in the
// language of the user; without one, the key in fixed pitch, never an empty string.
// `showKey` puts the key beside the label (lists where the key is what one types). The
// key in a tooltip and its copy come with the complete version (task 24).
const props = defineProps<{ value: string; label?: string | null; showKey?: boolean }>();
</script>

<template>
  <span class="k-key-label" :data-state="props.label ? 'labelled' : 'key-only'">
    <template v-if="props.label">
      <span class="k-key-label__label">{{ props.label }}</span>
      <code v-if="props.showKey" class="k-key-label__key">{{ props.value }}</code>
    </template>
    <code v-else class="k-key-label__key" data-alone>{{ props.value }}</code>
  </span>
</template>

<style>
@layer components {
  .k-key-label {
    display: inline-flex;
    gap: var(--k-space-2);
    align-items: baseline;
    min-inline-size: 0;
  }

  .k-key-label__label {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .k-key-label__key {
    color: var(--k-text-muted);
    white-space: nowrap;
  }

  .k-key-label__key[data-alone] {
    color: var(--k-text);
  }
}
</style>
