<script setup lang="ts">
// The panel of the work surface in the mock-ups: the one of the shell's screens (`Panel` of
// @kartenn/ui, tasks 021-11 and 22f-i1), its slots passed through, so that a mock-up and its
// screen draw the same panel.
import { Panel } from "@kartenn/ui";

const props = defineProps<{
  title: string;
  detail?: string | undefined;
  mono?: boolean;
  main?: boolean;
  flush?: boolean;
}>();
</script>

<template>
  <Panel
    :title="props.title"
    :detail="props.detail"
    :mono="props.mono"
    :main="props.main"
    :flush="props.flush"
  >
    <template v-for="(_, name) in $slots" #[name]="scope">
      <slot :name="name" v-bind="scope ?? {}" />
    </template>
  </Panel>
</template>
