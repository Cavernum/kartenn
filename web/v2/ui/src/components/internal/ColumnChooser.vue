<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

<script setup lang="ts">
// The column chooser of a grid (design « Adaptation à la largeur »): a narrow grid hides
// its columns of priority 2 and 3 by a container query, never without access. A menu of
// checkboxes, « Colonnes », lists the columns that may hide, checked when they show (read
// on the header cells when the menu opens); a choice holds whatever the width. The row of
// the chooser shows only where something is hidden: under `md` (`lg` when a column has
// priority 3), or when the user hid a column.
import { computed, ref } from "vue";
import { t } from "../../i18n.ts";
import {
  chooserItems,
  maxPriority,
  toggleColumn,
  type ColumnVisibility,
  type GridColumn,
} from "../DataGrid/grid.ts";
import Menu from "../Menu/Menu.vue";

const props = defineProps<{
  columns: GridColumn[];
  /** The grid whose header cells (`[role=columnheader][data-column]`) say what shows. */
  grid: HTMLElement | null;
}>();
const visibility = defineModel<ColumnVisibility>({ default: () => ({}) });

/** Whether the header of column `id` takes room on screen now. */
function shownNow(id: string): boolean {
  const header = props.grid?.querySelector(
    `[role="columnheader"][data-column="${CSS.escape(id)}"]`,
  );
  return !!header && header.getClientRects().length > 0;
}

/** The columns that show, read when the menu opens (a container query decided). */
const shown = ref(new Set<string>());
function onOpen(open: boolean): void {
  if (open) shown.value = new Set(props.columns.map((c) => c.id).filter(shownNow));
}
const items = computed(() =>
  chooserItems(props.columns, visibility.value, (id) => shown.value.has(id)),
);
function onSelect(id: string): void {
  const now = shown.value.has(id);
  visibility.value = toggleColumn(visibility.value, id, now);
  const next = new Set(shown.value);
  if (visibility.value[id]) next.add(id);
  else next.delete(id);
  shown.value = next;
}

const max = computed(() => maxPriority(props.columns));
const chosen = computed(() => Object.values(visibility.value).some((v) => v === false));
</script>

<template>
  <div
    v-if="max > 1"
    class="k-column-chooser"
    :data-max="max"
    :data-chosen="chosen ? 'true' : undefined"
  >
    <Menu
      :label="t('columns')"
      icon="columns-3"
      variant="ghost"
      size="sm"
      align="end"
      :items="items"
      @open="onOpen"
      @select="onSelect"
    />
  </div>
</template>

<style>
@layer components {
  .k-column-chooser {
    display: flex;
    justify-content: end;
  }

  /* Nothing hidden at this width, nothing chosen: no chooser (the queries of the grid). */
  @container (width >= 640px) {
    .k-column-chooser[data-max="2"]:not([data-chosen]) {
      display: none;
    }
  }

  @container (width >= 960px) {
    .k-column-chooser[data-max="3"]:not([data-chosen]) {
      display: none;
    }
  }
}
</style>
