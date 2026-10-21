<script setup lang="ts">
import type { ObjectRef } from "@kartenn/sdk";
import { onBeforeUnmount, ref, watch } from "vue";
import { useI18n } from "vue-i18n";

// Host of one extension (design « Chargement des plugins par le shell », hôtes) : creates
// the Custom Element of the plugin, gives it its properties, and shows an error box if
// the creation throws. `kartenn-close` from the element is passed on as `close`.

const props = defineProps<{
  tag: string;
  object?: ObjectRef;
  row?: unknown;
}>();
const emit = defineEmits<{ close: [] }>();

const { t } = useI18n();
const container = ref<HTMLElement | null>(null);
const failed = ref(false);
let element: HTMLElement | null = null;

function onClose(): void {
  emit("close");
}

function mount(): void {
  element?.removeEventListener("kartenn-close", onClose);
  element?.remove();
  element = null;
  failed.value = false;
  if (!container.value) return;
  try {
    const created = document.createElement(props.tag) as HTMLElement & {
      object?: ObjectRef;
      row?: unknown;
    };
    if (props.object) created.object = props.object;
    if (props.row !== undefined) created.row = props.row;
    created.addEventListener("kartenn-close", onClose);
    container.value.append(created);
    element = created;
  } catch (error) {
    console.warn(`kartenn: extension ${props.tag} failed`, error);
    failed.value = true;
  }
}

watch([container, () => props.tag, () => props.object, () => props.row], mount, {
  flush: "post",
});
onBeforeUnmount(() => {
  element?.removeEventListener("kartenn-close", onClose);
  element?.remove();
});
</script>

<template>
  <p v-if="failed" class="k-error" role="alert">{{ t("plugins.failed", { tag }) }}</p>
  <div ref="container" class="k-extension"></div>
</template>
