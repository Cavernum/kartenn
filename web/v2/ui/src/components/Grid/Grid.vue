<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

<script setup lang="ts">
// A layout grid (design « Inventaire », line 1): equal columns, a fixed number of them or
// as many panels of `--k-size-panel-sm` as fit (`auto`). A layout, not the data grid
// (`DataGrid`).
import type { Align, Space } from "../layout.ts";

const props = withDefaults(
  defineProps<{ columns?: 1 | 2 | 3 | 4 | 6 | "auto"; gap?: Space; align?: Align; as?: string }>(),
  { columns: "auto", gap: "4", align: "stretch", as: "div" },
);
</script>

<template>
  <component
    :is="props.as"
    class="k-grid"
    :data-columns="props.columns"
    :data-k-gap="props.gap"
    :data-k-align="props.align"
  >
    <slot />
  </component>
</template>

<style>
@layer components {
  .k-grid {
    display: grid;
    grid-template-columns: minmax(0, 1fr);
    gap: var(--k-layout-gap);
  }

  .k-grid[data-columns="2"] {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }

  .k-grid[data-columns="3"] {
    grid-template-columns: repeat(3, minmax(0, 1fr));
  }

  .k-grid[data-columns="4"] {
    grid-template-columns: repeat(4, minmax(0, 1fr));
  }

  .k-grid[data-columns="6"] {
    grid-template-columns: repeat(6, minmax(0, 1fr));
  }

  .k-grid[data-columns="auto"] {
    grid-template-columns: repeat(auto-fill, minmax(var(--k-size-panel-sm), 1fr));
  }
}
</style>
