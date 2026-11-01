<script setup lang="ts">
// The open anomalies of an object (REQ-021-9; maquette E2, onglet Anomalies) and their
// acknowledgement. The list has no filter by object: those of a domain or an entity are
// its own filter; those of an object of a domain are read in its domain's, page after page
// (ten at most), and kept to that object.
import { reads } from "@kartenn/sdk";
import { EmptyState, ErrorState, InlineAlert } from "@kartenn/ui";
import { computed, watchEffect } from "vue";
import { useI18n } from "vue-i18n";
import { useInfiniteQuery } from "../../data/query.ts";
import { useTime } from "../../time/index.ts";
import AnomalyGrid from "../anomalies/AnomalyGrid.vue";
import type { Anomaly } from "../anomalies/anomaly.ts";
import { problemOf, rowsOf } from "../data.ts";

const props = defineProps<{
  kind: string;
  id: string;
  /** The domain of the object, when it lies in one. */
  domain?: string | undefined;
  name: string;
}>();
const { t } = useI18n();
const time = useTime();

/** How many pages of a domain's anomalies are read for one of its objects. */
const PAGES = 10;
const filter = computed(() =>
  props.kind === "domain"
    ? { domain: props.id }
    : props.kind === "entity"
      ? { entity: props.id }
      : props.domain
        ? { domain: props.domain }
        : null,
);
const own = computed(() => props.kind === "domain" || props.kind === "entity");
const list = useInfiniteQuery(() => ({
  ...reads.list("/v1/anomalies", filter.value ?? {}, {
    validAt: time.value.validAt,
    asOf: time.value.asOf,
  }),
  enabled: filter.value !== null,
}));
watchEffect(() => {
  const pages = list.data.value?.pages.length ?? 0;
  if (!own.value && list.hasNextPage.value && !list.isFetching.value && pages < PAGES) {
    void list.fetchNextPage();
  }
});
const anomalies = computed(() =>
  (rowsOf(list.data.value) as Anomaly[]).filter(
    (anomaly) => own.value || anomaly.object.id === props.id,
  ),
);
/** The domain has more pages than are read for one object: the list may miss some. */
const partial = computed(
  () => !own.value && list.hasNextPage.value && (list.data.value?.pages.length ?? 0) >= PAGES,
);
</script>

<template>
  <InlineAlert v-if="partial" tone="warn" :title="t('views.anomalies.partialTitle')">
    {{ t("views.anomalies.partial", { n: PAGES }) }}
  </InlineAlert>
  <ErrorState
    v-if="list.isError.value && !list.isFetchNextPageError.value"
    :title="t('views.anomalies.error')"
    :problem="problemOf(list.error.value)"
    :retry-label="t('views.retry')"
    @retry="list.refetch()"
  />
  <EmptyState
    v-else-if="filter === null || (list.isSuccess.value && anomalies.length === 0)"
    icon="circle-check"
    :title="t('views.anomalies.noneOf')"
  />
  <AnomalyGrid
    v-else
    :label="t('views.anomalies.of', { name: props.name })"
    :anomalies="anomalies"
    :loading="list.isPending.value"
    :with-object="own"
  />
</template>
