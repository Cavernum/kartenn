<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

<script setup lang="ts">
// The command palette (APG Combobox with list autocomplete and automatic selection, in a
// modal Dialog; design « Inventaire », line 19; spec 021, REQ-021-3): Ctrl+K opens it (the
// shell's shortcut, or a control of the host that sets the model), the focus in its field;
// the rest of the page is hidden (`aria-modal`, written: Reka does not), Tab stays in it.
// Typing filters (the host filters, `query`, and gives the groups; Reka filters nothing,
// `ignore-filter`), the first entry highlighted, so that Enter runs the best one (« 10.1. »,
// Entrée → the object); Down and Up move (the highlighted option is the field's
// `aria-activedescendant` and the selected one, `aria-selected`), Enter runs it and
// closes; Escape closes; the focus goes back to what had it. What the list holds (its
// count, loading, nothing) is said by a polite region present before it. A shortcut of
// an entry is shown at its end as in `Menu`: a combination also in `aria-keyshortcuts`,
// a sequence (« g puis p ») in the entry's description. No ComboboxViewport (it injects a
// <style>, REQ-020-33).
import {
  ComboboxContent,
  ComboboxGroup,
  ComboboxInput,
  ComboboxItem,
  ComboboxLabel,
  ComboboxRoot,
  DialogContent,
  DialogOverlay,
  DialogPortal,
  DialogRoot,
  DialogTitle,
} from "reka-ui";
import {
  computed,
  nextTick,
  ref,
  useId,
  watch,
  watchPostEffect,
  type ComponentPublicInstance,
} from "vue";
import { injectPortalTarget } from "../../context.ts";
import { t } from "../../i18n.ts";
import Icon from "../Icon/Icon.vue";
import { useOpener } from "../internal/modal.ts";
import Kbd from "../Kbd/Kbd.vue";
import ShortcutHint from "../ShortcutHint/ShortcutHint.vue";
import { shortcutAria, shortcutSteps } from "../ShortcutHint/shortcutHint.ts";
import type { PaletteGroup, PaletteItem } from "./palette.ts";

const props = defineProps<{ groups: PaletteGroup[]; loading?: boolean }>();
const open = defineModel<boolean>("open", { default: false });
const query = defineModel<string>("query", { default: "" });
const emit = defineEmits<{ select: [id: string] }>();
const portal = injectPortalTarget();
const base = useId();
const onCloseFocus = useOpener(open);
// No entry: said outside the list (a listbox without options is invalid).
const empty = computed(() => props.groups.every((group) => group.items.length === 0));
const count = computed(() => props.groups.reduce((n, group) => n + group.items.length, 0));
/** What the list holds, in words (the polite region). */
const said = computed(() => {
  if (props.loading && empty.value) return t("loading");
  if (empty.value) return t("noResults");
  return `${count.value} ${t(count.value === 1 ? "resultOne" : "resultMany")}`;
});
// Reka leaves `aria-controls` empty with an inline list: it is read from the list each
// time the list is mounted (the dialog opens).
const list = ref<ComponentPublicInstance | null>(null);
const listId = ref<string | undefined>(undefined);
watchPostEffect(() => {
  listId.value = (list.value?.$el as HTMLElement | undefined)?.id || undefined;
});

const isSequence = (item: PaletteItem) =>
  !!item.shortcut && shortcutSteps(item.shortcut).length > 1;
const keysId = (item: PaletteItem) => `${base}-${item.id}-keys`;

/**
 * The highlighted option is the selected one (APG Combobox): Reka binds `aria-selected`
 * to a value taken, which the palette has not; it is set from the highlight once
 * rendered, and again as the highlight or the entries change.
 */
const highlighted = ref<unknown>();
watch(
  [highlighted, () => props.groups, listId],
  async () => {
    await nextTick();
    const el = listId.value ? document.getElementById(listId.value) : null;
    for (const option of el?.querySelectorAll("[role='option']") ?? [])
      option.setAttribute("aria-selected", String(option.hasAttribute("data-highlighted")));
  },
  { flush: "post" },
);

/**
 * Tab and Shift+Tab stay in the field, the one control of the dialog (its trap would take
 * them round to it; Reka's list, focusable by script, would take them otherwise). Escape
 * closes the dialog: the list, always open, is a layer of Reka's that would take the key.
 */
function onKeydown(event: KeyboardEvent): void {
  if (event.ctrlKey || event.altKey || event.metaKey || event.isComposing) return;
  if (event.key === "Tab") event.preventDefault();
  else if (event.key === "Escape" && !event.defaultPrevented) {
    event.preventDefault();
    open.value = false;
  }
}

function onSelect(id: string): void {
  emit("select", id);
  open.value = false;
}
</script>

<template>
  <DialogRoot v-model:open="open">
    <DialogPortal :to="portal">
      <DialogOverlay class="k-dialog__overlay" />
      <DialogContent
        class="k-palette"
        aria-modal="true"
        v-bind="{ 'aria-describedby': undefined }"
        @close-auto-focus="onCloseFocus"
      >
        <DialogTitle class="k-visually-hidden">{{ t("palette") }}</DialogTitle>
        <ComboboxRoot
          :open="true"
          ignore-filter
          :reset-search-term-on-blur="false"
          class="k-palette__combobox"
          @highlight="highlighted = $event"
        >
          <div class="k-palette__search">
            <Icon
              :name="props.loading ? 'loader-circle' : 'search'"
              :class="{ 'k-spinner': props.loading }"
            />
            <ComboboxInput
              v-model="query"
              class="k-palette__input"
              :placeholder="t('paletteSearch')"
              :aria-label="t('paletteSearch')"
              :aria-controls="listId"
              :aria-expanded="empty ? 'false' : 'true'"
              :aria-busy="props.loading ? 'true' : undefined"
              v-bind="empty ? { 'aria-activedescendant': undefined } : {}"
              auto-focus
              @keydown="onKeydown"
            />
            <Kbd :keys="['Escape']" aria-hidden="true" />
          </div>
          <!-- Present before the list, filled as it changes: its count, loading, nothing. -->
          <span class="k-visually-hidden" role="status" aria-live="polite">{{ said }}</span>
          <p v-if="empty" class="k-palette__empty" aria-hidden="true">{{ said }}</p>
          <ComboboxContent
            v-else
            ref="list"
            class="k-palette__list"
            position="inline"
            :aria-label="t('palette')"
          >
            <ComboboxGroup
              v-for="group in props.groups"
              :key="group.label"
              class="k-palette__group"
            >
              <ComboboxLabel class="k-palette__label">{{ group.label }}</ComboboxLabel>
              <ComboboxItem
                v-for="item in group.items"
                :key="item.id"
                :value="item.id"
                class="k-palette__item"
                :aria-keyshortcuts="item.shortcut ? shortcutAria(item.shortcut) : undefined"
                :aria-describedby="isSequence(item) ? keysId(item) : undefined"
                @select="onSelect(item.id)"
              >
                <Icon v-if="item.icon" :name="item.icon" class="k-palette__icon" />
                <span class="k-palette__text">{{ item.label }}</span>
                <code v-if="item.detail" class="k-palette__detail">{{ item.detail }}</code>
                <!-- Hidden from the name; a sequence is the entry's description. -->
                <ShortcutHint
                  v-if="item.shortcut"
                  :id="isSequence(item) ? keysId(item) : undefined"
                  :keys="item.shortcut"
                  aria-hidden="true"
                />
              </ComboboxItem>
            </ComboboxGroup>
          </ComboboxContent>
        </ComboboxRoot>
      </DialogContent>
    </DialogPortal>
  </DialogRoot>
</template>

<style>
@layer components {
  .k-palette {
    position: fixed;
    inset: var(--k-space-12) 0 auto;
    z-index: var(--k-z-modal);
    inline-size: min(var(--k-size-dialog-md), calc(100% - var(--k-space-8)));
    margin-inline: auto;
    color: var(--k-text);
    background-color: var(--k-surface);
    border: var(--k-border-width) solid var(--k-border-strong);
    border-radius: var(--k-radius-lg);
    box-shadow: var(--k-shadow-modal);

    /* Motion (design « Mouvement »): the one of a dialog, from its top edge. */
    transform-origin: top;
    transition-property: opacity, scale;
    transition-duration: max(var(--k-duration-medium), var(--k-duration-fade));
    transition-timing-function: var(--k-ease-standard);
  }

  @starting-style {
    .k-palette {
      opacity: 0;
      scale: var(--k-motion-scale);
    }
  }

  .k-palette[data-state="closed"] {
    pointer-events: none;
    animation-name: k-fade-out;
    animation-duration: max(var(--k-duration-short), var(--k-duration-fade));
    animation-timing-function: var(--k-ease-exit);
    animation-fill-mode: forwards;
  }

  .k-palette__search {
    display: flex;
    gap: var(--k-space-2);
    align-items: center;
    min-block-size: var(--k-size-status-bar);
    padding-inline: var(--k-space-3);
    color: var(--k-text-muted);
    border-block-end: var(--k-border-width) solid var(--k-border);
    border-start-start-radius: var(--k-radius-lg);
    border-start-end-radius: var(--k-radius-lg);
  }

  /* The field has no ring of its own: its row shows the focus (2.4.7), inside the dialog. */
  .k-palette__search:focus-within {
    outline: var(--k-focus-width) solid var(--k-focus-ring);
    outline-offset: calc(var(--k-focus-width) * -1);
  }

  .k-palette__input {
    flex: 1;
    min-inline-size: 0;
    padding: 0;
    font: inherit;
    font-size: var(--k-text-lg);
    color: var(--k-text);
    background: none;
    border: 0;
    outline: 0;
  }

  .k-palette__input::placeholder {
    color: var(--k-text-muted);
    opacity: 1;
  }

  .k-palette__list {
    display: grid;
    gap: var(--k-space-1);
    max-block-size: var(--k-size-dialog-sm);
    padding: var(--k-space-1);
    overflow: auto;
  }

  .k-palette__label {
    padding: var(--k-space-1) var(--k-space-2);
    font-size: var(--k-text-sm);
    line-height: var(--k-text-sm-line);
    font-weight: var(--k-weight-medium);
    color: var(--k-text-muted);
  }

  .k-palette__item {
    display: flex;
    gap: var(--k-space-2);
    align-items: center;
    min-block-size: var(--k-control-height-lg);
    padding-inline: var(--k-space-2);
    cursor: default;
    border-radius: var(--k-radius-md);
  }

  .k-palette__item[data-highlighted] {
    background-color: var(--k-accent-soft);
    box-shadow: var(--k-shadow-inset-accent);
  }

  .k-palette__icon {
    color: var(--k-text-muted);
  }

  .k-palette__text {
    flex: 1;
    min-inline-size: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .k-palette__detail {
    color: var(--k-text-muted);
  }

  .k-palette__empty {
    padding: var(--k-space-4);
    color: var(--k-text-muted);
    text-align: center;
  }
}
</style>
