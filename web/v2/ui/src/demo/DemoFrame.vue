<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

<script setup lang="ts">
// A piece of the frame for the catalogue: a container of fixed height, positioned, with an
// isolated surface that scrolls inside it (« Structure avant style ») and room for the
// right panel laid over it, and an `auto` track at the end for the strip of the closed
// panel. Shows the sticky headers, the scroll in place, the overlay. `phone`: the width of
// a compact window, and a containing block for what is fixed to the window (layout
// containment): a sheet covers the frame, not the page of the catalogue.
const props = withDefaults(defineProps<{ size?: "sm" | "md" | "phone" }>(), { size: "sm" });
</script>

<template>
  <div class="k-demo-frame" :data-size="props.size">
    <div class="k-demo-frame__surface"><slot /></div>
    <slot name="panel" />
  </div>
</template>

<style>
@layer components {
  .k-demo-frame {
    position: relative;
    display: grid;
    grid-template-rows: minmax(0, 1fr);
    grid-template-columns: minmax(0, 1fr) auto;
    block-size: var(--k-size-dialog-sm);
    overflow: hidden;
    background-color: var(--k-bg);
    border: var(--k-border-width) solid var(--k-border-strong);
    border-radius: var(--k-radius-md);
  }

  .k-demo-frame[data-size="md"] {
    block-size: var(--k-size-dialog-md);
  }

  .k-demo-frame[data-size="phone"] {
    inline-size: min(100%, var(--k-size-dialog-sm));
    block-size: var(--k-size-dialog-md);
    contain: layout paint;
  }

  .k-demo-frame__surface {
    display: flex;
    flex-direction: column;
    gap: var(--k-space-3);
    min-inline-size: 0;
    min-block-size: 0;
    padding: var(--k-space-3);
    overflow: auto;
    isolation: isolate;
  }
}
</style>
