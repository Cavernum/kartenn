<script setup lang="ts">
import { computed, shallowRef } from "vue";
import { useI18n } from "vue-i18n";
import { Pager, withQuery } from "../data/api.ts";
import type { Import } from "./admin.ts";
import PagedTable from "./PagedTable.vue";

// The imports of the user, most recent first (REQ-007-19, REQ-006-17 to 19); a row opens
// its follow-up.

const { t } = useI18n();
const pager = shallowRef(new Pager<Import>(withQuery("/v1/imports", { page_size: "100" })));
const columns = computed(() => [
  { key: "source", label: t("imports.source") },
  { key: "state", label: t("imports.state") },
  { key: "elements", label: t("imports.elements") },
  { key: "batches", label: t("imports.batches") },
]);
const cell = (row: Import, key: string): string => {
  if (key === "state") return t(`imports.states.${row.state}`);
  if (key === "batches") return t("imports.applied", row.batches);
  return String(row[key] ?? "—");
};
const href = (row: Import): string => `/imports/${row.id}`;
</script>

<template>
  <h1>{{ t("nav.imports") }}</h1>
  <PagedTable
    :pager="pager"
    :columns="columns"
    :caption="t('nav.imports')"
    :cell="cell"
    :href="href"
  />
</template>
