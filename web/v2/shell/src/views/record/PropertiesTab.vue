<script setup lang="ts">
// The resolved properties of an object (REQ-021-9, REQ-003-14; maquette E2): each its label
// and key, its value, and beside it the button of its explanation (`Popover`, read when it
// opens); a property the caller may not read is masked, said in words. Three states.
import { reads, type components } from "@kartenn/sdk";
import { DataGrid, EmptyState, IconButton, KeyLabel, Popover, type GridColumn } from "@kartenn/ui";
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { useQuery } from "../../data/query.ts";
import { useTime } from "../../time/index.ts";
import ProblemState from "../../problems/ProblemState.vue";
import { useDeclarations, useKeyLabel } from "../data.ts";
import ExplanationBody from "./ExplanationBody.vue";
import PropertyValue from "./PropertyValue.vue";

type Resolved = components["schemas"]["Resolved"];
interface Row extends Record<string, unknown> {
  key: string;
  value: unknown;
  masked: boolean;
}

const props = defineProps<{ kind: string; id: string; name: string }>();
const { t } = useI18n();
const time = useTime();
const keyLabel = useKeyLabel();
const { declarations } = useDeclarations();

const resolved = useQuery(() =>
  reads.sub<Resolved>(props.kind, props.id, "properties", null, {
    validAt: time.value.validAt,
    asOf: time.value.asOf,
  }),
);
const rows = computed<Row[]>(() => {
  const data = resolved.data.value?.data;
  if (!data) return [];
  const shown = Object.entries(data.properties as Record<string, unknown>).map(
    ([key, value]): Row => ({ key, value, masked: false }),
  );
  const masked = data.masked.map((key): Row => ({ key, value: null, masked: true }));
  return [...shown, ...masked].sort((a, b) => a.key.localeCompare(b.key));
});
const declarationOf = (key: string) => declarations.value.find((d) => d.key === key);
const targetOf = (key: string) => {
  const target = declarationOf(key)?.params?.target;
  return typeof target === "string" ? target : undefined;
};
const labelOf = (key: string) => keyLabel("property", key);

const columns = computed<GridColumn[]>(() => [
  { id: "key", label: t("views.record.property"), width: "fill" },
  { id: "value", label: t("views.record.value"), width: "lg" },
  { id: "explain", label: t("views.record.explanation"), width: "sm", actions: true },
]);
</script>

<template>
  <ProblemState
    v-if="resolved.isError.value"
    :title="t('views.record.propertiesError')"
    :error="resolved.error.value"
    :retry="resolved.refetch"
  />
  <EmptyState
    v-else-if="resolved.isSuccess.value && rows.length === 0"
    :title="t('views.record.noProperties')"
  >
    {{ t("views.record.noPropertiesText") }}
  </EmptyState>
  <DataGrid
    v-else
    :label="t('views.record.propertiesOf', { name: props.name })"
    :columns="columns"
    :rows="rows"
    row-key="key"
    :loading="resolved.isPending.value"
    :loading-rows="4"
  >
    <template #cell="{ row, column }">
      <KeyLabel v-if="column.id === 'key'" :value="row.key" :label="labelOf(row.key)" show-key />
      <template v-else-if="column.id === 'value'">
        <span v-if="row.masked">{{ t("views.record.masked") }}</span>
        <PropertyValue
          v-else
          :property="row.key"
          :value="row.value"
          :type="declarationOf(row.key)?.type"
          :target="targetOf(row.key)"
        />
      </template>
      <Popover
        v-else-if="!row.masked"
        :title="t('views.record.explanationOf', { name: labelOf(row.key) ?? row.key })"
      >
        <template #trigger>
          <IconButton
            icon="info"
            :label="t('views.record.explain', { name: labelOf(row.key) ?? row.key })"
            size="sm"
            variant="ghost"
          />
        </template>
        <ExplanationBody :id="props.id" :kind="props.kind" :property="row.key" />
      </Popover>
    </template>
  </DataGrid>
</template>
