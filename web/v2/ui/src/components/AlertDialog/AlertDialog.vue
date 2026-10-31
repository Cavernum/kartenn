<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

<script setup lang="ts">
// A confirmation (APG Alert Dialog; design « Inventaire », line 12): Reka's AlertDialog,
// `role="alertdialog"` (and `aria-modal`, which Reka does not write), named by its title,
// described by its message; no closing by a click outside; the focus starts on the cancel
// button, always (no `data-autofocus`, no first field: Enter never destroys by mistake),
// is trapped (Tab goes round) and goes back to what opened it. Escape cancels, as the
// cancel button does (`cancel`); the confirming button emits `confirm`; a close by the
// host's model emits neither. Its trigger: one control in the `trigger` slot, or any
// control that sets the model. Its look is the one of `Dialog` (its sheet).
import {
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogOverlay,
  AlertDialogPortal,
  AlertDialogRoot,
  AlertDialogTitle,
  AlertDialogTrigger,
} from "reka-ui";
import { injectPortalTarget } from "../../context.ts";
import { t } from "../../i18n.ts";
import Button from "../Button/Button.vue";
import { useOpener } from "../internal/modal.ts";

const props = withDefaults(
  defineProps<{
    title: string;
    description: string;
    confirm: string;
    cancel?: string | undefined;
    danger?: boolean;
  }>(),
  { cancel: undefined },
);
const open = defineModel<boolean>("open", { default: false });
const emit = defineEmits<{ confirm: []; cancel: [] }>();
const portal = injectPortalTarget();
const onCloseFocus = useOpener(open);

// Escape and « Annuler » cancel (`cancel`); the confirming button says so (`confirm`), then
// closes it. Closed by the host (its model), it says nothing.
function onEscape(event: KeyboardEvent): void {
  if (!event.defaultPrevented) emit("cancel");
}
function onConfirm(): void {
  emit("confirm");
  open.value = false;
}
</script>

<template>
  <AlertDialogRoot v-model:open="open">
    <AlertDialogTrigger v-if="$slots.trigger" as-child><slot name="trigger" /></AlertDialogTrigger>
    <AlertDialogPortal :to="portal">
      <AlertDialogOverlay class="k-dialog__overlay" />
      <AlertDialogContent
        class="k-dialog"
        data-size="sm"
        aria-modal="true"
        @close-auto-focus="onCloseFocus"
        @escape-key-down="onEscape"
      >
        <AlertDialogTitle class="k-dialog__title">{{ props.title }}</AlertDialogTitle>
        <AlertDialogDescription class="k-dialog__description">
          {{ props.description }}
        </AlertDialogDescription>
        <slot />
        <footer class="k-dialog__actions">
          <AlertDialogCancel as-child>
            <Button @click="emit('cancel')">{{ props.cancel ?? t("cancel") }}</Button>
          </AlertDialogCancel>
          <Button :variant="props.danger ? 'danger' : 'primary'" @click="onConfirm">{{
            props.confirm
          }}</Button>
        </footer>
      </AlertDialogContent>
    </AlertDialogPortal>
  </AlertDialogRoot>
</template>
