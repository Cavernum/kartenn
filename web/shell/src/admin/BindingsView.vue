<script setup lang="ts">
import { computed, reactive, shallowRef } from "vue";
import { useI18n } from "vue-i18n";
import { Pager, withQuery } from "../data/api.ts";
import { useWrites } from "../time/index.ts";
import PagedTable from "./PagedTable.vue";
import WriteOutcome from "./WriteOutcome.vue";
import { useAdminWrite } from "./write.ts";

// Bindings group × role × scope (REQ-007-19, spec 005; spec 025: a binding serves a group
// of kartenn): listed, posed, closed.

interface Binding extends Record<string, unknown> {
  id: string;
  group: string;
  role: string;
  scope: { kind: string; id?: string | null };
  valid_from: string;
}

const URL = withQuery("/v1/bindings", { page_size: "100" });
const { t } = useI18n();
const writes = useWrites();
const write = useAdminWrite();
const pager = shallowRef(new Pager<Binding>(URL));
const form = reactive({
  group: "",
  role: "",
  scopeKind: "global",
  scopeId: "",
  motive: "",
});

const columns = computed(() => [
  { key: "group", label: t("admin.group") },
  { key: "role", label: t("admin.role") },
  { key: "scope", label: t("admin.scope") },
  { key: "valid_from", label: t("admin.since") },
]);
const cell = (binding: Binding, key: string): string => {
  if (key === "scope")
    return binding.scope.id ? `${binding.scope.kind} ${binding.scope.id}` : binding.scope.kind;
  return String(binding[key] ?? "—");
};

function reload(): void {
  pager.value = new Pager<Binding>(URL);
  void pager.value.more();
}

async function bind(): Promise<void> {
  const scope =
    form.scopeKind === "global" ? { kind: "global" } : { kind: form.scopeKind, id: form.scopeId };
  const done = await write.run("POST", "/v1/bindings", {
    motive: form.motive,
    group: form.group,
    role: form.role,
    scope,
  });
  if (done) {
    form.motive = "";
    reload();
  }
}

async function unbind(binding: Binding): Promise<void> {
  if (await write.run("DELETE", `/v1/bindings/${binding.id}`, { motive: form.motive })) reload();
}
</script>

<template>
  <h1>{{ t("admin.bindings") }}</h1>
  <PagedTable :pager="pager" :columns="columns" :caption="t('admin.bindings')" :cell="cell">
    <template #actions="{ row }">
      <button type="button" :disabled="!form.motive || !writes.quickEdit" @click="unbind(row)">
        {{ t("admin.close", { name: `${row.role} ${row.group}` }) }}
      </button>
    </template>
  </PagedTable>
  <h2>{{ t("admin.bind") }}</h2>
  <form class="k-form" :aria-label="t('admin.bind')" @submit.prevent="bind">
    <label>
      {{ t("admin.group") }}
      <input v-model="form.group" name="group" required />
    </label>
    <label>
      {{ t("admin.role") }}
      <input v-model="form.role" name="role" required />
    </label>
    <label>
      {{ t("admin.scope") }}
      <select v-model="form.scopeKind" name="scope">
        <option value="global">{{ t("admin.scopes.global") }}</option>
        <option value="entity">{{ t("admin.scopes.entity") }}</option>
        <option value="site">{{ t("admin.scopes.site") }}</option>
        <option value="domain">{{ t("admin.scopes.domain") }}</option>
        <option value="prefix">{{ t("admin.scopes.prefix") }}</option>
      </select>
    </label>
    <label v-if="form.scopeKind !== 'global'">
      {{ t("admin.scopeId") }}
      <input v-model="form.scopeId" name="scope-id" required />
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
