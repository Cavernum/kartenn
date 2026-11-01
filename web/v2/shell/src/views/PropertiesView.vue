<script setup lang="ts">
// The declared properties (REQ-021-9; design 021 « Catalogue des écrans » : Propriétés
// déclarées; 003, 016): each its label and key, its type, its cardinality, the kinds that
// carry it, how it is inherited, whether it is expected or sensitive, and the values of a
// closed list on demand. Their gaps are a tab of the anomalies. Three states.
import {
  Cluster,
  DataGrid,
  EmptyState,
  IconButton,
  InlineAlert,
  KeyLabel,
  Popover,
  type GridColumn,
} from "@kartenn/ui";
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { useRouter } from "vue-router";
import ProblemState from "../problems/ProblemState.vue";
import { useDeclarations, useKeyLabel } from "./data.ts";
import type { DeclarationOfModel } from "./filters.ts";
import ViewPanel from "./parts/ViewPanel.vue";
import DeclarationValues from "./properties/DeclarationValues.vue";

type Row = DeclarationOfModel & {
  id: string;
  cardinality?: string;
  inherit?: string | null;
  sensitive?: boolean;
  expected?: { mode: string; source: string } | null;
};

const { t, te, locale } = useI18n();
const router = useRouter();
const keyLabel = useKeyLabel();
const { declarations, loading, settled, partial, failed, error, refetch } = useDeclarations();
/** Whole once every page is read; while they come, or past the cap, said partial. */
const count = computed(() => {
  if (loading.value || failed.value) return undefined;
  const n = new Intl.NumberFormat(locale.value).format(rows.value.length);
  if (partial.value) return t("views.properties.countPartial", { n }, rows.value.length);
  return settled.value ? n : t("views.list.loaded", { n }, rows.value.length);
});
const rows = computed(() => declarations.value as unknown as Row[]);

const columns = computed<GridColumn[]>(() => [
  { id: "key", label: t("views.record.property"), width: "fill" },
  { id: "type", label: t("views.properties.type"), width: "sm", mono: true },
  { id: "cardinality", label: t("views.properties.cardinality"), width: "sm", priority: 2 },
  { id: "carriers", label: t("views.properties.carriers"), width: "md", priority: 2 },
  { id: "inherit", label: t("views.properties.inherit"), width: "sm", priority: 3 },
  { id: "flags", label: t("views.properties.flags"), width: "md", priority: 3 },
  { id: "actions", label: t("views.actions"), width: "sm", actions: true },
]);
const words = (group: string, value: string) => {
  const key = `views.properties.${group}.${value}`;
  return te(key) ? t(key) : value;
};
const carriers = (row: Row) =>
  row.carriers.length === 0
    ? t("views.properties.everyKind")
    : row.carriers.map((kind) => keyLabel("kind", kind) ?? kind).join(", ");
const flags = (row: Row) =>
  [
    row.expected ? t("views.properties.expected") : null,
    row.sensitive ? t("views.properties.sensitive") : null,
  ]
    .filter(Boolean)
    .join(", ") || "—";
const gaps = (key: string) =>
  void router.push({ path: "/anomalies", query: { tab: "gaps", property: key } });
</script>

<template>
  <h1 class="k-visually-hidden">{{ t("nav.properties") }}</h1>
  <ViewPanel :title="t('nav.properties')" :detail="count" main flush>
    <!-- Pages left unread (the cap, or a page refused): the ones read stay shown. -->
    <InlineAlert v-if="partial" tone="warn" :title="t('views.properties.partialTitle')">
      {{ t("views.properties.partial") }}
    </InlineAlert>
    <ProblemState
      v-if="failed"
      :title="t('views.properties.error')"
      :error="error"
      :retry="refetch"
    />
    <EmptyState v-else-if="settled && rows.length === 0" :title="t('views.properties.none')" />
    <DataGrid
      v-else
      :label="t('nav.properties')"
      :columns="columns"
      :rows="rows as unknown as Record<string, unknown>[]"
      row-key="id"
      :loading="loading"
      :loading-rows="8"
    >
      <template #cell="{ row, column }">
        <KeyLabel
          v-if="column.id === 'key'"
          :value="(row as Row).key"
          :label="keyLabel('property', (row as Row).key)"
          show-key
        />
        <template v-else-if="column.id === 'type'">{{
          words("types", (row as Row).type)
        }}</template>
        <template v-else-if="column.id === 'cardinality'">{{
          words("cardinality", String((row as Row).cardinality ?? ""))
        }}</template>
        <template v-else-if="column.id === 'carriers'">{{ carriers(row as Row) }}</template>
        <template v-else-if="column.id === 'inherit'">{{
          (row as Row).inherit ? words("inheritance", String((row as Row).inherit)) : "—"
        }}</template>
        <template v-else-if="column.id === 'flags'">{{ flags(row as Row) }}</template>
        <Cluster v-else gap="1" :wrap="false">
          <Popover
            v-if="(row as Row).type === 'enum'"
            :title="t('views.properties.valuesOf', { name: (row as Row).key })"
          >
            <template #trigger>
              <IconButton
                icon="tag"
                :label="t('views.properties.valuesOf', { name: (row as Row).key })"
                size="sm"
                variant="ghost"
              />
            </template>
            <DeclarationValues :property="(row as Row).key" />
          </Popover>
          <IconButton
            v-if="(row as Row).expected"
            icon="triangle-alert"
            :label="t('views.properties.gapsOf', { name: (row as Row).key })"
            size="sm"
            variant="ghost"
            @click="gaps((row as Row).key)"
          />
        </Cluster>
      </template>
    </DataGrid>
  </ViewPanel>
</template>
