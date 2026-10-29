<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

<script setup lang="ts">
// The bottom panel of the frame (design « Adaptation à la largeur », « Panneau du bas »,
// 2026-10-26), sibling of `SidePanel` for wide content: the detail of an object in several
// columns, a linked table. A non-modal `complementary` region with tabs, under the surface:
// the host lays it in an `auto` row of its grid, at the end, and the surface loses height,
// never width; the panel covers nothing. The host is a positioned size container
// (`position: relative`, `container-type: size`): the panel never takes more than its
// container less `--k-size-panel-sm` for the surface, and while it closes it leaves the
// flow at once. 320 px high by default, resized by a handle at its block start (APG Window
// Splitter, horizontal): Up and Down arrows by a step (Up raises it), Home and End to the
// bounds, the pointer by dragging — or in one click, without dragging (WCAG 2.5.7), by the
// menu « Hauteur du panneau » beside the close button: « Basse » (the lower bound),
// « Standard » (320 px), « Haute » (the upper bound), the current one checked. The line of
// the handle is the outline of the panel (`.k-panel-handle`, components/shared.css).
// Escape closes it and gives the focus back to what opened it; closed, it leaves nothing
// behind (the host reopens it, from a selection). The footer shows only when the host
// fills it for the tab in front. With `sheet` (compact frame), it is the same sheet as the
// side panel: the whole window, a back button at its start, a modal dialog that takes the
// focus when it opens and makes everything beside it `inert`, up to the nearest
// `[data-k-modal-scope]` (or `body`).
import { computed, nextTick, onBeforeUnmount, ref, useId, useSlots, watch } from "vue";
import { injectPortalTarget } from "../../context.ts";
import { t } from "../../i18n.ts";
import IconButton from "../IconButton/IconButton.vue";
import { inertOthers } from "../internal/inertOthers.ts";
import { filled, ownsEscape, scrollCue } from "../internal/panel.ts";
import Menu from "../Menu/Menu.vue";
import type { MenuItem } from "../Menu/menu.ts";
import Tabs from "../Tabs/Tabs.vue";
import type { TabItem } from "../Tabs/tabs.ts";
import { PANEL_HEIGHT_STANDARD, heightPresets } from "./bottomPanel.ts";

defineOptions({ inheritAttrs: false });

const props = withDefaults(
  defineProps<{
    /** The name of the region. */
    label: string;
    /** The id of the region, for the `aria-controls` of what opens it. */
    id?: string | undefined;
    tabs: TabItem[];
    /** Bounds of the height, in CSS pixels. */
    min?: number;
    max?: number;
    /** The step of an arrow on the handle. */
    step?: number;
    /**
     * A sheet over the whole window (compact frame): a back button at the start instead of
     * the cross, no handle, the focus moved to its active tab when it opens.
     */
    sheet?: boolean;
    /**
     * A `SidePanel` open in the flow at its end (beside the host, not `cover` nor `sheet`):
     * the panel runs on under the gap kept before that panel, so that the two meet on the
     * line of its handle; its content stays clear of the outer half of that handle.
     * Ignored with `sheet`.
     */
    besidePanel?: boolean;
  }>(),
  { id: undefined, min: 160, max: 640, step: 16 },
);
const open = defineModel<boolean>("open", { default: true });
/** The active tab. */
const tab = defineModel<string>("tab", { default: "" });
/** The height of the panel, in CSS pixels. */
const height = defineModel<number>("height", { default: PANEL_HEIGHT_STANDARD });
const emit = defineEmits<{ close: [] }>();
const slots = useSlots();

/** The tab in front, for the footer. */
const current = computed(() => tab.value || props.tabs[0]?.value);

/** The footer shows only when the host fills it for the tab in front: no empty band. */
const footerShown = (): boolean => !!slots.footer && filled(slots.footer({ tab: current.value }));

const generated = useId();
const id = computed(() => props.id ?? generated);
const panel = ref<HTMLElement | null>(null);
const size = computed(() => ({ "--k-bottom-panel-height": `${height.value}px` }));

/**
 * The highest the panel shows under the surface: its container less the room kept for the
 * surface (the same bound as its style); the upper bound when it cannot be measured.
 */
function room(): number {
  const el = panel.value;
  const host = el?.parentElement;
  if (!el || !host || props.sheet) return props.max;
  const style = getComputedStyle(el);
  const kept = Number.parseFloat(style.getPropertyValue("--k-size-panel-sm")) || 0;
  const gap = Number.parseFloat(style.marginBlockStart) || 0;
  return Math.max(props.min, host.clientHeight - kept - gap);
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
  opener = null;
}

const focusTab = () =>
  panel.value?.querySelector<HTMLElement>('[role="tab"][aria-selected="true"]')?.focus();

// A sheet hides all else: opened, it takes the focus on its active tab, and what lies beside
// it goes inert; under the surface, it hides nothing.
const portal = injectPortalTarget();
let restore: (() => void) | null = null;
function release(): void {
  restore?.();
  restore = null;
}
watch(
  [open, panel, () => props.sheet],
  async ([now, el, sheet], previous) => {
    release();
    if (!now || !el || !sheet) return;
    const focused = document.activeElement;
    restore = inertOthers(el, "scope", [document.querySelector(portal)]);
    if (!previous?.[0] || (focused instanceof HTMLElement && focused.closest("[inert]"))) {
      await nextTick();
      focusTab();
    }
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
    stopCue = el ? scrollCue(el, ":scope > .k-bottom-panel__tabs > .k-tabs__panel") : null;
  },
  { flush: "post" },
);
onBeforeUnmount(() => stopCue?.());

function onKeydown(event: KeyboardEvent): void {
  if (event.key === "Escape" && !event.defaultPrevented && !ownsEscape(event.target)) {
    event.preventDefault();
    void close();
  }
}

/** The handle sits at the block start: Up raises the panel, Down lowers it. */
function onHandleKeydown(event: KeyboardEvent): void {
  const next =
    event.key === "ArrowUp"
      ? height.value + props.step
      : event.key === "ArrowDown"
        ? height.value - props.step
        : event.key === "Home"
          ? props.min
          : event.key === "End"
            ? props.max
            : null;
  if (next === null) return;
  event.preventDefault();
  height.value = clamp(next);
}

/** The heights in one click (WCAG 2.5.7), the current one checked. */
const presets = computed(() => heightPresets(props.min, props.max));
const heightItems = computed<MenuItem[]>(() =>
  presets.value.map((preset) => ({
    kind: "radio" as const,
    id: preset.id,
    label: t(
      preset.id === "low" ? "panelLow" : preset.id === "high" ? "panelHigh" : "panelStandard",
    ),
    checked: height.value === preset.height,
  })),
);
function onHeight(id: string): void {
  const preset = presets.value.find((p) => p.id === id);
  if (preset) height.value = clamp(preset.height);
}

let drag: { y: number; height: number } | null = null;
function onPointerdown(event: PointerEvent): void {
  if (event.button !== 0) return;
  (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
  drag = { y: event.clientY, height: height.value };
}
function onPointermove(event: PointerEvent): void {
  if (drag) height.value = clamp(drag.height + drag.y - event.clientY);
}
function onPointerup(event: PointerEvent): void {
  if (!drag) return;
  (event.currentTarget as HTMLElement).releasePointerCapture(event.pointerId);
  drag = null;
}
</script>

<template>
  <Transition name="k-bottom-panel">
    <component
      :is="props.sheet ? 'div' : 'aside'"
      v-if="open"
      v-bind="$attrs"
      :id="id"
      ref="panel"
      class="k-bottom-panel"
      :role="props.sheet ? 'dialog' : undefined"
      :aria-modal="props.sheet ? 'true' : undefined"
      :aria-label="props.label"
      :style="size"
      :data-sheet="props.sheet ? 'true' : undefined"
      :data-beside-panel="props.besidePanel && !props.sheet ? 'true' : undefined"
      @keydown="onKeydown"
    >
      <!-- The back button of a sheet comes first, where it shows. -->
      <IconButton
        v-if="props.sheet"
        class="k-bottom-panel__back"
        icon="arrow-left"
        size="sm"
        :label="t('panelBack')"
        @click="close"
      />
      <div
        v-if="!props.sheet"
        class="k-panel-handle"
        role="separator"
        tabindex="0"
        aria-orientation="horizontal"
        :aria-label="t('panelResize')"
        :aria-controls="id"
        :aria-valuenow="height"
        :aria-valuemin="props.min"
        :aria-valuemax="props.max"
        @keydown="onHandleKeydown"
        @pointerdown="onPointerdown"
        @pointermove="onPointermove"
        @pointerup="onPointerup"
        @pointercancel="onPointerup"
      ></div>
      <Tabs v-model="tab" :tabs="props.tabs" :label="t('panelTabs')" class="k-bottom-panel__tabs">
        <template v-for="item in props.tabs" :key="item.value" #[item.value]>
          <slot :name="item.value" />
        </template>
      </Tabs>
      <div v-if="!props.sheet" class="k-bottom-panel__actions">
        <Menu
          :label="t('panelHeight')"
          :items="heightItems"
          icon="move-vertical"
          icon-only
          variant="ghost"
          size="sm"
          align="end"
          @select="onHeight"
        />
        <IconButton
          class="k-bottom-panel__close"
          icon="x"
          size="sm"
          :label="t('panelClose')"
          @click="close"
        />
      </div>
      <footer v-if="footerShown()" class="k-bottom-panel__footer">
        <slot name="footer" :tab="current" />
      </footer>
    </component>
  </Transition>
</template>

<style>
@layer components {
  /* Under the surface, in the row its host gives it, the width of its container; never
   * higher than the container less the room of the surface. A gap above it, where the
   * outer half of the handle lies. Its outline is the line of the handle. */
  .k-bottom-panel {
    --k-bottom-panel-height: var(--k-size-panel-md);

    position: relative;
    display: grid;
    grid-template-rows: minmax(0, 1fr) auto;
    grid-template-columns: minmax(0, 1fr);
    block-size: min(var(--k-bottom-panel-height), var(--k-size-panel-room-block));
    min-inline-size: 0;
    margin-block-start: var(--k-space-3);
    color: var(--k-text);
    background-color: var(--k-surface);
  }

  /* Closing, it leaves the flow at once: the surface takes its height back. */
  .k-bottom-panel-leave-active:not([data-sheet]) {
    position: absolute;
    inset-block-end: 0;
    inset-inline: 0;
  }

  /* A sheet: the whole window, over the frame; its back button at the start. */
  .k-bottom-panel[data-sheet] {
    position: fixed;
    inset: 0;
    z-index: var(--k-z-overlay);
    block-size: auto;
    margin-block-start: 0;
  }

  .k-bottom-panel[data-sheet] > .k-bottom-panel__tabs > .k-tabs__list {
    padding-inline: var(--k-space-8) var(--k-space-2);
  }

  .k-bottom-panel__back {
    position: absolute;
    inset-block-start: calc((var(--k-control-height-lg) - var(--k-control-height-sm)) / 2);
    inset-inline-start: var(--k-space-2);
  }

  .k-bottom-panel__back:dir(rtl) .k-icon {
    scale: -1 1;
  }

  /* Motion (design « Mouvement »): opened or closed (never at the first render), the panel
   * rises from its edge and fades; the surface takes its new height at once. */
  .k-bottom-panel-enter-active {
    transition-property: opacity, translate;
    transition-duration: max(var(--k-duration-medium), var(--k-duration-fade));
    transition-timing-function: var(--k-ease-standard);
  }

  .k-bottom-panel-leave-active {
    pointer-events: none;
    transition-property: opacity, translate;
    transition-duration: max(var(--k-duration-short), var(--k-duration-fade));
    transition-timing-function: var(--k-ease-exit);
  }

  .k-bottom-panel-enter-from,
  .k-bottom-panel-leave-to {
    opacity: 0;
    translate: 0 var(--k-motion-shift-lg);
  }

  .k-bottom-panel__tabs {
    grid-template-rows: auto minmax(0, 1fr);
    gap: 0;
    min-block-size: 0;
  }

  /* Room at the end of the tab list for the height menu and the close button; above, the
   * inner half of the handle. */
  .k-bottom-panel:not([data-sheet]) > .k-bottom-panel__tabs > .k-tabs__list {
    padding-inline: var(--k-space-3)
      calc(var(--k-control-height-sm) * 2 + var(--k-space-1) + var(--k-space-2) * 2);
  }

  .k-bottom-panel__tabs > .k-tabs__panel {
    min-block-size: 0;
    padding: var(--k-space-3);
    overflow: auto;
    background-color: var(--k-surface);
  }

  /* A scroll cue, as in the side panel: a shade at the bottom edge while more lies below,
   * above the content, out of the layout and of the pointer's way. */
  .k-bottom-panel__tabs > .k-tabs__panel[data-more]::after {
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

  /* The height menu and the close button, at the end of the tab list. */
  .k-bottom-panel__actions {
    position: absolute;
    inset-block-start: calc((var(--k-control-height-lg) - var(--k-control-height-sm)) / 2);
    inset-inline-end: var(--k-space-2);
    display: flex;
    gap: var(--k-space-1);
  }

  .k-bottom-panel__footer {
    display: flex;
    flex-wrap: wrap;
    gap: var(--k-space-2);
    align-items: center;
    justify-content: end;
    padding: var(--k-space-2) var(--k-space-3);
    border-block-start: var(--k-border-width) solid var(--k-border);
  }

  /* Beside a side panel open at its end (`besidePanel`): it runs on under the gap kept
   * before that panel (its `margin-inline-start`, where the outer half of its handle lies),
   * so that no strip of the page shows between the two panels: they meet on the line of the
   * handle, as either meets the strip of the closed side panel (retour du 2026-10-29). Its
   * handle, tab list and footer run on to that line; its tab panel, actions and footer
   * content stay clear of the handle. */
  .k-bottom-panel[data-beside-panel] {
    margin-inline-end: calc(var(--k-space-3) * -1);
  }

  .k-bottom-panel[data-beside-panel] > .k-bottom-panel__tabs > .k-tabs__list {
    padding-inline-end: calc(
      var(--k-control-height-sm) * 2 + var(--k-space-1) + var(--k-space-2) * 2 + var(--k-space-3)
    );
  }

  .k-bottom-panel[data-beside-panel] > .k-bottom-panel__tabs > .k-tabs__panel {
    margin-inline-end: var(--k-space-3);
  }

  .k-bottom-panel[data-beside-panel] > .k-bottom-panel__actions {
    inset-inline-end: calc(var(--k-space-2) + var(--k-space-3));
  }

  .k-bottom-panel[data-beside-panel] > .k-bottom-panel__footer {
    padding-inline-end: calc(var(--k-space-3) * 2);
  }
}
</style>
