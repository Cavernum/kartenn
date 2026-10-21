<script setup lang="ts">
import { computed, reactive, ref, shallowRef, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useRoute } from "vue-router";
import { read, send, type Problem } from "../data/api.ts";
import { problemText } from "../i18n/index.ts";
import { useWrites } from "../time/index.ts";
import type { Identity } from "./admin.ts";
import WriteOutcome from "./WriteOutcome.vue";
import { useAdminWrite } from "./write.ts";

// An identity (REQ-007-19, spec 005 and 004/REQ-004-33): disabled or enabled again, its
// sessions revoked, its identity data pseudonymised at once (irreversible, confirmed).

const MERGE_PATCH = { "content-type": "application/merge-patch+json" };

const i18n = useI18n();
const { t } = i18n;
const route = useRoute();
const writes = useWrites();
const write = useAdminWrite();
const id = computed(() => String(route.params.id));
const identity = shallowRef<Identity | null>(null);
const problem = shallowRef<Problem | null>(null);
const form = reactive({ motive: "", confirm: false });
const pseudonymised = ref(false);

async function load(): Promise<void> {
  const answer = await read<Identity>(`/v1/identities/${id.value}`);
  identity.value = answer.ok ? answer.body : null;
  problem.value = answer.ok ? null : answer.problem;
}
watch(id, () => void load(), { immediate: true });

async function patch(change: { disabled?: boolean; revoke_sessions?: boolean }): Promise<void> {
  const url = `/v1/identities/${id.value}`;
  if (await write.run("PATCH", url, { motive: form.motive, ...change }, MERGE_PATCH)) {
    form.motive = "";
    await load();
  }
}

async function pseudonymise(): Promise<void> {
  const answer = await send<Identity>("POST", `/v1/identities/${id.value}/pseudonymisation`, {
    motive: form.motive,
  });
  write.problem.value = answer.ok ? null : answer.problem;
  if (answer.ok) {
    pseudonymised.value = true;
    Object.assign(form, { motive: "", confirm: false });
    await load();
  }
}
</script>

<template>
  <p v-if="problem" role="alert" class="k-error">{{ problemText(i18n, problem) }}</p>
  <template v-else-if="identity">
    <h1>{{ t("admin.identityTitle", { name: identity.name }) }}</h1>
    <dl class="k-fields">
      <dt>{{ t("admin.kind") }}</dt>
      <dd>{{ identity.kind }}</dd>
      <dt>{{ t("admin.issuer") }}</dt>
      <dd>{{ identity.issuer ?? "—" }}</dd>
      <dt>{{ t("admin.state") }}</dt>
      <dd>{{ identity.disabled ? t("admin.disabled") : t("admin.enabled") }}</dd>
      <dt>{{ t("admin.revokedAfter") }}</dt>
      <dd>{{ identity.revoked_after ?? "—" }}</dd>
      <dt>{{ t("field.id") }}</dt>
      <dd>{{ identity.id }}</dd>
    </dl>
    <form class="k-form" :aria-label="t('admin.identityActions')" @submit.prevent>
      <label>
        {{ t("admin.motive") }}
        <input v-model="form.motive" name="motive" required />
      </label>
      <div class="k-toolbar">
        <button
          type="button"
          :disabled="!form.motive || write.busy.value || !writes.quickEdit"
          @click="patch({ disabled: !identity.disabled })"
        >
          {{ identity.disabled ? t("admin.enable") : t("admin.disable") }}
        </button>
        <button
          type="button"
          :disabled="!form.motive || write.busy.value || !writes.quickEdit"
          @click="patch({ revoke_sessions: true })"
        >
          {{ t("admin.revokeSessions") }}
        </button>
      </div>
      <label v-if="identity.kind === 'person'">
        <span>
          <input v-model="form.confirm" type="checkbox" name="confirm" />
          {{ t("admin.pseudonymiseConfirm") }}
        </span>
      </label>
      <button
        v-if="identity.kind === 'person'"
        type="button"
        :disabled="!form.motive || !form.confirm || !writes.quickEdit"
        @click="pseudonymise"
      >
        {{ t("admin.pseudonymise") }}
      </button>
    </form>
    <p v-if="pseudonymised" role="status">{{ t("admin.pseudonymised") }}</p>
    <WriteOutcome :outcome="write.outcome.value" :problem="write.problem.value" />
  </template>
</template>
