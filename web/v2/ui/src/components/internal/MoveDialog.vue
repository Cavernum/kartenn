<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

<script setup lang="ts">
// « Déplacer vers… » of a tree of declared parents (design « Contrat de déplacement »;
// REQ-020-31): a dialog whose combobox lists the accepted targets only (`targets`, checked
// by the host's `canMove`), the first level among them when accepted. Escape closes the
// list, then the dialog (as `FilterPicker`, decision A8); the focus goes back to where it
// was. Moving emits the chosen target; the tree turns it into an intent.
import { ref, watch } from "vue";
import { fill, t } from "../../i18n.ts";
import Button from "../Button/Button.vue";
import Combobox from "../Combobox/Combobox.vue";
import Dialog from "../Dialog/Dialog.vue";
import type { ListOption } from "../Select/select.ts";
import { ROOT_TARGET } from "./treeMove.ts";

const props = defineProps<{
  /** The label of the node to move. */
  node: string;
  /** The accepted targets that match a text (`ROOT_TARGET` for the first level). */
  targets: (text: string) => ListOption[];
}>();
const open = defineModel<boolean>("open", { default: false });
const emit = defineEmits<{ move: [target: string | null] }>();

const query = ref("");
const chosen = ref<string | undefined>(undefined);
const error = ref<string | undefined>(undefined);
watch(open, (now) => {
  if (!now) return;
  query.value = "";
  chosen.value = undefined;
  error.value = undefined;
});
watch(chosen, () => (error.value = undefined));

const suggest = async (text: string) => props.targets(text);

function confirm(): void {
  if (chosen.value === undefined) {
    error.value = t("moveTargetNone");
    return;
  }
  emit("move", chosen.value === ROOT_TARGET ? null : chosen.value);
  open.value = false;
}
</script>

<template>
  <Dialog v-model:open="open" :title="fill('moveTitle', { node: props.node })" size="sm">
    <Combobox
      v-model="query"
      v-model:value="chosen"
      :label="t('moveTarget')"
      :hint="t('moveTargetHint')"
      :error="error"
      :suggest="suggest"
      :debounce="0"
      @submit="confirm"
    />
    <template #actions>
      <Button variant="ghost" @click="open = false">{{ t("cancel") }}</Button>
      <Button variant="primary" @click="confirm">{{ t("moveConfirm") }}</Button>
    </template>
  </Dialog>
</template>
