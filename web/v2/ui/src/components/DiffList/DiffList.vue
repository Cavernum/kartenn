<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

<script setup lang="ts">
// A differential grouped by object (Disclosure; design « Inventaire », line 29): each
// object is a disclosure of its changes, attribute by attribute, before and after; each
// change has its sign (+, −, ~, !), its word (hidden on screen) and its colour.
// Beyond `VIRTUAL_LINES` lines (headings and open changes), the list is virtualised
// (`@tanstack/vue-virtual`): it scrolls in its own place (`--k-diff-list-max`, a token),
// and only the changes near the view are in the document, the others replaced by gaps of
// their measured height, each shown change keeping its place in its object
// (`aria-setsize`, `aria-posinset`). Every object heading stays in the document: Tab and
// the navigation by headings reach every object, whatever the scroll.
import { useVirtualizer } from "@tanstack/vue-virtual";
import { computed, onMounted, ref, useId, type ComponentPublicInstance } from "vue";
import { t } from "../../i18n.ts";
import Icon from "../Icon/Icon.vue";
import { DIFF_SIGNS, type DiffChange, type DiffGroup, type DiffKind } from "./diff.ts";

const props = withDefaults(defineProps<{ groups: DiffGroup[]; collapsed?: string[] }>(), {
  collapsed: () => [],
});

/** Lines (headings and open changes) beyond which the list is virtualised (design). */
const VIRTUAL_LINES = 200;
/** Changes rendered beyond the view, on each side. */
const OVERSCAN = 8;

const id = useId();
const root = ref<HTMLElement | null>(null);
const closed = ref(new Set(props.collapsed));
function toggle(group: string): void {
  const next = new Set(closed.value);
  if (next.has(group)) next.delete(group);
  else next.add(group);
  closed.value = next;
}
const word = (kind: DiffKind) => t(kind);

/** The lines in their order: each heading, then the changes of an open object. */
const keys = computed(() =>
  props.groups.flatMap((group) => [
    `h:${group.id}`,
    ...(closed.value.has(group.id) ? [] : group.changes.map((c) => `c:${group.id}:${c.attribute}`)),
  ]),
);
const virtual = computed(() => keys.value.length > VIRTUAL_LINES);

/** The height of a line before it is measured: the row height of the density. */
const estimate = ref(28);
onMounted(() => {
  const row = root.value
    ? parseFloat(getComputedStyle(root.value).getPropertyValue("--k-row-height"))
    : NaN;
  if (Number.isFinite(row)) estimate.value = row;
});

const virtualizer = useVirtualizer<HTMLElement, HTMLElement>(
  computed(() => ({
    count: virtual.value ? keys.value.length : 0,
    getScrollElement: () => root.value,
    estimateSize: () => estimate.value,
    getItemKey: (index: number) => keys.value[index] ?? index,
    overscan: OVERSCAN,
  })),
);
function measure(el: Element | ComponentPublicInstance | null): void {
  if (virtual.value && el instanceof HTMLElement) virtualizer.value.measureElement(el);
}

type Item =
  | { key: string; change: DiffChange; index: number; position: number }
  | { key: string; gap: number };

/** Each object, its heading line and what its list shows: changes, and gaps for the rest. */
const layout = computed(() => {
  const v = virtual.value ? virtualizer.value : null;
  const shown = v ? new Set(v.getVirtualItems().map((item) => item.index)) : null;
  const sizes = v?.measurementsCache ?? [];
  let index = 0;
  return props.groups.map((group) => {
    const head = index++;
    const items: Item[] = [];
    if (!closed.value.has(group.id)) {
      let gap = 0;
      let gapKey = "";
      group.changes.forEach((change, position) => {
        const i = index++;
        if (!shown || shown.has(i)) {
          if (gap) items.push({ key: gapKey, gap });
          gap = 0;
          items.push({ key: keys.value[i]!, change, index: i, position: position + 1 });
        } else {
          if (!gap) gapKey = `gap:${keys.value[i]}`;
          gap += sizes[i]?.size ?? estimate.value;
        }
      });
      if (gap) items.push({ key: gapKey, gap });
    }
    return { group, head, items };
  });
});
</script>

<template>
  <div ref="root" class="k-diff-list" :data-virtual="virtual ? 'true' : undefined">
    <section v-for="part in layout" :key="part.group.id" class="k-diff-list__group">
      <h3 :ref="measure" class="k-diff-list__heading" :data-index="virtual ? part.head : undefined">
        <button
          type="button"
          class="k-diff-list__toggle"
          :aria-expanded="!closed.has(part.group.id)"
          :aria-controls="`${id}-${part.group.id}`"
          @click="toggle(part.group.id)"
        >
          <Icon
            name="chevron-right"
            class="k-chevron"
            :data-open="closed.has(part.group.id) ? undefined : 'true'"
          />
          <span class="k-diff-list__sign" :data-kind="part.group.kind" aria-hidden="true">{{
            DIFF_SIGNS[part.group.kind]
          }}</span>
          <span class="k-visually-hidden">{{ word(part.group.kind) }} :</span>
          <span class="k-diff-list__title">{{ part.group.title }}</span>
          <code v-if="part.group.objectKey" class="k-diff-list__key">{{
            part.group.objectKey
          }}</code>
          <span class="k-diff-list__count">{{ part.group.changes.length }}</span>
        </button>
      </h3>
      <ul
        v-show="!closed.has(part.group.id)"
        :id="`${id}-${part.group.id}`"
        class="k-diff-list__changes"
      >
        <template v-for="item in part.items" :key="item.key">
          <li
            v-if="'change' in item"
            :ref="measure"
            class="k-diff-list__change"
            :data-kind="item.change.kind"
            :data-index="virtual ? item.index : undefined"
            :aria-setsize="virtual ? part.group.changes.length : undefined"
            :aria-posinset="virtual ? item.position : undefined"
          >
            <span class="k-diff-list__sign" :data-kind="item.change.kind" aria-hidden="true">{{
              DIFF_SIGNS[item.change.kind]
            }}</span>
            <span class="k-visually-hidden">{{ word(item.change.kind) }} :</span>
            <span class="k-diff-list__attribute">{{ item.change.attribute }}</span>
            <span class="k-diff-list__values" :class="{ 'k-mono': item.change.mono }">
              <del v-if="item.change.before !== undefined" class="k-diff-list__before"
                ><span class="k-visually-hidden">{{ t("before") }} : </span
                >{{ item.change.before }}</del
              >
              <Icon
                v-if="item.change.before !== undefined && item.change.after !== undefined"
                name="arrow-right"
                class="k-diff-list__arrow"
              />
              <ins v-if="item.change.after !== undefined" class="k-diff-list__after"
                ><span class="k-visually-hidden">{{ t("after") }} : </span
                >{{ item.change.after }}</ins
              >
            </span>
          </li>
          <li
            v-else
            role="none"
            class="k-diff-list__gap"
            aria-hidden="true"
            :style="{ '--k-diff-list-gap': `${item.gap}px` }"
          ></li>
        </template>
      </ul>
    </section>
  </div>
</template>

<style>
@layer components {
  /* A container: its host gives it its width. */
  .k-diff-list {
    --k-diff-list-max: var(--k-size-list-max);

    display: grid;
    grid-template-columns: minmax(0, 1fr);
    container-type: inline-size;
    background-color: var(--k-surface);
    border: var(--k-border-width) solid var(--k-border-strong);
    border-radius: var(--k-radius-md);
  }

  /* Virtualised: it scrolls in its own place; the scroll is the virtualiser's to keep
   * when measured lines replace their gaps (no anchoring of the browser on top). */
  /* Positioned: the words hidden off screen (`k-visually-hidden`, absolute) scroll with
   * it, instead of stretching the page to the height of every line. */
  .k-diff-list[data-virtual] {
    position: relative;
    align-content: start;
    max-block-size: var(--k-diff-list-max);
    overflow: auto;
    overflow-anchor: none;
  }

  .k-diff-list__group + .k-diff-list__group > .k-diff-list__heading {
    border-block-start: var(--k-border-width) solid var(--k-border);
  }

  .k-diff-list__heading {
    margin: 0;
    font: inherit;
  }

  .k-diff-list__toggle {
    display: flex;
    gap: var(--k-space-2);
    align-items: center;
    inline-size: 100%;
    min-block-size: var(--k-row-height);
    padding-inline: var(--k-space-2) var(--k-cell-padding-inline);
    font: inherit;
    font-weight: var(--k-weight-medium);
    color: var(--k-text);
    text-align: start;
    cursor: pointer;
    background-color: var(--k-surface-sunken);
    border: 0;
  }

  .k-diff-list__toggle:hover {
    background-color: var(--k-hover);
  }

  .k-diff-list__key {
    color: var(--k-text-muted);
  }

  .k-diff-list__count {
    margin-inline-start: auto;
    font-size: var(--k-text-xs);
    line-height: var(--k-text-xs-line);
    font-variant-numeric: tabular-nums;
    color: var(--k-text-muted);
  }

  .k-diff-list__sign {
    inline-size: var(--k-space-3);
    font-family: var(--k-font-mono);
    font-weight: var(--k-weight-semibold);
    text-align: center;
  }

  .k-diff-list__sign[data-kind="added"] {
    color: var(--k-diff-added);
  }

  .k-diff-list__sign[data-kind="removed"] {
    color: var(--k-diff-removed);
  }

  .k-diff-list__sign[data-kind="changed"] {
    color: var(--k-diff-changed);
  }

  .k-diff-list__sign[data-kind="anomaly"] {
    color: var(--k-state-review-fg);
  }

  .k-diff-list__changes {
    display: grid;
    padding: 0;
    margin: 0;
    list-style: none;
  }

  .k-diff-list__change {
    display: grid;
    grid-template-columns: var(--k-space-3) var(--k-size-col-md) minmax(0, 1fr);
    gap: var(--k-space-2);
    align-items: center;
    min-block-size: var(--k-row-height);
    padding-inline: var(--k-space-8) var(--k-cell-padding-inline);
    border-block-start: var(--k-border-width) solid var(--k-border);
  }

  /* The changes out of the view, at their measured height (set by CSSOM). */
  .k-diff-list__gap {
    block-size: var(--k-diff-list-gap);
  }

  .k-diff-list__attribute {
    color: var(--k-text-muted);
  }

  .k-diff-list__values {
    display: flex;
    flex-wrap: wrap;
    gap: var(--k-space-1);
    align-items: center;
    min-inline-size: 0;
  }

  .k-diff-list__before {
    color: var(--k-text-muted);
    text-decoration: line-through;
  }

  .k-diff-list__after {
    text-decoration: none;
  }

  .k-diff-list__change[data-kind="added"] .k-diff-list__after {
    color: var(--k-diff-added);
  }

  .k-diff-list__change[data-kind="removed"] .k-diff-list__before {
    color: var(--k-diff-removed);
  }

  .k-diff-list__arrow {
    color: var(--k-text-muted);
  }

  /* A long value breaks rather than widen the list. */
  .k-diff-list__before,
  .k-diff-list__after {
    min-inline-size: 0;
    overflow-wrap: anywhere;
  }

  /* Under `xs` (design « Adaptation à la largeur »): the values under the attribute. */
  @container (width < 360px) {
    .k-diff-list .k-diff-list__change {
      grid-template-columns: var(--k-space-3) minmax(0, 1fr);
      padding-inline-start: var(--k-space-2);
    }

    .k-diff-list .k-diff-list__values {
      grid-column: 2;
    }
  }
}
</style>
