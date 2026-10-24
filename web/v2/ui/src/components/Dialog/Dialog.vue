<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

<script setup lang="ts">
// A modal dialog (APG Dialog (Modal); design « Inventaire », line 12): Reka's Dialog, the
// focus trapped and given back, Escape closes; a title (its name), an optional description,
// the body and the actions (slot `actions`, the primary one last).
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
    size?: "sm" | "md" | "lg";
  }>(),
  { description: undefined, size: "md" },
);
const open = defineModel<boolean>("open", { default: false });
const portal = injectPortalTarget();

/** On open, the focus goes to the first field of the body, else where Reka puts it. */
function focusFirstField(event: Event): void {
  const content = event.target as HTMLElement | null;
  const field = content?.querySelector<HTMLElement>(
    ".k-dialog__body input:not([disabled]), .k-dialog__body select, .k-dialog__body textarea",
  );
  if (field) {
    event.preventDefault();
    field.focus();
  }
}
</script>

<template>
  <DialogRoot v-model:open="open">
    <slot name="trigger" />
    <DialogPortal :to="portal">
      <DialogOverlay class="k-dialog__overlay" />
      <DialogContent
        class="k-dialog"
        :data-size="props.size"
        v-bind="props.description ? {} : { 'aria-describedby': undefined }"
        @open-auto-focus="focusFirstField"
      >
        <header class="k-dialog__header">
          <DialogTitle class="k-dialog__title">{{ props.title }}</DialogTitle>
          <DialogClose as-child>
            <IconButton icon="x" :label="t('close')" size="sm" />
          </DialogClose>
        </header>
        <DialogDescription v-if="props.description" class="k-dialog__description">
          {{ props.description }}
        </DialogDescription>
        <div class="k-dialog__body"><slot /></div>
        <footer v-if="$slots.actions" class="k-dialog__actions"><slot name="actions" /></footer>
      </DialogContent>
    </DialogPortal>
  </DialogRoot>
</template>

<style>
@layer components {
  .k-dialog__overlay {
    position: fixed;
    inset: 0;
    z-index: var(--k-z-modal);
    background-color: var(--k-scrim);
  }

  .k-dialog {
    --k-dialog-size: var(--k-size-dialog-md);

    position: fixed;
    inset: 0;
    z-index: var(--k-z-modal);
    display: grid;
    gap: var(--k-space-3);
    inline-size: min(var(--k-dialog-size), calc(100% - var(--k-space-8)));
    block-size: fit-content;
    max-block-size: calc(100% - var(--k-space-12) * 2);
    padding: var(--k-space-4);
    margin: auto;
    overflow: auto;
    color: var(--k-text);
    background-color: var(--k-surface);
    border: var(--k-border-width) solid var(--k-border-strong);
    border-radius: var(--k-radius-lg);
    box-shadow: var(--k-shadow-modal);
  }

  .k-dialog[data-size="sm"] {
    --k-dialog-size: var(--k-size-dialog-sm);
  }

  .k-dialog[data-size="lg"] {
    --k-dialog-size: var(--k-size-dialog-lg);
  }

  .k-dialog__header {
    display: flex;
    gap: var(--k-space-2);
    align-items: center;
    justify-content: space-between;
  }

  .k-dialog__title {
    font-size: var(--k-text-lg);
    line-height: var(--k-text-lg-line);
    font-weight: var(--k-weight-semibold);
  }

  .k-dialog__description {
    color: var(--k-text-muted);
  }

  .k-dialog__actions {
    display: flex;
    gap: var(--k-space-2);
    justify-content: end;
    padding-block-start: var(--k-space-1);
  }
}
</style>
