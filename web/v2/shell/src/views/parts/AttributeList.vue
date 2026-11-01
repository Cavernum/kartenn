<script setup lang="ts">
// A list of attributes (term, value) in two columns, for the panels of a record. A part of
// the shell (task 11), taken from the mock-ups (`EcranAttributs`, which now renders it):
// not a component of @kartenn/ui. A value may be replaced by a slot named after its term
// key. One grid: the columns are set once on the list, every item is a subgrid of it
// (design « Adaptation à la largeur »); the columns stay in fractions, so that blocks of
// the same width, stacked, align their values. Blocks side by side in a `.k-attrs-tiles`
// grid share its rows (row subgrid): the n-th attribute of each block sits on the same
// line. With `columns` (a wide panel, the bottom panel of E1), the pairs of tracks repeat
// as many times as the width holds: attributes in several columns, each still a subgrid,
// terms and values of a column aligned.
import { computed } from "vue";

const props = defineProps<{
  items: { key: string; label: string; value?: string; mono?: boolean }[];
  columns?: boolean;
}>();
/** The rows a block takes in a grid of tiles (CSSOM, REQ-020-14). */
const rows = computed(() => ({ "--k-attrs-rows": String(props.items.length) }));
</script>

<template>
  <dl class="k-attrs" :style="rows" :data-columns="props.columns ? 'true' : undefined">
    <div v-for="item in props.items" :key="item.key" class="k-attrs__item">
      <dt class="k-attrs__term">{{ item.label }}</dt>
      <dd class="k-attrs__value" :class="{ 'k-mono': item.mono }">
        <slot :name="item.key">{{ item.value }}</slot>
      </dd>
    </div>
  </dl>
</template>

<style>
@layer components {
  .k-attrs {
    display: grid;
    grid-template-columns: minmax(0, 2fr) minmax(0, 3fr);
    gap: var(--k-space-1) var(--k-space-2);
    align-content: start;
    max-inline-size: var(--k-measure-prose);
  }

  .k-attrs__item {
    display: grid;
    grid-column: 1 / -1;
    grid-template-columns: subgrid;
    align-items: baseline;
    min-block-size: var(--k-row-height);
  }

  /* In columns: a pair of tracks (term, value) per column, as many as fit, never narrower
   * than a column of grid each; every attribute takes one pair. */
  .k-attrs[data-columns] {
    grid-template-columns: repeat(
      auto-fill,
      minmax(var(--k-size-col-sm), 2fr) minmax(var(--k-size-col-md), 3fr)
    );
    column-gap: var(--k-space-3);
    max-inline-size: none;
  }

  .k-attrs[data-columns] > .k-attrs__item {
    grid-column: span 2;
  }

  /* Side by side in a grid of tiles: the rows of the tiles are shared. */
  .k-attrs-tiles > .k-attrs {
    grid-row: span var(--k-attrs-rows);
    grid-template-rows: subgrid;
    row-gap: var(--k-space-1);
  }

  .k-attrs__term {
    color: var(--k-text-muted);
  }

  .k-attrs__value {
    min-inline-size: 0;
  }
}
</style>
