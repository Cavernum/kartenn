<script setup lang="ts">
// The panel of the work surface in the mock-ups: the one of the shell's screens
// (`views/parts/ViewPanel.vue`, task 021-11), its slots passed through, so that a mock-up
// and its screen draw the same panel.
import ViewPanel from "../../views/parts/ViewPanel.vue";

const props = defineProps<{
  title: string;
  detail?: string | undefined;
  mono?: boolean;
  main?: boolean;
  flush?: boolean;
}>();
</script>

<template>
  <ViewPanel
    :title="props.title"
    :detail="props.detail"
    :mono="props.mono"
    :main="props.main"
    :flush="props.flush"
  >
    <template v-for="(_, name) in $slots" #[name]="scope">
      <slot :name="name" v-bind="scope ?? {}" />
    </template>
  </ViewPanel>
</template>
