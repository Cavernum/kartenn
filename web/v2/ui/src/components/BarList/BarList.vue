<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

<script setup lang="ts">
// A distribution in horizontal bars (design « Inventaire », line 36; revue du 2026-10-25):
// one line per entry, its label, a bar, its value, all against one scale (`max`, else the
// largest value). A table (row headers, values): the value is always written, the bar only
// shows it (`aria-hidden`); a value beyond the scale fills the bar and marks it. Bars
// drawn in CSS from a ratio set by CSSOM (REQ-020-14); tokens only.
import { computed } from "vue";
import { currentLocale, t } from "../../i18n.ts";
import type { ChartPoint } from "../BarChart/chart.ts";

const props = withDefaults(
  defineProps<{
    /** What the bars measure: the caption of the table (hidden; the host titles it). */
    label: string;
    items: ChartPoint[];
    /** The top of the scale (100 for a percentage); the largest value when unset. */
    max?: number | undefined;
    /** Labels are keys in fixed pitch. */
    mono?: boolean;
    pointLabel?: string | undefined;
    valueLabel?: string | undefined;
    format?: ((value: number) => string) | undefined;
  }>(),
  { max: undefined, pointLabel: undefined, valueLabel: undefined, format: undefined },
);

const write = computed(
  () => props.format ?? ((value: number) => new Intl.NumberFormat(currentLocale()).format(value)),
);
const top = computed(() => props.max ?? Math.max(0, ...props.items.map((i) => i.value)));
const lines = computed(() =>
  props.items.map((item) => {
    const ratio = top.value > 0 ? Math.max(0, item.value / top.value) : 0;
    return {
      ...item,
      over: ratio > 1,
      ratio: { "--k-bar-list-ratio": String(Math.min(1, ratio)) },
    };
  }),
);
</script>

<template>
  <!-- Explicit roles: laid out as a grid, a table loses its own in some engines. -->
  <table class="k-bar-list" role="table" :aria-label="props.label">
    <caption class="k-visually-hidden">
      {{
        props.label
      }}
    </caption>
    <thead class="k-visually-hidden" role="rowgroup">
      <tr role="row">
        <th scope="col" role="columnheader">{{ props.pointLabel ?? t("chartPoint") }}</th>
        <th scope="col" role="columnheader">{{ props.valueLabel ?? t("chartValue") }}</th>
      </tr>
    </thead>
    <tbody class="k-bar-list__body" role="rowgroup">
      <tr
        v-for="line in lines"
        :key="line.key"
        role="row"
        class="k-bar-list__line"
        :data-state="line.over ? 'over' : undefined"
      >
        <th
          scope="row"
          role="rowheader"
          class="k-bar-list__label"
          :class="{ 'k-mono': props.mono }"
        >
          {{ line.label }}
        </th>
        <td role="cell" class="k-bar-list__value">
          <span class="k-bar-list__track" aria-hidden="true"
            ><span class="k-bar-list__fill" :style="line.ratio"></span
          ></span>
          <span class="k-num"
            >{{ write(line.value)
            }}<span v-if="line.over" class="k-visually-hidden"> ({{ t("chartOver") }})</span></span
          >
        </td>
      </tr>
    </tbody>
  </table>
</template>

<style>
@layer components {
  /* One grid: labels as wide as the longest, then the bar and its value. */
  .k-bar-list {
    display: grid;
    grid-template-columns: max-content minmax(0, 1fr);
    gap: var(--k-space-1) var(--k-space-3);
    min-inline-size: 0;
    border-collapse: collapse;
  }

  .k-bar-list__body,
  .k-bar-list__line {
    display: grid;
    grid-column: 1 / -1;
    grid-template-columns: subgrid;
    row-gap: var(--k-space-1);
  }

  .k-bar-list__label {
    font-weight: var(--k-weight-regular);
    text-align: start;
  }

  .k-bar-list__label.k-mono {
    font-size: var(--k-text-mono);
  }

  .k-bar-list__value {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    gap: var(--k-space-2);
    align-items: center;
    text-align: end;
  }

  .k-bar-list__track {
    display: block;
    block-size: var(--k-space-2);
    background-color: var(--k-surface-sunken);
    border-radius: var(--k-radius-sm);
  }

  .k-bar-list__fill {
    display: block;
    inline-size: calc(100% * var(--k-bar-list-ratio, 0));
    block-size: 100%;
    background-color: var(--k-accent);
    border-radius: var(--k-radius-sm);
  }

  /* Beyond the scale: full, and cut by a stroke of the text colour at its end. */
  .k-bar-list__line[data-state="over"] .k-bar-list__track {
    border-inline-end: calc(var(--k-border-width) * 2) solid var(--k-text);
  }
}
</style>
