<script setup lang="ts">
// The service account chosen, in the Détail tab of the right panel (005, K8; REQ-007-19,
// REQ-021-12; maquette E7, vue `secret`): its name and its state; « Nouveau secret » at its
// top, the secret shown once (`SecretDialog`); its secrets, newest first, never their
// value: the key of the ring that made the digest, the creation, the expiry, each with
// « Révoquer », to the draft (`revoke_service_secret`), the row marked there and its action
// « Annuler la révocation ». Its state is that of its identity (`/admin/identities/:id`).
import { reads, type Operation } from "@kartenn/sdk";
import {
  EmptyState,
  Icon,
  IconButton,
  Skeleton,
  Stack,
  StatusBadge,
  Toolbar,
  type ToolbarAction,
} from "@kartenn/ui";
import { computed, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { RouterLink } from "vue-router";
import { useQuery } from "../data/query.ts";
import ProblemState from "../problems/ProblemState.vue";
import { useSession } from "../rights/index.ts";
import { useTime } from "../time/index.ts";
import RefusalAlert from "../views/edit/RefusalAlert.vue";
import { keepRecordName } from "../views/names.ts";
import {
  ACCOUNT_ADMIN,
  activeSecrets,
  globalRefusal,
  secretRows,
  type AccountItem,
  type SecretRow,
} from "./access.ts";
import AdminLines from "./AdminLines.vue";
import { useAdminDraft } from "./draft.ts";
import SecretDialog from "./SecretDialog.vue";

const props = defineProps<{ id: string }>();
const { t } = useI18n();
const session = useSession();
const time = useTime();
const draft = useAdminDraft();

const read = useQuery(() => reads.read("/v1/service-accounts/{id}", { params: { id: props.id } }));
const account = computed(() => read.data.value?.data as AccountItem | undefined);
watch(
  () => account.value?.name,
  (name) => name && keepRecordName(props.id, name),
  { immediate: true },
);
const secrets = computed(() =>
  account.value ? secretRows(account.value.secrets, draft.operations.value, props.id) : [],
);

const refusal = computed(() =>
  globalRefusal({
    me: session.value.me,
    permission: ACCOUNT_ADMIN,
    present: time.value.present,
    past: t("edit.why.past"),
  }),
);
const refusalOf = () => refusal.value ?? {};
const renewing = ref(false);
const actions = computed<ToolbarAction[]>(() => [
  { id: "renew", label: t("admin.secret.renew"), icon: "key-round", ...refusalOf() },
]);

const day = (instant: string | null | undefined) => instant?.slice(0, 10) ?? null;
/** A secret named by its creation, to the minute (the key of the ring is shared by many). */
const stamp = (instant: string) => instant.slice(0, 16).replace("T", " ");
async function onSecret(row: SecretRow): Promise<void> {
  const what = t("admin.secret.revoked", {
    date: stamp(row.created_at),
    account: account.value?.name,
  });
  if (row.pending === "removed" && row.index !== undefined) await draft.drop(row.index, what);
  else {
    await draft.add(
      [{ op: "revoke_service_secret", account: props.id, secret: row.id } as Operation],
      what,
    );
  }
}
</script>

<template>
  <ProblemState
    v-if="read.isError.value"
    :title="t('admin.accounts.readError')"
    :error="read.error.value"
    :retry="read.refetch"
  />
  <Skeleton v-else-if="!account" variant="rows" :rows="6" />
  <Stack v-else gap="4" class="k-admin-detail">
    <Stack gap="2">
      <h2 class="k-admin-heading k-mono">{{ account.name }}</h2>
      <span
        ><StatusBadge
          :state="account.disabled ? 'neutral' : 'ok'"
          :label="account.disabled ? t('admin.accounts.disabled') : t('admin.accounts.enabled')"
      /></span>
      <Toolbar
        :label="t('admin.accounts.detailActions')"
        :actions="actions"
        align="start"
        @action="renewing = true"
      />
      <RefusalAlert v-if="draft.refusal.value" :refusal="draft.refusal.value" :note="null" />
    </Stack>

    <section aria-labelledby="k-admin-secrets">
      <Stack gap="2">
        <h3 id="k-admin-secrets" class="k-admin-heading">
          {{ t("admin.secret.listTitle", { n: activeSecrets(account) }) }}
        </h3>
        <EmptyState v-if="secrets.length === 0" :title="t('admin.secret.none')">
          {{ t("admin.secret.noneText") }}
        </EmptyState>
        <AdminLines v-else :label="t('admin.secret.list', { account: account.name })">
          <li v-for="row in secrets" :key="row.id" :data-pending="row.pending">
            <span class="k-admin-lines__text">
              <Icon
                v-if="row.pending"
                name="minus"
                meaningful
                :label="t('admin.secret.pending')"
                class="k-admin-lines__mark"
              />
              <span class="k-admin-lines__label"
                >{{ t("admin.secret.created", { date: stamp(row.created_at) }) }}
                <span class="k-mono k-admin-muted k-admin-lines__key">{{
                  t("admin.secret.key", { key: row.key_id })
                }}</span></span
              >
            </span>
            <span class="k-admin-lines__value">
              <StatusBadge
                v-if="row.revoked"
                state="neutral"
                :label="t('admin.secret.isRevoked')"
              />
              <template v-else>{{
                day(row.expires_at)
                  ? t("admin.secret.until", { date: day(row.expires_at) })
                  : t("admin.secret.noExpiry")
              }}</template>
            </span>
            <span class="k-admin-lines__actions">
              <IconButton
                v-if="!row.revoked"
                data-k-row-action
                :icon="row.pending === 'removed' ? 'undo-2' : 'trash'"
                size="sm"
                :label="
                  t(row.pending === 'removed' ? 'admin.secret.undoRevoke' : 'admin.secret.revoke', {
                    date: stamp(row.created_at),
                  })
                "
                :loading="draft.busy.value"
                v-bind="row.pending ? {} : refusalOf()"
                @click="onSecret(row)"
              />
            </span>
          </li>
        </AdminLines>
      </Stack>
    </section>
    <p class="k-admin-muted">
      {{ t("admin.accounts.state") }}
      <RouterLink :to="`/admin/identities/${encodeURIComponent(account.id)}`">{{
        t("admin.accounts.identity")
      }}</RouterLink>
    </p>
    <SecretDialog v-model:open="renewing" :account="{ id: account.id, name: account.name }" />
  </Stack>
</template>
