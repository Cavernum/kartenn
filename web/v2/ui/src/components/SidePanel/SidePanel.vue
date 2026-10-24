<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

<script setup lang="ts">
// The right panel of the frame (design « Inventaire », line 30; spec 021, « Cadre »;
// 024/M8): a non-modal `complementary` region with tabs (« Détail », « Brouillon n »),
// laid over the end of the surface: the view keeps its width, the panel covers its end.
// The host gives it a positioned container (`position: relative`) and makes the surface an
// isolated stacking context, so that no sticky header of the surface passes over it.
// 420 px by default, resized by a handle at its inline start (APG Window Splitter, written
// here: Reka's Splitter injects a <style>, refused by the CSP): Left and Right arrows by a
// step, Home and End to the bounds, the pointer by dragging. Escape closes it and gives
// the focus back to what opened it. Closed, it leaves nothing behind.
import { computed, nextTick, ref, useId, watch } from "vue";
import { t } from "../../i18n.ts";
import IconButton from "../IconButton/IconButton.vue";
import { isRtl } from "../internal/gridKeys.ts";
import Tabs from "../Tabs/Tabs.vue";
import type { TabItem } from "../Tabs/tabs.ts";

const props = withDefaults(
  defineProps<{
    /** The name of the region. */
    label: string;
    /** The id of the region, for the `aria-controls` of its toggle. */
    id?: string | undefined;
    tabs: TabItem[];
    /** Bounds of the width, in CSS pixels. */
    min?: number;
    max?: number;
    /** The step of an arrow on the handle. */
    step?: number;
  }>(),
  { id: undefined, min: 320, max: 720, step: 16 },
);
const open = defineModel<boolean>("open", { default: true });
/** The active tab. */
const tab = defineModel<string>("tab", { default: "" });
/** The width of the panel, in CSS pixels. */
const width = defineModel<number>("width", { default: 420 });
const emit = defineEmits<{ close: [] }>();

const generated = useId();
const id = computed(() => props.id ?? generated);
const panel = ref<HTMLElement | null>(null);
const handle = ref<HTMLElement | null>(null);
const size = computed(() => ({ "--k-side-panel-width": `${width.value}px` }));
const clamp = (value: number) => Math.round(Math.max(props.min, Math.min(props.max, value)));

// The focus comes back to what opened the panel (WCAG 2.4.3): the element that had it
// when the panel opened, if the focus was in the panel when it closed.
let opener: HTMLElement | null = null;
watch(
  open,
  (now) => {
    if (typeof document === "undefined") return;
    if (now) opener = document.activeElement as HTMLElement | null;
  },
  { immediate: true },
);

async function close(): Promise<void> {
  const inside = panel.value?.contains(document.activeElement) ?? false;
  open.value = false;
  emit("close");
  await nextTick();
  if (inside && opener && opener !== document.body && opener.isConnected) opener.focus();
  opener = null;
}

function onKeydown(event: KeyboardEvent): void {
  if (event.key === "Escape" && !event.defaultPrevented) {
    event.preventDefault();
    void close();
  }
}

/** The handle sits at the inline start: moving it towards the start widens the panel. */
function onHandleKeydown(event: KeyboardEvent): void {
  const wider = isRtl(handle.value!) ? "ArrowRight" : "ArrowLeft";
  const narrower = isRtl(handle.value!) ? "ArrowLeft" : "ArrowRight";
  const next =
    event.key === wider
      ? width.value + props.step
      : event.key === narrower
        ? width.value - props.step
        : event.key === "Home"
          ? props.min
          : event.key === "End"
            ? props.max
            : null;
  if (next === null) return;
  event.preventDefault();
  width.value = clamp(next);
}

let drag: { x: number; width: number; sign: number } | null = null;
function onPointerdown(event: PointerEvent): void {
  if (event.button !== 0) return;
  handle.value!.setPointerCapture(event.pointerId);
  drag = { x: event.clientX, width: width.value, sign: isRtl(handle.value!) ? -1 : 1 };
}
function onPointermove(event: PointerEvent): void {
  if (drag) width.value = clamp(drag.width + (drag.x - event.clientX) * drag.sign);
}
function onPointerup(event: PointerEvent): void {
  if (!drag) return;
  handle.value!.releasePointerCapture(event.pointerId);
  drag = null;
}
</script>

<template>
  <aside
    v-if="open"
    :id="id"
    ref="panel"
    class="k-side-panel"
    :aria-label="props.label"
    :style="size"
    @keydown="onKeydown"
  >
    <div
      ref="handle"
      class="k-side-panel__handle"
      role="separator"
      tabindex="0"
      aria-orientation="vertical"
      :aria-label="t('panelResize')"
      :aria-controls="id"
      :aria-valuenow="width"
      :aria-valuemin="props.min"
      :aria-valuemax="props.max"
      @keydown="onHandleKeydown"
      @pointerdown="onPointerdown"
      @pointermove="onPointermove"
      @pointerup="onPointerup"
      @pointercancel="onPointerup"
    ></div>
    <Tabs v-model="tab" :tabs="props.tabs" :label="props.label" class="k-side-panel__tabs">
      <template v-for="item in props.tabs" :key="item.value" #[item.value]>
        <slot :name="item.value" />
      </template>
    </Tabs>
    <IconButton
      class="k-side-panel__close"
      icon="x"
      size="sm"
      :label="t('panelClose')"
      @click="close"
    />
    <footer v-if="$slots.footer" class="k-side-panel__footer">
      <slot name="footer" :tab="tab || props.tabs[0]?.value" />
    </footer>
  </aside>
</template>

<style>
@layer components {
  /* Over the end of the surface, at the height of its container. */
  .k-side-panel {
    --k-side-panel-width: var(--k-size-panel-lg);

    position: absolute;
    inset-block: 0;
    inset-inline-end: 0;
    z-index: var(--k-z-sticky);
    display: grid;
    grid-template-rows: minmax(0, 1fr) auto;
    grid-template-columns: minmax(0, 1fr);
    inline-size: min(var(--k-side-panel-width), 100%);
    color: var(--k-text);
    background-color: var(--k-surface);
    border-inline-start: var(--k-border-width) solid var(--k-border-strong);
    box-shadow: var(--k-shadow-overlay);
  }

  /* The handle: a strip astride the inline-start edge. */
  .k-side-panel__handle {
    position: absolute;
    inset-block: 0;
    inset-inline-start: calc(var(--k-space-1) * -1);
    z-index: var(--k-z-sticky);
    inline-size: var(--k-space-2);
    cursor: col-resize;
    touch-action: none;
  }

  .k-side-panel__handle:hover,
  .k-side-panel__handle:focus-visible {
    background-color: var(--k-accent);
  }

  .k-side-panel__handle:focus-visible {
    outline-offset: 0;
  }

  .k-side-panel__tabs {
    grid-template-rows: auto minmax(0, 1fr);
    gap: 0;
    min-block-size: 0;
  }

  /* Room at the end of the tab list for the close button. */
  .k-side-panel__tabs > .k-tabs__list {
    padding-inline: var(--k-space-2) var(--k-space-8);
  }

  .k-side-panel__tabs > .k-tabs__panel {
    min-block-size: 0;
    padding: var(--k-space-3);
    overflow: auto;
  }

  .k-side-panel__close {
    position: absolute;
    inset-block-start: calc((var(--k-control-height-lg) - var(--k-control-height-sm)) / 2);
    inset-inline-end: var(--k-space-2);
  }

  .k-side-panel__footer {
    display: grid;
    gap: var(--k-space-2);
    padding: var(--k-space-2) var(--k-space-3) var(--k-space-3);
    border-block-start: var(--k-border-width) solid var(--k-border);
  }
}
</style>
