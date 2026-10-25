<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

<script setup lang="ts">
// An inline message (design « Inventaire », line 26): the violations under a form, a
// warning above a table. `role="alert"` for an error (read at once), `status` otherwise.
// Colour, icon and title together (REQ-020-7); each violation shows its code, copyable
// (REQ-020-22). `icon` replaces the icon of the tone when it says more (a lock for an
// object read only); the colour and the role stay those of the tone.
import type { IconName } from "../../icons/index.ts";
import CodeBadge from "../CodeBadge/CodeBadge.vue";
import Icon from "../Icon/Icon.vue";
import type { AlertTone } from "./alert.ts";

const props = withDefaults(
  defineProps<{
    tone?: AlertTone;
    title: string;
    /** Violations: a readable message and the code of each. */
    items?: { message: string; code?: string }[];
    /** An icon that says more than the tone's (e.g. `lock`). */
    icon?: IconName | undefined;
  }>(),
  { tone: "info", items: () => [], icon: undefined },
);

const ICONS: Record<AlertTone, IconName> = {
  info: "info",
  ok: "circle-check",
  warn: "triangle-alert",
  ko: "circle-x",
};
</script>

<template>
  <div
    class="k-inline-alert"
    :data-tone="props.tone"
    :role="props.tone === 'ko' ? 'alert' : 'status'"
  >
    <Icon :name="props.icon ?? ICONS[props.tone]" class="k-inline-alert__icon" />
    <div class="k-inline-alert__body">
      <p class="k-inline-alert__title">{{ props.title }}</p>
      <div v-if="$slots.default" class="k-inline-alert__text"><slot /></div>
      <ul v-if="props.items.length" class="k-inline-alert__items">
        <li v-for="(item, i) in props.items" :key="i" class="k-inline-alert__item">
          <span>{{ item.message }}</span>
          <CodeBadge v-if="item.code" :code="item.code" />
        </li>
      </ul>
    </div>
  </div>
</template>

<style>
@layer components {
  .k-inline-alert {
    --k-inline-alert-fg: var(--k-state-info-fg);
    --k-inline-alert-bg: var(--k-state-info-bg);
    --k-inline-alert-border: var(--k-state-info-border);

    display: flex;
    gap: var(--k-space-2);
    align-items: start;
    padding: var(--k-space-2) var(--k-space-3);
    color: var(--k-text);
    background-color: var(--k-inline-alert-bg);
    border: var(--k-border-width) solid var(--k-inline-alert-border);
    border-radius: var(--k-radius-md);
  }

  .k-inline-alert[data-tone="ok"] {
    --k-inline-alert-fg: var(--k-state-ok-fg);
    --k-inline-alert-bg: var(--k-state-ok-bg);
    --k-inline-alert-border: var(--k-state-ok-border);
  }

  .k-inline-alert[data-tone="warn"] {
    --k-inline-alert-fg: var(--k-state-warn-fg);
    --k-inline-alert-bg: var(--k-state-warn-bg);
    --k-inline-alert-border: var(--k-state-warn-border);
  }

  .k-inline-alert[data-tone="ko"] {
    --k-inline-alert-fg: var(--k-state-ko-fg);
    --k-inline-alert-bg: var(--k-state-ko-bg);
    --k-inline-alert-border: var(--k-state-ko-border);
  }

  .k-inline-alert__icon {
    margin-block-start: var(--k-space-0-5);
    color: var(--k-inline-alert-fg);
  }

  .k-inline-alert__body {
    display: grid;
    flex: 1;
    gap: var(--k-space-1);
    min-inline-size: 0;
  }

  .k-inline-alert__title {
    margin: 0;
    font-weight: var(--k-weight-semibold);
    color: var(--k-inline-alert-fg);
  }

  .k-inline-alert__items {
    display: grid;
    gap: var(--k-space-1);
    padding: 0;
    margin: 0;
    list-style: none;
  }

  .k-inline-alert__item {
    display: flex;
    flex-wrap: wrap;
    gap: var(--k-space-2);
    align-items: center;
  }
}
</style>
