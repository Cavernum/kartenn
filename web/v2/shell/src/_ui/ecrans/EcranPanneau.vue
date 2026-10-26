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
      <!-- The detail beside the heading, not in it: the heading (and the region it names)
           says the title alone, never « VLAN plugin VLAN ». -->
      <div class="k-ecran-panel__title">
        <h2 :id="id" class="k-ecran-panel__heading">{{ props.title }}</h2>
        <span v-if="props.detail" class="k-ecran-panel__detail" :class="{ 'k-mono': props.mono }">{{
          props.detail
        }}</span>
      </div>
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
    /* One elastic column: a long header or grid never widens the panel. */
    grid-template-columns: minmax(0, 1fr);
    min-inline-size: 0;
    background-color: var(--k-surface);
    border: var(--k-border-width) solid var(--k-border-strong);
    border-radius: var(--k-radius-md);
  }

  /* The title, then the actions at the end. When the title (whole, on one line) and the
   * actions no longer fit side by side, the actions go to a line of their own, at the
   * end (design « Adaptation à la largeur »): neither covers the other. */
  .k-ecran-panel__header {
    display: flex;
    flex-wrap: wrap;
    gap: var(--k-space-2);
    align-items: center;
    justify-content: space-between;
    min-block-size: var(--k-size-status-bar);
    padding-block: var(--k-space-1);
    padding-inline: var(--k-space-3) var(--k-space-2);
    border-block-end: var(--k-border-width) solid var(--k-border);
  }

  /* A title too long even for a line of its own goes to the line, its detail first,
   * never clipped (« Structure avant style », messages): a narrow tile keeps all its
   * words. */
  .k-ecran-panel__title {
    display: flex;
    flex-wrap: wrap;
    column-gap: var(--k-space-2);
    flex: 0 1 auto;
    align-items: baseline;
    max-inline-size: 100%;
    overflow-wrap: break-word;
    font-size: var(--k-text-md);
    line-height: var(--k-text-md-line);
    font-weight: var(--k-weight-semibold);
  }

  .k-ecran-panel[data-main] .k-ecran-panel__title {
    font-size: var(--k-text-lg);
    line-height: var(--k-text-lg-line);
  }

  .k-ecran-panel__heading {
    font: inherit;
  }

  .k-ecran-panel__detail {
    min-inline-size: 0;
    color: var(--k-text-muted);
    font-weight: var(--k-weight-regular);
    font-variant-numeric: tabular-nums;
  }

  /* Buttons alone keep their width. A toolbar takes the rest of the header, on one line,
   * and folds into « … » instead of pushing the panel wider (024/M15): its width does not
   * count, its floor does (`--k-toolbar-floor`, its main action and « … »), so that it
   * goes under the title only when even folded it no longer fits beside it. */
  .k-ecran-panel__actions {
    display: flex;
    flex: none;
    flex-wrap: nowrap;
    gap: var(--k-space-2);
    align-items: center;
    justify-content: end;
    max-inline-size: 100%;
    margin-inline-start: auto;
  }

  .k-ecran-panel__actions:has(> .k-toolbar) {
    flex: 1 1 0;
  }

  .k-ecran-panel__actions > .k-toolbar {
    flex: 1 1 0;
    min-inline-size: var(--k-toolbar-floor);
    contain: inline-size;
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
  .k-ecran-panel__body[data-flush] > .k-data-grid-frame > .k-data-grid,
  .k-ecran-panel__body[data-flush] > .k-tree-grid-frame > .k-tree-grid {
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

  /* The right panel over the end of the surface (wide, standard; its width published as
   * `--k-side-panel-inset`): the footer of a main panel keeps its counts out from under
   * it, wrapping in the part left in view (relecture du 2026-10-26). */
  .k-ecran-panel[data-main] > .k-ecran-panel__footer {
    /* At most half the footer (a percentage: no weight on the width of the panel). */
    padding-inline-end: max(
      var(--k-space-3),
      min(50%, calc(var(--k-side-panel-inset, var(--k-space-3)) - var(--k-space-3)))
    );
  }
}
</style>
