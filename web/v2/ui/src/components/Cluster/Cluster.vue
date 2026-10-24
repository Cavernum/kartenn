<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

<script setup lang="ts">
// A cluster (design « Inventaire », line 1): children in a row that wraps, a gap of the
// spacing scale; a toolbar, a row of chips, the actions of a form.
import type { Align, Space } from "../layout.ts";

const props = withDefaults(
  defineProps<{
    gap?: Space;
    align?: Align;
    justify?: "start" | "between" | "end";
    wrap?: boolean;
    as?: string;
  }>(),
  { gap: "2", align: "center", justify: "start", wrap: true, as: "div" },
);
</script>

<template>
  <component
    :is="props.as"
    class="k-cluster"
    :data-k-gap="props.gap"
    :data-k-align="props.align"
    :data-justify="props.justify"
    :data-wrap="props.wrap ? 'wrap' : 'nowrap'"
  >
    <slot />
  </component>
</template>

<style>
@layer components {
  .k-cluster {
    display: flex;
    flex-wrap: wrap;
    gap: var(--k-layout-gap);
    min-inline-size: 0;
  }

  .k-cluster[data-wrap="nowrap"] {
    flex-wrap: nowrap;
  }

  .k-cluster[data-justify="between"] {
    justify-content: space-between;
  }

  .k-cluster[data-justify="end"] {
    justify-content: end;
  }
}
</style>
