<script setup lang="ts">
import { computed, shallowRef, watch } from "vue";
import { useI18n } from "vue-i18n";
import { PAGE_SIZE, Pager, withQuery } from "../data/api.ts";
import { cell, pageOf } from "../data/collections.ts";
import DataTable from "../data/DataTable.vue";
import { problemText } from "../i18n/index.ts";
import { useTime } from "../time/index.ts";

// What a domain or a prefix holds, one level down (`roots`, `children`): prefixes, ranges
// and addresses, each opening its record.

type Row = Record<string, unknown>;

const props = defineProps<{ url: string; caption: string }>();
const i18n = useI18n();
const { t } = i18n;
const time = useTime();

const first = computed(() =>
  withQuery(props.url, {
    page_size: String(PAGE_SIZE),
    valid_at: time.value.validAt,
    as_of: time.value.asOf,
  }),
);
const pager = shallowRef(new Pager<Row>(first.value));
watch(first, (url) => (pager.value = new Pager<Row>(url)));

const columns = computed(() => [
  { key: "name", label: t("field.value") },
  { key: "kind", label: t("field.kind") },
  { key: "detail", label: t("field.detail") },
]);

function text(row: Row, key: string): string {
  if (key === "name") {
    if (row.kind === "range") return `${cell(row, "first")} – ${cell(row, "last")}`;
    return cell(row, "value");
  }
  if (key === "kind") return t(`object.types.${String(row.kind)}`);
  return cell(row, row.kind === "prefix" ? "nature" : "usage");
}
</script>

<template>
  <p v-if="pager.problem.value" role="alert" class="k-error">
    {{ problemText(i18n, pager.problem.value) }}
  </p>
  <DataTable
    v-else
    :rows="pager.items.value"
    :columns="columns"
    :caption="caption"
    :done="pager.done.value"
    :loading="pager.loading.value"
    :href="(row) => pageOf(row.href)"
    :cell="text"
    @more="pager.more()"
  />
</template>
