<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

<script setup lang="ts">
// A side dialog (APG Dialog (Modal); design « Inventaire », line 13): a Reka Dialog on the
// side of the page, for a long form; the focus is trapped and given back, Escape closes.
// Modal only: the non-modal panel beside the surface is `SidePanel` (024/M8).
import {
  DialogClose,
  DialogContent,
  DialogDescription,
  DialogOverlay,
  DialogPortal,
  DialogRoot,
  DialogTitle,
} from "reka-ui";
import { injectPortalTarget } from "../../context.ts";
import { t } from "../../i18n.ts";
import IconButton from "../IconButton/IconButton.vue";

const props = withDefaults(
  defineProps<{
    title: string;
    description?: string | undefined;
    side?: "start" | "end";
    size?: "sm" | "md" | "lg";
  }>(),
  { description: undefined, side: "end", size: "lg" },
);
const open = defineModel<boolean>("open", { default: false });
const portal = injectPortalTarget();
</script>

<template>
  <DialogRoot v-model:open="open">
    <DialogPortal :to="portal">
      <DialogOverlay class="k-dialog__overlay" />
      <DialogContent
        class="k-drawer"
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
</template>

<style>
@layer components {
  .k-drawer {
    --k-drawer-size: var(--k-size-panel-lg);

    position: fixed;
    inset-block: 0;
    inset-inline-end: 0;
    z-index: var(--k-z-modal);
    display: grid;
    grid-template-rows: auto auto minmax(0, 1fr) auto;
    inline-size: var(--k-drawer-size);
    max-inline-size: calc(100% - var(--k-space-12));
    min-block-size: 0;
    color: var(--k-text);
    background-color: var(--k-surface);
    border-inline-start: var(--k-border-width) solid var(--k-border-strong);
    box-shadow: var(--k-shadow-modal);
  }

  /* Motion (design « Mouvement »): in from its edge, out by a fade (the sheet of `Dialog`,
   * which also fades the scrim). */
  .k-drawer {
    transition-property: opacity, translate;
    transition-duration: max(var(--k-duration-medium), var(--k-duration-fade));
    transition-timing-function: var(--k-ease-standard);
  }

  @starting-style {
    .k-drawer {
      opacity: 0;
      translate: var(--k-motion-shift-lg) 0;
    }

    .k-drawer[data-side="start"],
    .k-drawer:dir(rtl) {
      translate: calc(var(--k-motion-shift-lg) * -1) 0;
    }

    .k-drawer[data-side="start"]:dir(rtl) {
      translate: var(--k-motion-shift-lg) 0;
    }
  }

  .k-drawer[data-state="closed"] {
    pointer-events: none;
    animation-name: k-fade-out;
    animation-duration: max(var(--k-duration-short), var(--k-duration-fade));
    animation-timing-function: var(--k-ease-exit);
    animation-fill-mode: forwards;
  }

  .k-drawer[data-side="start"] {
    inset-inline: 0 auto;
    border-inline-start: 0;
    border-inline-end: var(--k-border-width) solid var(--k-border-strong);
  }

  .k-drawer[data-size="sm"] {
    --k-drawer-size: var(--k-size-panel-sm);
  }

  .k-drawer[data-size="md"] {
    --k-drawer-size: var(--k-size-panel-md);
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
