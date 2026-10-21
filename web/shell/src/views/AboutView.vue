<script setup lang="ts">
import { SDK_VERSION } from "@kartenn/sdk";
import { onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import shell from "../../package.json";
import { usePlugins } from "../plugins/registry.ts";

const { t } = useI18n();
const server = ref<string | null>(null);
const plugins = usePlugins();
const timeZone = Intl.DateTimeFormat().resolvedOptions().timeZone;

onMounted(async () => {
  const response = await fetch("/v1/capabilities").catch(() => null);
  if (!response?.ok) return;
  const capabilities = (await response.json().catch(() => ({}))) as {
    modules?: { name: string; version: string }[];
  };
  server.value = capabilities.modules?.find((m) => m.name === "core")?.version ?? null;
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
  <!-- Loaded and set-aside plugins, with their reason (REQ-007-10). -->
  <ul v-if="plugins.length > 0">
    <li v-for="{ plugin, status } in plugins" :key="plugin.name">
      {{ plugin.name }} {{ plugin.version }} —
      <template v-if="status.state === 'rejected'">
        {{ t("about.rejected", { reason: t(`plugins.reason.${status.reason}`) }) }}
      </template>
      <template v-else>{{ t(`plugins.${status.state}`) }}</template>
    </li>
  </ul>
  <p v-else>{{ t("about.noPlugins") }}</p>
</template>
