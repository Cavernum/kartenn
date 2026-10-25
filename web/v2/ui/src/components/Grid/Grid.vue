<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

<script setup lang="ts">
// A layout grid (design « Inventaire », line 1): equal columns, at most a fixed number of
// them or as many tiles of `--k-size-tile-min` as fit (`auto`). Intrinsic (design
// « Adaptation à la largeur »): the columns follow the width the grid is given, never the
// window — a fixed number gives way to fewer columns when each would be narrower than
// `--k-size-col-md`, and a tile never overflows a narrower grid. A layout, not the data
// grid (`DataGrid`).
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

  /* At most n columns: each the n-th of the width (less the gaps), never under `col-md`. */
  .k-grid[data-columns="2"] {
    grid-template-columns: repeat(
      auto-fill,
      minmax(min(100%, max(var(--k-size-col-md), (100% - var(--k-layout-gap)) / 2)), 1fr)
    );
  }

  .k-grid[data-columns="3"] {
    grid-template-columns: repeat(
      auto-fill,
      minmax(min(100%, max(var(--k-size-col-md), (100% - 2 * var(--k-layout-gap)) / 3)), 1fr)
    );
  }

  .k-grid[data-columns="4"] {
    grid-template-columns: repeat(
      auto-fill,
      minmax(min(100%, max(var(--k-size-col-md), (100% - 3 * var(--k-layout-gap)) / 4)), 1fr)
    );
  }

  .k-grid[data-columns="6"] {
    grid-template-columns: repeat(
      auto-fill,
      minmax(min(100%, max(var(--k-size-col-md), (100% - 5 * var(--k-layout-gap)) / 6)), 1fr)
    );
  }

  /* As many tiles as fit; fewer tiles than room, they share it (`auto-fit`). */
  .k-grid[data-columns="auto"] {
    grid-template-columns: repeat(auto-fit, minmax(min(100%, var(--k-size-tile-min)), 1fr));
  }

  /* The captions of the tiles, where they wrap, break into lines of even length
   * (progressive; the style alone: a caption kept on one line stays so). */
  .k-grid[data-columns="auto"] :is(h2, h3, h4) {
    text-wrap-style: balance;
  }
}
</style>
