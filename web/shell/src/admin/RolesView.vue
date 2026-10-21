<script setup lang="ts">
import { computed, reactive, shallowRef } from "vue";
import { useI18n } from "vue-i18n";
import { Pager, withQuery } from "../data/api.ts";
import { useWrites } from "../time/index.ts";
import { entriesOf } from "./admin.ts";
import PagedTable from "./PagedTable.vue";
import WriteOutcome from "./WriteOutcome.vue";
import { useAdminWrite } from "./write.ts";

// Roles (REQ-007-19, spec 005): the built-in ones and those of the organisation, a role
// defined or changed (`PUT /v1/roles/{key}`), a role of the organisation closed.

interface Role extends Record<string, unknown> {
  key: string;
  label: string;
  builtin: boolean;
  entries: string[];
}

const { t } = useI18n();
const writes = useWrites();
const write = useAdminWrite();
const pager = shallowRef(new Pager<Role>(withQuery("/v1/roles", { page_size: "100" })));
const form = reactive({ key: "", label: "", entries: "", motive: "" });

const columns = computed(() => [
  { key: "key", label: t("admin.key") },
  { key: "label", label: t("admin.label") },
  { key: "builtin", label: t("admin.builtin") },
  { key: "entries", label: t("admin.entries") },
]);
const cell = (role: Role, key: string): string =>
  key === "builtin"
    ? role.builtin
      ? t("admin.yes")
      : t("admin.no")
    : key === "entries"
      ? role.entries.join(", ")
      : String(role[key] ?? "—");

function reload(): void {
  pager.value = new Pager<Role>(withQuery("/v1/roles", { page_size: "100" }));
  void pager.value.more();
}

function edit(role: Role): void {
  Object.assign(form, { key: role.key, label: role.label, entries: role.entries.join("\n") });
}

async function save(): Promise<void> {
  const done = await write.run("PUT", `/v1/roles/${encodeURIComponent(form.key)}`, {
    motive: form.motive,
    label: form.label,
    entries: entriesOf(form.entries),
  });
  if (done) {
    form.motive = "";
    reload();
  }
}

async function close(role: Role): Promise<void> {
  if (!form.motive) return;
  if (
    await write.run("DELETE", `/v1/roles/${encodeURIComponent(role.key)}`, { motive: form.motive })
  )
    reload();
}
</script>

<template>
  <h1>{{ t("admin.roles") }}</h1>
  <PagedTable :pager="pager" :columns="columns" :caption="t('admin.roles')" :cell="cell">
    <template #actions="{ row }">
      <div v-if="!row.builtin" class="k-toolbar">
        <button type="button" @click="edit(row)">{{ t("admin.edit", { name: row.key }) }}</button>
        <button type="button" :disabled="!form.motive || !writes.quickEdit" @click="close(row)">
          {{ t("admin.close", { name: row.key }) }}
        </button>
      </div>
    </template>
  </PagedTable>
  <h2>{{ t("admin.defineRole") }}</h2>
  <form class="k-form" :aria-label="t('admin.defineRole')" @submit.prevent="save">
    <label>
      {{ t("admin.key") }}
      <input v-model="form.key" name="key" required pattern="[a-z][a-z0-9_-]*" />
    </label>
    <label>
      {{ t("admin.label") }}
      <input v-model="form.label" name="label" required />
    </label>
    <label>
      {{ t("admin.entriesHint") }}
      <textarea v-model="form.entries" name="entries" rows="5"></textarea>
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
