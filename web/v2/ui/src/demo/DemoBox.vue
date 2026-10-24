<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

<script setup lang="ts">
// A placeholder of the catalogue, to show the layouts (Stack, Cluster, Grid, Split).
const props = defineProps<{ label: string }>();
</script>

<template>
  <div class="k-demo-box">{{ props.label }}</div>
</template>

<style>
@layer components {
  .k-demo-box {
    display: grid;
    place-items: center;
    min-block-size: var(--k-control-height-lg);
    min-inline-size: var(--k-space-12);
    padding-inline: var(--k-space-3);
    color: var(--k-text-muted);
    background-color: var(--k-surface-sunken);
    border: var(--k-border-width) dashed var(--k-border-control);
    border-radius: var(--k-radius-md);
  }
}
</style>
