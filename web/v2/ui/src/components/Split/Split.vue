<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

<script setup lang="ts">
// A split (design « Inventaire », line 1): a side of fixed width (`--k-size-panel-*`) and
// a main part that takes the rest; the side at the start or the end, in reading order
// too. No resizing handle:
// Reka's Splitter injects a <style> (REQ-020-33).
import type { Space } from "../layout.ts";

const props = withDefaults(
  defineProps<{ side?: "start" | "end"; sideSize?: "sm" | "md" | "lg"; gap?: Space }>(),
  { side: "start", sideSize: "md", gap: "4" },
);
</script>

<template>
  <div
    class="k-split"
    :data-side="props.side"
    :data-side-size="props.sideSize"
    :data-k-gap="props.gap"
  >
    <div v-if="props.side === 'start'" class="k-split__side"><slot name="side" /></div>
    <div class="k-split__main"><slot /></div>
    <div v-if="props.side === 'end'" class="k-split__side"><slot name="side" /></div>
  </div>
</template>

<style>
@layer components {
  .k-split {
    --k-split-side: var(--k-size-panel-md);

    display: grid;
    grid-template-columns: var(--k-split-side) minmax(0, 1fr);
    gap: var(--k-layout-gap);
    min-block-size: 0;
  }

  .k-split[data-side-size="sm"] {
    --k-split-side: var(--k-size-panel-sm);
  }

  .k-split[data-side-size="lg"] {
    --k-split-side: var(--k-size-panel-lg);
  }

  .k-split[data-side="end"] {
    grid-template-columns: minmax(0, 1fr) var(--k-split-side);
  }

  .k-split__side,
  .k-split__main {
    min-inline-size: 0;
  }
}
</style>
