<script setup lang="ts">
import { computed, reactive, shallowRef, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useRoute, useRouter } from "vue-router";
import { PAGE_SIZE, Pager, withQuery } from "../data/api.ts";
import { cell, COLUMNS, FILTERS, linkOf, pageOf, type Collection } from "../data/collections.ts";
import DataTable from "../data/DataTable.vue";
import { problemText } from "../i18n/index.ts";
import { useTime } from "../time/index.ts";

// A list of the core (REQ-007-17): filters in the URL, continuous scrolling by pages of
// 100, the dates of the context.

type Row = Record<string, unknown>;

const i18n = useI18n();
const { t } = i18n;
const route = useRoute();
const router = useRouter();
const time = useTime();

const collection = computed(() => String(route.params.collection) as Collection);
const filters = computed(() => FILTERS[collection.value]);
const draft = reactive<Record<string, string>>({});

const first = computed(() => {
  const params: Record<string, string | null> = { page_size: String(PAGE_SIZE) };
  for (const name of filters.value) {
    const value = route.query[name];
    params[name] = typeof value === "string" ? value : null;
  }
  params.valid_at = time.value.validAt;
  params.as_of = time.value.asOf;
  return withQuery(`/v1/${collection.value}`, params);
});

const pager = shallowRef<Pager<Row>>(new Pager<Row>(first.value));
watch(
  first,
  (url) => {
    pager.value = new Pager<Row>(url);
    for (const name of filters.value) {
      const value = route.query[name];
      draft[name] = typeof value === "string" ? value : "";
    }
  },
  { immediate: true },
);

const columns = computed(() =>
  COLUMNS[collection.value].map((key) => ({ key, label: t(`field.${key}`) })),
);
const problem = computed(() => {
  const value = pager.value.problem.value;
  return value ? problemText(i18n, value) : null;
});

function apply(): void {
  const query: Record<string, string> = {};
  for (const [name, value] of Object.entries(route.query)) {
    if (typeof value === "string" && !filters.value.includes(name)) query[name] = value;
  }
  for (const name of filters.value) {
    const value = draft[name]?.trim();
    if (value) query[name] = value;
  }
  void router.push({ path: route.path, query });
}

const href = (row: Row): string | null =>
  pageOf((row.links as { self?: string } | undefined)?.self);
</script>

<template>
  <h1>{{ t(`nav.${collection}`) }}</h1>
  <form class="k-filters" role="search" :aria-label="t('list.filters')" @submit.prevent="apply">
    <label v-for="name in filters" :key="name">
      {{ t(`field.${name}`) }}
      <input v-model="draft[name]" :name="name" />
    </label>
    <button type="submit">{{ t("list.apply") }}</button>
  </form>
  <p v-if="pager.restarted.value" role="status" class="k-muted">
    {{ t("problem.invalid-cursor") }}
  </p>
  <p v-if="problem" role="alert" class="k-error">{{ problem }}</p>
  <p class="k-muted" aria-live="polite">
    {{
      pager.done.value
        ? t("list.count", { n: pager.items.value.length })
        : t("list.loaded", { n: pager.items.value.length })
    }}
  </p>
  <DataTable
    :rows="pager.items.value"
    :columns="columns"
    :caption="t(`nav.${collection}`)"
    :done="pager.done.value"
    :loading="pager.loading.value"
    :href="href"
    :cell="cell"
    :link="linkOf"
    @more="pager.more()"
  />
</template>
