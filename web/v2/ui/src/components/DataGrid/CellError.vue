<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

<script setup lang="ts">
// The error of a cell of a `DataGrid` (cinquième relecture du 2026-10-26): never part of
// the cell, which keeps its size. The message stands in a floating layer, in the layer of
// floating layers (`providePortalTarget`), under the cell (above it near the bottom of the
// viewport, aligned on its end near the inline edge); one outline joins the cell and the
// message into a tab that grows out of the cell (`cellError.ts`), the cell's content left
// uncovered. The grid opens it while the cell is hovered or holds the focus, and Escape
// closes it (WCAG 1.4.13); the pointer may go from the cell onto the message, which is
// adjacent (no gap, no grace delay). The message stays in the document, hidden, when
// closed: the cell names it by `aria-describedby`. Not a tooltip (it holds the copy button
// of the code, a widget of the cell: out of the tab order, reached from the cell by Enter
// or F2, left by Escape or F2, as in the cell). Placed through CSSOM (REQ-020-14),
// again when the page scrolls or the viewport changes; hidden while the cell is out of
// the view of its grid.
import { nextTick, onBeforeUnmount, ref, watch } from "vue";
import { injectPortalTarget } from "../../context.ts";
import CodeBadge from "../CodeBadge/CodeBadge.vue";
import { placeCellError } from "./cellError.ts";

const props = defineProps<{
  id: string;
  error: string;
  code?: string | undefined;
  /** The cell, while the message is open. */
  anchor: HTMLElement | null;
}>();
const emit = defineEmits<{
  /** The pointer left the message for somewhere else than its cell. */
  leave: [];
  /** Escape, heard by the document while the message is open and nothing took it. */
  dismiss: [];
  /** Escape or F2 on the copy button of the code: the focus goes back to the cell. */
  back: [];
  /** The focus left the message for somewhere else than its cell. */
  blur: [];
}>();
const portal = injectPortalTarget();
const layer = ref<HTMLElement | null>(null);
const message = ref<HTMLElement | null>(null);
const fill = ref("");
const outline = ref("");
const box = ref({ width: 0, height: 0 });
const side = ref<"block-end" | "block-start">("block-end");
const align = ref<"start" | "end">("start");
const clipped = ref(false);

/** Whether the cell lies outside the view of its grid (scrolled away, under its header). */
function outOfView(cell: DOMRect, anchor: HTMLElement): boolean {
  const grid = anchor.closest<HTMLElement>(".k-data-grid");
  if (!grid) return false;
  const view = grid.getBoundingClientRect();
  const head = grid.querySelector<HTMLElement>(".k-data-grid__head")?.offsetHeight ?? 0;
  return (
    cell.top < view.top + head - 1 ||
    cell.bottom > view.bottom + 1 ||
    cell.right <= view.left ||
    cell.left >= view.right
  );
}

function place(): void {
  const el = layer.value;
  const text = message.value;
  const anchor = props.anchor;
  if (!el || !text || !anchor) return;
  // The grid keeps one tab stop: the copy button of the code is reached by Enter or F2.
  for (const widget of text.querySelectorAll<HTMLElement>("button")) widget.tabIndex = -1;
  const cell = anchor.getBoundingClientRect();
  clipped.value = outOfView(cell, anchor);
  el.style.setProperty("--k-cell-error-min", `${cell.width}px`);
  const style = getComputedStyle(el);
  const rtl = style.direction === "rtl";
  const width = document.documentElement.clientWidth;
  const placed = placeCellError({
    cell: { x: cell.left, y: cell.top, width: cell.width, height: cell.height },
    message: { width: text.offsetWidth, height: text.offsetHeight },
    viewport: { width, height: document.documentElement.clientHeight },
    radius: parseFloat(style.getPropertyValue("--k-cell-error-radius")) || 0,
    stroke: parseFloat(style.getPropertyValue("--k-cell-error-stroke")) || 1,
    margin: parseFloat(style.getPropertyValue("--k-cell-error-margin")) || 0,
    rtl,
  });
  // Coordinates from the inline start of the viewport, whatever the direction.
  const x = rtl ? width - placed.x - placed.width : placed.x;
  el.style.setProperty("--k-cell-error-x", `${Math.round(x * 100) / 100}px`);
  el.style.setProperty("--k-cell-error-y", `${Math.round(placed.y * 100) / 100}px`);
  el.style.setProperty("--k-cell-error-width", `${placed.width}px`);
  el.style.setProperty("--k-cell-error-height", `${placed.height}px`);
  el.style.setProperty("--k-cell-error-top", `${placed.messageTop}px`);
  box.value = { width: placed.width, height: placed.height };
  side.value = placed.side;
  align.value = placed.align;
  fill.value = placed.fill;
  outline.value = placed.outline;
}

const onMove = () => place();
/** Escape with the focus elsewhere (the message opened by hovering): it closes too. */
function onKeydown(event: KeyboardEvent): void {
  if (event.key === "Escape" && !event.defaultPrevented) {
    event.preventDefault();
    emit("dismiss");
  }
}
function listen(on: boolean): void {
  if (on) {
    window.addEventListener("scroll", onMove, { capture: true, passive: true });
    window.addEventListener("resize", onMove);
    document.addEventListener("keydown", onKeydown);
  } else {
    window.removeEventListener("scroll", onMove, { capture: true });
    window.removeEventListener("resize", onMove);
    document.removeEventListener("keydown", onKeydown);
  }
}
watch(
  () => props.anchor,
  async (anchor, before) => {
    if (anchor && !before) listen(true);
    if (!anchor && before) listen(false);
    if (anchor) {
      await nextTick();
      place();
    }
  },
);
onBeforeUnmount(() => listen(false));

/** The keys of a widget of the cell (APG Grid): Escape or F2 give the focus back to it. */
function onMessageKeydown(event: KeyboardEvent): void {
  if (event.key === "Escape" || event.key === "F2") {
    event.preventDefault();
    emit("back");
  }
}

function onFocusout(event: FocusEvent): void {
  const to = event.relatedTarget as Node | null;
  if (!to || !(props.anchor?.contains(to) || message.value?.contains(to))) emit("blur");
}

/** The pointer leaves the message: it closes, unless it went back onto its cell. */
function onPointerleave(event: PointerEvent): void {
  if (event.pointerType === "touch") return;
  const to = event.relatedTarget as Node | null;
  if (!to || !props.anchor?.contains(to)) emit("leave");
}
</script>

<template>
  <!-- Deferred: the layer of the frame may come after the grid in the same render. -->
  <Teleport defer :to="portal">
    <div
      ref="layer"
      class="k-cell-error"
      :data-side="side"
      :data-align="align"
      :data-clipped="clipped ? 'true' : undefined"
      :hidden="!props.anchor"
    >
      <svg
        class="k-cell-error__shape"
        :viewBox="`0 0 ${box.width} ${box.height}`"
        :width="box.width"
        :height="box.height"
        aria-hidden="true"
        focusable="false"
      >
        <path class="k-cell-error__fill" :d="fill" />
        <path class="k-cell-error__outline" :d="outline" />
      </svg>
      <div
        :id="props.id"
        ref="message"
        class="k-cell-error__message"
        @pointerleave="onPointerleave"
        @keydown="onMessageKeydown"
        @focusout="onFocusout"
      >
        <span>{{ props.error }}</span>
        <CodeBadge v-if="props.code" :code="props.code" />
      </div>
    </div>
  </Teleport>
</template>

<style>
@layer components {
  /* The layer: the cell and its message, physical box placed by CSSOM. Only the message
   * takes the pointer: the cell under the layer stays the cell. */
  .k-cell-error {
    --k-cell-error-radius: var(--k-radius-md);
    --k-cell-error-stroke: var(--k-border-width);
    --k-cell-error-margin: var(--k-space-2);

    position: fixed;
    inset-block-start: var(--k-cell-error-y);
    inset-inline-start: var(--k-cell-error-x);
    z-index: var(--k-z-overlay);
    inline-size: var(--k-cell-error-width);
    block-size: var(--k-cell-error-height);
    pointer-events: none;
  }

  .k-cell-error[hidden] {
    display: none;
  }

  /* Scrolled out of the view of its grid: the message waits, unseen. */
  .k-cell-error[data-clipped] {
    visibility: hidden;
  }

  .k-cell-error__shape {
    position: absolute;
    inset: 0;
    overflow: visible;
  }

  .k-cell-error__fill {
    fill: var(--k-state-ko-bg);
  }

  .k-cell-error__outline {
    fill: none;
    stroke: var(--k-state-ko-fg);
    stroke-width: var(--k-border-width);
    stroke-linejoin: round;
  }

  /* The message: as wide as its text (up to a panel), never narrower than its cell. */
  .k-cell-error__message {
    position: absolute;
    inset-block-start: var(--k-cell-error-top);
    inset-inline-start: var(--k-space-0);
    display: flex;
    flex-wrap: nowrap;
    gap: var(--k-space-2);
    align-items: center;
    box-sizing: border-box;
    inline-size: max-content;
    min-inline-size: var(--k-cell-error-min);
    max-inline-size: var(--k-size-panel-lg);
    padding: var(--k-space-1) var(--k-space-2);
    font-family: var(--k-font-sans);
    font-size: var(--k-text-sm);
    line-height: var(--k-text-sm-line);
    font-weight: var(--k-weight-regular);
    color: var(--k-text);
    white-space: normal;
    pointer-events: auto;
  }

  /* The sentence wraps; the code beside it stays whole (024/M16). */
  .k-cell-error__message > span:first-child {
    min-inline-size: 0;
  }

  .k-cell-error__message > .k-code-badge {
    flex: none;
  }

  /* Motion (design « Mouvement »): the tab fades in from its cell, and leaves at once, as a
   * tooltip. The fade is not motion: it stays under reduced motion. */
  .k-cell-error {
    transition-property: opacity;
    transition-duration: max(var(--k-duration-short), var(--k-duration-fade));
    transition-timing-function: var(--k-ease-standard);
  }

  @starting-style {
    .k-cell-error {
      opacity: 0;
    }
  }
}
</style>
