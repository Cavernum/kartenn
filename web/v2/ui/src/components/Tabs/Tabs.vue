<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

<script setup lang="ts">
// Tabs (APG Tabs, manual activation; design « Inventaire », line 10): Reka's TabsRoot,
// arrows move the focus, Enter or Space activates. Each panel is the slot named after the
// tab's value. A plugin tab may be loading or failed: an icon and a hidden word say so.
import { TabsContent, TabsList, TabsRoot, TabsTrigger } from "reka-ui";
import { computed } from "vue";
import { t } from "../../i18n.ts";
import Icon from "../Icon/Icon.vue";
import type { TabItem } from "./tabs.ts";

const props = defineProps<{ tabs: TabItem[]; label: string }>();
const model = defineModel<string>({ default: "" });
/** The active tab: the model, else the first one. */
const active = computed({
  get: () => model.value || props.tabs[0]?.value || "",
  set: (value: string) => (model.value = value),
});
</script>

<template>
  <TabsRoot v-model="active" class="k-tabs" activation-mode="manual">
    <TabsList class="k-tabs__list" :aria-label="props.label">
      <TabsTrigger
        v-for="tab in props.tabs"
        :key="tab.value"
        :value="tab.value"
        :disabled="tab.disabled ?? false"
        class="k-tabs__tab"
        :data-tab-state="tab.state"
      >
        <Icon v-if="tab.state === 'loading'" name="loader-circle" class="k-spinner" />
        <Icon v-else-if="tab.state === 'error'" name="circle-x" class="k-tabs__error" />
        <Icon v-else-if="tab.icon" :name="tab.icon" />
        <span>{{ tab.label }}</span>
        <span v-if="tab.count !== undefined" class="k-tabs__count">{{ tab.count }}</span>
        <span v-if="tab.state" class="k-visually-hidden"
          >({{ tab.state === "loading" ? t("tabLoading") : t("tabError") }})</span
        >
      </TabsTrigger>
    </TabsList>
    <TabsContent
      v-for="tab in props.tabs"
      :key="tab.value"
      :value="tab.value"
      class="k-tabs__panel"
    >
      <slot :name="tab.value" />
    </TabsContent>
  </TabsRoot>
</template>

<style>
@layer components {
  .k-tabs {
    display: grid;
    grid-template-columns: minmax(0, 1fr);
    gap: var(--k-space-3);
    min-inline-size: 0;
  }

  .k-tabs__list {
    display: flex;
    gap: var(--k-space-1);
    overflow-x: auto;
    border-block-end: var(--k-border-width) solid var(--k-border-strong);
  }

  .k-tabs__tab {
    display: inline-flex;
    gap: var(--k-space-1);
    align-items: center;
    min-block-size: var(--k-control-height-lg);
    padding-inline: var(--k-control-padding-inline-md);
    margin-block-end: calc(var(--k-border-width) * -1);
    font: inherit;
    font-weight: var(--k-weight-medium);
    color: var(--k-text-muted);
    white-space: nowrap;
    cursor: pointer;
    background: none;
    border: 0;
    border-block-end: var(--k-focus-width) solid transparent;
    border-start-start-radius: var(--k-radius-md);
    border-start-end-radius: var(--k-radius-md);
    transition-property: color, border-color;
    transition-duration: var(--k-duration-short);
    transition-timing-function: var(--k-ease-standard);
  }

  .k-tabs__tab:hover:not([data-disabled]) {
    color: var(--k-text);
  }

  .k-tabs__tab[data-state="active"] {
    color: var(--k-text);
    border-block-end-color: var(--k-accent);
  }

  .k-tabs__tab[data-disabled] {
    cursor: not-allowed;
    opacity: 0.6;
  }

  .k-tabs__tab:focus-visible {
    outline-offset: calc(var(--k-focus-width) * -1);
  }

  .k-tabs__error {
    color: var(--k-state-ko-fg);
  }

  .k-tabs__count {
    min-inline-size: var(--k-space-4);
    padding-inline: var(--k-space-1);
    font-size: var(--k-text-xs);
    line-height: var(--k-text-xs-line);
    font-variant-numeric: tabular-nums;
    color: var(--k-text-muted);
    text-align: center;
    background-color: var(--k-surface-sunken);
    border-radius: var(--k-radius-full);
  }

  .k-tabs__panel:focus-visible {
    outline-offset: var(--k-focus-offset);
  }
}
</style>
