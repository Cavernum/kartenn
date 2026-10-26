<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

<script setup lang="ts">
// A histogram of a series over time (design « Inventaire », line 35; revue du 2026-10-25):
// one bar per point, in reading order, drawn in SVG by hand (no chart dependency). The plot
// is measured (`ResizeObserver`) and drawn in CSS pixels, so that it follows the width of
// its container without stretching its strokes; its height is a token. The SVG is one
// image (`role="img"`) named by a summary (count, bounds, maximum, last value); a table
// hidden on screen carries every value. Hovering a bar shows its label and value in a
// tooltip, decorative since the table says the same. Tokens only: light and dark follow.
import { computed, onBeforeUnmount, onMounted, ref, shallowRef, watch } from "vue";
import { currentLocale, t } from "../../i18n.ts";
import Tooltip from "../Tooltip/Tooltip.vue";
import { useTooltip } from "../Tooltip/tooltip.ts";
import { layoutBars, niceCeil, type ChartPoint } from "./chart.ts";

const props = withDefaults(
  defineProps<{
    /** What the series counts: the name of the image and the caption of its table. */
    label: string;
    points: ChartPoint[];
    /** Headers of the table: what a point is, what its value is. */
    pointLabel?: string | undefined;
    valueLabel?: string | undefined;
    /** Writes a value (numbers in the current language by default). */
    format?: ((value: number) => string) | undefined;
  }>(),
  { pointLabel: undefined, valueLabel: undefined, format: undefined },
);

const plot = ref<HTMLElement | null>(null);
const size = ref({ width: 0, height: 0 });
let observer: ResizeObserver | undefined;
onMounted(() => {
  const el = plot.value!;
  const measure = () => (size.value = { width: el.clientWidth, height: el.clientHeight });
  measure();
  if (typeof ResizeObserver !== "undefined") {
    observer = new ResizeObserver(measure);
    observer.observe(el);
  }
});
onBeforeUnmount(() => observer?.disconnect());

const write = computed(
  () => props.format ?? ((value: number) => new Intl.NumberFormat(currentLocale()).format(value)),
);
const peak = computed(() => Math.max(0, ...props.points.map((p) => p.value)));
const scale = computed(() => niceCeil(peak.value));
const bars = computed(() =>
  layoutBars(
    props.points.map((p) => p.value),
    size.value.width,
    size.value.height,
    scale.value,
  ),
);
/** The graduation: the top of the scale, its half, zero (from the top). */
const ticks = computed(() => [scale.value, scale.value / 2, 0]);

const summary = computed(() => {
  const points = props.points;
  if (!points.length) return `${props.label} : ${t("chartEmpty")}`;
  const first = points[0]!;
  const last = points.at(-1)!;
  const top = points.find((p) => p.value === peak.value)!;
  return (
    `${props.label} : ${points.length} ${t("chartValues")}, ${first.label} – ${last.label} ; ` +
    `${t("chartMax")} ${write.value(top.value)} (${top.label}) ; ` +
    `${t("chartLast")} ${write.value(last.value)}`
  );
});

const tip = useTooltip();
const hovered = shallowRef<number | null>(null);
// A tap shows the value of its bar until the next press elsewhere (« Tactile »): a touch
// pointer leaves the chart as the finger lifts.
function enter(event: PointerEvent, i: number): void {
  hovered.value = i;
  const bar = event.currentTarget as unknown as HTMLElement;
  if (event.pointerType === "touch") tip.pin(bar);
  else tip.show(bar);
}
function leave(event: PointerEvent): void {
  if (event.pointerType === "touch") return;
  hovered.value = null;
  tip.hide(true);
}
watch(tip.open, (open) => {
  if (!open) hovered.value = null;
});
</script>

<template>
  <figure class="k-bar-chart">
    <div class="k-bar-chart__scale" aria-hidden="true">
      <span v-for="(tick, i) in ticks" :key="i" class="k-bar-chart__tick" :data-at="i">{{
        write(tick)
      }}</span>
    </div>
    <div ref="plot" class="k-bar-chart__plot">
      <svg
        class="k-bar-chart__svg"
        role="img"
        :aria-label="summary"
        :width="size.width"
        :height="size.height"
        :viewBox="`0 0 ${size.width} ${size.height}`"
        shape-rendering="crispEdges"
        @pointerleave="leave"
      >
        <line
          v-for="(_, i) in ticks"
          :key="i"
          class="k-bar-chart__rule"
          :data-at="i"
          x1="0"
          :x2="size.width"
          :y1="i === 2 ? size.height - 0.5 : Math.round((size.height * i) / 2) + 0.5"
          :y2="i === 2 ? size.height - 0.5 : Math.round((size.height * i) / 2) + 0.5"
        />
        <g
          v-for="(bar, i) in bars"
          :key="props.points[i]!.key"
          class="k-bar-chart__bar"
          :data-hover="hovered === i ? 'true' : undefined"
        >
          <rect
            class="k-bar-chart__hit"
            :x="bar.slotX"
            y="0"
            :width="bar.slotWidth"
            :height="size.height"
            @pointerenter="enter($event, i)"
          />
          <rect
            class="k-bar-chart__fill"
            :x="bar.x"
            :y="bar.y"
            :width="bar.width"
            :height="bar.height"
          />
        </g>
      </svg>
    </div>
    <div v-if="props.points.length" class="k-bar-chart__axis" aria-hidden="true">
      <span>{{ props.points[0]!.label }}</span>
      <span>{{ props.points.at(-1)!.label }}</span>
    </div>
    <!-- Hidden in a box, not on the table: the caption sits in the wrapper box of the
         table, outside its `overflow` and `clip-path` in some engines, and would show over
         the plot (revue du 2026-10-25, soir). -->
    <div class="k-visually-hidden">
      <table>
        <caption>
          {{
            props.label
          }}
        </caption>
        <thead>
          <tr>
            <th scope="col">{{ props.pointLabel ?? t("chartPoint") }}</th>
            <th scope="col">{{ props.valueLabel ?? t("chartValue") }}</th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="point in props.points" :key="point.key">
            <th scope="row">{{ point.label }}</th>
            <td>{{ write(point.value) }}</td>
          </tr>
        </tbody>
      </table>
    </div>
    <Tooltip :control="tip" decorative>
      <template v-if="hovered !== null && props.points[hovered]">
        {{ props.points[hovered]!.label }} :
        <strong class="k-num">{{ write(props.points[hovered]!.value) }}</strong>
      </template>
    </Tooltip>
  </figure>
</template>

<style>
@layer components {
  /* The graduation at the inline start, the plot, then the first and last labels under it. */
  .k-bar-chart {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr);
    column-gap: var(--k-space-2);
    min-inline-size: 0;
    margin: 0;
    padding-block-start: calc(var(--k-text-xs-line) / 2);
    font-size: var(--k-text-xs);
    line-height: var(--k-text-xs-line);
    font-variant-numeric: tabular-nums;
    color: var(--k-text-muted);
  }

  /* As high as the plot: both share the first row of the grid. */
  .k-bar-chart__scale {
    position: relative;
    min-inline-size: var(--k-space-5);
    text-align: end;
  }

  /* Each figure centred on its rule: the top, the half, the base. */
  .k-bar-chart__tick {
    position: absolute;
    inset-inline-end: 0;
    translate: 0 -50%;
  }

  .k-bar-chart__tick[data-at="0"] {
    inset-block-start: 0;
  }

  .k-bar-chart__tick[data-at="1"] {
    inset-block-start: 50%;
  }

  .k-bar-chart__tick[data-at="2"] {
    inset-block-start: 100%;
  }

  .k-bar-chart__plot {
    block-size: calc(var(--k-space-10) * 3);
    min-inline-size: 0;
  }

  .k-bar-chart__svg {
    display: block;
    overflow: visible;
  }

  .k-bar-chart__rule {
    stroke: var(--k-border);
  }

  .k-bar-chart__rule[data-at="2"] {
    stroke: var(--k-border-strong);
  }

  /* Over its slot, which alone takes the pointer. */
  .k-bar-chart__fill {
    pointer-events: none;
    fill: var(--k-accent);
  }

  .k-bar-chart__bar[data-hover] .k-bar-chart__fill {
    fill: var(--k-accent-hover);
  }

  .k-bar-chart__bar[data-hover] .k-bar-chart__hit {
    fill: var(--k-hover);
  }

  .k-bar-chart__hit {
    fill: transparent;
  }

  .k-bar-chart__axis {
    display: flex;
    grid-column: 2;
    justify-content: space-between;
    padding-block-start: var(--k-space-1);
  }
}
</style>
