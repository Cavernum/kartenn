<script setup lang="ts">
// The group chosen, in the Détail tab of the right panel (spec 025, REQ-025-12; design 021
// « Administration »; maquette E7, vues `groupes`, `groupe-edition`, `groupe-refus`,
// `secours`): its label and key, « Modifier » (`e`) and « Clore… » at its top; its members
// (identities and groups of an IdP, their kind, « Retirer »), then its roles on scopes
// (label and key of the role, scope, « Modifier », « Retirer »), « Ajouter » under each
// list. Every action goes to the draft, its row marked there: a removal stays, struck, its
// action « Annuler le retrait »; an addition comes at the end, its action takes it out.
// An action refused stays visible with its permission or its reason; a group whose writes
// are all refused for one reason says it at its top, with the code of the rule.
import { changesets, queryClient, reads, type Violation } from "@kartenn/sdk";
import {
  AlertDialog,
  Button,
  CodeBadge,
  Icon,
  IconButton,
  InlineAlert,
  KeyLabel,
  Skeleton,
  Stack,
  StatusBadge,
  Toolbar,
  type ToolbarAction,
} from "@kartenn/ui";
import { computed, ref, shallowReactive, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useQuery } from "../data/query.ts";
import ProblemState from "../problems/ProblemState.vue";
import { covers, useSession } from "../rights/index.ts";
import { useViewAction } from "../shortcuts/view.ts";
import { useTime } from "../time/index.ts";
import { collectionOfKind, nameOf, type Item } from "../views/collections.ts";
import { useKeyLabel } from "../views/data.ts";
import RefusalAlert from "../views/edit/RefusalAlert.vue";
import { keepRecordName } from "../views/names.ts";
import AdminLines from "./AdminLines.vue";
import BindingDialog from "./BindingDialog.vue";
import { useAdminDraft } from "./draft.ts";
import GroupForm from "./GroupForm.vue";
import MemberDialog from "./MemberDialog.vue";
import {
  bindingRows,
  currentCount,
  draftChanges,
  escalationProbe,
  groupRefusals,
  memberName,
  memberRows,
  RBAC_ADMIN,
  type BindingRow,
  type BindingScope,
  type GroupItem,
  type MemberRow,
  type Refusal,
} from "./model.ts";
import { useList } from "./reads.ts";

const props = defineProps<{ id: string }>();
const { t, locale } = useI18n();
const session = useSession();
const time = useTime();
const keyLabel = useKeyLabel();
const draft = useAdminDraft();

const read = useQuery(() => reads.read("/v1/groups/{id}", { params: { id: props.id } }));
const group = computed(() => read.data.value?.data as GroupItem | undefined);
const key = computed(() => group.value?.key ?? "");
watch(key, (k) => k && keepRecordName(props.id, k), { immediate: true });
const label = computed(() =>
  group.value ? (group.value.labels[locale.value] ?? keyLabel("group", group.value.key)) : null,
);

const members = useList("/v1/groups/{id}/members", null, () => ({ id: props.id }));
const bindings = useList(
  "/v1/bindings",
  () => ({ group: key.value }),
  undefined,
  () => key.value !== "",
);
const identities = useList("/v1/identities");
const roles = useList("/v1/roles");
const identityById = computed(() => new Map(identities.rows.value.map((i) => [i.id, i])));

const memberList = computed(() =>
  memberRows(members.rows.value, draft.operations.value, key.value, identityById.value),
);
const bindingList = computed(() =>
  bindingRows(bindings.rows.value, draft.operations.value, key.value),
);
const memberChanges = computed(() => draftChanges(memberList.value));
const bindingChanges = computed(() => draftChanges(bindingList.value));
const closing = computed(() =>
  draft.operations.value.findIndex(
    (op) =>
      (op as { op: string; key?: string }).op === "close_group" &&
      (op as { key?: string }).key === key.value,
  ),
);

// A user who administers part of the organisation only: whether the group is bound wider
// than their scope, as the server simulates its closure and the end of its bindings.
const globalAdmin = computed(
  () => session.value.me?.global_permissions.some((e) => covers(e, RBAC_ADMIN)) ?? false,
);
const probing = computed(
  () =>
    !!group.value &&
    !group.value.builtin &&
    bindings.query.isSuccess.value &&
    session.value.rights.has(RBAC_ADMIN) &&
    !globalAdmin.value &&
    time.value.present,
);
const probe = useQuery(() => ({
  ...changesets.simulate({
    motive: "-",
    operations: group.value ? escalationProbe(group.value, bindings.rows.value) : [],
  }),
  enabled: probing.value,
}));
const refusals = computed(() =>
  group.value
    ? groupRefusals({
        group: group.value,
        bindings: bindings.rows.value,
        rights: session.value.rights,
        globalAdmin: globalAdmin.value,
        present: time.value.present,
        violations: probing.value
          ? ((probe.data.value?.violations as Violation[] | undefined) ?? null)
          : null,
        words: {
          past: t("edit.why.past"),
          wider: (code) => t("admin.why.wider", { code }),
          builtin: t("admin.why.builtin"),
          builtinMembers: t("admin.why.builtinMembers"),
          widerTitle: t("admin.why.widerTitle"),
          builtinTitle: t("admin.why.builtinTitle"),
        },
      })
    : { bindings: new Map<string, Refusal>() },
);
const drafted = computed(() => ({ reason: t("admin.why.drafted") }));
const refusalOf = (refusal: Refusal | undefined) => refusal ?? {};

// The names of the objects of the scopes, read once each: the words of a binding.
const scopeNames = shallowReactive(new Map<string, string>());
watch(
  () => bindingList.value.map((b) => b.scope),
  (scopes) => {
    for (const scope of scopes) {
      const collection = collectionOfKind(scope.kind);
      if (!scope.id || !collection || scopeNames.has(scope.id)) continue;
      const id = scope.id;
      void queryClient
        .fetchQuery(reads.object<Item>(collection.kind, id))
        .then((entry) => scopeNames.set(id, nameOf(entry.data)))
        .catch(() => undefined);
    }
  },
  { immediate: true },
);
const scopeWords = (scope: BindingScope) => {
  if (scope.kind === "global" || !scope.id) return t("admin.bindings.scopes.global");
  const kind = t(`admin.bindings.scopes.${scope.kind}`);
  return `${kind} ${scopeNames.get(scope.id) ?? scope.id}`;
};
const roleLabel = (role: string) => {
  const found = roles.rows.value.find((r) => r.key === role);
  return found?.labels[locale.value] ?? keyLabel("role", role);
};
const bindingWords = (role: string, scope: BindingScope) => `${role} · ${scopeWords(scope)}`;
const kindWords = (row: MemberRow) =>
  row.kind === "idp_group"
    ? t("admin.members.kind.idp_group")
    : t(`admin.members.kind.${identityById.value.get(row.identity ?? "")?.kind ?? "identity"}`);
/** A name cut at ` / ` (`émetteur / groupe`): it wraps there, never inside a key. */
const parts = (name: string) => name.split(" / ");

// The actions.
const editing = ref(false);
const closeOpen = ref(false);
const adding = ref(false);
const binding = ref(false);
const changed = ref<BindingRow | undefined>(undefined);
const actions = computed<ToolbarAction[]>(() => [
  {
    id: "edit",
    label: t("admin.groups.edit"),
    icon: "pencil",
    shortcut: ["e"],
    priority: 2,
    ...refusalOf(refusals.value.edit),
  },
  {
    id: "close",
    label: t("admin.groups.close"),
    icon: "trash",
    ...refusalOf(refusals.value.close ?? (closing.value >= 0 ? drafted.value : undefined)),
  },
]);
function onAction(id: string): void {
  if (id === "edit") editing.value = true;
  if (id === "close") closeOpen.value = true;
}
useViewAction("edit", {
  label: () => t("admin.groups.editTitle", { group: key.value }),
  run: () => (editing.value = true),
  allowed: () => !!group.value && !refusals.value.edit,
});
async function close(): Promise<void> {
  await draft.add(
    [{ op: "close_group", key: key.value }],
    t("admin.groups.closed", { group: key.value }),
  );
}
async function removeMember(row: MemberRow): Promise<void> {
  if (!row.id) return;
  await draft.add(
    [{ op: "remove_member", id: row.id }],
    t("admin.members.removed", { name: memberName(row), group: key.value }),
  );
}
async function removeBinding(row: BindingRow): Promise<void> {
  if (!row.id) return;
  await draft.add(
    [{ op: "unbind", id: row.id }],
    t("admin.bindings.removed", { binding: bindingWords(row.role, row.scope), group: key.value }),
  );
}
// The action of a row, by its state: « Retirer », « Annuler le retrait », or the addition
// taken out of the draft — its row gone, the focus to « Ajouter » of its section.
const membersAdd = ref<HTMLElement | null>(null);
const bindingsAdd = ref<HTMLElement | null>(null);
const focusAdd = (at: HTMLElement | null) => at?.querySelector<HTMLElement>("button")?.focus();
function memberAction(row: MemberRow) {
  const name = memberName(row);
  if (row.pending === "removed") {
    return { icon: "undo-2" as const, label: t("admin.members.undoRemove", { name }) };
  }
  if (row.pending === "added") {
    return { icon: "x" as const, label: t("admin.members.dropAdd", { name }) };
  }
  return { icon: "trash" as const, label: t("admin.members.remove", { name }) };
}
async function onMember(row: MemberRow): Promise<void> {
  if (!row.pending) return removeMember(row);
  const added = row.pending === "added";
  const what = t(added ? "admin.members.added" : "admin.members.removed", {
    name: memberName(row),
    group: key.value,
  });
  if ((await draft.drop(row.op!, what)) && added) focusAdd(membersAdd.value);
}
function bindingAction(row: BindingRow) {
  const binding = bindingWords(row.role, row.scope);
  if (row.pending === "removed") {
    return { icon: "undo-2" as const, label: t("admin.bindings.undoRemove", { binding }) };
  }
  if (row.pending === "added") {
    return { icon: "x" as const, label: t("admin.bindings.dropAdd", { binding }) };
  }
  return { icon: "trash" as const, label: t("admin.bindings.remove", { binding }) };
}
async function onBinding(row: BindingRow): Promise<void> {
  if (!row.pending) return removeBinding(row);
  const binding = bindingWords(row.role, row.scope);
  const added = row.pending === "added";
  const what = added
    ? t("admin.bindings.added", { binding, group: key.value })
    : t("admin.bindings.removed", { binding, group: key.value });
  if ((await draft.drop(row.op!, what)) && added) focusAdd(bindingsAdd.value);
}
function changeBinding(row: BindingRow): void {
  changed.value = row;
  binding.value = true;
}
function addBinding(): void {
  changed.value = undefined;
  binding.value = true;
}
const until = (date: string) => t("admin.until", { date: date.slice(0, 10) });
</script>

<template>
  <ProblemState
    v-if="read.isError.value"
    :title="t('admin.groups.readError')"
    :error="read.error.value"
    :retry="read.refetch"
  />
  <Skeleton v-else-if="!group" variant="rows" :rows="6" />
  <Stack v-else gap="4" class="k-admin-detail">
    <Stack gap="2">
      <h2 class="k-admin-heading">
        <template v-if="label">{{ `${label} ` }}</template>
        <span class="k-mono k-admin-muted">{{ group.key }}</span>
      </h2>
      <span v-if="group.builtin"
        ><StatusBadge state="info" :label="t('admin.groups.builtin')"
      /></span>
      <InlineAlert
        v-if="refusals.readOnly"
        tone="warn"
        icon="lock"
        :title="refusals.readOnly.title"
      >
        <CodeBadge :code="refusals.readOnly.code" />
      </InlineAlert>
      <InlineAlert v-if="closing >= 0" tone="info" :title="t('admin.groups.closedInDraft')" />
      <Toolbar
        :label="t('admin.groups.detailActions')"
        :actions="actions"
        align="start"
        @action="onAction"
      />
      <RefusalAlert v-if="draft.refusal.value" :refusal="draft.refusal.value" :note="null" />
    </Stack>

    <section aria-labelledby="k-admin-members">
      <Stack gap="2">
        <h3 id="k-admin-members" class="k-admin-heading">
          {{ t("admin.members.title", { n: currentCount(memberList) }) }}
          <span v-if="memberChanges.removed || memberChanges.added" class="k-admin-muted">{{
            t("admin.changes", memberChanges)
          }}</span>
        </h3>
        <ProblemState
          v-if="members.query.isError.value"
          :title="t('admin.members.error')"
          :error="members.query.error.value"
          :retry="members.query.refetch"
        />
        <Skeleton v-else-if="members.query.isPending.value" variant="rows" :rows="3" />
        <p v-else-if="memberList.length === 0" class="k-admin-muted">
          {{ t("admin.members.none") }}
        </p>
        <AdminLines v-else :label="t('admin.members.list', { group: group.key })">
          <li v-for="row in memberList" :key="row.key" :data-pending="row.pending">
            <span class="k-admin-lines__text">
              <Icon
                v-if="row.pending"
                :name="row.pending === 'removed' ? 'minus' : 'plus'"
                meaningful
                :label="t(`admin.pending.${row.pending}`)"
                class="k-admin-lines__mark"
              />
              <span class="k-admin-lines__label" :class="{ 'k-mono': row.kind === 'idp_group' }"
                ><template v-for="(part, i) in parts(memberName(row))" :key="i"
                  ><template v-if="i"> / </template
                  ><span class="k-admin-lines__nowrap">{{ part }}</span></template
                ></span
              >
            </span>
            <span class="k-admin-lines__value"
              ><span>{{ kindWords(row) }}</span
              ><span v-if="row.until"
                ><time class="k-num" :datetime="row.until">{{ until(row.until) }}</time></span
              ></span
            >
            <!-- One button whatever the state of the row: patched, never replaced, it keeps
                 the focus when its action changes (2.4.3). -->
            <span class="k-admin-lines__actions">
              <IconButton
                data-k-row-action
                :icon="memberAction(row).icon"
                size="sm"
                :label="memberAction(row).label"
                :loading="draft.busy.value"
                v-bind="row.pending ? {} : refusalOf(refusals.members)"
                @click="onMember(row)"
              />
            </span>
          </li>
        </AdminLines>
        <span ref="membersAdd">
          <Button
            size="sm"
            variant="ghost"
            icon="plus"
            v-bind="refusalOf(refusals.members)"
            @click="adding = true"
            >{{ t("admin.members.add") }}</Button
          >
        </span>
      </Stack>
    </section>

    <section aria-labelledby="k-admin-bindings">
      <Stack gap="2">
        <h3 id="k-admin-bindings" class="k-admin-heading">
          {{ t("admin.bindings.title", { n: currentCount(bindingList) }) }}
          <span v-if="bindingChanges.removed || bindingChanges.added" class="k-admin-muted">{{
            t("admin.changes", bindingChanges)
          }}</span>
        </h3>
        <ProblemState
          v-if="bindings.query.isError.value"
          :title="t('admin.bindings.error')"
          :error="bindings.query.error.value"
          :retry="bindings.query.refetch"
        />
        <Skeleton v-else-if="bindings.query.isPending.value" variant="rows" :rows="3" />
        <p v-else-if="bindingList.length === 0" class="k-admin-muted">
          {{ t("admin.bindings.none") }}
        </p>
        <AdminLines v-else :label="t('admin.bindings.list', { group: group.key })">
          <li v-for="row in bindingList" :key="row.key" :data-pending="row.pending">
            <span class="k-admin-lines__text">
              <Icon
                v-if="row.pending"
                :name="row.pending === 'removed' ? 'minus' : 'plus'"
                meaningful
                :label="t(`admin.pending.${row.pending}`)"
                class="k-admin-lines__mark"
              />
              <span class="k-admin-lines__label"
                ><KeyLabel :value="row.role" :label="roleLabel(row.role)" stacked
              /></span>
            </span>
            <span class="k-admin-lines__value"
              ><span>{{ scopeWords(row.scope) }}</span
              ><span v-if="row.until"
                ><time class="k-num" :datetime="row.until">{{ until(row.until) }}</time></span
              ></span
            >
            <span class="k-admin-lines__actions">
              <IconButton
                v-if="!row.pending"
                icon="pencil"
                size="sm"
                :label="t('admin.bindings.edit', { binding: bindingWords(row.role, row.scope) })"
                v-bind="refusalOf(refusals.bindings.get(row.key))"
                @click="changeBinding(row)"
              />
              <!-- The same button in every state of the row: it keeps the focus. -->
              <IconButton
                data-k-row-action
                :icon="bindingAction(row).icon"
                size="sm"
                :label="bindingAction(row).label"
                :loading="draft.busy.value"
                v-bind="row.pending ? {} : refusalOf(refusals.bindings.get(row.key))"
                @click="onBinding(row)"
              />
            </span>
          </li>
        </AdminLines>
        <span ref="bindingsAdd">
          <Button
            size="sm"
            variant="ghost"
            icon="plus"
            v-bind="refusalOf(refusals.addBinding)"
            @click="addBinding"
            >{{ t("admin.bindings.add") }}</Button
          >
        </span>
      </Stack>
    </section>
    <GroupForm v-model:open="editing" :group="group" />
    <AlertDialog
      v-model:open="closeOpen"
      :title="t('admin.groups.closeTitle', { group: group.key })"
      :description="t('admin.groups.closeText')"
      :confirm="t('admin.groups.closeConfirm')"
      danger
      @confirm="close"
    />
    <MemberDialog
      v-model:open="adding"
      :group="group.key"
      :identities="identities.rows.value"
      :members="memberList"
    />
    <BindingDialog
      v-model:open="binding"
      :group="group.key"
      :roles="roles.rows.value"
      :rows="bindingList"
      :editing="changed"
      :words="bindingWords"
      :scope-name="(id) => scopeNames.get(id)"
    />
  </Stack>
</template>
