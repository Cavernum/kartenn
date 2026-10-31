<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

<script setup lang="ts">
// A breadcrumb (APG Breadcrumb; design « Inventaire », line 17): a `navigation` landmark
// with its name, an ordered list of links, the last one the current page
// (`aria-current="page"`); separators are decorative. A crumb may be a key in fixed pitch
// (an identifier, a prefix). Short of room, a crumb is cut by an ellipsis: its whole
// label comes in a tooltip under the pointer and the keyboard focus (decorative: the link
// keeps its whole name), never a text cut with no way to read it.
import { t } from "../../i18n.ts";
import Icon from "../Icon/Icon.vue";
import LinkTo from "../internal/LinkTo.vue";
import Tooltip from "../Tooltip/Tooltip.vue";
import { useTooltip } from "../Tooltip/tooltip.ts";
import type { Crumb } from "./breadcrumb.ts";

const props = defineProps<{ items: Crumb[]; label?: string }>();

const tip = useTooltip();
/** Whether the crumb the pointer or the focus came to is cut, measured as it comes. */
let cut = false;
const shared = tip.listeners(() => cut, { pin: () => false, passEscape: () => true });
const listeners = Object.fromEntries(
  Object.entries(shared).map(([name, listener]) => [
    name,
    (event: Event) => {
      const el = event.currentTarget as HTMLElement;
      cut = el.scrollWidth > el.clientWidth;
      listener(event);
    },
  ]),
);
</script>

<template>
  <nav class="k-breadcrumb" :aria-label="props.label ?? t('breadcrumb')">
    <ol class="k-breadcrumb__list">
      <li v-for="(item, i) in props.items" :key="i" class="k-breadcrumb__item">
        <Icon v-if="i > 0" name="chevron-right" class="k-breadcrumb__separator" />
        <span
          v-if="i === props.items.length - 1"
          class="k-breadcrumb__current"
          :class="{ 'k-mono': item.mono }"
          aria-current="page"
          >{{ item.label }}</span
        >
        <LinkTo
          v-else-if="item.to"
          :to="item.to"
          class="k-breadcrumb__link"
          :class="{ 'k-mono': item.mono }"
          v-on="listeners"
          >{{ item.label }}</LinkTo
        >
        <span v-else class="k-breadcrumb__text" :class="{ 'k-mono': item.mono }">{{
          item.label
        }}</span>
      </li>
    </ol>
    <Tooltip v-if="tip.open.value" :control="tip" decorative>{{
      tip.anchor.value?.textContent
    }}</Tooltip>
  </nav>
</template>

<style>
@layer components {
  .k-breadcrumb {
    min-inline-size: 0;
  }

  /* In the compact frame (its `data-width`, design « Adaptation à la largeur »), the
   * current page alone: the path is the drawer's navigation. */
  [data-width="compact"] .k-breadcrumb__item:not(:last-child),
  [data-width="compact"] .k-breadcrumb__separator {
    display: none;
  }

  .k-breadcrumb__list {
    display: flex;
    align-items: center;
    min-inline-size: 0;
    padding: 0;
    margin: 0;
    list-style: none;
  }

  .k-breadcrumb__item {
    display: flex;
    gap: var(--k-space-1);
    align-items: center;
    min-inline-size: 0;
    padding-inline-end: var(--k-space-1);
  }

  /* Short of room, the current page wins: it keeps its whole name (up to the width of the
   * path) and the earlier crumbs give way first (relecture du 2026-10-26). */
  .k-breadcrumb__item:last-child {
    flex-shrink: 0;
    max-inline-size: 100%;
  }

  .k-breadcrumb__separator {
    flex: none;
    color: var(--k-text-muted);
  }

  .k-breadcrumb__link,
  .k-breadcrumb__text {
    overflow: hidden;
    color: var(--k-text-muted);
    text-overflow: ellipsis;
    white-space: nowrap;
    text-decoration: none;
    border-radius: var(--k-radius-sm);
  }

  .k-breadcrumb__link:hover {
    color: var(--k-text);
    text-decoration: underline;
  }

  .k-breadcrumb__current {
    overflow: hidden;
    font-weight: var(--k-weight-medium);
    color: var(--k-text);
    text-overflow: ellipsis;
    white-space: nowrap;
  }
}
</style>
