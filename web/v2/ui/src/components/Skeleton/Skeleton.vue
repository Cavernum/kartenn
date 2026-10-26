<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

<script setup lang="ts">
// The loading state of a data view (design « Inventaire », line 25; REQ-020-23): bars of
// the size of what comes (rows of `--k-row-height`, lines of text), so that the data take
// the same place when they arrive (no layout shift). A hidden word in a status region says
// it (no `aria-busy` around it: a busy region holds back its announcements).
// Every bar and block takes the shared fill `k-skeleton-fill` (shared.css): a sheen that
// runs as long as the place loads, still under reduced motion (the system's or
// `data-motion="reduce"`), its fill alone telling the place. `cell`: one bar alone, for a
// cell of a grid that says its loading itself (`DataGrid`). The live region is there,
// empty, before the word comes: one inserted with its text is not announced (relecture du
// 2026-10-26).
import { onBeforeUnmount, onMounted, ref } from "vue";
import { t } from "../../i18n.ts";

const props = withDefaults(
  defineProps<{ variant?: "rows" | "text" | "block" | "cell"; rows?: number; columns?: number }>(),
  { variant: "rows", rows: 5, columns: 4 },
);

const said = ref(false);
let frame = 0;
onMounted(() => {
  frame = requestAnimationFrame(() => (said.value = true));
});
onBeforeUnmount(() => cancelAnimationFrame(frame));
</script>

<template>
  <span
    v-if="props.variant === 'cell'"
    class="k-skeleton__bar k-skeleton-fill"
    :data-width="(props.rows + props.columns) % 3"
    aria-hidden="true"
  ></span>
  <div v-else class="k-skeleton" :data-variant="props.variant">
    <span class="k-visually-hidden" role="status">{{ said ? t("loading") : "" }}</span>
    <template v-if="props.variant === 'rows'">
      <div v-for="row in props.rows" :key="row" class="k-skeleton__row" aria-hidden="true">
        <span
          v-for="column in props.columns"
          :key="column"
          class="k-skeleton__bar k-skeleton-fill"
          :data-width="(row + column) % 3"
        ></span>
      </div>
    </template>
    <template v-else-if="props.variant === 'text'">
      <span
        v-for="row in props.rows"
        :key="row"
        class="k-skeleton__bar k-skeleton__line k-skeleton-fill"
        :data-width="row === props.rows ? 2 : 0"
        aria-hidden="true"
      ></span>
    </template>
    <span v-else class="k-skeleton__block k-skeleton-fill" aria-hidden="true"></span>
  </div>
</template>

<style>
@layer components {
  /* As wide as its container, whatever the layout around it (a flex column aligned at
   * the start would shrink it to nothing). */
  .k-skeleton {
    display: grid;
    inline-size: 100%;
    min-inline-size: 0;
  }

  .k-skeleton[data-variant="text"] {
    gap: var(--k-space-2);
  }

  .k-skeleton__row {
    display: flex;
    gap: var(--k-space-4);
    align-items: center;
    block-size: var(--k-row-height);
    padding-inline: var(--k-cell-padding-inline);
    border-block-end: var(--k-border-width) solid var(--k-border);
  }

  .k-skeleton__row > .k-skeleton__bar {
    flex: 1;
  }

  .k-skeleton__bar {
    display: block;
    inline-size: 100%;
    block-size: var(--k-space-3);
    border-radius: var(--k-radius-sm);
  }

  .k-skeleton__bar[data-width="1"] {
    inline-size: 75%;
  }

  .k-skeleton__bar[data-width="2"] {
    inline-size: 50%;
  }

  .k-skeleton__line {
    block-size: var(--k-text-md-line);
  }

  .k-skeleton__block {
    display: block;
    min-block-size: var(--k-size-panel-sm);
    border-radius: var(--k-radius-md);
  }
}
</style>
