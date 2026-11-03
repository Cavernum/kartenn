<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

<script setup lang="ts">
// A list of properties (term, value) in two columns (spec 021/A27): the panels of a record
// in the shell, the records of a plugin written in Vue without a sheet of its own (A23).
// Taken from the shell (task 021-11, its mock-ups `EcranAttributs`). A value may be
// replaced by a slot named after its term key; a value absent (`undefined`, `null`, `""`,
// or a slot that renders nothing) is said, never blank (`NoValue`): a dash shown,
// « non renseigné » for the screen readers. One grid: the columns are set once on the
// list, every item is a subgrid of it (design « Adaptation à la largeur »); the columns stay in fractions, so
// that blocks of the same width, stacked, align their values. `tile`: blocks side by side
// in a grid (`Grid columns="auto"`) share its rows (row subgrid), the n-th property of
// each block on the same line. With `columns` (a wide panel, the bottom panel of E1), the
// pairs of tracks repeat as many times as the width holds: properties in several columns,
// each still a subgrid, terms and values of a column aligned.
import { Comment, Fragment, Text, computed, useSlots, type VNode } from "vue";
import NoValue from "../NoValue/NoValue.vue";
import type { AttributeItem } from "./attributes.ts";

const props = defineProps<{ items: AttributeItem[]; columns?: boolean; tile?: boolean }>();
const slots = useSlots();
/** The rows a block takes in a grid of tiles (CSSOM, REQ-020-14). */
const rows = computed(() => ({ "--k-attribute-list-rows": String(props.items.length) }));
/** Nodes that show nothing: comments, blank text, fragments of them. */
const blank = (nodes: VNode[]): boolean =>
  nodes.every((node) =>
    node.type === Comment
      ? true
      : node.type === Text
        ? !String(node.children ?? "").trim()
        : node.type === Fragment
          ? blank(Array.isArray(node.children) ? (node.children as VNode[]) : [])
          : false,
  );
const empty = (item: AttributeItem) => {
  const slot = slots[item.key];
  if (slot) return blank(slot());
  return item.value === undefined || item.value === null || item.value === "";
};
</script>

<template>
  <dl
    class="k-attribute-list"
    :style="rows"
    :data-columns="props.columns ? 'true' : undefined"
    :data-tile="props.tile ? 'true' : undefined"
  >
    <div v-for="item in props.items" :key="item.key" class="k-attribute-list__item">
      <dt class="k-attribute-list__term">{{ item.label }}</dt>
      <dd v-if="empty(item)" class="k-attribute-list__value" data-empty="true"><NoValue /></dd>
      <dd v-else class="k-attribute-list__value" :class="{ 'k-mono': item.mono }">
        <slot :name="item.key">{{ item.value }}</slot>
      </dd>
    </div>
  </dl>
</template>

<style>
@layer components {
  .k-attribute-list {
    display: grid;
    grid-template-columns: minmax(0, 2fr) minmax(0, 3fr);
    gap: var(--k-space-1) var(--k-space-2);
    align-content: start;
    max-inline-size: var(--k-measure-prose);
  }

  .k-attribute-list__item {
    display: grid;
    grid-column: 1 / -1;
    grid-template-columns: subgrid;
    align-items: baseline;
    min-block-size: var(--k-row-height);
  }

  /* In columns: a pair of tracks (term, value) per column, as many as fit, never narrower
   * than a column of grid each; every property takes one pair. */
  .k-attribute-list[data-columns] {
    grid-template-columns: repeat(
      auto-fill,
      minmax(var(--k-size-col-sm), 2fr) minmax(var(--k-size-col-md), 3fr)
    );
    column-gap: var(--k-space-3);
    max-inline-size: none;
  }

  .k-attribute-list[data-columns] > .k-attribute-list__item {
    grid-column: span 2;
  }

  /* A tile of a grid: the rows of the tiles side by side are shared. */
  .k-attribute-list[data-tile] {
    grid-row: span var(--k-attribute-list-rows);
    grid-template-rows: subgrid;
    row-gap: var(--k-space-1);
  }

  .k-attribute-list__term {
    color: var(--k-text-muted);
  }

  .k-attribute-list__value {
    min-inline-size: 0;
    overflow-wrap: anywhere;
  }
}
</style>
