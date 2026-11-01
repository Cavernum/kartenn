<script setup lang="ts">
// An action of a plugin on an object (point `action`, REQ-007-13, 021-13; the type
// `changeset` among others): its element in a dialog named by the action's label, given the
// object; `kartenn-close` from the element closes it, Échap and the cross too, and the focus
// goes back to what opened it (the entry of the menu, by the dialog of @kartenn/ui).
import type { ObjectRef } from "@kartenn/sdk";
import { Dialog } from "@kartenn/ui";
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import ExtensionElement from "./ExtensionElement.vue";
import { labelOf, type Placed } from "./registry.ts";

const props = defineProps<{ extension: Placed | null; object: ObjectRef }>();
const open = defineModel<boolean>("open", { required: true });

const { t } = useI18n();
const title = computed(() => (props.extension ? labelOf(t, props.extension) : ""));
</script>

<template>
  <Dialog v-model:open="open" :title="title" size="md">
    <ExtensionElement
      v-if="props.extension && open"
      :tag="props.extension.element"
      :plugin="props.extension.plugin"
      :label="title"
      :object="props.object"
      @close="open = false"
    />
  </Dialog>
</template>
