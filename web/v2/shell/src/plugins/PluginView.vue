<script setup lang="ts">
// A full-page view of a plugin, `/x/<plugin>/<id>/*` (point `view`, REQ-007-13, 021-13),
// under its space: the element of the plugin, which writes its own heading, created again
// for another path under the view (its records), kept when only the query changes. Before the
// capabilities are read, or while the plugin loads, a skeleton; set aside, the encart of
// its reason with its code; unknown, or its permission not held, nothing at this address.
import { EmptyState, Heading, Skeleton, Stack } from "@kartenn/ui";
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import { useRoute } from "vue-router";
import { useSession } from "../rights/index.ts";
import ExtensionElement from "./ExtensionElement.vue";
import { allowed, labelOf, useCapabilitiesRead, usePlugins } from "./registry.ts";

const { t } = useI18n();
const route = useRoute();
const session = useSession();
const plugins = usePlugins();
const read = useCapabilitiesRead();

const found = computed(() => {
  const entry = plugins.value.find((p) => p.plugin.name === String(route.params.plugin));
  const view = entry?.plugin.extensions.find(
    (e) => e.point === "view" && e.id === String(route.params.view),
  );
  if (!entry || !view || !allowed(view, session.value.rights)) return null;
  return { entry, view: { ...view, plugin: entry.plugin.name } };
});
const title = computed(() => (found.value ? labelOf(t, found.value.view) : ""));
/** Whether an encart stands in the place of the view's element. */
const failed = ref(false);
</script>

<template>
  <template v-if="found">
    <!-- The element writes the heading of its page; until it does, or in its place when
         it failed, the shell's. -->
    <Heading v-if="found.entry.status.state !== 'loaded' || failed" :level="1">
      {{ title }}
    </Heading>
    <!-- One element per address of the view (`/x/<plugin>/<id>/<rest>`): a record of the
         plugin reads its page at its creation, as a screen of the shell. -->
    <ExtensionElement
      :key="`${found.view.element} ${route.path}`"
      :tag="found.view.element"
      :plugin="found.entry.plugin.name"
      :label="title"
      :status="found.entry.status"
      @failed="(now) => (failed = now)"
    />
  </template>
  <template v-else-if="!read">
    <Heading :level="1">{{ t("plugins.loading") }}</Heading>
    <Stack gap="2" aria-busy="true">
      <Skeleton variant="text" :rows="3" />
    </Stack>
  </template>
  <template v-else>
    <Heading :level="1">{{ t("app.notFound") }}</Heading>
    <EmptyState :title="t('app.notFoundTitle')">{{ t("app.notFoundText") }}</EmptyState>
  </template>
</template>
