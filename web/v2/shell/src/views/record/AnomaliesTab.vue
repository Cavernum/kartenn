<script setup lang="ts">
// The open anomalies of an object (REQ-021-9; maquette E2, onglet Anomalies) and their
// acknowledgement. The list has no filter by object (a gap of the contract 006): those of a
// domain or an entity are its own filter, read page by page as the grid's window nears the
// end; those of an object of a domain are read in its domain's, page after page (ten at
// most), and kept to that object; those of an object outside any domain (a site) cannot be
// read, which the tab says. A refused cursor starts the pages again from the first, as the
// lists do (`list.ts`).
import { queryClient, reads } from "@kartenn/sdk";
import { EmptyState, InlineAlert, type GridWindow } from "@kartenn/ui";
import { computed, nextTick, ref, useTemplateRef, watch, watchEffect } from "vue";
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
const read = () =>
  reads.list("/v1/anomalies", filter.value ?? {}, {
    validAt: time.value.validAt,
    asOf: time.value.asOf,
  });
const list = useInfiniteQuery(() => ({ ...read(), enabled: filter.value !== null }));
// A refused cursor (expired, or of another list): the pages read again from the first,
// and said. Once by itself (the pages of an object's domain are read without the reader,
// a cursor always refused would loop); refused again, the alert offers to start again.
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
// The anomalies of a domain or an entity: the next page as the window nears the end.
const view = ref<ListWindow>({ end: 0, size: 0 });
useNextPage(list, () => (own.value ? view.value : { end: 0, size: 0 }));
const onWindow = (window: GridWindow) => (view.value = { end: window.end, size: window.size });
// Those of an object of a domain: its domain's pages, ten at most, kept to the object.
watchEffect(() => {
  const pages = list.data.value?.pages.length ?? 0;
  // A page refused is not asked again by itself: the alert offers to (`NextPageAlert`).
  const refused = list.isFetchNextPageError.value;
  if (!own.value && list.hasNextPage.value && !list.isFetching.value && !refused && pages < PAGES) {
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
const emptied = useFocusWhenEmptied(() => anomalies.value, useTemplateRef<HTMLElement>("empty"));
</script>

<template>
  <EmptyState v-if="filter === null" icon="circle-dashed" :title="t('views.anomalies.unreadable')">
    {{ t("views.anomalies.unreadableText") }}
  </EmptyState>
  <template v-else>
    <InlineAlert v-if="partial" tone="warn" :title="t('views.anomalies.partialTitle')">
      {{ t("views.anomalies.partial", { n: PAGES }) }}
    </InlineAlert>
    <NextPageAlert v-if="nextFailed" :error="list.error.value" :retry="retryNext" />
    <ProblemState
      v-if="list.isError.value && !list.isFetchNextPageError.value"
      :title="t('views.anomalies.error')"
      :error="list.error.value"
      :retry="list.refetch"
    />
    <div
      v-else-if="(list.isSuccess.value || nextFailed) && anomalies.length === 0"
      ref="empty"
      tabindex="-1"
    >
      <!-- Pages left unread (the ten read, or the next one refused): « in the pages read ». -->
      <EmptyState
        icon="circle-check"
        :title="partial || nextFailed ? t('views.anomalies.noneRead') : t('views.anomalies.noneOf')"
      />
    </div>
    <AnomalyGrid
      v-else
      :label="t('views.anomalies.of', { name: props.name })"
      :anomalies="anomalies"
      :loading="list.isPending.value"
      :with-object="own"
      @window="onWindow"
      @acknowledged="emptied.arm()"
    />
    <span class="k-visually-hidden" role="status">{{ said }}</span>
  </template>
</template>
