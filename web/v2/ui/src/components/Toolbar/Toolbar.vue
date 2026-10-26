<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

<script setup lang="ts">
// A toolbar (APG Toolbar; design « Inventaire », line 33; « Structure avant style »;
// REQ-020-36): one stop in the tab order, the arrows (and Home, End) between its actions.
// It never wraps: a ResizeObserver measures it, and the actions that no longer fit go,
// from the lowest priority and from the end, into a « … » menu at its end. A refused
// action stays, focusable, with its reason (024/M14).
import { computed, nextTick, onBeforeUnmount, onMounted, onUpdated, ref, watch } from "vue";
import { t } from "../../i18n.ts";
import Button from "../Button/Button.vue";
import IconButton from "../IconButton/IconButton.vue";
import { isRtl } from "../internal/gridKeys.ts";
import type { ControlSize } from "../layout.ts";
import Menu from "../Menu/Menu.vue";
import type { MenuItem } from "../Menu/menu.ts";
import { floorWidth, foldActions, type ToolbarAction } from "./toolbar.ts";

const props = withDefaults(
  defineProps<{
    label: string;
    actions: ToolbarAction[];
    size?: ControlSize;
    /** Where the actions sit in the bar. */
    align?: "start" | "end";
    /**
     * Entries that always live in « … », after the folded actions: one menu of more
     * actions per bar, never two buttons « … » side by side (relecture du 2026-10-26).
     */
    more?: MenuItem[];
  }>(),
  { size: "md", align: "start", more: () => [] },
);
const emit = defineEmits<{ action: [id: string] }>();

const bar = ref<HTMLElement | null>(null);
const folded = ref<string[]>([]);
const shown = computed(() => props.actions.filter((a) => !folded.value.includes(a.id)));
const foldedItems = computed<MenuItem[]>(() => {
  const items: MenuItem[] = props.actions
    .filter((a) => folded.value.includes(a.id))
    .map((a) => ({
      id: a.id,
      label: a.label,
      ...(a.icon ? { icon: a.icon } : {}),
      ...(a.shortcut ? { shortcut: a.shortcut } : {}),
      ...(a.disabled ? { disabled: true } : {}),
      ...(a.reason ? { reason: a.reason } : {}),
      ...(a.forbidden ? { forbidden: a.forbidden } : {}),
    }));
  if (items.length && props.more.length) items.push({ kind: "separator" });
  return [...items, ...props.more];
});
const blocked = (a: ToolbarAction) => ({
  ...(a.forbidden ? { forbidden: a.forbidden } : {}),
  ...(a.reason ? { reason: a.reason } : {}),
});

/** The measured width of each action, taken while it is in the bar. */
const widths = new Map<string, number>();
const buttons = (selector: string) => [
  ...(bar.value?.querySelectorAll<HTMLElement>(`:scope > .k-button${selector}`) ?? []),
];
/** The items the focus moves over: a natively disabled one is out of the tab order. */
const items = () => buttons(":not(:disabled)");

function measure(): void {
  for (const el of buttons("")) {
    const id = el.dataset.action;
    if (id) widths.set(id, el.getBoundingClientRect().width);
  }
}

function fold(): void {
  const el = bar.value;
  if (!el) return;
  measure();
  const style = getComputedStyle(el);
  const gap = parseFloat(style.columnGap) || 0;
  const more = parseFloat(style.getPropertyValue("--k-toolbar-more")) || 0;
  // With entries of its own, « … » is always there: its room is taken first.
  const always = props.more.length > 0;
  const room = always ? el.clientWidth - more - gap : el.clientWidth;
  const next = foldActions(props.actions, widths, room, gap, always ? 0 : more);
  if (next.join() !== folded.value.join()) folded.value = next;
  // The narrowest it folds to, for a host that lays it beside a title (CSSOM, REQ-020-14).
  const floor = Math.ceil(
    always
      ? floorWidth(props.actions, widths, gap, 0) + gap + more
      : floorWidth(props.actions, widths, gap, more),
  );
  el.style.setProperty("--k-toolbar-floor", `${floor}px`);
}

let observer: ResizeObserver | undefined;
onMounted(() => {
  fold();
  observer = new ResizeObserver(() => fold());
  observer.observe(bar.value!);
});
onBeforeUnmount(() => observer?.disconnect());
// New actions: all back in the bar to be measured, then folded again.
watch(
  () => props.actions,
  async () => {
    folded.value = [];
    await nextTick();
    fold();
  },
  { deep: true },
);

/** The roving focus: the index of the item in the tab order. */
const current = ref(0);
function rove(): void {
  const all = items();
  current.value = Math.min(current.value, Math.max(0, all.length - 1));
  all.forEach((el, i) => (el.tabIndex = i === current.value ? 0 : -1));
}
onMounted(rove);
onUpdated(rove);

function onFocusin(event: FocusEvent): void {
  const i = items().indexOf(event.target as HTMLElement);
  if (i >= 0 && i !== current.value) {
    current.value = i;
    rove();
  }
}

function onKeydown(event: KeyboardEvent): void {
  const all = items();
  const i = all.indexOf(event.target as HTMLElement);
  if (i < 0) return;
  const next = isRtl(bar.value!) ? "ArrowLeft" : "ArrowRight";
  const previous = isRtl(bar.value!) ? "ArrowRight" : "ArrowLeft";
  const to =
    event.key === next
      ? Math.min(all.length - 1, i + 1)
      : event.key === previous
        ? Math.max(0, i - 1)
        : event.key === "Home"
          ? 0
          : event.key === "End"
            ? all.length - 1
            : null;
  if (to === null) return;
  event.preventDefault();
  current.value = to;
  rove();
  all[to]!.focus();
}
</script>

<template>
  <div
    ref="bar"
    class="k-toolbar"
    role="toolbar"
    :aria-label="props.label"
    :data-size="props.size"
    :data-align="props.align"
    @keydown="onKeydown"
    @focusin="onFocusin"
  >
    <template v-for="a in shown" :key="a.id">
      <IconButton
        v-if="a.iconOnly && a.icon"
        :data-action="a.id"
        :icon="a.icon"
        :label="a.label"
        :variant="a.variant ?? 'ghost'"
        :size="props.size"
        :disabled="a.disabled ?? false"
        v-bind="blocked(a)"
        @click="emit('action', a.id)"
      />
      <Button
        v-else
        :data-action="a.id"
        :icon="a.icon"
        :variant="a.variant ?? 'secondary'"
        :size="props.size"
        :shortcut="a.shortcut"
        :disabled="a.disabled ?? false"
        v-bind="blocked(a)"
        @click="emit('action', a.id)"
        >{{ a.label }}</Button
      >
    </template>
    <Menu
      v-if="folded.length || props.more.length"
      :label="t('moreActions')"
      icon="ellipsis"
      icon-only
      align="end"
      variant="ghost"
      :size="props.size"
      :items="foldedItems"
      @select="emit('action', $event)"
    />
  </div>
</template>

<style>
@layer components {
  /* One line, always: what does not fit goes into « … ». */
  .k-toolbar {
    --k-toolbar-more: var(--k-control-height-md);

    display: flex;
    flex-wrap: nowrap;
    gap: var(--k-space-2);
    align-items: center;
    min-inline-size: 0;
  }

  .k-toolbar[data-size="sm"] {
    --k-toolbar-more: var(--k-control-height-sm);

    gap: var(--k-space-1);
  }

  .k-toolbar[data-size="lg"] {
    --k-toolbar-more: var(--k-control-height-lg);
  }

  .k-toolbar[data-align="end"] {
    justify-content: end;
  }
}
</style>
