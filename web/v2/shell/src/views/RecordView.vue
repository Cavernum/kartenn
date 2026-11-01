<script setup lang="ts">
// The record of an object of the referential (REQ-021-9; design 021 « Écrans » : fiche en
// panneaux; maquette E2): its name and kind, the tabs of its sections — resolved properties
// and their explanation, content (a domain, a prefix), history, anomalies — the tab in the
// URL (`?tab=history`); beside them its attributes and, for a prefix, its occupancy and
// next free value. Read at the dates of the page; an object out of reach (404, 403) says
// so with its codes, never a blank page.
import { reads } from "@kartenn/sdk";
import {
  CodeBadge,
  ErrorState,
  KeyLabel,
  Menu,
  Skeleton,
  Split,
  Stack,
  Tabs,
  type TabItem,
} from "@kartenn/ui";
import { computed, ref, watchEffect } from "vue";
import { useI18n } from "vue-i18n";
import { useRoute, useRouter } from "vue-router";
import { useQuery } from "../data/query.ts";
import ExtensionElement from "../plugins/ExtensionElement.vue";
import PluginAction from "../plugins/PluginAction.vue";
import { labelOf, type Placed } from "../plugins/registry.ts";
import {
  actionItems,
  objectRef,
  placed,
  placeId,
  tabItems,
  useExtensions,
} from "../plugins/slots.ts";
import KLink from "../prefetch/KLink.vue";
import { useTime } from "../time/index.ts";
import {
  fieldText,
  isSummary,
  nameOf,
  pageOfHref,
  type Collection,
  type Item,
} from "./collections.ts";
import { problemOf, useKeyLabel } from "./data.ts";
import { keepRecordName } from "./names.ts";
import AttributeList from "./parts/AttributeList.vue";
import ViewPanel from "./parts/ViewPanel.vue";
import AnomaliesTab from "./record/AnomaliesTab.vue";
import ContentTab from "./record/ContentTab.vue";
import HistoryTab from "./record/HistoryTab.vue";
import OccupancyPanel from "./record/OccupancyPanel.vue";
import PropertiesTab from "./record/PropertiesTab.vue";

const props = defineProps<{ collection: Collection; id: string }>();
const { t, te } = useI18n();
const route = useRoute();
const router = useRouter();
const time = useTime();
const keyLabel = useKeyLabel();

const record = useQuery(() =>
  reads.object(props.collection.kind, props.id, {
    validAt: time.value.validAt,
    asOf: time.value.asOf,
  }),
);
const item = computed(() => record.data.value?.data as Item | undefined);
const name = computed(() => (item.value ? nameOf(item.value) : props.id));
watchEffect(() => {
  if (item.value) keepRecordName(props.id, nameOf(item.value));
});
const kindWords = computed(() => keyLabel("kind", props.collection.type) ?? props.collection.kind);
const domain = computed(() => (isSummary(item.value?.domain) ? item.value!.domain : undefined));

/** A key of a closed list of the core in words (`network` → « Réseau »). */
const valueWords = (field: string, value: unknown) => {
  const key = `views.value.${field}.${String(value)}`;
  return te(key) ? t(key) : null;
};
const detail = computed(() => {
  const data = item.value;
  if (!data) return undefined;
  const nature = data.nature ?? data.usage;
  const field = data.nature !== undefined ? "nature" : "usage";
  return [kindWords.value, nature ? (valueWords(field, nature) ?? String(nature)) : null]
    .filter(Boolean)
    .join(" · ");
});

/** The kinds that hold others: a domain its prefixes, a prefix its children. */
const contentKind = computed(() =>
  props.collection.kind === "domain" || props.collection.kind === "prefix"
    ? props.collection.kind
    : null,
);
const hasContent = computed(() => contentKind.value !== null);

// The places of the plugins on the record of this kind (REQ-021-13, task 16): their tabs
// after the core's, their panels beside its own, their actions in the menu; each element
// given the object, its error kept in its place.
const tr = (key: string) => t(key);
const pluginTabs = useExtensions("tab", () => props.collection.kind);
const pluginPanels = useExtensions("panel", () => props.collection.kind);
const pluginActions = useExtensions("action", () => props.collection.kind);
const object = computed(() =>
  objectRef(
    props.collection.kind,
    props.id,
    String(
      (item.value?.links as { self?: unknown } | undefined)?.self ??
        `/v1/${props.collection.id}/${props.id}`,
    ),
  ),
);
/** The menu of actions: those of the plugins, for now (the core's come with task 12). */
const actions = computed(() =>
  actionItems(pluginActions.value, tr).filter((i) => i.kind !== "separator"),
);
const acting = ref<Placed | null>(null);
const actionOpen = ref(false);
function onAction(id: string): void {
  acting.value = placed(pluginActions.value, id) ?? null;
  actionOpen.value = acting.value !== null;
}

const tabs = computed<TabItem[]>(() => [
  { value: "properties", label: t("views.record.properties") },
  ...(hasContent.value ? [{ value: "content", label: t("views.record.content") }] : []),
  { value: "history", label: t("views.record.history") },
  { value: "anomalies", label: t("views.record.anomalies") },
  ...tabItems(pluginTabs.value, tr),
]);
const tab = computed({
  get: () => {
    const asked = route.query.tab;
    return typeof asked === "string" && tabs.value.some((tab) => tab.value === asked)
      ? asked
      : "properties";
  },
  set: (next: string) => {
    const query = { ...route.query, tab: next === "properties" ? undefined : next };
    void router.replace({ query });
  },
});

/** The attributes of each kind, the identifier last (copiable). */
const FIELDS: Record<string, readonly string[]> = {
  domain: ["slug"],
  entity: ["slug", "parent"],
  site: ["slug", "parent"],
  prefix: ["domain", "nature", "point_to_point", "anycast", "public"],
  range: ["domain", "first", "last", "usage", "public"],
  address: ["domain", "usage", "public"],
};
const MONO = new Set(["slug", "first", "last", "parent", "domain"]);
const attributes = computed(() => [
  ...(FIELDS[props.collection.kind] ?? []).map((field) => {
    const value = item.value?.[field];
    return {
      key: field,
      label: t(`views.field.${field}`),
      value: typeof value === "boolean" ? t(`views.value.boolean.${value}`) : fieldText(value),
      mono: MONO.has(field),
    };
  }),
  { key: "id", label: t("views.field.id") },
]);
const linkOf = (field: string) => {
  const value = item.value?.[field];
  return isSummary(value) ? pageOfHref(value.href) : null;
};
</script>

<template>
  <h1 class="k-visually-hidden">{{ kindWords }} {{ name }}</h1>
  <ErrorState
    v-if="record.isError.value"
    :title="t('views.record.error', { kind: kindWords })"
    :problem="problemOf(record.error.value)"
    :retry-label="t('views.retry')"
    @retry="record.refetch()"
  >
    {{ t("views.errorText") }}
  </ErrorState>
  <ViewPanel v-else :title="name" :detail="detail" main>
    <template v-if="actions.length > 0" #actions>
      <Menu
        :label="t('views.actions')"
        icon="ellipsis"
        align="end"
        :items="actions"
        @select="onAction"
      />
    </template>
    <Split side="end" side-size="sm" gap="3">
      <Skeleton v-if="record.isPending.value" variant="rows" :rows="6" />
      <Tabs v-else v-model="tab" :tabs="tabs" :label="t('views.record.sections')">
        <template #properties>
          <PropertiesTab :id="props.id" :kind="props.collection.kind" :name="name" />
        </template>
        <template v-if="hasContent" #content>
          <ContentTab v-if="contentKind" :id="props.id" :kind="contentKind" :name="name" />
        </template>
        <template #history>
          <HistoryTab :id="props.id" :kind="props.collection.kind" :name="name" />
        </template>
        <template #anomalies>
          <AnomaliesTab
            :id="props.id"
            :kind="props.collection.kind"
            :domain="domain?.id"
            :name="name"
          />
        </template>
        <template v-for="extension in pluginTabs" :key="placeId(extension)" #[placeId(extension)]>
          <ExtensionElement
            :tag="extension.element"
            :plugin="extension.plugin"
            :label="labelOf(tr, extension)"
            :object="object"
          />
        </template>
      </Tabs>
      <template #side>
        <Stack gap="3">
          <ViewPanel :title="t('views.record.attributes')">
            <Skeleton v-if="record.isPending.value" variant="text" :rows="4" />
            <AttributeList v-else :items="attributes">
              <template v-for="field in ['domain', 'parent']" :key="field" #[field]>
                <KLink v-if="linkOf(field)" :to="linkOf(field)!">{{
                  fieldText(item?.[field])
                }}</KLink>
                <template v-else>{{ fieldText(item?.[field]) }}</template>
              </template>
              <template #nature>
                <KeyLabel
                  :value="String(item?.nature)"
                  :label="valueWords('nature', item?.nature)"
                />
              </template>
              <template #usage>
                <KeyLabel :value="String(item?.usage)" :label="valueWords('usage', item?.usage)" />
              </template>
              <template #id><CodeBadge :code="props.id" /></template>
            </AttributeList>
          </ViewPanel>
          <OccupancyPanel v-if="props.collection.kind === 'prefix'" :id="props.id" />
          <ViewPanel
            v-for="extension in pluginPanels"
            :key="placeId(extension)"
            :title="labelOf(tr, extension)"
            :detail="t('views.home.plugin', { name: extension.plugin })"
          >
            <ExtensionElement
              :tag="extension.element"
              :plugin="extension.plugin"
              :label="labelOf(tr, extension)"
              :object="object"
            />
          </ViewPanel>
        </Stack>
      </template>
    </Split>
  </ViewPanel>
  <PluginAction v-model:open="actionOpen" :extension="acting" :object="object" />
</template>
