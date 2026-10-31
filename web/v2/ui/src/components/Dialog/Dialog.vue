<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

<script setup lang="ts">
// A modal dialog (APG Dialog (Modal); design « Inventaire », line 12): Reka's Dialog, the
// rest of the page hidden (`aria-hidden`, written by Reka; `aria-modal`, which it does not
// write), the focus trapped (Tab and Shift+Tab go round) and given back to what opened it,
// Escape and the close button close; a title (its name), an optional description, the body
// and the actions (slot `actions`, the primary one last). The trigger is the host's: one
// control in the `trigger` slot (made Reka's trigger: `aria-haspopup="dialog"`,
// `aria-expanded`), or any control that sets the model.
import {
  DialogClose,
  DialogContent,
  DialogDescription,
  DialogOverlay,
  DialogPortal,
  DialogRoot,
  DialogTitle,
  DialogTrigger,
} from "reka-ui";
import { injectPortalTarget } from "../../context.ts";
import { t } from "../../i18n.ts";
import IconButton from "../IconButton/IconButton.vue";
import { focusFirstField, useOpener } from "../internal/modal.ts";

const props = withDefaults(
  defineProps<{
    title: string;
    description?: string | undefined;
    size?: "sm" | "md" | "lg";
    /**
     * The body is held within the dialog: a child that scrolls (a grid) scrolls in place,
     * its header sticky, the title and the actions in view (`ExpandableGrid`).
     */
    contained?: boolean;
  }>(),
  { description: undefined, size: "md" },
);
const open = defineModel<boolean>("open", { default: false });
const portal = injectPortalTarget();
const onCloseFocus = useOpener(open);
/** On open: the element marked `data-autofocus`, else the first field of the body. */
const onOpenFocus = (event: Event) => focusFirstField(event, ".k-dialog__body");
</script>

<template>
  <DialogRoot v-model:open="open">
    <DialogTrigger v-if="$slots.trigger" as-child><slot name="trigger" /></DialogTrigger>
    <DialogPortal :to="portal">
      <DialogOverlay class="k-dialog__overlay" />
      <DialogContent
        class="k-dialog"
        :data-size="props.size"
        :data-contained="props.contained ? 'true' : undefined"
        aria-modal="true"
        v-bind="props.description ? {} : { 'aria-describedby': undefined }"
        @open-auto-focus="onOpenFocus"
        @close-auto-focus="onCloseFocus"
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

  /* Motion (design « Mouvement »): the scrim fades in, the dialog rises from 98 % of its
   * size; out, both fade. Under reduced motion the scale is none and the fade keeps
   * `fade`. Interactive from the first frame; the focus is placed at once, never after the
   * end. Without `@starting-style`, the dialog just appears. */
  .k-dialog__overlay,
  .k-dialog {
    transition-property: opacity, scale;
    transition-duration: max(var(--k-duration-medium), var(--k-duration-fade));
    transition-timing-function: var(--k-ease-standard);
  }

  @starting-style {
    .k-dialog__overlay,
    .k-dialog {
      opacity: 0;
    }

    .k-dialog {
      scale: var(--k-motion-scale);
    }
  }

  .k-dialog__overlay[data-state="closed"],
  .k-dialog[data-state="closed"] {
    pointer-events: none;
    animation-name: k-fade-out;
    animation-duration: max(var(--k-duration-short), var(--k-duration-fade));
    animation-timing-function: var(--k-ease-exit);
    animation-fill-mode: forwards;
  }

  .k-dialog[data-size="sm"] {
    --k-dialog-size: var(--k-size-dialog-sm);
  }

  .k-dialog[data-size="lg"] {
    --k-dialog-size: var(--k-size-dialog-lg);
  }

  /* In the compact frame (its `data-width`, design « Adaptation à la largeur »), a large
   * dialog (the enlarged view of a grid among them) takes the whole window. */
  [data-width="compact"] .k-dialog[data-size="lg"] {
    inline-size: 100%;
    block-size: 100%;
    max-block-size: 100%;
    border: 0;
    border-radius: 0;
  }

  /* Contained: a column whose body shrinks to the room left, and its child with it. */
  .k-dialog[data-contained] {
    display: flex;
    flex-direction: column;
  }

  .k-dialog[data-contained] > .k-dialog__body {
    display: flex;
    flex-direction: column;
    min-block-size: 0;
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
