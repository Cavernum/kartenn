<script setup lang="ts">
// A panel of the work surface (spec 021, « Cadre » : « panneaux --k-surface »): a titled
// section, its actions at the end of the header, its body. The page title is the hidden h1
// of the frame; every panel title is an h2, and only the main panel is a named region (the
// list of landmarks stays short). Not in the inventory of the 020:
// written for the mock-ups and to be decided with them (task 15) — a `Panel` of
// @kartenn/ui or a part of the frame of the 021.
import { useId } from "vue";

const props = withDefaults(
  defineProps<{
    title: string;
    /** A detail after the title: a count, a key in fixed pitch. */
    detail?: string | undefined;
    mono?: boolean;
    /** The main panel of the screen: larger title, named region. */
    main?: boolean;
    /** A body without padding (grids, trees, lists that draw their own lines). */
    flush?: boolean;
  }>(),
  { detail: undefined },
);
const id = useId();
</script>

<template>
  <section
    class="k-ecran-panel"
    :aria-labelledby="props.main ? id : undefined"
    :data-main="props.main ? 'true' : undefined"
  >
    <header class="k-ecran-panel__header">
      <h2 :id="id" class="k-ecran-panel__title">
        {{ props.title }}
        <span v-if="props.detail" class="k-ecran-panel__detail" :class="{ 'k-mono': props.mono }">{{
          props.detail
        }}</span>
      </h2>
      <div v-if="$slots.actions" class="k-ecran-panel__actions"><slot name="actions" /></div>
    </header>
    <div v-if="$slots.toolbar" class="k-ecran-panel__toolbar"><slot name="toolbar" /></div>
    <div class="k-ecran-panel__body" :data-flush="props.flush ? 'true' : undefined">
      <slot />
    </div>
    <footer v-if="$slots.footer" class="k-ecran-panel__footer"><slot name="footer" /></footer>
  </section>
</template>

<style>
@layer components {
  .k-ecran-panel {
    display: grid;
    grid-template-rows: auto auto minmax(0, 1fr) auto;
    min-inline-size: 0;
    background-color: var(--k-surface);
    border: var(--k-border-width) solid var(--k-border-strong);
    border-radius: var(--k-radius-md);
  }

  .k-ecran-panel__header {
    display: flex;
    gap: var(--k-space-2);
    align-items: center;
    justify-content: space-between;
    min-block-size: var(--k-size-status-bar);
    padding-inline: var(--k-space-3) var(--k-space-2);
    border-block-end: var(--k-border-width) solid var(--k-border);
  }

  .k-ecran-panel__title {
    display: flex;
    gap: var(--k-space-2);
    flex: none;
    align-items: baseline;
    max-inline-size: 60%;
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
    font-size: var(--k-text-md);
    line-height: var(--k-text-md-line);
    font-weight: var(--k-weight-semibold);
  }

  .k-ecran-panel[data-main] .k-ecran-panel__title {
    font-size: var(--k-text-lg);
    line-height: var(--k-text-lg-line);
  }

  .k-ecran-panel__detail {
    min-inline-size: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    color: var(--k-text-muted);
    font-weight: var(--k-weight-regular);
    font-variant-numeric: tabular-nums;
  }

  /* The actions take the rest of the header, on one line: a toolbar in there folds into
   * « … » instead of pushing the panel wider (024/M15). */
  .k-ecran-panel__actions {
    display: flex;
    flex: 1 1 0;
    flex-wrap: nowrap;
    gap: var(--k-space-2);
    align-items: center;
    justify-content: end;
    min-inline-size: 0;
  }

  .k-ecran-panel__actions > .k-toolbar {
    flex: 1 1 0;
  }

  .k-ecran-panel__toolbar {
    padding: var(--k-space-2) var(--k-space-3);
    border-block-end: var(--k-border-width) solid var(--k-border);
  }

  .k-ecran-panel__body {
    display: grid;
    gap: var(--k-space-3);
    align-content: start;
    min-inline-size: 0;
    padding: var(--k-space-3);
  }

  .k-ecran-panel__body[data-flush] {
    padding: 0;
  }

  /* A grid or a tree in a flush panel: the panel draws the outer border. */
  .k-ecran-panel__body[data-flush] > .k-data-grid,
  .k-ecran-panel__body[data-flush] > .k-tree-grid {
    border: 0;
    border-radius: 0;
  }

  .k-ecran-panel__footer {
    display: flex;
    flex-wrap: wrap;
    gap: var(--k-space-2);
    align-items: center;
    justify-content: space-between;
    padding: var(--k-space-2) var(--k-space-3);
    border-block-start: var(--k-border-width) solid var(--k-border);
  }
}
</style>
