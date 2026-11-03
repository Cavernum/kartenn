<script setup lang="ts">
// The identity chosen, in the Détail tab of the right panel (005; spec 025, REQ-025-11;
// REQ-021-12; design 021 « Administration »; maquette E7, vue `identite`): its name, its
// kind (person, service account, break-glass account) and its state; « Désactiver » (or
// « Réactiver ») and « Révoquer les sessions », to the draft (`set_identity_state`), and
// « Pseudonymiser… » for a person or a requester (016), at once and confirmed (004,
// REQ-004-33: irreversible).
// In read: its groups of kartenn with « via » (direct, or the group of its IdP that makes
// it a member), its effective rights (role, scope, the group that gives it), and the
// groups of its IdP at its last login, without date (025, Q-025-1), each with the group of
// kartenn it gives; an identity never logged in by an IdP has a line of empty state.
import { api, keys, reads, type Operation } from "@kartenn/sdk";
import {
  Button,
  Dialog,
  InlineAlert,
  KeyLabel,
  Skeleton,
  Stack,
  StatusBadge,
  TextField,
  Toolbar,
  injectToasts,
  type ToolbarAction,
} from "@kartenn/ui";
import { computed, ref, useId, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useQuery, useQueryClient } from "../data/query.ts";
import ProblemState from "../problems/ProblemState.vue";
import { useSession } from "../rights/index.ts";
import { useTime } from "../time/index.ts";
import { useKeyLabel } from "../views/data.ts";
import RefusalAlert from "../views/edit/RefusalAlert.vue";
import type { Refusal as WriteRefusal } from "../views/edit/inline.ts";
import { keepRecordName } from "../views/names.ts";
import {
  ACCOUNT_ADMIN,
  draftedStates,
  globalRefusal,
  idpGroupRows,
  PSEUDONYMISE,
  pseudonymisable,
  refusalOf,
  type IdentityDetail,
} from "./access.ts";
import AdminLines from "./AdminLines.vue";
import { useAdminDraft } from "./draft.ts";
import { useList, useScopeNames } from "./reads.ts";

const props = defineProps<{ id: string }>();
const { t, locale } = useI18n();
const session = useSession();
const time = useTime();
const keyLabel = useKeyLabel();
const draft = useAdminDraft();
const toasts = injectToasts();
const queryClient = useQueryClient();

const read = useQuery(() => reads.read("/v1/identities/{id}", { params: { id: props.id } }));
const identity = computed(() => read.data.value?.data as IdentityDetail | undefined);
watch(
  () => identity.value?.name,
  (name) => name && keepRecordName(props.id, name),
  { immediate: true },
);

const groups = useList("/v1/groups");
const roles = useList("/v1/roles");
const groupLabel = (key: string) =>
  groups.rows.value.find((g) => g.key === key)?.labels[locale.value] ?? keyLabel("group", key);
const roleLabel = (key: string) =>
  roles.rows.value.find((r) => r.key === key)?.labels[locale.value] ?? keyLabel("role", key);
const scopeName = useScopeNames(() => identity.value?.bindings.map((b) => b.scope) ?? []);
const scopeWords = (scope: { kind: string; id?: string | null }) =>
  scope.kind === "global" || !scope.id
    ? t("admin.bindings.scopes.global")
    : `${t(`admin.bindings.scopes.${scope.kind}`)} ${scopeName(scope.id) ?? scope.id}`;
const idpGroups = computed(() => (identity.value ? idpGroupRows(identity.value) : []));
/** A group of the issuer as the mock-up writes it: `émetteur / groupe`, never cut inside. */
const idpParts = (name: string) =>
  identity.value?.issuer ? [identity.value.issuer, name] : [name];

// The state: to the draft, refused while a change of it already waits there.
const drafted = computed(() => draftedStates(draft.operations.value, props.id));
const pendingWords = computed(() =>
  drafted.value.map((d) =>
    d.revokeSessions
      ? t("admin.identities.revokeDrafted")
      : d.disabled
        ? t("admin.identities.disableDrafted")
        : t("admin.identities.enableDrafted"),
  ),
);
const refusal = computed(() =>
  globalRefusal({
    me: session.value.me,
    permission: ACCOUNT_ADMIN,
    present: time.value.present,
    past: t("edit.why.past"),
  }),
);
const stateRefusal = computed(
  () => refusal.value ?? (drafted.value.length ? { reason: t("admin.why.drafted") } : {}),
);
const pseudonymiseRefusal = computed(
  () =>
    globalRefusal({
      me: session.value.me,
      permission: PSEUDONYMISE,
      present: time.value.present,
      past: t("edit.why.past"),
    }) ?? {},
);
const actions = computed<ToolbarAction[]>(() => {
  const shown = identity.value;
  if (!shown) return [];
  return [
    {
      id: "state",
      label: shown.disabled ? t("admin.identities.enable") : t("admin.identities.disable"),
      icon: shown.disabled ? "user" : "user-x",
      priority: 2,
      ...stateRefusal.value,
    },
    {
      id: "revoke",
      label: t("admin.identities.revoke"),
      icon: "log-out",
      priority: 1,
      ...stateRefusal.value,
    },
    ...(pseudonymisable(shown.kind)
      ? [
          {
            id: "pseudonymise",
            label: t("admin.identities.pseudonymise"),
            icon: "eye-off",
            ...pseudonymiseRefusal.value,
          } satisfies ToolbarAction,
        ]
      : []),
  ];
});
async function onAction(id: string): Promise<void> {
  const shown = identity.value;
  if (!shown) return;
  if (id === "pseudonymise") {
    pseudonymising.value = true;
    return;
  }
  const op =
    id === "revoke"
      ? { op: "set_identity_state", identity: props.id, revoke_sessions: true }
      : { op: "set_identity_state", identity: props.id, disabled: !shown.disabled };
  const what = t(
    id === "revoke"
      ? "admin.identities.whatRevoke"
      : shown.disabled
        ? "admin.identities.whatEnable"
        : "admin.identities.whatDisable",
    { name: shown.name },
  );
  await draft.add([op as Operation], what);
}

// « Pseudonymiser… »: at once, with a motive, confirmed (irreversible).
const pseudonymising = ref(false);
const formId = useId();
const motive = ref("");
const tried = ref(false);
const busy = ref(false);
const pseudonymiseRefused = ref<WriteRefusal | null>(null);
watch(pseudonymising, (now) => {
  if (!now) return;
  motive.value = "";
  tried.value = false;
  pseudonymiseRefused.value = null;
});
const motiveError = computed(() =>
  tried.value && !motive.value.trim()
    ? t("admin.required", { field: t("admin.motive") })
    : undefined,
);
async function pseudonymise(): Promise<void> {
  tried.value = true;
  if (motiveError.value || busy.value) return;
  busy.value = true;
  pseudonymiseRefused.value = null;
  try {
    await api.post("/v1/identities/{id}/pseudonymisation", {
      path: { id: props.id },
      body: { motive: motive.value.trim() },
    });
    pseudonymising.value = false;
    toasts?.show({ tone: "ok", title: t("admin.identities.pseudonymised") });
    await queryClient.invalidateQueries({ queryKey: keys.allOn("/v1/identities") });
    await queryClient.invalidateQueries({ queryKey: keys.allOn("/v1/identities/{id}") });
  } catch (error) {
    pseudonymiseRefused.value = refusalOf(error);
  } finally {
    busy.value = false;
  }
}
</script>

<template>
  <ProblemState
    v-if="read.isError.value"
    :title="t('admin.identities.readError')"
    :error="read.error.value"
    :retry="read.refetch"
  />
  <Skeleton v-else-if="!identity" variant="rows" :rows="6" />
  <Stack v-else gap="4" class="k-admin-detail">
    <Stack gap="2">
      <h2 class="k-admin-heading">
        {{ `${identity.name} ` }}
        <span class="k-admin-muted">{{ t(`admin.members.kind.${identity.kind}`) }}</span>
      </h2>
      <span
        ><StatusBadge
          :state="identity.disabled ? 'neutral' : 'ok'"
          :label="
            identity.disabled ? t('admin.identities.disabled') : t('admin.identities.enabled')
          "
      /></span>
      <InlineAlert v-for="(words, i) in pendingWords" :key="i" tone="info" :title="words" />
      <Toolbar
        :label="t('admin.identities.actions')"
        :actions="actions"
        align="start"
        @action="onAction"
      />
      <RefusalAlert v-if="draft.refusal.value" :refusal="draft.refusal.value" :note="null" />
    </Stack>

    <section aria-labelledby="k-admin-identity-groups">
      <Stack gap="2">
        <h3 id="k-admin-identity-groups" class="k-admin-heading">
          {{ t("admin.identities.groups", { n: identity.groups.length }) }}
        </h3>
        <p v-if="identity.groups.length === 0" class="k-admin-muted">
          {{ t("admin.identities.noGroups") }}
        </p>
        <AdminLines v-else :label="t('admin.identities.groupList', { name: identity.name })" read>
          <li v-for="g in identity.groups" :key="`${g.key} ${g.via}`">
            <KeyLabel :value="g.key" :label="groupLabel(g.key)" stacked />
            <span v-if="g.via === 'direct'" class="k-admin-lines__value">{{
              t("admin.identities.direct")
            }}</span>
            <span v-else class="k-admin-lines__value"
              ><span
                >{{ t("admin.identities.via") }}
                <span class="k-mono"
                  ><template v-for="(part, i) in idpParts(g.via)" :key="i"
                    ><template v-if="i"> / </template
                    ><span class="k-admin-lines__nowrap">{{ part }}</span></template
                  ></span
                ></span
              ></span
            >
          </li>
        </AdminLines>
      </Stack>
    </section>

    <section aria-labelledby="k-admin-identity-rights">
      <Stack gap="2">
        <h3 id="k-admin-identity-rights" class="k-admin-heading">
          {{ t("admin.identities.rights", { n: identity.bindings.length }) }}
        </h3>
        <p v-if="identity.bindings.length === 0" class="k-admin-muted">
          {{ t("admin.identities.noRights") }}
        </p>
        <AdminLines v-else :label="t('admin.identities.rightList', { name: identity.name })" read>
          <li
            v-for="b in identity.bindings"
            :key="`${b.group} ${b.role} ${b.scope.kind} ${b.scope.id ?? ''}`"
          >
            <KeyLabel :value="b.role" :label="roleLabel(b.role)" stacked />
            <span class="k-admin-lines__value"
              ><span>{{ scopeWords(b.scope) }}</span
              ><span>{{ t("admin.identities.byGroup", { group: b.group }) }}</span></span
            >
          </li>
        </AdminLines>
      </Stack>
    </section>

    <section aria-labelledby="k-admin-identity-login">
      <Stack gap="2">
        <h3 id="k-admin-identity-login" class="k-admin-heading">
          {{ t("admin.identities.lastLogin", { n: idpGroups.length }) }}
        </h3>
        <p v-if="idpGroups.length === 0" class="k-admin-muted">
          {{ t("admin.identities.neverLogged") }}
        </p>
        <AdminLines
          v-else
          :label="t('admin.identities.lastLoginList', { name: identity.name })"
          read
        >
          <li v-for="g in idpGroups" :key="g.name">
            <span class="k-mono"
              ><template v-for="(part, i) in idpParts(g.name)" :key="i"
                ><template v-if="i"> / </template
                ><span class="k-admin-lines__nowrap">{{ part }}</span></template
              ></span
            >
            <span class="k-admin-lines__value">{{
              g.groups.length ? g.groups.join(", ") : t("admin.identities.noKartennGroup")
            }}</span>
          </li>
        </AdminLines>
      </Stack>
    </section>

    <Dialog
      v-model:open="pseudonymising"
      :title="t('admin.identities.pseudonymiseTitle', { name: identity.name })"
      size="md"
    >
      <form :id="formId" novalidate @submit.prevent="pseudonymise">
        <Stack gap="3">
          <InlineAlert tone="warn" :title="t('admin.identities.pseudonymiseWarn')">
            {{ t("admin.identities.pseudonymiseText") }}
          </InlineAlert>
          <TextField
            v-model="motive"
            :label="t('admin.motive')"
            :error="motiveError"
            required
            autocomplete="off"
          />
          <RefusalAlert v-if="pseudonymiseRefused" :refusal="pseudonymiseRefused" :note="null" />
        </Stack>
      </form>
      <template #actions>
        <Button variant="ghost" @click="pseudonymising = false">{{ t("admin.cancel") }}</Button>
        <Button variant="danger" icon="eye-off" type="submit" :form="formId" :loading="busy">{{
          t("admin.identities.pseudonymiseConfirm")
        }}</Button>
      </template>
    </Dialog>
  </Stack>
</template>
