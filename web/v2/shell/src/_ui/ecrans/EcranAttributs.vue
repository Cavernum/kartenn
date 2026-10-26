<script setup lang="ts">
// A list of attributes (term, value) in two columns, for the side panels of a record. Not
// in the inventory of the 020: written for the mock-ups and to be decided with them (task
// 15). A value may be replaced by a slot named after its term key. One grid: the columns
// are set once on the list, every item is a subgrid of it (design « Adaptation à la
// largeur »); the columns stay in fractions, so that blocks of the same width, stacked,
// align their values. Blocks side by side in a `.k-ecran-attrs-tiles` grid share its rows
// (row subgrid): the n-th attribute of each block sits on the same line.
import { computed } from "vue";

const props = defineProps<{
  items: { key: string; label: string; value?: string; mono?: boolean }[];
}>();
/** The rows a block takes in a grid of tiles (CSSOM, REQ-020-14). */
const rows = computed(() => ({ "--k-ecran-attrs-rows": String(props.items.length) }));
</script>

<template>
  <dl class="k-ecran-attrs" :style="rows">
    <div v-for="item in props.items" :key="item.key" class="k-ecran-attrs__item">
      <dt class="k-ecran-attrs__term">{{ item.label }}</dt>
      <dd class="k-ecran-attrs__value" :class="{ 'k-mono': item.mono }">
        <slot :name="item.key">{{ item.value }}</slot>
      </dd>
    </div>
  </dl>
</template>

<style>
@layer components {
  .k-ecran-attrs {
    display: grid;
    grid-template-columns: minmax(0, 2fr) minmax(0, 3fr);
    gap: var(--k-space-1) var(--k-space-2);
    align-content: start;
    max-inline-size: var(--k-measure-prose);
  }

  .k-ecran-attrs__item {
    display: grid;
    grid-column: 1 / -1;
    grid-template-columns: subgrid;
    align-items: baseline;
    min-block-size: var(--k-row-height);
  }

  /* Side by side in a grid of tiles: the rows of the tiles are shared. */
  .k-ecran-attrs-tiles > .k-ecran-attrs {
    grid-row: span var(--k-ecran-attrs-rows);
    grid-template-rows: subgrid;
    row-gap: var(--k-space-1);
  }

  .k-ecran-attrs__term {
    color: var(--k-text-muted);
  }

  .k-ecran-attrs__value {
    min-inline-size: 0;
  }
}
</style>
