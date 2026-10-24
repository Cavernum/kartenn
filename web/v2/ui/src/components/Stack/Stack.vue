<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

<script setup lang="ts">
// A vertical stack (design « Inventaire », line 1): children one under the other, a gap of
// the spacing scale; replaces the layout CSS of a screen.
import type { Align, Space } from "../layout.ts";

const props = withDefaults(defineProps<{ gap?: Space; align?: Align; as?: string }>(), {
  gap: "2",
  align: "stretch",
  as: "div",
});
</script>

<template>
  <component :is="props.as" class="k-stack" :data-k-gap="props.gap" :data-k-align="props.align">
    <slot />
  </component>
</template>

<style>
@layer components {
  .k-stack {
    display: flex;
    flex-direction: column;
    gap: var(--k-layout-gap);
    min-inline-size: 0;
  }
}
</style>
