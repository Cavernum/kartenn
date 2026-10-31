<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

<script setup lang="ts">
// The right panel of the frame (design « Inventaire », line 30; spec 021, « Cadre »;
// 024/M8): a non-modal `complementary` region with tabs (« Détail », « Brouillon n »),
// beside the surface (design « Adaptation à la largeur », « tout s'adapte », 2026-10-26):
// the host lays it in an `auto` track of its grid, at the end, and the whole surface
// shrinks to the width it leaves; the panel covers nothing. The host is a positioned
// inline-size container (`position: relative`, `container-type: inline-size`): the panel
// never takes more than its container less `--k-size-panel-md` for the surface, and while
// it closes it leaves the flow at once (the surface takes its width back without waiting).
// 420 px by default, resized by a handle at its inline start (APG Window Splitter, written
// here: Reka's Splitter injects a <style>, refused by the CSP): Left and Right arrows by a
// step, Home and End to the bounds, the pointer by dragging — or in one click, without
// dragging (WCAG 2.5.7), by the menu « Largeur du panneau » beside the close button:
// « Étroit » (the lower bound), « Standard » (420 px), « Large » (the upper bound), the
// current one checked (`menuitemradio`). The line of the handle is the outline of the
// panel (`.k-panel-handle`, components/shared.css). Escape closes it and gives the focus
// back to what opened it. Closed, it leaves nothing behind. On a narrow window the host
// sets `cover`: the panel takes the whole width of its container, over it, without handle
// (spec 021, « Cadre » : at 768 px the panel covers the surface). With `rail`, the closed
// panel leaves a strip in the flow at the end of its container (the same `auto` track): a
// chevron button, « Ouvrir le panneau », reopens it on its last tab and moves the focus
// there; closing it from inside gives the focus back to the strip. The footer shows only
// when the host fills it for the tab in front: no empty band under a tab without actions.
// With `sheet` (compact frame, design « Adaptation à la largeur »), it covers the whole
// window, a back button at its start instead of the cross, and takes the focus when it
// opens.
//
// What it hides leaves the focus order (WCAG 2.4.3, 2.4.11; relecture du 2026-10-26): with
// `cover`, the other children of its container are `inert` while it is open; a sheet is a
// modal dialog (`role="dialog"`, `aria-modal`) and everything beside it, up to the nearest
// `[data-k-modal-scope]` (or `body`), is `inert` — the target of floating layers stays live.
// Beside the surface, it hides nothing.
import { computed, nextTick, onBeforeUnmount, ref, useId, useSlots, watch } from "vue";
import { injectPortalTarget } from "../../context.ts";
import { t } from "../../i18n.ts";
import IconButton from "../IconButton/IconButton.vue";
import { isRtl } from "../internal/gridKeys.ts";
import { inertOthers } from "../internal/inertOthers.ts";
import { filled, ownsEscape, scrollCue } from "../internal/panel.ts";
import Menu from "../Menu/Menu.vue";
import type { MenuItem } from "../Menu/menu.ts";
import Tabs from "../Tabs/Tabs.vue";
import type { TabItem } from "../Tabs/tabs.ts";
import { PANEL_STANDARD, widthPresets } from "./sidePanel.ts";

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
const width = defineModel<number>("width", { default: PANEL_STANDARD });
const emit = defineEmits<{ close: [] }>();
const slots = useSlots();

/** The tab in front, for the footer. */
const current = computed(() => tab.value || props.tabs[0]?.value);

/** The footer shows only when the host fills it for the tab in front: no empty band. */
const footerShown = (): boolean => !!slots.footer && filled(slots.footer({ tab: current.value }));

const generated = useId();
const id = computed(() => props.id ?? generated);
const panel = ref<HTMLElement | null>(null);
const strip = ref<HTMLElement | null>(null);
const handle = ref<HTMLElement | null>(null);
const size = computed(() => ({ "--k-side-panel-width": `${width.value}px` }));
/**
 * The widest the panel shows beside the surface: its container less the room kept for the
 * surface (the same bound as its style); the upper bound when it cannot be measured.
 */
function room(): number {
  const el = panel.value;
  const host = el?.parentElement;
  if (!el || !host || props.cover || props.sheet) return props.max;
  const kept = Number.parseFloat(getComputedStyle(el).getPropertyValue("--k-size-panel-md")) || 0;
  const gap = Number.parseFloat(getComputedStyle(el).marginInlineStart) || 0;
  return Math.max(props.min, host.clientWidth - kept - gap);
}
const clamp = (value: number) =>
  Math.round(Math.max(props.min, Math.min(props.max, room(), value)));

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

const focusTab = () =>
  panel.value?.querySelector<HTMLElement>('[role="tab"][aria-selected="true"]')?.focus();

// A sheet hides all else: opened, it takes the focus on its active tab.
watch(open, async (now) => {
  if (!now || !props.sheet) return;
  await nextTick();
  focusTab();
});

// What the open panel covers entirely goes inert (`cover`, `sheet`); beside the surface,
// it covers nothing.
const portal = injectPortalTarget();
let restore: (() => void) | null = null;
function release(): void {
  restore?.();
  restore = null;
}
watch(
  [open, panel, () => props.cover, () => props.sheet],
  ([now, el, cover, sheet]) => {
    // Released as soon as it closes, before its exit ends: the focus goes back at once.
    release();
    if (!now || !el || (!cover && !sheet)) return;
    const focused = document.activeElement;
    restore = inertOthers(el, sheet ? "scope" : "siblings", [document.querySelector(portal)]);
    // The focus was in what just went inert: it moves to the panel, not to `body`.
    if (focused instanceof HTMLElement && focused.closest("[inert]")) focusTab();
  },
  { flush: "post" },
);
onBeforeUnmount(release);

// The shade at the bottom edge of the tab panel while more lies below (`data-more`).
let stopCue: (() => void) | null = null;
watch(
  panel,
  (el) => {
    stopCue?.();
    stopCue = el ? scrollCue(el, ":scope > .k-side-panel__tabs > .k-tabs__panel") : null;
  },
  { flush: "post" },
);
onBeforeUnmount(() => stopCue?.());

/** The strip reopens the panel and moves the focus to its active tab. */
async function reopen(): Promise<void> {
  open.value = true;
  await nextTick();
  panel.value?.querySelector<HTMLElement>('[role="tab"][aria-selected="true"]')?.focus();
}

function onKeydown(event: KeyboardEvent): void {
  if (event.key === "Escape" && !event.defaultPrevented && !ownsEscape(event.target)) {
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

/** The widths in one click (WCAG 2.5.7), the current one checked. */
const presets = computed(() => widthPresets(props.min, props.max));
const widthItems = computed<MenuItem[]>(() =>
  presets.value.map((preset) => ({
    kind: "radio" as const,
    id: preset.id,
    label: t(
      preset.id === "narrow" ? "panelNarrow" : preset.id === "wide" ? "panelWide" : "panelStandard",
    ),
    checked: width.value === preset.width,
  })),
);
function onWidth(id: string): void {
  const preset = presets.value.find((p) => p.id === id);
  if (preset) width.value = preset.width;
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
    <component
      :is="props.sheet ? 'div' : 'aside'"
      v-if="open"
      v-bind="$attrs"
      :id="id"
      ref="panel"
      class="k-side-panel"
      :role="props.sheet ? 'dialog' : undefined"
      :aria-modal="props.sheet ? 'true' : undefined"
      :aria-label="props.label"
      :style="size"
      :data-cover="props.cover ? 'true' : undefined"
      :data-sheet="props.sheet ? 'true' : undefined"
      @keydown="onKeydown"
    >
      <!-- The back button of a sheet comes first, where it shows. -->
      <IconButton
        v-if="props.sheet"
        class="k-side-panel__back"
        icon="arrow-left"
        size="sm"
        :label="t('panelBack')"
        @click="close"
      />
      <div
        v-if="!props.cover && !props.sheet"
        ref="handle"
        class="k-panel-handle"
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
      <Tabs v-model="tab" :tabs="props.tabs" :label="t('panelTabs')" class="k-side-panel__tabs">
        <template v-for="item in props.tabs" :key="item.value" #[item.value]>
          <slot :name="item.value" />
        </template>
      </Tabs>
      <div v-if="!props.sheet" class="k-side-panel__actions">
        <Menu
          v-if="!props.cover"
          :label="t('panelWidth')"
          :items="widthItems"
          icon="move-horizontal"
          icon-only
          variant="ghost"
          size="sm"
          align="end"
          @select="onWidth"
        />
        <IconButton
          class="k-side-panel__close"
          icon="x"
          size="sm"
          :label="t('panelClose')"
          @click="close"
        />
      </div>
      <footer v-if="footerShown()" class="k-side-panel__footer">
        <slot name="footer" :tab="current" />
      </footer>
    </component>
  </Transition>
  <!-- The strip of the closed panel takes the track of the panel; none while it is open. -->
  <Transition name="k-side-panel-rail">
    <div v-if="props.rail && !open" ref="strip" class="k-side-panel-rail">
      <IconButton
        class="k-side-panel-rail__open"
        icon="chevron-left"
        size="sm"
        variant="secondary"
        :label="t('panelOpen')"
        @click="reopen"
      />
    </div>
  </Transition>
</template>

<style>
@layer components {
  /* Beside the surface, in the track its host gives it, at the height of its container;
   * never more than the container less the room of the surface. A gap before it, where the
   * outer half of the handle lies: the handle covers neither the surface nor its scroll
   * bar. Its outline is the line of the handle. */
  .k-side-panel {
    --k-side-panel-width: var(--k-size-panel-lg);

    position: relative;
    display: grid;
    grid-template-rows: minmax(0, 1fr) auto;
    grid-template-columns: minmax(0, 1fr);
    inline-size: min(var(--k-side-panel-width), var(--k-size-panel-room-inline));
    min-block-size: 0;
    margin-inline-start: var(--k-space-3);
    color: var(--k-text);
    background-color: var(--k-surface);
  }

  /* Closing, it leaves the flow at once: the surface takes its width back without waiting
   * for the end of the exit, which runs over it. */
  .k-side-panel-leave-active:not([data-cover], [data-sheet]) {
    position: absolute;
    inset-block: 0;
    inset-inline-end: 0;
  }

  /* Over the whole container (narrow window), out of the flow. */
  .k-side-panel[data-cover] {
    position: absolute;
    inset-block: 0;
    inset-inline-end: 0;
    z-index: var(--k-z-sticky);
    inline-size: 100%;
    margin-inline-start: 0;
  }

  /* A sheet: the whole window, over the frame; its back button at the start. A modal
   * layer, on their plane (the toasts under it, its menus over it). */
  .k-side-panel[data-sheet] {
    position: fixed;
    inset: 0;
    z-index: var(--k-z-modal);
    inline-size: 100%;
    margin-inline-start: 0;
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
   * render), the panel slides from its edge and fades; the surface takes its new width at
   * once, without animation. Interactive from its first frame, the focus placed at once. Vue's
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

  .k-side-panel__tabs {
    grid-template-rows: auto minmax(0, 1fr);
    gap: 0;
    min-block-size: 0;
  }

  /* Room at the end of the tab list for the close button. */
  .k-side-panel__tabs > .k-tabs__list {
    padding-inline: var(--k-space-2) var(--k-space-8);
  }

  /* Resizable, the width menu comes before it; the first tab clear of the inner half of
   * the handle. */
  .k-side-panel:not([data-cover], [data-sheet]) > .k-side-panel__tabs > .k-tabs__list {
    padding-inline-start: var(--k-space-3);
    padding-inline-end: calc(
      var(--k-control-height-sm) * 2 + var(--k-space-1) + var(--k-space-2) * 2
    );
  }

  .k-side-panel__tabs > .k-tabs__panel {
    min-block-size: 0;
    padding: var(--k-space-3);
    overflow: auto;
    background-color: var(--k-surface);
  }

  /* A scroll cue (relecture du 2026-10-26, reprise le 2026-10-27): a shade at the bottom
   * edge of the tab panel while more lies below (`data-more`, `scrollCue`). It sticks to
   * the edge above the content, encarts and their backgrounds included; its negative
   * margin keeps it out of the layout, and the pointer goes through it. */
  .k-side-panel__tabs > .k-tabs__panel[data-more]::after {
    position: sticky;
    inset-block-end: 0;
    z-index: calc(var(--k-z-sticky) + 2);
    display: block;
    block-size: var(--k-space-2);
    margin-block-start: calc(var(--k-space-2) * -1);
    margin-inline: calc(var(--k-space-3) * -1);
    pointer-events: none;
    content: "";
    background-image: linear-gradient(to top, var(--k-border-strong), transparent);
  }

  /* The width menu and the close button, at the end of the tab list. */
  .k-side-panel__actions {
    position: absolute;
    inset-block-start: calc((var(--k-control-height-lg) - var(--k-control-height-sm)) / 2);
    inset-inline-end: var(--k-space-2);
    display: flex;
    gap: var(--k-space-1);
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
