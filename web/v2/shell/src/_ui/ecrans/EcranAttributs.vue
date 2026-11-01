<script setup lang="ts">
// The list of attributes in the mock-ups: the one of the shell's screens
// (`views/parts/AttributeList.vue`, task 021-11), its slots passed through, so that a
// mock-up and its screen draw the same list.
import AttributeList from "../../views/parts/AttributeList.vue";

const props = defineProps<{
  items: { key: string; label: string; value?: string; mono?: boolean }[];
  columns?: boolean;
}>();
</script>

<template>
  <AttributeList :items="props.items" :columns="props.columns">
    <template v-for="(_, name) in $slots" #[name]="scope">
      <slot :name="name" v-bind="scope ?? {}" />
    </template>
  </AttributeList>
</template>
