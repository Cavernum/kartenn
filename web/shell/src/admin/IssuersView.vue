<script setup lang="ts">
import { computed, reactive, shallowRef } from "vue";
import { useI18n } from "vue-i18n";
import { Pager, withQuery } from "../data/api.ts";
import { useWrites } from "../time/index.ts";
import PagedTable from "./PagedTable.vue";
import WriteOutcome from "./WriteOutcome.vue";
import { useAdminWrite } from "./write.ts";

// Attachments of OIDC issuers to entities or to the whole organisation (REQ-007-19,
// spec 005, ADR 0021): listed, attached, detached.

interface Attachment extends Record<string, unknown> {
  issuer: string;
  entity?: string | null;
}

const URL = withQuery("/v1/issuer-attachments", { page_size: "100" });
const { t } = useI18n();
const writes = useWrites();
const write = useAdminWrite();
const pager = shallowRef(new Pager<Attachment>(URL));
const form = reactive({ issuer: "", entity: "", motive: "" });

const columns = computed(() => [
  { key: "issuer", label: t("admin.issuer") },
  { key: "entity", label: t("admin.entity") },
]);
const cell = (row: Attachment, key: string): string =>
  key === "entity" ? (row.entity ?? t("admin.scopes.global")) : String(row[key] ?? "—");

function reload(): void {
  pager.value = new Pager<Attachment>(URL);
  void pager.value.more();
}

async function attach(): Promise<void> {
  const done = await write.run("POST", "/v1/issuer-attachments", {
    motive: form.motive,
    issuer: form.issuer,
    entity: form.entity || null,
  });
  if (done) {
    form.motive = "";
    reload();
  }
}

async function detach(row: Attachment): Promise<void> {
  const url = withQuery("/v1/issuer-attachments", { issuer: row.issuer, entity: row.entity });
  if (await write.run("DELETE", url, { motive: form.motive })) reload();
}
</script>

<template>
  <h1>{{ t("admin.issuers") }}</h1>
  <PagedTable :pager="pager" :columns="columns" :caption="t('admin.issuers')" :cell="cell">
    <template #actions="{ row }">
      <button type="button" :disabled="!form.motive || !writes.quickEdit" @click="detach(row)">
        {{ t("admin.detach", { name: row.issuer }) }}
      </button>
    </template>
  </PagedTable>
  <h2>{{ t("admin.attach") }}</h2>
  <form class="k-form" :aria-label="t('admin.attach')" @submit.prevent="attach">
    <label>
      {{ t("admin.issuer") }}
      <input v-model="form.issuer" name="issuer" required />
    </label>
    <label>
      {{ t("admin.entityHint") }}
      <input v-model="form.entity" name="entity" />
    </label>
    <label>
      {{ t("admin.motive") }}
      <input v-model="form.motive" name="motive" required />
    </label>
    <button type="submit" :disabled="write.busy.value || !writes.quickEdit">
      {{ t("admin.save") }}
    </button>
  </form>
  <WriteOutcome :outcome="write.outcome.value" :problem="write.problem.value" />
</template>
