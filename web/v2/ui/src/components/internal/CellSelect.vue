<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

<script setup lang="ts">
// The editor of a cell whose value is one of a closed list (`options` of `DataGrid` and
// `TreeGrid`; spec 020, « Contrat d'édition », task 021-12g): Reka's Select (APG
// Select-Only Combobox, as `Select`), its button in the frame of the editor, without a
// `FormField` (the cell names it). The list opens as the editor takes the focus (Enter, F2
// or a double click on the cell): the arrows, Home, End and typing move in it; Enter (or a
// click, or Space) chooses and commits, Ctrl+Enter commits marked « now »; Tab and
// Shift+Tab choose, commit and go on along the row; Escape closes the list without
// choosing, the focus back on the button, the editor still open — the next Escape, heard
// by the grid, cancels. On the button, the list closed: Enter, Space, Down or Up open it
// again, typing chooses in place, Tab commits what it holds, Ctrl+Enter too, marked. The
// list is in the layer of floating layers: the focus going there is not the editor's
// leaving (`useCellEditor`, `aria-controls`). The look of the list is `Select/options.css`.
import {
  SelectContent,
  SelectItem,
  SelectItemIndicator,
  SelectItemText,
  SelectPortal,
  SelectRoot,
  SelectTrigger,
  SelectValue,
} from "reka-ui";
import {
  nextTick,
  onBeforeUnmount,
  onMounted,
  ref,
  shallowRef,
  useId,
  watch,
  type ComponentPublicInstance,
} from "vue";
import { injectPortalTarget } from "../../context.ts";
import Icon from "../Icon/Icon.vue";
import type { ListOption } from "../Select/select.ts";

const props = defineProps<{
  options: readonly ListOption[];
  /** The value the editor holds (its draft). */
  value: string;
  /** The name of the editor: the column and the row. */
  label: string;
  invalid?: boolean | undefined;
  describedBy?: string | undefined;
  /** The chosen label in fixed pitch (the column's `mono`). */
  mono?: boolean | undefined;
}>();
const emit = defineEmits<{
  /** A value held without committing (typing on the closed button). */
  update: [value: string];
  /** A value chosen in the list: commit it, the focus `move` columns along; `now`: marked. */
  done: [value: string, move: number, now: boolean];
}>();

// Reka takes no empty value for an option (the empty string is its placeholder): « no
// value » (the host's `""`, a value removed) stands under a key no host gives (a private-use code point).
const NONE = String.fromCharCode(0xe000);
const toReka = (value: string) => (value === "" ? NONE : value);
const fromReka = (value: unknown) => (value === NONE || typeof value !== "string" ? "" : value);

const portal = injectPortalTarget();
const id = useId();
const open = ref(false);
const trigger = ref<ComponentPublicInstance | null>(null);
// Closed, the list stays in place (as `Select`): it goes to the layer while it shows.
const teleported = ref(false);
/** The list opens once, as the editor takes the focus; later by its keys. */
let opened = false;
/** Gone (the editor closed): the list's leaving gives the focus to no one. */
let leaving = false;
onBeforeUnmount(() => (leaving = true));

function onOpen(next: boolean): void {
  open.value = next;
  if (next) teleported.value = true;
}
function onFocus(): void {
  if (opened) return;
  opened = true;
  onOpen(true);
}

/** Ctrl+Enter (⌘+Enter) chose: the commit marked « now ». */
let now = false;
function choose(value: string, move: number): void {
  const marked = now;
  now = false;
  open.value = false;
  emit("done", value, move, marked);
}
/** Enter, Space or a click on an option (Reka's `select`, before it takes the value). */
function onSelect(option: ListOption): void {
  if (!option.disabled) choose(option.value, 0);
}

/** In the list: Ctrl+Enter marks the choice; Tab chooses and goes on (Reka keeps Tab). */
function onListKey(event: KeyboardEvent): void {
  // Every key says it again: a Ctrl+Enter that chose nothing (a disabled option) never
  // marks a later choice.
  now = event.key === "Enter" && (event.ctrlKey || event.metaKey);
  if (event.key !== "Tab" || event.ctrlKey || event.altKey || event.metaKey) return;
  event.preventDefault();
  const list = event.currentTarget as HTMLElement;
  const focused = (document.activeElement as HTMLElement | null)?.closest("[role='option']");
  const index = focused ? [...list.querySelectorAll("[role='option']")].indexOf(focused) : -1;
  const option = props.options[index];
  choose(option && !option.disabled ? option.value : props.value, event.shiftKey ? -1 : 1);
}

/** On the closed button, Ctrl+Enter commits what it holds, marked (Reka would open). */
function onButtonKey(event: KeyboardEvent): void {
  if (event.key !== "Enter" || !(event.ctrlKey || event.metaKey) || open.value) return;
  event.preventDefault();
  now = true;
  choose(props.value, 0);
}

/** Closed without a choice: the focus back on the button (Reka); gone: to the grid's cell. */
function onClosed(event: Event): void {
  teleported.value = false;
  if (leaving) event.preventDefault();
}

// The list stands against the frame of the editor (the button is inside its padding), at
// least as wide as it, measured when it opens (as `Select`).
const frame = shallowRef<HTMLElement | undefined>(undefined);
onMounted(() => {
  frame.value = (trigger.value?.$el as HTMLElement | undefined)?.parentElement ?? undefined;
});
watch(open, async (shown) => {
  if (!shown) return;
  await nextTick();
  const button = trigger.value?.$el as HTMLElement | undefined;
  if (!button) return;
  const list = document.getElementById(button.getAttribute("aria-controls") ?? "");
  const width = (frame.value ?? button).offsetWidth;
  list?.style.setProperty("--k-list-anchor-width", `${width}px`);
});
</script>

<template>
  <SelectRoot
    :model-value="toReka(props.value)"
    :open="open"
    @update:open="onOpen"
    @update:model-value="(next: unknown) => emit('update', fromReka(next))"
  >
    <SelectTrigger
      ref="trigger"
      v-bind="frame ? { reference: frame } : {}"
      class="k-cell-select"
      :aria-label="props.label"
      :aria-invalid="props.invalid ? 'true' : undefined"
      :aria-describedby="props.describedBy"
      @focus="onFocus"
      @keydown.capture="onButtonKey"
    >
      <!-- A value held out of the list (an old key) is still shown, as the cell wrote it. -->
      <SelectValue
        class="k-cell-select__value"
        :class="{ 'k-mono': props.mono }"
        :placeholder="props.value"
      />
      <Icon name="chevron-down" class="k-cell-select__chevron" />
    </SelectTrigger>
    <SelectPortal :to="portal" :disabled="!teleported">
      <SelectContent
        class="k-list"
        position="popper"
        align="start"
        :side-offset="4"
        :aria-label="props.label"
        @keydown.capture="onListKey"
        @close-auto-focus="onClosed"
        @pointerdown.capture="now = false"
      >
        <!-- As `Select`: an option with a detail is named by both. -->
        <SelectItem
          v-for="(option, index) in props.options"
          :key="option.value"
          class="k-list__option"
          :value="toReka(option.value)"
          :text-value="option.label"
          :disabled="option.disabled === true"
          v-bind="
            option.detail
              ? { 'aria-labelledby': `${id}-option-${index} ${id}-option-${index}-detail` }
              : {}
          "
          @select="onSelect(option)"
        >
          <span class="k-list__check">
            <SelectItemIndicator><Icon name="check" /></SelectItemIndicator>
          </span>
          <Icon v-if="option.icon" :name="option.icon" class="k-list__icon" />
          <SelectItemText
            v-bind="option.detail ? { id: `${id}-option-${index}` } : {}"
            class="k-list__text"
            :class="{ 'k-mono': option.mono }"
            >{{ option.label }}</SelectItemText
          >
          <code v-if="option.detail" :id="`${id}-option-${index}-detail`" class="k-list__detail">{{
            option.detail
          }}</code>
        </SelectItem>
      </SelectContent>
    </SelectPortal>
  </SelectRoot>
</template>

<style>
@layer components {
  /* The button fills the frame of the editor, as its input would; the frame has the ring. */
  .k-cell-select {
    display: flex;
    flex: 1;
    gap: var(--k-space-1);
    align-items: center;
    align-self: stretch;
    min-inline-size: 0;
    padding: 0;
    font: inherit;
    color: inherit;
    text-align: start;
    cursor: default;
    background: none;
    border: 0;
    outline: 0;
  }

  .k-field-frame:has(> .k-cell-select):focus-within {
    outline: var(--k-focus-width) solid var(--k-focus-ring);
    outline-offset: var(--k-focus-offset);
  }

  .k-cell-select__value {
    flex: 1;
    min-inline-size: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .k-cell-select__value.k-mono {
    font-family: var(--k-font-mono);
    font-size: var(--k-text-mono);
  }

  .k-cell-select__chevron {
    flex: none;
    color: var(--k-text-muted);
    transition-property: rotate;
    transition-duration: var(--k-duration-short);
    transition-timing-function: var(--k-ease-standard);
  }

  .k-cell-select[aria-expanded="true"] .k-cell-select__chevron {
    rotate: var(--k-turn-half);
  }
}
</style>
