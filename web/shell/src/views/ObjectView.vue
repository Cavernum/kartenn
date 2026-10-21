<script setup lang="ts">
import type { ObjectRef } from "@kartenn/sdk";
import {
  DialogContent,
  DialogOverlay,
  DialogPortal,
  DialogRoot,
  DialogTitle,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuPortal,
  DropdownMenuRoot,
  DropdownMenuTrigger,
  TabsContent,
  TabsList,
  TabsRoot,
  TabsTrigger,
} from "reka-ui";
import { computed, ref, shallowRef, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useRoute } from "vue-router";
import { read } from "../data/api.ts";
import { cell, linkOf, nameOf, TYPES, type Collection } from "../data/collections.ts";
import ExtensionElement from "../plugins/ExtensionElement.vue";
import { extensionsOf, pluginKey, usePlugins, type Placed } from "../plugins/registry.ts";
import AnomaliesPanel from "../record/AnomaliesPanel.vue";
import ContentPanel from "../record/ContentPanel.vue";
import HistoryPanel from "../record/HistoryPanel.vue";
import OccupancyPanel from "../record/OccupancyPanel.vue";
import PropertiesPanel from "../record/PropertiesPanel.vue";
import { useSession } from "../rights/index.ts";
import { datedSearch, useTime, useWrites } from "../time/index.ts";

// The record of an object of the core (REQ-007-17): attributes, resolved properties and
// their explanation, history, anomalies, what a domain or prefix holds, occupancy and next
// free of a prefix; with the extension points of plugins (REQ-007-13): tabs, side panels,
// actions.

type Item = Record<string, unknown>;

const { t } = useI18n();
const route = useRoute();
const session = useSession();
const plugins = usePlugins();
const time = useTime();
// Under `as_of`, the actions of plugins are writes refused like the others (REQ-007-23).
const writes = useWrites();

const collection = computed(() => String(route.params.collection) as Collection);
const object = computed<ObjectRef>(() => ({
  type: TYPES[collection.value] ?? collection.value,
  id: String(route.params.id),
  href: `/v1/${collection.value}/${String(route.params.id)}`,
}));
const item = shallowRef<Item | null>(null);
const missing = ref(false);
const tab = ref("attributes");

const tabs = computed(() =>
  extensionsOf(plugins.value, "tab", session.value.rights, object.value.type),
);
const panels = computed(() =>
  extensionsOf(plugins.value, "panel", session.value.rights, object.value.type),
);
const actions = computed(() =>
  extensionsOf(plugins.value, "action", session.value.rights, object.value.type),
);
const open = ref<Placed | null>(null);

watch(
  [object, () => time.value.validAt, () => time.value.asOf],
  async () => {
    const answer = await read<Item>(object.value.href + datedSearch(time.value));
    missing.value = !answer.ok;
    item.value = answer.ok ? answer.body : null;
  },
  { immediate: true },
);
watch(object, () => (tab.value = "attributes"));

const fields = computed(() =>
  item.value ? Object.keys(item.value).filter((key) => key !== "links" && key !== "id") : [],
);
/** Where a domain's or a prefix's content is listed, if it has one. */
const content = computed(() => {
  if (collection.value === "domains") return `${object.value.href}/roots`;
  if (collection.value === "prefixes") return `${object.value.href}/children`;
  return null;
});
/** The anomalies of the object: by its domain, by itself for a domain, by entity. */
const anomalyFilters = computed<Record<string, string | null>>(() => {
  const value = item.value;
  if (!value) return {};
  if (collection.value === "domains") return { domain: object.value.id };
  if (collection.value === "entities") return { entity: object.value.id };
  const domain = value.domain as { id?: string } | undefined;
  return domain?.id ? { domain: domain.id } : {};
});
const heading = computed(() =>
  t("object.title", {
    type: t(`object.types.${object.value.type}`),
    id: item.value ? nameOf(item.value) : object.value.id,
  }),
);
</script>

<template>
  <h1>{{ heading }}</h1>
  <p v-if="missing" class="k-muted">{{ t("object.missing") }}</p>
  <div class="k-object">
    <div class="k-object-main">
      <div v-if="actions.length > 0" class="k-toolbar">
        <DropdownMenuRoot :modal="false">
          <DropdownMenuTrigger :disabled="!writes.draft">
            {{ t("object.actions") }}
          </DropdownMenuTrigger>
          <DropdownMenuPortal>
            <DropdownMenuContent class="k-menu" align="start" :side-offset="4">
              <DropdownMenuItem
                v-for="action in actions"
                :key="`${action.plugin}.${action.id}`"
                class="k-menu-item"
                @select="open = action"
              >
                {{ t(pluginKey(action, action.label)) }}
              </DropdownMenuItem>
            </DropdownMenuContent>
          </DropdownMenuPortal>
        </DropdownMenuRoot>
      </div>
      <TabsRoot v-model="tab">
        <TabsList class="k-tabs" :aria-label="t('object.tabs')">
          <TabsTrigger class="k-tab" value="attributes">{{ t("object.attributes") }}</TabsTrigger>
          <TabsTrigger v-if="content" class="k-tab" value="content">
            {{ t("record.content") }}
          </TabsTrigger>
          <TabsTrigger class="k-tab" value="properties">{{ t("record.properties") }}</TabsTrigger>
          <TabsTrigger class="k-tab" value="history">{{ t("record.history") }}</TabsTrigger>
          <TabsTrigger class="k-tab" value="anomalies">{{ t("record.anomalies") }}</TabsTrigger>
          <TabsTrigger
            v-for="extension in tabs"
            :key="`${extension.plugin}.${extension.id}`"
            class="k-tab"
            :value="`${extension.plugin}.${extension.id}`"
          >
            {{ t(pluginKey(extension, extension.label)) }}
          </TabsTrigger>
        </TabsList>
        <TabsContent value="attributes">
          <dl v-if="item" class="k-fields">
            <template v-for="key in fields" :key="key">
              <dt>{{ t(`field.${key}`) }}</dt>
              <dd>
                <RouterLink v-if="linkOf(item, key)" :to="linkOf(item, key)!">
                  {{ cell(item, key) }}
                </RouterLink>
                <template v-else>{{ cell(item, key) }}</template>
              </dd>
            </template>
            <dt>{{ t("field.id") }}</dt>
            <dd>{{ object.id }}</dd>
          </dl>
          <OccupancyPanel v-if="collection === 'prefixes' && item" :href="object.href" />
        </TabsContent>
        <TabsContent v-if="content" value="content">
          <ContentPanel :url="content" :caption="t('record.content')" />
        </TabsContent>
        <TabsContent value="properties">
          <PropertiesPanel :href="object.href" />
        </TabsContent>
        <TabsContent value="history">
          <HistoryPanel :href="object.href" />
        </TabsContent>
        <TabsContent value="anomalies">
          <AnomaliesPanel v-if="item" :filters="anomalyFilters" :object="object.id" />
        </TabsContent>
        <TabsContent
          v-for="extension in tabs"
          :key="`${extension.plugin}.${extension.id}`"
          :value="`${extension.plugin}.${extension.id}`"
        >
          <ExtensionElement :tag="extension.element" :object="object" />
        </TabsContent>
      </TabsRoot>
    </div>
    <aside v-if="panels.length > 0" class="k-object-side" :aria-label="t('object.panels')">
      <section v-for="panel in panels" :key="`${panel.plugin}.${panel.id}`">
        <ExtensionElement :tag="panel.element" :object="object" />
      </section>
    </aside>
  </div>
  <DialogRoot :open="open !== null" @update:open="(value) => !value && (open = null)">
    <DialogPortal>
      <DialogOverlay class="k-overlay" />
      <DialogContent v-if="open" class="k-dialog" :aria-describedby="undefined">
        <DialogTitle>{{ t(pluginKey(open, open.label)) }}</DialogTitle>
        <ExtensionElement :tag="open.element" :object="object" @close="open = null" />
      </DialogContent>
    </DialogPortal>
  </DialogRoot>
</template>
