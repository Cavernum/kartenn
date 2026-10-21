<script setup lang="ts">
import { computed, reactive, ref, shallowRef } from "vue";
import { useI18n } from "vue-i18n";
import { PAGE_SIZE, Pager, withQuery, type Problem } from "../data/api.ts";
import { problemText } from "../i18n/index.ts";
import { shellFetch } from "../session/index.ts";
import { exportName } from "./admin.ts";
import PagedTable from "./PagedTable.vue";

// The audit journal (REQ-007-19, spec 012/REQ-012-17): searched by period, name, actor or
// object, most recent first; the same search exported as NDJSON, saved as a file.

const FILTERS = ["from", "to", "name", "actor", "object"] as const;
type Event = Record<string, unknown>;

const i18n = useI18n();
const { t } = i18n;
const form = reactive<Record<string, string>>(
  Object.fromEntries(FILTERS.map((name) => [name, ""])),
);
const criteria = ref<Record<string, string>>({});
const url = computed(() => withQuery("/v1/audit-events", criteria.value));
const pager = shallowRef(new Pager<Event>(withQuery(url.value, { page_size: String(PAGE_SIZE) })));
const exported = shallowRef<Problem | null>(null);
const saved = ref<string | null>(null);

const columns = computed(() => [
  { key: "occurred_at", label: t("admin.occurred") },
  { key: "name", label: t("admin.event") },
  { key: "actor", label: t("admin.actor") },
  { key: "outcome", label: t("admin.outcome") },
  { key: "target_ids", label: t("admin.targets") },
]);
const cell = (event: Event, key: string): string => {
  if (key === "actor")
    return `${String(event.actor_kind ?? "—")} ${String(event.actor_id ?? "")}`.trim();
  const value = event[key];
  if (Array.isArray(value)) return value.length > 0 ? value.join(", ") : "—";
  return value === null || value === undefined ? "—" : String(value);
};

function search(): void {
  criteria.value = Object.fromEntries(
    FILTERS.map((name) => [name, form[name]?.trim() ?? ""]).filter(([, value]) => value),
  );
  pager.value = new Pager<Event>(withQuery(url.value, { page_size: String(PAGE_SIZE) }));
  void pager.value.more();
}

/** The export of the search, saved by the browser as a file (never kept by the shell). */
async function exportAll(): Promise<void> {
  saved.value = null;
  const response = await shellFetch(url.value, {
    headers: { accept: "application/x-ndjson" },
  }).catch(() => null);
  if (!response?.ok) {
    exported.value = {
      type: "about:blank",
      title: "export",
      detail: null,
      status: response?.status ?? 0,
    };
    return;
  }
  exported.value = null;
  const blob = await response.blob();
  const name = exportName(new Date());
  const link = document.createElement("a");
  link.href = URL.createObjectURL(blob);
  link.download = name;
  link.click();
  // Released once the browser has taken the download over.
  setTimeout(() => URL.revokeObjectURL(link.href), 60_000);
  saved.value = name;
}
</script>

<template>
  <h1>{{ t("admin.audit") }}</h1>
  <form
    class="k-filters"
    role="search"
    :aria-label="t('admin.auditSearch')"
    @submit.prevent="search"
  >
    <label v-for="name in FILTERS" :key="name">
      {{ t(`admin.auditFilters.${name}`) }}
      <input v-model="form[name]" :name="name" />
    </label>
    <button type="submit">{{ t("list.apply") }}</button>
    <button type="button" @click="exportAll">{{ t("admin.export") }}</button>
  </form>
  <p v-if="exported" role="alert" class="k-error">{{ problemText(i18n, exported) }}</p>
  <p v-if="saved" role="status">{{ t("admin.exported", { name: saved }) }}</p>
  <PagedTable
    :key="url"
    :pager="pager"
    :columns="columns"
    :caption="t('admin.audit')"
    :cell="cell"
  />
</template>
