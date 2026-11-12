<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

<script setup lang="ts">
// The question of a modified panel before it closes (`SidePanel`, `BottomPanel`; Q-021-22,
// task 021-12qa): « Abandonner les modifications ? », « Abandonner » (danger) and
// « Continuer la saisie », the focus on the latter (`AlertDialog`). `answer` says the
// choice: true for « Abandonner »; false for « Continuer la saisie » or Escape.
// « Abandonner » also closes the dialog, whose `false` then follows its `true`:
// `usePanelClose` takes the first answer of a question only.
import { t } from "../../i18n.ts";
import AlertDialog from "../AlertDialog/AlertDialog.vue";

const props = defineProps<{ open: boolean }>();
const emit = defineEmits<{ answer: [drop: boolean] }>();
</script>

<template>
  <AlertDialog
    :open="props.open"
    :title="t('panelLeaveTitle')"
    :description="t('panelLeaveText')"
    :confirm="t('panelLeaveConfirm')"
    :cancel="t('panelLeaveCancel')"
    danger
    @update:open="(now: boolean) => now || emit('answer', false)"
    @confirm="emit('answer', true)"
  />
</template>
