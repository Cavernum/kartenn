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
// the focus back to what opened it. Closed, it leaves nothing behind. On a narrow window
// the host sets `cover`: the panel takes the whole width of its container, without handle
// (spec 021, « Cadre » : at 768 px the panel covers the surface). With `rail`, the closed
// panel leaves a strip in the flow at the end of its container (the host gives it an
// `auto` track): a chevron button, « Ouvrir le panneau », reopens it on its last tab and
// moves the focus there; closing it from inside gives the focus back to the strip. The
// strip covers nothing, at any width (revue du 2026-10-25). The footer shows only when the
// host fills it for the tab in front: no empty band under a tab without actions. With
// `sheet` (compact frame, design « Adaptation à la largeur »), it covers the whole window,
// a back button at its start instead of the cross, and takes the focus when it opens.
import {
  Comment,
  Fragment,
  Text,
  computed,
  isVNode,
  nextTick,
  ref,
  useId,
  useSlots,
  watch,
  type VNodeArrayChildren,
} from "vue";
import { t } from "../../i18n.ts";
import IconButton from "../IconButton/IconButton.vue";
import { isRtl } from "../internal/gridKeys.ts";
import Tabs from "../Tabs/Tabs.vue";
import type { TabItem } from "../Tabs/tabs.ts";

defineOptions({ inheritAttrs: false });

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
    /** Over the whole container (narrow window): full width, not resizable. */
    cover?: boolean;
    /** Closed, leave a strip with a button that reopens it. */
    rail?: boolean;
    /**
     * A sheet over the whole window (compact frame): a back button at the start instead of
     * the cross, no handle, the focus moved to its active tab when it opens.
     */
    sheet?: boolean;
  }>(),
  { id: undefined, min: 320, max: 720, step: 16 },
);
const open = defineModel<boolean>("open", { default: true });
/** The active tab. */
const tab = defineModel<string>("tab", { default: "" });
/** The width of the panel, in CSS pixels. */
const width = defineModel<number>("width", { default: 420 });
const emit = defineEmits<{ close: [] }>();
const slots = useSlots();

/** The tab in front, for the footer. */
const current = computed(() => tab.value || props.tabs[0]?.value);

/** Whether rendered nodes hold anything but comments and blank text (an empty slot). */
function filled(nodes: VNodeArrayChildren): boolean {
  return nodes.some((node) => {
    if (node === null || node === undefined || typeof node === "boolean") return false;
    if (typeof node === "string" || typeof node === "number") return String(node).trim() !== "";
    if (Array.isArray(node)) return filled(node);
    if (!isVNode(node) || node.type === Comment) return false;
    if (node.type === Text) return String(node.children ?? "").trim() !== "";
    if (node.type === Fragment) return Array.isArray(node.children) && filled(node.children);
    return true;
  });
}

/** The footer shows only when the host fills it for the tab in front: no empty band. */
const footerShown = (): boolean => !!slots.footer && filled(slots.footer({ tab: current.value }));

const generated = useId();
const id = computed(() => props.id ?? generated);
const panel = ref<HTMLElement | null>(null);
const strip = ref<HTMLElement | null>(null);
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
  else if (inside) strip.value?.querySelector<HTMLElement>("button")?.focus();
  opener = null;
}

// A sheet hides all else: opened, it takes the focus on its active tab.
watch(open, async (now) => {
  if (!now || !props.sheet) return;
  await nextTick();
  panel.value?.querySelector<HTMLElement>('[role="tab"][aria-selected="true"]')?.focus();
});

/** The strip reopens the panel and moves the focus to its active tab. */
async function reopen(): Promise<void> {
  open.value = true;
  await nextTick();
  panel.value?.querySelector<HTMLElement>('[role="tab"][aria-selected="true"]')?.focus();
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
  <Transition name="k-side-panel">
    <aside
      v-if="open"
      v-bind="$attrs"
      :id="id"
      ref="panel"
      class="k-side-panel"
      :aria-label="props.label"
      :style="size"
      :data-cover="props.cover ? 'true' : undefined"
      :data-sheet="props.sheet ? 'true' : undefined"
      @keydown="onKeydown"
    >
      <div
        v-if="!props.cover && !props.sheet"
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
        v-if="props.sheet"
        class="k-side-panel__back"
        icon="arrow-left"
        size="sm"
        :label="t('panelBack')"
        @click="close"
      />
      <IconButton
        v-else
        class="k-side-panel__close"
        icon="x"
        size="sm"
        :label="t('panelClose')"
        @click="close"
      />
      <footer v-if="footerShown()" class="k-side-panel__footer">
        <slot name="footer" :tab="current" />
      </footer>
    </aside>
  </Transition>
  <div v-if="props.rail" ref="strip" class="k-side-panel-rail">
    <Transition name="k-side-panel-rail">
      <IconButton
        v-if="!open"
        class="k-side-panel-rail__open"
        icon="chevron-left"
        size="sm"
        variant="secondary"
        :label="t('panelOpen')"
        aria-expanded="false"
        @click="reopen"
      />
    </Transition>
  </div>
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

  .k-side-panel[data-cover] {
    inline-size: 100%;
    border-inline-start: 0;
  }

  /* A sheet: the whole window, over the frame; its back button at the start. */
  .k-side-panel[data-sheet] {
    position: fixed;
    inset: 0;
    z-index: var(--k-z-overlay);
    inline-size: 100%;
    border-inline-start: 0;
    box-shadow: none;
  }

  .k-side-panel[data-sheet] > .k-side-panel__tabs > .k-tabs__list {
    padding-inline: var(--k-space-8) var(--k-space-2);
  }

  .k-side-panel__back {
    position: absolute;
    inset-block-start: calc((var(--k-control-height-lg) - var(--k-control-height-sm)) / 2);
    inset-inline-start: var(--k-space-2);
  }

  .k-side-panel__back:dir(rtl) .k-icon {
    scale: -1 1;
  }

  /* Motion (design « Mouvement »): opened or closed by the user (never at the first
   * render), the panel slides from its edge and fades; the surface under it keeps its
   * width. Interactive from its first frame, the focus placed at once. Vue's
   * `<Transition>` classes, the one place of a class of state. */
  .k-side-panel-enter-active {
    transition-property: opacity, translate;
    transition-duration: max(var(--k-duration-medium), var(--k-duration-fade));
    transition-timing-function: var(--k-ease-standard);
  }

  .k-side-panel-leave-active {
    pointer-events: none;
    transition-property: opacity, translate;
    transition-duration: max(var(--k-duration-short), var(--k-duration-fade));
    transition-timing-function: var(--k-ease-exit);
  }

  .k-side-panel-enter-from,
  .k-side-panel-leave-to {
    opacity: 0;
    translate: var(--k-motion-shift-lg) 0;
  }

  .k-side-panel-enter-from:dir(rtl),
  .k-side-panel-leave-to:dir(rtl) {
    translate: calc(var(--k-motion-shift-lg) * -1) 0;
  }

  /* The chevron of the strip comes the same way, in. */
  .k-side-panel-rail-enter-active {
    transition-property: opacity, translate;
    transition-duration: max(var(--k-duration-medium), var(--k-duration-fade));
    transition-timing-function: var(--k-ease-standard);
  }

  .k-side-panel-rail-enter-from {
    opacity: 0;
    translate: var(--k-motion-shift-sm) 0;
  }

  .k-side-panel-rail-enter-from:dir(rtl) {
    translate: calc(var(--k-motion-shift-sm) * -1) 0;
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

  /* A finger: the handle takes 24 px, still astride the edge (WCAG 2.5.8); its colour
   * keeps the width of the strip. */
  @media (any-pointer: coarse) {
    .k-side-panel__handle {
      inset-inline-start: calc(var(--k-control-height-sm) / -2);
      inline-size: var(--k-control-height-sm);
      padding-inline: calc((var(--k-control-height-sm) - var(--k-space-2)) / 2);
      background-clip: content-box;
    }
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

  /* The strip of the closed panel: in the flow, at the end, the width of a small control
   * and its margins; its button at the height of the tab list. */
  .k-side-panel-rail {
    display: flex;
    flex-direction: column;
    align-items: center;
    inline-size: calc(var(--k-control-height-sm) + var(--k-space-2));
    min-block-size: 0;
    padding-block-start: calc((var(--k-control-height-lg) - var(--k-control-height-sm)) / 2);
    background-color: var(--k-surface);
    border-inline-start: var(--k-border-width) solid var(--k-border-strong);
  }

  /* The chevron points to where the panel comes from. */
  .k-side-panel-rail__open:dir(rtl) .k-icon {
    scale: -1 1;
  }

  .k-side-panel__footer {
    display: grid;
    gap: var(--k-space-2);
    padding: var(--k-space-2) var(--k-space-3) var(--k-space-3);
    border-block-start: var(--k-border-width) solid var(--k-border);
  }
}
</style>
