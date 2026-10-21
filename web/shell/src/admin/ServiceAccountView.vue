<script setup lang="ts">
import { computed, reactive, shallowRef, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useRoute } from "vue-router";
import { read, type Problem } from "../data/api.ts";
import { problemText } from "../i18n/index.ts";
import { useWrites } from "../time/index.ts";
import type { Account, SecretShown } from "./admin.ts";
import SecretDialog from "./SecretDialog.vue";
import WriteOutcome from "./WriteOutcome.vue";
import { useAdminWrite } from "./write.ts";

// A service account (REQ-007-19): its secrets, never their value; a new one shown once; a
// secret revoked.

const i18n = useI18n();
const { t } = i18n;
const route = useRoute();
const writes = useWrites();
const write = useAdminWrite();
const id = computed(() => String(route.params.id));
const account = shallowRef<Account | null>(null);
const problem = shallowRef<Problem | null>(null);
const secret = shallowRef<SecretShown | null>(null);
const form = reactive({ expires: "", motive: "" });

async function load(): Promise<void> {
  const answer = await read<Account>(`/v1/service-accounts/${id.value}`);
  account.value = answer.ok ? answer.body : null;
  problem.value = answer.ok ? null : answer.problem;
}
watch(id, () => void load(), { immediate: true });

async function add(): Promise<void> {
  const done = await write.run("POST", `/v1/service-accounts/${id.value}/secrets`, {
    motive: form.motive,
    expires_at: form.expires || null,
  });
  if (!done) return;
  secret.value = done.secret;
  Object.assign(form, { expires: "", motive: "" });
  await load();
}

async function revoke(secretId: string): Promise<void> {
  const url = `/v1/service-accounts/${id.value}/secrets/${secretId}`;
  if (await write.run("DELETE", url, { motive: form.motive })) await load();
}
</script>

<template>
  <p v-if="problem" role="alert" class="k-error">{{ problemText(i18n, problem) }}</p>
  <template v-else-if="account">
    <h1>{{ t("admin.accountTitle", { name: account.name }) }}</h1>
    <p>{{ account.disabled ? t("admin.disabled") : t("admin.enabled") }}</p>
    <table class="k-table">
      <caption>
        {{
          t("admin.secrets")
        }}
      </caption>
      <thead>
        <tr>
          <th scope="col">{{ t("admin.keyId") }}</th>
          <th scope="col">{{ t("admin.created") }}</th>
          <th scope="col">{{ t("admin.expires") }}</th>
          <th scope="col">{{ t("admin.state") }}</th>
          <th scope="col">{{ t("admin.actions") }}</th>
        </tr>
      </thead>
      <tbody>
        <tr v-for="item in account.secrets" :key="item.id">
          <td class="k-mono">{{ item.key_id }}</td>
          <td>{{ item.created_at }}</td>
          <td>{{ item.expires_at ?? "—" }}</td>
          <td>{{ item.revoked ? t("admin.revoked") : t("admin.active") }}</td>
          <td>
            <button
              v-if="!item.revoked"
              type="button"
              :disabled="!form.motive || !writes.quickEdit"
              @click="revoke(item.id)"
            >
              {{ t("admin.revoke", { name: item.key_id }) }}
            </button>
          </td>
        </tr>
      </tbody>
    </table>
    <h2>{{ t("admin.newSecret") }}</h2>
    <form class="k-form" :aria-label="t('admin.newSecret')" @submit.prevent="add">
      <label>
        {{ t("admin.expiresHint") }}
        <input v-model="form.expires" name="expires" placeholder="2027-01-01T00:00:00Z" />
      </label>
      <label>
        {{ t("admin.motive") }}
        <input v-model="form.motive" name="motive" required />
      </label>
      <button type="submit" :disabled="write.busy.value || !writes.quickEdit">
        {{ t("admin.generate") }}
      </button>
    </form>
    <WriteOutcome :outcome="write.outcome.value" :problem="write.problem.value" />
    <SecretDialog :secret="secret" @close="secret = null" />
  </template>
</template>
