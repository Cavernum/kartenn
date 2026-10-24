<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

<script setup lang="ts">
// A keyboard shortcut (design « Inventaire », line 28): one keycap per key of a
// combination, nested <kbd> as HTML has it for a combination; shown in menus, tooltips,
// buttons and the help `?`.
import { computed } from "vue";
import { hasModifier, keyLabel } from "./kbd.ts";

const props = defineProps<{ keys: string[] }>();
const modified = computed(() => hasModifier(props.keys));
</script>

<template>
  <kbd class="k-kbd">
    <kbd v-for="(key, i) in props.keys" :key="i" class="k-kbd__key">{{
      keyLabel(key, modified)
    }}</kbd>
  </kbd>
</template>

<style>
@layer components {
  .k-kbd {
    display: inline-flex;
    gap: var(--k-space-0-5);
    align-items: center;
    font-family: var(--k-font-sans);
    font-size: var(--k-text-xs);
    line-height: var(--k-text-xs-line);
  }

  .k-kbd__key {
    display: inline-grid;
    place-items: center;
    min-inline-size: var(--k-space-4);
    padding-inline: var(--k-space-1);
    font-family: var(--k-font-sans);
    font-size: var(--k-text-xs);
    line-height: var(--k-text-xs-line);
    color: var(--k-text-muted);
    background-color: var(--k-surface-sunken);
    border: var(--k-border-width) solid var(--k-border-strong);
    border-radius: var(--k-radius-sm);
  }
}
</style>
