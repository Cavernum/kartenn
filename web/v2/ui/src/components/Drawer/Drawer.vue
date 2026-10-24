<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

<script setup lang="ts">
// A side panel (design « Inventaire », line 13): non-modal by default, a `complementary`
// region that takes its place beside the surface and pushes it (the draft panel, never
// over a cell being edited); modal, a Reka Dialog on the side of the page. Escape closes
// both while the focus is inside.
import {
  DialogClose,
  DialogContent,
  DialogDescription,
  DialogOverlay,
  DialogPortal,
  DialogRoot,
  DialogTitle,
} from "reka-ui";
import { nextTick, useId, watch } from "vue";
import { injectPortalTarget } from "../../context.ts";
import { t } from "../../i18n.ts";
import IconButton from "../IconButton/IconButton.vue";

const props = withDefaults(
  defineProps<{
    title: string;
    description?: string | undefined;
    modal?: boolean;
    side?: "start" | "end";
    size?: "sm" | "md" | "lg";
  }>(),
  { description: undefined, side: "end", size: "lg" },
);
const open = defineModel<boolean>("open", { default: false });
const portal = injectPortalTarget();
const titleId = useId();

// Non modal: the focus comes back to where it was when the panel opened (WCAG 2.4.3).
let opener: HTMLElement | null = null;
watch(open, async (now) => {
  if (props.modal || typeof document === "undefined") return;
  if (now) opener = document.activeElement as HTMLElement | null;
  else {
    await nextTick();
    if (document.activeElement === document.body) opener?.focus();
    opener = null;
  }
});

function onKeydown(event: KeyboardEvent): void {
  if (event.key === "Escape" && !event.defaultPrevented) {
    event.preventDefault();
    open.value = false;
  }
}
</script>

<template>
  <DialogRoot v-if="props.modal" v-model:open="open">
    <DialogPortal :to="portal">
      <DialogOverlay class="k-dialog__overlay" />
      <DialogContent
        class="k-drawer"
        data-modal="true"
        v-bind="props.description ? {} : { 'aria-describedby': undefined }"
        :data-side="props.side"
        :data-size="props.size"
      >
        <header class="k-drawer__header">
          <DialogTitle class="k-drawer__title">{{ props.title }}</DialogTitle>
          <DialogClose as-child>
            <IconButton icon="x" :label="t('close')" size="sm" />
          </DialogClose>
        </header>
        <DialogDescription v-if="props.description" class="k-drawer__description">
          {{ props.description }}
        </DialogDescription>
        <div class="k-drawer__body"><slot /></div>
        <footer v-if="$slots.actions" class="k-drawer__actions"><slot name="actions" /></footer>
      </DialogContent>
    </DialogPortal>
  </DialogRoot>
  <aside
    v-else-if="open"
    class="k-drawer"
    :data-side="props.side"
    :data-size="props.size"
    :aria-labelledby="titleId"
    @keydown="onKeydown"
  >
    <header class="k-drawer__header">
      <h2 :id="titleId" class="k-drawer__title">{{ props.title }}</h2>
      <IconButton icon="x" :label="t('close')" size="sm" @click="open = false" />
    </header>
    <p v-if="props.description" class="k-drawer__description">{{ props.description }}</p>
    <div class="k-drawer__body"><slot /></div>
    <footer v-if="$slots.actions" class="k-drawer__actions"><slot name="actions" /></footer>
  </aside>
</template>

<style>
@layer components {
  .k-drawer {
    --k-drawer-size: var(--k-size-panel-lg);

    display: grid;
    grid-template-rows: auto auto minmax(0, 1fr) auto;
    inline-size: var(--k-drawer-size);
    min-block-size: 0;
    color: var(--k-text);
    background-color: var(--k-surface);
    border-inline-start: var(--k-border-width) solid var(--k-border-strong);
  }

  .k-drawer[data-side="start"] {
    border-inline-start: 0;
    border-inline-end: var(--k-border-width) solid var(--k-border-strong);
  }

  .k-drawer[data-size="sm"] {
    --k-drawer-size: var(--k-size-panel-sm);
  }

  .k-drawer[data-size="md"] {
    --k-drawer-size: var(--k-size-panel-md);
  }

  .k-drawer[data-modal] {
    position: fixed;
    inset-block: 0;
    inset-inline-end: 0;
    z-index: var(--k-z-modal);
    max-inline-size: calc(100% - var(--k-space-12));
    box-shadow: var(--k-shadow-modal);
  }

  .k-drawer[data-modal][data-side="start"] {
    inset-inline: 0 auto;
  }

  .k-drawer__header {
    display: flex;
    gap: var(--k-space-2);
    align-items: center;
    justify-content: space-between;
    min-block-size: var(--k-size-status-bar);
    padding-inline: var(--k-space-4) var(--k-space-2);
    border-block-end: var(--k-border-width) solid var(--k-border);
  }

  .k-drawer__title {
    font-size: var(--k-text-lg);
    line-height: var(--k-text-lg-line);
    font-weight: var(--k-weight-semibold);
  }

  .k-drawer__description {
    padding: var(--k-space-2) var(--k-space-4) 0;
    color: var(--k-text-muted);
  }

  .k-drawer__body {
    padding: var(--k-space-3) var(--k-space-4);
    overflow: auto;
  }

  .k-drawer__actions {
    display: flex;
    gap: var(--k-space-2);
    justify-content: end;
    padding: var(--k-space-2) var(--k-space-4);
    border-block-start: var(--k-border-width) solid var(--k-border);
  }
}
</style>
