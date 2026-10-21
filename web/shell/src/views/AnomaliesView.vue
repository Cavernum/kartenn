<script setup lang="ts">
import { computed, reactive, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useRoute, useRouter } from "vue-router";
import AnomaliesPanel from "../record/AnomaliesPanel.vue";

// The open anomalies, filtered by rule, domain or entity, and their acknowledgement
// (REQ-007-17).

const FILTERS = ["rule", "domain", "entity"] as const;

const { t } = useI18n();
const route = useRoute();
const router = useRouter();
const draft = reactive<Record<string, string>>({});

const filters = computed(() =>
  Object.fromEntries(
    FILTERS.map((name) => [name, typeof route.query[name] === "string" ? route.query[name] : null]),
  ),
);
watch(
  filters,
  (value) => {
    for (const name of FILTERS) draft[name] = value[name] ?? "";
  },
  { immediate: true },
);

function apply(): void {
  const query = { ...route.query };
  for (const name of FILTERS) {
    const value = draft[name]?.trim();
    if (value) query[name] = value;
    else delete query[name];
  }
  void router.push({ path: route.path, query });
}
</script>

<template>
  <h1>{{ t("nav.anomalies") }}</h1>
  <form class="k-filters" role="search" :aria-label="t('list.filters')" @submit.prevent="apply">
    <label v-for="name in FILTERS" :key="name">
      {{ t(`field.${name}`) }}
      <input v-model="draft[name]" :name="name" />
    </label>
    <button type="submit">{{ t("list.apply") }}</button>
  </form>
  <AnomaliesPanel :filters="filters" />
</template>
