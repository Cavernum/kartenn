<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

<script setup lang="ts">
// A popover (a non-modal Dialog; design « Inventaire », line 16): the explanation of a
// value, the detail of a code, beside the control that opens it (Reka's Popover). The
// trigger (`trigger` slot, one control: a `Button`, an `IconButton`) says
// `aria-haspopup="dialog"` and `aria-expanded`. Open, the dialog is named by its title (a
// heading) and takes the focus itself, its body read from there, its controls next by Tab
// (no `aria-describedby`: a body of paragraphs and controls would be read twice, APG
// Dialog); Escape, the close button or a press outside closes it, the first two give
// the focus back to the trigger. Nothing around it is hidden or inert: it reads as if it
// followed its trigger in the tab order (Shift+Tab from its start goes back to the
// trigger, Tab past its end to what follows the trigger), and closes as the focus leaves
// it. Bounded by the viewport, its body scrolls (1.4.10), a tab stop while it does; it stays in the viewport even when
// its trigger is scrolled out of it (a cell of a grid scrolled sideways: `sticky`).
import { PopoverClose, PopoverContent, PopoverPortal, PopoverRoot, PopoverTrigger } from "reka-ui";
import { onBeforeUnmount, ref, useId, watch, type ComponentPublicInstance } from "vue";
import { injectPortalTarget } from "../../context.ts";
import { t } from "../../i18n.ts";
import IconButton from "../IconButton/IconButton.vue";
import { setRoom } from "../internal/room.ts";
import { tabOut } from "../internal/tabOrder.ts";

const props = withDefaults(
  defineProps<{
    /** Its title, which names the dialog. */
    title: string;
    side?: "top" | "right" | "bottom" | "left";
    align?: "start" | "center" | "end";
    /** Its width: `sm` (240 px), `md` (320 px). */
    size?: "sm" | "md";
    defaultOpen?: boolean;
  }>(),
  { side: "bottom", align: "start", size: "md" },
);
const open = defineModel<boolean>("open", { default: false });
if (props.defaultOpen) open.value = true;
const portal = injectPortalTarget();
const base = useId();
const titleId = `${base}-title`;
const trigger = ref<ComponentPublicInstance | null>(null);
const triggerEl = () => trigger.value?.$el as HTMLElement | undefined;
/** Left by Tab or Shift+Tab: where the focus goes once it is closed. */
let leaving: HTMLElement | undefined;

/**
 * A body that scrolls (a narrow or zoomed viewport, 1.4.10) is reached by Tab and scrolled
 * by the arrows (2.1.1): a tab stop, a group named by the title, while it overflows.
 */
const body = ref<HTMLElement | null>(null);
const scrolls = ref(false);
let watcher: ResizeObserver | undefined;
watch(body, (el) => {
  watcher?.disconnect();
  scrolls.value = false;
  if (!el) return;
  watcher = new ResizeObserver(() => (scrolls.value = el.scrollHeight > el.clientHeight));
  for (const part of [el, ...el.children]) watcher.observe(part);
});
onBeforeUnmount(() => watcher?.disconnect());

/**
 * Open, the focus goes to the dialog itself, which Reka names after its trigger: named
 * here after its title instead (Reka writes its own `aria-labelledby` over any prop; Vue
 * does not write it again while it does not change). Bounded by the viewport (1.4.10):
 * the room beside the trigger, set by CSSOM.
 */
function onOpenFocus(event: Event): void {
  // The event comes from Reka's wrapper around the dialog (the scope of its focus).
  leaving = undefined;
  const wrapper = event.target as HTMLElement;
  const el = wrapper.querySelector<HTMLElement>(":scope > [role='dialog']");
  if (!el) return;
  event.preventDefault();
  el.setAttribute("aria-labelledby", titleId);
  const rect = triggerEl()?.getBoundingClientRect();
  if (rect) setRoom(el, rect, "popover");
  el.focus({ preventScroll: true });
}

/**
 * Closed by Tab or Shift+Tab, the focus has gone on: Reka would bring it back to the
 * trigger (it says so twice, from its wrapper then from its scope).
 */
function onCloseFocus(event: Event): void {
  if (leaving) event.preventDefault();
}

/**
 * Tab past the end, Shift+Tab from the start: the popover closes, the focus goes on.
 * Heard before Reka's scope, which would take the focus round inside (`loop`).
 */
function onKey(event: KeyboardEvent): void {
  const from = triggerEl();
  const to = from && tabOut(event, event.currentTarget as HTMLElement, from);
  if (!to) return;
  event.preventDefault();
  event.stopPropagation();
  leaving = to;
  open.value = false;
  leaving.focus();
}
</script>

<template>
  <PopoverRoot v-model:open="open">
    <PopoverTrigger ref="trigger" class="k-popover__trigger" as-child
      ><slot name="trigger"
    /></PopoverTrigger>
    <PopoverPortal :to="portal">
      <PopoverContent
        class="k-popover k-popover__dialog"
        :data-size="props.size"
        :side="props.side"
        :align="props.align"
        :side-offset="4"
        :collision-padding="8"
        sticky="always"
        @open-auto-focus="onOpenFocus"
        @close-auto-focus="onCloseFocus"
        @keydown.capture="onKey"
      >
        <header class="k-popover__header">
          <h2 :id="titleId" class="k-popover__title">{{ props.title }}</h2>
          <PopoverClose as-child>
            <IconButton icon="x" :label="t('close')" size="sm" variant="ghost" />
          </PopoverClose>
        </header>
        <div
          ref="body"
          class="k-popover__body"
          v-bind="scrolls ? { tabindex: 0, role: 'group', 'aria-labelledby': titleId } : {}"
        >
          <slot />
        </div>
      </PopoverContent>
    </PopoverPortal>
  </PopoverRoot>
</template>

<style>
@layer components {
  .k-popover__dialog {
    --k-popover-size: var(--k-size-panel-md);

    /* A column: under a bounded height (no definite one), the body shrinks and scrolls; a
     * grid track of `1fr` would keep its content's height and spill out. */
    display: flex;
    flex-direction: column;
    gap: var(--k-space-2);
    inline-size: var(--k-popover-size);
    max-inline-size: var(--k-popover-room-inline, var(--k-popover-size));
    max-block-size: var(--k-popover-room-block, var(--k-size-dialog-sm));
    padding: var(--k-space-3);
  }

  /* Its trigger shows that it is open (a menu's turns its chevron). */
  .k-popover__trigger[aria-expanded="true"] {
    background-color: var(--k-active);
  }

  .k-popover__dialog[data-size="sm"] {
    --k-popover-size: var(--k-size-panel-sm);
  }

  .k-popover__header {
    display: flex;
    flex: none;
    gap: var(--k-space-2);
    align-items: center;
    justify-content: space-between;
    min-inline-size: 0;
  }

  /* A heading (browse mode), at the size of the text around it. */
  .k-popover__title {
    min-inline-size: 0;
    margin: 0;
    font-size: var(--k-text-md);
    line-height: var(--k-text-md-line);
    font-weight: var(--k-weight-semibold);
  }

  .k-popover__body {
    display: grid;
    flex: 1 1 auto;
    gap: var(--k-space-2);
    align-content: start;
    min-block-size: 0;
    overflow: auto;
  }

  .k-popover__body > :where(p) {
    margin: 0;
  }
}
</style>
