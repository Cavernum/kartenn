<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { useRoute } from "vue-router";
import { useSession } from "../rights/index.ts";
import ExtensionElement from "./ExtensionElement.vue";
import { extensionsOf, usePlugins } from "./registry.ts";

// A full-page view of a plugin, `/x/<plugin>/<id>/*` (point `view`, REQ-007-13).

const { t } = useI18n();
const route = useRoute();
const session = useSession();
const plugins = usePlugins();

const name = computed(() => String(route.params.plugin));
const status = computed(() => plugins.value.find((p) => p.plugin.name === name.value)?.status);
const view = computed(() =>
  extensionsOf(plugins.value, "view", session.value.rights).find(
    (e) => e.plugin === name.value && e.id === String(route.params.id),
  ),
);
</script>

<template>
  <ExtensionElement v-if="view" :key="view.element" :tag="view.element" />
  <p v-else-if="status?.state === 'loading'" class="k-muted">{{ t("app.loading") }}</p>
  <template v-else>
    <h1>{{ t("notFound.title") }}</h1>
    <RouterLink to="/">{{ t("notFound.back") }}</RouterLink>
  </template>
</template>
