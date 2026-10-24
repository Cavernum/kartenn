<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

<script setup lang="ts">
// A confirmation (APG Alert Dialog; design « Inventaire », line 12): Reka's AlertDialog,
// `role="alertdialog"`, no closing by a click outside; the focus starts on the cancel
// button, so that Enter never destroys by mistake. Its look is the one of `Dialog` (its
// sheet).
import {
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogOverlay,
  AlertDialogPortal,
  AlertDialogRoot,
  AlertDialogTitle,
} from "reka-ui";
import { injectPortalTarget } from "../../context.ts";
import { t } from "../../i18n.ts";
import Button from "../Button/Button.vue";

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
</script>

<template>
  <AlertDialogRoot v-model:open="open">
    <slot name="trigger" />
    <AlertDialogPortal :to="portal">
      <AlertDialogOverlay class="k-dialog__overlay" />
      <AlertDialogContent class="k-dialog" data-size="sm">
        <AlertDialogTitle class="k-dialog__title">{{ props.title }}</AlertDialogTitle>
        <AlertDialogDescription class="k-dialog__description">
          {{ props.description }}
        </AlertDialogDescription>
        <slot />
        <footer class="k-dialog__actions">
          <AlertDialogCancel as-child>
            <Button @click="emit('cancel')">{{ props.cancel ?? t("cancel") }}</Button>
          </AlertDialogCancel>
          <AlertDialogAction as-child>
            <Button :variant="props.danger ? 'danger' : 'primary'" @click="emit('confirm')">{{
              props.confirm
            }}</Button>
          </AlertDialogAction>
        </footer>
      </AlertDialogContent>
    </AlertDialogPortal>
  </AlertDialogRoot>
</template>
