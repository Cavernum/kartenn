<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

<script setup lang="ts">
// A panel of the work surface (spec 021, « Cadre » : « panneaux --k-surface »): a titled
// section, its actions at the end of the header, an optional toolbar under it, its body, an
// optional footer. The page title is the h1 of the view; every panel title is an h2, and
// only the main panel is a named region (the list of landmarks stays short). The panel of
// the shell's screens (`ViewPanel`, tasks 021-11 and 22f-h), made public for the plugins
// (021-22f-i1), so that a plugin's list screen fills the window as the shell's do.
import { useId } from "vue";

const props = withDefaults(
  defineProps<{
    title: string;
    /** A detail after the title: a count, a key in fixed pitch. */
    detail?: string | undefined;
    mono?: boolean;
    /** The main panel of the screen, one per screen: larger title, named region. */
    main?: boolean;
    /** A body without padding (grids, trees, lists that draw their own lines). */
    flush?: boolean;
    /**
     * The main panel of a list screen, its one `DataGrid` a child of its body (never
     * wrapped): once virtualised, the grid takes the rest of the panel and is its one
     * scroller. The height of the panel is its host's: `data-list` on the section and on
     * the body is what the host reads to give it (021-22f-h).
     */
    list?: boolean;
  }>(),
  { detail: undefined },
);
const id = useId();
</script>

<template>
  <section
    class="k-panel"
    :aria-labelledby="props.main ? id : undefined"
    :data-main="props.main ? 'true' : undefined"
    :data-list="props.list ? 'true' : undefined"
  >
    <header class="k-panel__header">
      <!-- The detail beside the heading, not in it: the heading (and the region it names)
           says the title alone, never « Flux plugin Flux ». -->
      <div class="k-panel__title">
        <h2 :id="id" class="k-panel__heading">{{ props.title }}</h2>
        <span v-if="props.detail" class="k-panel__detail" :class="{ 'k-mono': props.mono }">{{
          props.detail
        }}</span>
      </div>
      <div v-if="$slots.actions" class="k-panel__actions"><slot name="actions" /></div>
    </header>
    <div v-if="$slots.toolbar" class="k-panel__toolbar"><slot name="toolbar" /></div>
    <div
      class="k-panel__body"
      :data-flush="props.flush ? 'true' : undefined"
      :data-list="props.list ? 'true' : undefined"
    >
      <slot />
    </div>
    <footer v-if="$slots.footer" class="k-panel__footer"><slot name="footer" /></footer>
  </section>
</template>

<style>
@layer components {
  .k-panel {
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
  .k-panel__header {
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
  .k-panel__title {
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

  .k-panel[data-main] .k-panel__title {
    font-size: var(--k-text-lg);
    line-height: var(--k-text-lg-line);
  }

  .k-panel__heading {
    font: inherit;
  }

  .k-panel__detail {
    min-inline-size: 0;
    color: var(--k-text-muted);
    font-weight: var(--k-weight-regular);
    font-variant-numeric: tabular-nums;
  }

  /* Buttons alone keep their width. A toolbar takes the rest of the header, on one line,
   * and folds into « … » instead of pushing the panel wider (024/M15): its width does not
   * count, its floor does (`--k-toolbar-floor`, its main action and « … »), so that it
   * goes under the title only when even folded it no longer fits beside it. */
  .k-panel__actions {
    display: flex;
    flex: none;
    flex-wrap: nowrap;
    gap: var(--k-space-2);
    align-items: center;
    justify-content: end;
    max-inline-size: 100%;
    margin-inline-start: auto;
  }

  .k-panel__actions:has(> .k-toolbar) {
    flex: 1 1 0;
  }

  .k-panel__actions > .k-toolbar {
    flex: 1 1 0;
    min-inline-size: var(--k-toolbar-floor);
    contain: inline-size;
  }

  .k-panel__toolbar {
    padding: var(--k-space-2) var(--k-space-3);
    border-block-end: var(--k-border-width) solid var(--k-border);
  }

  .k-panel__body {
    display: grid;
    gap: var(--k-space-3);
    align-content: start;
    min-inline-size: 0;
    padding: var(--k-space-3);
  }

  .k-panel__body[data-flush] {
    padding: 0;
  }

  /* The panel of a list screen (`list`, its grid a child of its body) whose grid is
   * virtualised (a list of more than one page, or of more than 200 rows): its grid takes
   * the rest of the panel, a list's height no longer, and is the one scroller of the
   * screen, no box in a page that scrolls too. The alerts above it keep their height. Not
   * below the height of a header and six rows (a short or zoomed window, WCAG 1.4.10): the
   * host scrolls then. A list not virtualised keeps its own height. The panel fills the
   * height its host gives it (the shell's surface, when `[data-list]` holds a virtualised
   * grid), never less than its content (021-22f-h).
   * Alone (a tab, a drawer, any host of automatic height), the grid starts from a list's
   * height (`--k-size-list-max`), as a grid outside a panel does, and grows or shrinks to the
   * height a host gives. A host that fills (a one-row `minmax(auto, 1fr)` grid, whose
   * minimum is the panel's content) sets `--k-panel-list-basis` to `var(--k-space-0)`: its
   * content then counts the grid at its floor, a header and six rows, never at a list's
   * height, and a short window still shows the panel whole (021-22f-i1). */
  /* Each part of a list panel in its own row, present or not: the body always in the
   * elastic one, a toolbar or a footer missing (the other panels, never stretched by a
   * host, keep their placement). */
  .k-panel[data-list] {
    grid-template-areas: "header" "toolbar" "body" "footer";
  }

  .k-panel[data-list] > .k-panel__header {
    grid-area: header;
  }

  .k-panel[data-list] > .k-panel__toolbar {
    grid-area: toolbar;
  }

  .k-panel__body[data-list] {
    grid-area: body;
  }

  .k-panel[data-list] > .k-panel__footer {
    grid-area: footer;
  }

  .k-panel__body[data-list]:has(> .k-data-grid-frame > [data-virtual]) {
    display: flex;
    flex-direction: column;
  }

  .k-panel__body[data-list] > * {
    flex: none;
  }

  .k-panel__body[data-list] > .k-data-grid-frame:has(> [data-virtual]) {
    flex: 1 1 var(--k-panel-list-basis, var(--k-size-list-max));
    min-block-size: calc(var(--k-header-height) + 6 * var(--k-row-height));
  }

  .k-panel__body[data-list] > .k-data-grid-frame > [data-virtual] {
    max-block-size: none;
  }

  /* A grid or a tree in a flush panel: the panel draws the outer border. */
  .k-panel__body[data-flush] > .k-data-grid-frame > .k-data-grid,
  .k-panel__body[data-flush] > .k-tree-grid-frame > .k-tree-grid {
    border: 0;
    border-radius: 0;
  }

  .k-panel__footer {
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
