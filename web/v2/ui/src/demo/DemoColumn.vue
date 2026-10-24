<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

<script setup lang="ts">
// A column of the width of a panel, for the catalogue: a form, a toolbar shown at the
// width they get in the right panel (`lg`, 420 px) or narrower (`md`, `sm`).
const props = withDefaults(defineProps<{ size?: "sm" | "md" | "lg" }>(), { size: "lg" });
</script>

<template>
  <div class="k-demo-column" :data-size="props.size"><slot /></div>
</template>

<style>
@layer components {
  .k-demo-column {
    display: grid;
    gap: var(--k-space-2);
    inline-size: var(--k-size-panel-lg);
    max-inline-size: 100%;
  }

  .k-demo-column[data-size="md"] {
    inline-size: var(--k-size-panel-md);
  }

  .k-demo-column[data-size="sm"] {
    inline-size: var(--k-size-panel-sm);
  }
}
</style>
