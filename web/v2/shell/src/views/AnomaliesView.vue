<script setup lang="ts">
// Anomalies and gaps (REQ-021-9; design 021 « Catalogue des écrans » : `/anomalies`,
// `?tab=gaps`): the warnings of the rules, which an acknowledgement with a motive closes,
// and the expected properties missing; one tab each, the tab in the URL.
import { Tabs, type TabItem } from "@kartenn/ui";
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { useRoute, useRouter } from "vue-router";
import AnomalyList from "./anomalies/AnomalyList.vue";
import GapList from "./anomalies/GapList.vue";
import ViewPanel from "./parts/ViewPanel.vue";

const { t } = useI18n();
const route = useRoute();
const router = useRouter();
const tabs = computed<TabItem[]>(() => [
  { value: "anomalies", label: t("nav.anomalies") },
  { value: "gaps", label: t("views.anomalies.gaps") },
]);
const tab = computed({
  get: () => (route.query.tab === "gaps" ? "gaps" : "anomalies"),
  // Each tab has its own filters: the other's are left behind, the dates of the page kept.
  set: (next: string) => {
    const { valid_at, as_of } = route.query;
    const dates = { ...(valid_at ? { valid_at } : {}), ...(as_of ? { as_of } : {}) };
    void router.replace({ query: next === "gaps" ? { ...dates, tab: "gaps" } : dates });
  },
});
</script>

<template>
  <h1 class="k-visually-hidden">{{ t("nav.anomalies") }}</h1>
  <ViewPanel :title="t('nav.anomalies')" main>
    <Tabs v-model="tab" :tabs="tabs" :label="t('views.anomalies.sections')">
      <template #anomalies><AnomalyList /></template>
      <template #gaps><GapList /></template>
    </Tabs>
  </ViewPanel>
</template>
