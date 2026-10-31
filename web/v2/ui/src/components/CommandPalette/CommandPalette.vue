<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

<script setup lang="ts">
// The command palette (Combobox in a modal Dialog; design « Inventaire », line 19): Ctrl+K
// opens it (the shell's shortcut), the field filters, the arrows move in the list, Enter
// runs. The host filters (`query`) and gives the groups; Reka filters nothing
// (`ignore-filter`). No ComboboxViewport (it injects a <style>, REQ-020-33).
import {
  ComboboxContent,
  ComboboxGroup,
  ComboboxInput,
  ComboboxItem,
  ComboboxLabel,
  ComboboxRoot,
  DialogContent,
  DialogDescription,
  DialogOverlay,
  DialogPortal,
  DialogRoot,
  DialogTitle,
} from "reka-ui";
import { computed, ref, watchPostEffect, type ComponentPublicInstance } from "vue";
import { injectPortalTarget } from "../../context.ts";
import { t } from "../../i18n.ts";
import Icon from "../Icon/Icon.vue";
import Kbd from "../Kbd/Kbd.vue";
import ShortcutHint from "../ShortcutHint/ShortcutHint.vue";
import type { PaletteGroup } from "./palette.ts";

const props = defineProps<{ groups: PaletteGroup[]; loading?: boolean }>();
const open = defineModel<boolean>("open", { default: false });
const query = defineModel<string>("query", { default: "" });
const emit = defineEmits<{ select: [id: string] }>();
const portal = injectPortalTarget();
// No entry: a status says so, outside the list (a listbox without options is invalid).
const empty = computed(() => props.groups.every((group) => group.items.length === 0));
// Reka leaves `aria-controls` empty with an inline list: it is read from the list each
// time the list is mounted (the dialog opens).
const list = ref<ComponentPublicInstance | null>(null);
const listId = ref<string | undefined>(undefined);
watchPostEffect(() => {
  listId.value = (list.value?.$el as HTMLElement | undefined)?.id || undefined;
});

function onSelect(id: string): void {
  emit("select", id);
  open.value = false;
}
</script>

<template>
  <DialogRoot v-model:open="open">
    <DialogPortal :to="portal">
      <DialogOverlay class="k-dialog__overlay" />
      <DialogContent class="k-palette">
        <DialogTitle class="k-visually-hidden">{{ t("palette") }}</DialogTitle>
        <DialogDescription class="k-visually-hidden">{{ t("paletteSearch") }}</DialogDescription>
        <ComboboxRoot :open="true" ignore-filter class="k-palette__combobox">
          <div class="k-palette__search">
            <Icon
              :name="props.loading ? 'loader-circle' : 'search'"
              :class="{ 'k-spinner': props.loading }"
            />
            <ComboboxInput
              v-model="query"
              class="k-palette__input"
              :placeholder="t('paletteSearch')"
              :aria-label="t('palette')"
              :aria-controls="listId"
              :aria-expanded="empty ? 'false' : 'true'"
              :aria-busy="props.loading ? 'true' : undefined"
              auto-focus
            />
            <Kbd :keys="['Escape']" aria-hidden="true" />
          </div>
          <p v-if="empty" class="k-palette__empty" role="status">
            {{ props.loading ? t("loading") : t("noResults") }}
          </p>
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
                @select="onSelect(item.id)"
              >
                <Icon v-if="item.icon" :name="item.icon" class="k-palette__icon" />
                <span class="k-palette__text">{{ item.label }}</span>
                <code v-if="item.detail" class="k-palette__detail">{{ item.detail }}</code>
                <ShortcutHint v-if="item.shortcut" :keys="item.shortcut" aria-hidden="true" />
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
