<script setup lang="ts">
// A list of attributes (term, value) in two columns, for the side panels of a record. Not
// in the inventory of the 020: written for the mock-ups and to be decided with them (task
// 15). A value may be replaced by a slot named after its term key.
const props = defineProps<{
  items: { key: string; label: string; value?: string; mono?: boolean }[];
}>();
</script>

<template>
  <dl class="k-ecran-attrs">
    <div v-for="item in props.items" :key="item.key" class="k-ecran-attrs__item">
      <dt class="k-ecran-attrs__term">{{ item.label }}</dt>
      <dd class="k-ecran-attrs__value" :class="{ 'k-mono': item.mono }">
        <slot :name="item.key">{{ item.value }}</slot>
      </dd>
    </div>
  </dl>
</template>

<style>
@layer components {
  .k-ecran-attrs {
    display: grid;
    gap: var(--k-space-1);
    max-inline-size: var(--k-measure-prose);
  }

  .k-ecran-attrs__item {
    display: grid;
    grid-template-columns: minmax(0, 2fr) minmax(0, 3fr);
    gap: var(--k-space-2);
    align-items: baseline;
    min-block-size: var(--k-row-height);
  }

  .k-ecran-attrs__term {
    color: var(--k-text-muted);
  }

  .k-ecran-attrs__value {
    min-inline-size: 0;
  }
}
</style>
