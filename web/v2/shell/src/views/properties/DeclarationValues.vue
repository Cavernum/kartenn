<script setup lang="ts">
// The values of a closed list of a declared property (016, `/values`): each its label and
// key, and its period of validity when it has one; read when the popover opens.
import { reads, type components } from "@kartenn/sdk";
import { ErrorState, KeyLabel, Skeleton } from "@kartenn/ui";
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { useInfiniteQuery } from "../../data/query.ts";
import { problemOf, rowsOf, useKeyLabel } from "../data.ts";

type Value = components["schemas"]["EnumValueItem"];

const props = defineProps<{ property: string }>();
const { t } = useI18n();
const keyLabel = useKeyLabel();
const values = useInfiniteQuery(() =>
  reads.list("/v1/property-declarations/{key}/values", null, null, { key: props.property }),
);
const items = computed(() => rowsOf(values.data.value) as Value[]);
const day = (value: string | null | undefined) => (value ? value.slice(0, 10) : "");
</script>

<template>
  <Skeleton v-if="values.isPending.value" variant="text" :rows="3" />
  <ErrorState
    v-else-if="values.isError.value"
    :title="t('views.properties.valuesError')"
    :problem="problemOf(values.error.value)"
  />
  <p v-else-if="items.length === 0">{{ t("views.properties.noValues") }}</p>
  <ul v-else class="k-declaration-values">
    <li v-for="value in items" :key="value.key">
      <KeyLabel
        :value="value.key"
        :label="keyLabel('value', `${props.property}/${value.key}`)"
        show-key
      />
      <span v-if="value.valid_from || value.valid_to" class="k-mono">
        {{ day(value.valid_from) }} → {{ day(value.valid_to) }}</span
      >
    </li>
  </ul>
</template>

<style>
@layer components {
  .k-declaration-values {
    display: grid;
    gap: var(--k-space-1);
    padding: 0;
    list-style: none;
  }
}
</style>
