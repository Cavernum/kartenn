<script setup lang="ts">
import { SDK_VERSION } from "@kartenn/sdk";
import { onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import shell from "../../package.json";

type Named = { name: string; version: string };

const { t } = useI18n();
const server = ref<string | null>(null);
const plugins = ref<Named[]>([]);
const timeZone = Intl.DateTimeFormat().resolvedOptions().timeZone;

// Loaded and set-aside plugins, with their motive, come with the loader (task 15).
onMounted(async () => {
  const response = await fetch("/v1/capabilities");
  if (!response.ok) return;
  const capabilities = (await response.json()) as { modules?: Named[]; ui_plugins?: Named[] };
  server.value = capabilities.modules?.find((m) => m.name === "core")?.version ?? null;
  plugins.value = capabilities.ui_plugins ?? [];
});
</script>

<template>
  <h1>{{ t("about.title") }}</h1>
  <dl>
    <dt>{{ t("about.version") }}</dt>
    <dd>{{ shell.version }}</dd>
    <dt>{{ t("about.sdk") }}</dt>
    <dd>{{ SDK_VERSION }}</dd>
    <template v-if="server">
      <dt>{{ t("about.server") }}</dt>
      <dd>{{ server }}</dd>
    </template>
  </dl>
  <p class="k-muted">{{ t("about.timeZone", { zone: timeZone }) }}</p>
  <h2>{{ t("about.plugins") }}</h2>
  <ul v-if="plugins.length > 0">
    <li v-for="plugin in plugins" :key="plugin.name">{{ plugin.name }} {{ plugin.version }}</li>
  </ul>
  <p v-else>{{ t("about.noPlugins") }}</p>
</template>
