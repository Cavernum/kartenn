<script setup lang="ts">
// The lists of the right panel of the administration, one entry per line in aligned columns
// (024-11; design 021 « Administration »; maquette E7): the entry on the left (label then
// key, wrapping before the key, never inside it), its value on the right (kind, scope,
// served name, with the end of a temporary membership or binding on a second line), then
// its actions in a last column at a fixed place. A row waiting in the draft has the draft
// look of the grids and its sign; a removal stays in place, struck. A part of the shell
// taken from the mock-up, not a component of @kartenn/ui: its items are `li` of the views.
const props = withDefaults(
  defineProps<{
    label: string;
    /** Read only: the entry and its value, no column of actions. */
    read?: boolean;
  }>(),
  { read: false },
);
</script>

<template>
  <ul class="k-admin-lines" :aria-label="props.label" :data-read="props.read ? 'true' : undefined">
    <slot />
  </ul>
</template>

<style>
@layer components {
  .k-admin-lines {
    display: grid;
    /* The value shrinks before the actions leave a narrow panel: a long code is cut, read
     * whole in its bubble (`CodeBadge`). */
    grid-template-columns: minmax(0, 1fr) minmax(0, max-content) max-content;
    gap: var(--k-space-1) var(--k-space-3);
    align-items: center;
    padding: 0;
    margin: 0;
    list-style: none;
  }

  .k-admin-lines[data-read="true"] {
    grid-template-columns: minmax(0, 1fr) minmax(0, auto);
  }

  .k-admin-lines > li {
    display: grid;
    grid-column: 1 / -1;
    grid-template-columns: subgrid;
    align-items: center;
    min-block-size: var(--k-row-height);
    padding-inline: var(--k-space-2);
    border-radius: var(--k-radius-sm);
  }

  .k-admin-lines > li > * {
    min-inline-size: 0;
  }

  /* A key or a part of a name never breaks at its hyphens; the line breaks before it. */
  .k-admin-lines code,
  .k-admin-lines__nowrap {
    white-space: nowrap;
  }

  /* The key of a role under its label, on every row: one height for all. */
  .k-admin-lines__key {
    display: block;
  }

  .k-admin-lines__text {
    display: inline-flex;
    gap: var(--k-space-1);
    align-items: center;
    min-inline-size: 0;
  }

  .k-admin-lines__mark {
    flex: none;
  }

  /* A value and its detail (an end date, the granting group) on a second line. */
  .k-admin-lines__value {
    display: grid;
    color: var(--k-text-muted);
  }

  .k-admin-lines__actions {
    display: inline-flex;
    gap: var(--k-space-1);
    align-items: center;
    justify-self: end;
  }

  .k-admin-lines > li[data-pending] {
    background-color: var(--k-accent-soft);
    box-shadow: var(--k-shadow-inset-accent);
  }

  .k-admin-lines > li[data-pending="removed"] .k-admin-lines__label,
  .k-admin-lines > li[data-pending="removed"] .k-admin-lines__value {
    color: var(--k-text-muted);
    text-decoration: line-through;
  }

  .k-admin-lines > li[data-pending="removed"] .k-admin-lines__mark {
    color: var(--k-diff-removed);
  }

  .k-admin-lines > li[data-pending="added"] .k-admin-lines__mark {
    color: var(--k-diff-added);
  }

  /* The titles of the right panel of the administration (its object, its sections), and
   * the words that go muted beside them. */
  .k-admin-heading {
    margin: 0;
    font-size: var(--k-text-md);
    line-height: var(--k-text-md-line);
    font-weight: var(--k-weight-semibold);
  }

  .k-admin-muted {
    font-weight: var(--k-weight-regular);
    color: var(--k-text-muted);
  }
}
</style>
