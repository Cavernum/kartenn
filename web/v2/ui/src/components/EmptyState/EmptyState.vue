<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

<script setup lang="ts">
// The empty state of a data view (design « Inventaire », line 25; REQ-020-23): one sentence
// and, when one exists, one action (slot `action`); an icon, muted. Never a blank view, no
// illustration.
import type { IconName } from "../../icons/index.ts";
import Icon from "../Icon/Icon.vue";

const props = withDefaults(defineProps<{ title: string; icon?: IconName }>(), {
  icon: "circle-dashed",
});
</script>

<template>
  <div class="k-empty-state">
    <Icon :name="props.icon" :size="20" class="k-empty-state__icon" />
    <p class="k-empty-state__title">{{ props.title }}</p>
    <p v-if="$slots.default" class="k-empty-state__text"><slot /></p>
    <div v-if="$slots.action" class="k-empty-state__action"><slot name="action" /></div>
  </div>
</template>

<style>
@layer components {
  .k-empty-state {
    display: grid;
    gap: var(--k-space-2);
    justify-items: center;
    padding: var(--k-space-8) var(--k-space-4);
    text-align: center;
  }

  .k-empty-state__icon {
    color: var(--k-text-muted);
  }

  .k-empty-state__title {
    margin: 0;
    font-weight: var(--k-weight-medium);
  }

  .k-empty-state__text {
    max-inline-size: var(--k-measure-prose);
    margin: 0;
    color: var(--k-text-muted);
  }

  .k-empty-state__action {
    margin-block-start: var(--k-space-1);
  }
}
</style>
