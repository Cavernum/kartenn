<script setup lang="ts">
import { computed, shallowRef } from "vue";
import { useI18n } from "vue-i18n";
import { Pager, withQuery } from "../data/api.ts";
import type { Identity } from "./admin.ts";
import PagedTable from "./PagedTable.vue";

// The identities (REQ-007-19, spec 005): people, service and break-glass accounts, their
// state; a row opens the identity.

const { t } = useI18n();
const pager = shallowRef(new Pager<Identity>(withQuery("/v1/identities", { page_size: "100" })));
const columns = computed(() => [
  { key: "name", label: t("admin.name") },
  { key: "kind", label: t("admin.kind") },
  { key: "issuer", label: t("admin.issuer") },
  { key: "disabled", label: t("admin.state") },
]);
const cell = (identity: Identity, key: string): string => {
  if (key === "disabled") return identity.disabled ? t("admin.disabled") : t("admin.enabled");
  return String(identity[key] ?? "—");
};
const href = (identity: Identity): string => `/admin/identities/${identity.id}`;
</script>

<template>
  <h1>{{ t("admin.identities") }}</h1>
  <PagedTable
    :pager="pager"
    :columns="columns"
    :caption="t('admin.identities')"
    :cell="cell"
    :href="href"
  />
</template>
