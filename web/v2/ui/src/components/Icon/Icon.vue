<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

<script setup lang="ts">
// An icon of the closed list (REQ-020-15): decorative and hidden from assistive
// technologies by default; an icon that alone carries a meaning takes `label`, its
// accessible name (role img). The type of the props makes `meaningful` without a label
// a compile error.
import { computed } from "vue";
import { icons } from "../../icons/index.ts";
import type { IconProps } from "./icon.ts";

const props = withDefaults(defineProps<IconProps>(), { size: 16, label: undefined });

const node = computed(() => icons[props.name]);
</script>

<template>
  <svg
    class="k-icon"
    :data-size="props.size"
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    stroke-linecap="round"
    stroke-linejoin="round"
    focusable="false"
    :role="props.label ? 'img' : undefined"
    :aria-label="props.label"
    :aria-hidden="props.label ? undefined : 'true'"
  >
    <component :is="tag" v-for="([tag, attrs], i) in node" :key="i" v-bind="attrs" />
  </svg>
</template>

<style>
@layer components {
  .k-icon {
    flex: none;
    inline-size: var(--k-icon-size);
    block-size: var(--k-icon-size);
    stroke-width: var(--k-icon-stroke);
  }

  .k-icon[data-size="20"] {
    inline-size: var(--k-icon-size-lg);
    block-size: var(--k-icon-size-lg);
  }
}
</style>
