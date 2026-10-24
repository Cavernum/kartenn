<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

<script setup lang="ts">
// A tooltip (APG Tooltip; design « Inventaire », line 15): the bubble of a `useTooltip`
// control, in the layer of floating layers (`providePortalTarget`), placed under its
// anchor (or after it, `side="inline-end"`), turned to the other side when it would leave
// the viewport. Never the only bearer of a needed information: the anchor names it by
// `aria-describedby` (the bubble stays in the document, hidden, when closed), or the host
// says it otherwise and marks the bubble `decorative`. Placed through CSSOM (REQ-020-14).
import { nextTick, onBeforeUnmount, ref, watch } from "vue";
import { injectPortalTarget } from "../../context.ts";
import type { TooltipControl } from "./tooltip.ts";

const props = withDefaults(
  defineProps<{
    control: TooltipControl;
    side?: "block-end" | "inline-end";
    /** The anchor says the same thing otherwise: the bubble is hidden from the tree. */
    decorative?: boolean;
  }>(),
  { side: "block-end" },
);
const portal = injectPortalTarget();
const bubble = ref<HTMLElement | null>(null);
const placed = ref<string>(props.side);

/** Places the bubble against its anchor, inside the viewport. */
function place(): void {
  const el = bubble.value;
  const anchor = props.control.anchor.value;
  if (!el || !anchor) return;
  const a = anchor.getBoundingClientRect();
  const b = el.getBoundingClientRect();
  const gap = parseFloat(getComputedStyle(el).getPropertyValue("--k-tooltip-gap")) || 0;
  const width = document.documentElement.clientWidth;
  const height = document.documentElement.clientHeight;
  const clamp = (v: number, max: number) => Math.max(gap, Math.min(v, max - gap));
  let x: number;
  let y: number;
  if (props.side === "inline-end") {
    const after = a.right + gap + b.width <= width;
    x = after ? a.right + gap : a.left - gap - b.width;
    y = clamp(a.top + (a.height - b.height) / 2, height - b.height);
    placed.value = after ? "inline-end" : "inline-start";
  } else {
    const below = a.bottom + gap + b.height <= height;
    x = clamp(a.left + (a.width - b.width) / 2, width - b.width);
    y = below ? a.bottom + gap : a.top - gap - b.height;
    placed.value = below ? "block-end" : "block-start";
  }
  // Coordinates from the inline start of the viewport, whatever the direction.
  if (getComputedStyle(el).direction === "rtl") x = width - x - b.width;
  el.style.setProperty("--k-tooltip-x", `${Math.round(x)}px`);
  el.style.setProperty("--k-tooltip-y", `${Math.round(y)}px`);
}

/** The page moves under an open tooltip: it closes rather than float away from its anchor. */
const onScroll = () => props.control.hide();
watch(
  () => props.control.open.value,
  async (open) => {
    if (open) {
      await nextTick();
      place();
      window.addEventListener("scroll", onScroll, { capture: true, passive: true });
    } else window.removeEventListener("scroll", onScroll, { capture: true });
  },
);
onBeforeUnmount(() => window.removeEventListener("scroll", onScroll, { capture: true }));
</script>

<template>
  <Teleport :to="portal">
    <div
      :id="props.control.id"
      ref="bubble"
      class="k-tooltip"
      role="tooltip"
      :data-side="placed"
      :hidden="!props.control.open.value"
      :aria-hidden="props.decorative ? 'true' : undefined"
      @pointerenter="props.control.hold()"
      @pointerleave="props.control.hide(true)"
    >
      <slot />
    </div>
  </Teleport>
</template>

<style>
@layer components {
  .k-tooltip {
    --k-tooltip-gap: var(--k-space-1);

    position: fixed;
    inset-block-start: var(--k-tooltip-y);
    inset-inline-start: var(--k-tooltip-x);
    z-index: var(--k-z-toast);
    max-inline-size: var(--k-size-tooltip-max);
    padding: var(--k-space-1) var(--k-space-2);
    font-size: var(--k-text-sm);
    line-height: var(--k-text-sm-line);
    font-weight: var(--k-weight-regular);
    color: var(--k-text);
    text-align: start;
    white-space: normal;
    background-color: var(--k-surface);
    border: var(--k-border-width) solid var(--k-border-strong);
    border-radius: var(--k-radius-md);
    box-shadow: var(--k-shadow-overlay);
  }

  .k-tooltip[hidden] {
    display: none;
  }

  .k-tooltip code {
    font-family: var(--k-font-mono);
    font-size: var(--k-text-mono);
  }
}
</style>
