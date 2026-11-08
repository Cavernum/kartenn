<script setup lang="ts">
// The open anomalies of an object (REQ-021-9; maquette E2, onglet Anomalies) and their
// acknowledgement, read page by page as the grid's window nears the end: those of a domain
// or an entity are its own filter, each row with its object; those of any other object
// its filter `object` (021/A37), within a domain or outside any (a site). An object the
// caller may not read lists none, as an unknown one. A refused cursor starts the pages
// again from the first, as the lists do (`list.ts`).
import { queryClient, reads } from "@kartenn/sdk";
import { EmptyState, type GridWindow } from "@kartenn/ui";
import { computed, nextTick, ref, useTemplateRef, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useInfiniteQuery } from "../../data/query.ts";
import { useNextPage, type ListWindow } from "../../prefetch/nextPage.ts";
import { problemSlug } from "../../problems/index.ts";
import { useTime } from "../../time/index.ts";
import AnomalyGrid from "../anomalies/AnomalyGrid.vue";
import { useFocusWhenEmptied, type Anomaly } from "../anomalies/anomaly.ts";
import NextPageAlert from "../../problems/NextPageAlert.vue";
import ProblemState from "../../problems/ProblemState.vue";
import { rowsOf } from "../data.ts";

const props = defineProps<{
  kind: string;
  id: string;
  name: string;
}>();
const { t } = useI18n();
const time = useTime();

/** A domain or an entity lists those of the objects it holds, each with its object. */
const holder = computed(() => props.kind === "domain" || props.kind === "entity");
const filter = computed(() =>
  props.kind === "domain"
    ? { domain: props.id }
    : props.kind === "entity"
      ? { entity: props.id }
      : { object: props.id },
);
const read = () =>
  reads.list("/v1/anomalies", filter.value, {
    validAt: time.value.validAt,
    asOf: time.value.asOf,
  });
const list = useInfiniteQuery(read);
// A refused cursor (expired, or of another list): the pages read again from the first,
// and said. Once by itself; refused again, the alert offers to start again, never the
// refused cursor resent.
const restarted = ref(false);
/** What the status says: emptied then set again, so that each restart is announced. */
const said = ref("");
// Until the next read, its filter or its dates changed.
watch(
  () => JSON.stringify(read().queryKey),
  () => {
    restarted.value = false;
    said.value = "";
  },
);
const cursorRefused = computed(
  () => list.isFetchNextPageError.value && problemSlug(list.error.value) === "invalid-cursor",
);
function restart(): void {
  restarted.value = true;
  said.value = "";
  void nextTick(() => (said.value = t("views.list.restarted")));
  void queryClient.resetQueries({ queryKey: read().queryKey, exact: true });
}
watch(cursorRefused, (refused) => {
  if (refused && !restarted.value) restart();
});
/** A next page refused, the rows kept: said above them, never the refused cursor resent. */
const nextFailed = computed(
  () => list.isFetchNextPageError.value && !(cursorRefused.value && !restarted.value),
);
const retryNext = () => (cursorRefused.value ? restart() : void list.fetchNextPage());
// The next page as the window nears the end.
const view = ref<ListWindow>({ end: 0, size: 0 });
useNextPage(list, view);
const onWindow = (window: GridWindow) => (view.value = { end: window.end, size: window.size });
const anomalies = computed(() => rowsOf(list.data.value) as Anomaly[]);
const emptied = useFocusWhenEmptied(() => anomalies.value, useTemplateRef<HTMLElement>("empty"));
</script>

<template>
  <NextPageAlert v-if="nextFailed" :error="list.error.value" :retry="retryNext" />
  <ProblemState
    v-if="list.isError.value && !list.isFetchNextPageError.value"
    :title="t('views.anomalies.error')"
    :error="list.error.value"
    :retry="list.refetch"
  />
  <!-- A next page is asked only with rows in view: none read is none at all. -->
  <div v-else-if="list.isSuccess.value && anomalies.length === 0" ref="empty" tabindex="-1">
    <EmptyState icon="circle-check" :title="t('views.anomalies.noneOf')" />
  </div>
  <AnomalyGrid
    v-else
    :label="t('views.anomalies.of', { name: props.name })"
    :anomalies="anomalies"
    :loading="list.isPending.value"
    :with-object="holder"
    @window="onWindow"
    @acknowledged="emptied.arm()"
  />
  <span class="k-visually-hidden" role="status">{{ said }}</span>
</template>
