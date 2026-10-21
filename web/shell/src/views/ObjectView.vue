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
import { computed, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useRoute } from "vue-router";
import ExtensionElement from "../plugins/ExtensionElement.vue";
import { extensionsOf, pluginKey, usePlugins, type Placed } from "../plugins/registry.ts";
import { useSession } from "../rights/index.ts";
import { datedSearch, useTime, useWrites } from "../time/index.ts";

// The record of an object, with the extension points of plugins (REQ-007-13): tabs,
// side panels, actions. Its own attributes come with the lists and records of task 17.

const TYPES: Record<string, string> = {
  entities: "entity",
  sites: "site",
  domains: "domain",
  prefixes: "prefix",
  ranges: "range",
  addresses: "address",
};

const { t } = useI18n();
const route = useRoute();
const session = useSession();
const plugins = usePlugins();
const time = useTime();
// Under `as_of`, the actions of plugins are writes refused like the others (REQ-007-23).
const writes = useWrites();

const collection = computed(() => String(route.params.collection));
const object = computed<ObjectRef>(() => ({
  type: TYPES[collection.value] ?? collection.value,
  id: String(route.params.id),
  href: `/v1/${collection.value}/${String(route.params.id)}`,
}));
const fields = ref<[string, string][]>([]);
const missing = ref(false);

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
    const response = await fetch(object.value.href + datedSearch(time.value)).catch(() => null);
    if (!response) return;
    missing.value = !response.ok;
    const body = response.ok
      ? ((await response.json().catch(() => ({}))) as Record<string, unknown>)
      : {};
    fields.value = Object.entries(body)
      .filter(([key, value]) => key !== "links" && (typeof value !== "object" || value === null))
      .map(([key, value]) => [key, value === null ? "—" : String(value)]);
  },
  { immediate: true },
);
</script>

<template>
  <h1>{{ t("object.title", { type: t(`object.types.${object.type}`), id: object.id }) }}</h1>
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
      <TabsRoot default-value="attributes">
        <TabsList class="k-tabs" :aria-label="t('object.tabs')">
          <TabsTrigger class="k-tab" value="attributes">{{ t("object.attributes") }}</TabsTrigger>
          <TabsTrigger
            v-for="tab in tabs"
            :key="`${tab.plugin}.${tab.id}`"
            class="k-tab"
            :value="`${tab.plugin}.${tab.id}`"
          >
            {{ t(pluginKey(tab, tab.label)) }}
          </TabsTrigger>
        </TabsList>
        <TabsContent value="attributes">
          <dl class="k-fields">
            <template v-for="[key, value] in fields" :key="key">
              <dt>{{ key }}</dt>
              <dd>{{ value }}</dd>
            </template>
          </dl>
        </TabsContent>
        <TabsContent
          v-for="tab in tabs"
          :key="`${tab.plugin}.${tab.id}`"
          :value="`${tab.plugin}.${tab.id}`"
        >
          <ExtensionElement :tag="tab.element" :object="object" />
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
