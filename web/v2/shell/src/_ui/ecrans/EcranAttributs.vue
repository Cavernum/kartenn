<script setup lang="ts">
// The list of attributes in the mock-ups: the one of the shell's screens (`AttributeList`
// of @kartenn/ui, task 020-29), its slots passed through, so that a mock-up and its screen
// draw the same list.
import { AttributeList, type AttributeItem } from "@kartenn/ui";

const props = defineProps<{ items: AttributeItem[]; columns?: boolean }>();
</script>

<template>
  <AttributeList :items="props.items" :columns="props.columns">
    <template v-for="(_, name) in $slots" #[name]="scope">
      <slot :name="name" v-bind="scope ?? {}" />
    </template>
  </AttributeList>
</template>
