<script setup lang="ts">
import { computed, reactive, shallowRef } from "vue";
import { useI18n } from "vue-i18n";
import { Pager, withQuery } from "../data/api.ts";
import { useWrites } from "../time/index.ts";
import type { Account, SecretShown } from "./admin.ts";
import PagedTable from "./PagedTable.vue";
import SecretDialog from "./SecretDialog.vue";
import WriteOutcome from "./WriteOutcome.vue";
import { useAdminWrite } from "./write.ts";

// Service accounts (REQ-007-19, spec 005, K8): listed, created with their first secret,
// shown once.

const URL = withQuery("/v1/service-accounts", { page_size: "100" });
const { t } = useI18n();
const writes = useWrites();
const write = useAdminWrite();
const pager = shallowRef(new Pager<Account>(URL));
const form = reactive({ name: "", expires: "", motive: "" });
const secret = shallowRef<SecretShown | null>(null);

const columns = computed(() => [
  { key: "name", label: t("admin.name") },
  { key: "disabled", label: t("admin.state") },
  { key: "secrets", label: t("admin.secrets") },
]);
const cell = (account: Account, key: string): string => {
  if (key === "disabled") return account.disabled ? t("admin.disabled") : t("admin.enabled");
  if (key === "secrets") return String(account.secrets.filter((s) => !s.revoked).length);
  return String(account[key] ?? "—");
};
const href = (account: Account): string => `/admin/service-accounts/${account.id}`;

async function create(): Promise<void> {
  const done = await write.run("POST", "/v1/service-accounts", {
    motive: form.motive,
    name: form.name,
    expires_at: form.expires || null,
  });
  if (!done) return;
  secret.value = done.secret;
  Object.assign(form, { name: "", expires: "", motive: "" });
  pager.value = new Pager<Account>(URL);
  void pager.value.more();
}
</script>

<template>
  <h1>{{ t("admin.serviceAccounts") }}</h1>
  <PagedTable
    :pager="pager"
    :columns="columns"
    :caption="t('admin.serviceAccounts')"
    :cell="cell"
    :href="href"
  />
  <h2>{{ t("admin.newAccount") }}</h2>
  <form class="k-form" :aria-label="t('admin.newAccount')" @submit.prevent="create">
    <label>
      {{ t("admin.name") }}
      <input v-model="form.name" name="name" required pattern="[^:]+" />
    </label>
    <label>
      {{ t("admin.expiresHint") }}
      <input v-model="form.expires" name="expires" placeholder="2027-01-01T00:00:00Z" />
    </label>
    <label>
      {{ t("admin.motive") }}
      <input v-model="form.motive" name="motive" required />
    </label>
    <button type="submit" :disabled="write.busy.value || !writes.quickEdit">
      {{ t("admin.create") }}
    </button>
  </form>
  <WriteOutcome :outcome="write.outcome.value" :problem="write.problem.value" />
  <SecretDialog :secret="secret" @close="secret = null" />
</template>
