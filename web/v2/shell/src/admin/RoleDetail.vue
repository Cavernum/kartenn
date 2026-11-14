<script setup lang="ts">
// The role chosen, in the Détail tab of the right panel (005, REQ-005-17, 25; REQ-021-12;
// maquette E7, vue `roles`): its label and key, « Modifier » (`e`), « Dupliquer »,
// « Supprimer… » at its top; its permissions under their served name (`module.action`
// when the resource is the module, spec 018), in fixed pitch, with their label when the
// dictionary has one, each with « Retirer », « Ajouter une permission » under the list;
// then the groups bound to it, read only. As the draft would leave it: a definition of the
// role waiting there marks its permissions (a removal struck, an addition at the end).
// Off the present and under 1024 px wide (the gate of writes, `useAdminGate`), every write
// is refused with that reason, after the permission; a dialog left open keeps its input.
import { labelDictionary, type Dictionary, type Entry } from "@kartenn/sdk";
import {
  AlertDialog,
  Button,
  CodeBadge,
  Combobox,
  Dialog,
  EmptyState,
  Icon,
  IconButton,
  InlineAlert,
  KeyLabel,
  Skeleton,
  Stack,
  StatusBadge,
  Toolbar,
  filterOptions,
  type ListOption,
  type ToolbarAction,
} from "@kartenn/ui";
import { computed, ref, useId, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useLabel, useQuery } from "../data/query.ts";
import ProblemState from "../problems/ProblemState.vue";
import { useSession } from "../rights/index.ts";
import { useViewAction } from "../shortcuts/view.ts";
import type { WriteRefusal } from "../time/index.ts";
import { useKeyLabel } from "../views/data.ts";
import RefusalAlert from "../views/edit/RefusalAlert.vue";
import { useStatus } from "../views/edit/say.ts";
import AdminLines from "./AdminLines.vue";
import { useAdminDraft } from "./draft.ts";
import { refusalSentence } from "./gate.ts";
import {
  draftedRole,
  entryRows,
  roleRefusals,
  type EntryRow,
  type Refusal,
  type RoleItem,
} from "./model.ts";
import { useList } from "./reads.ts";
import RoleForm from "./RoleForm.vue";

const props = defineProps<{ id: string }>();
const { t, locale } = useI18n();
const session = useSession();
const keyLabel = useKeyLabel();
const label = useLabel();
// A refusal of the gate at sending, without toasts: said by the status of the panel.
const draft = useAdminDraft({ say: (text) => status(text) });
/** The form of « Ajouter une permission », which the button of its actions submits. */
const formId = useId();

const roles = useList("/v1/roles");
const role = computed(() => roles.rows.value.find((r) => r.key === props.id));
const bindings = useList("/v1/bindings");
const bound = computed(() => bindings.rows.value.filter((b) => b.role === props.id));
const boundGroups = computed(() => new Set(bound.value.map((b) => b.group)).size);
const groups = useList("/v1/groups");
const groupLabel = (key: string) => {
  const g = groups.rows.value.find((x) => x.key === key);
  return g?.labels[locale.value] ?? keyLabel("group", key);
};
const scopeWords = (scope: { kind: string; id?: string | null }) =>
  scope.kind === "global"
    ? t("admin.bindings.scopes.global")
    : `${t(`admin.bindings.scopes.${scope.kind}`)} ${scope.id ?? ""}`;

const drafted = computed(() => draftedRole(draft.operations.value, props.id));
const entries = computed(() =>
  role.value
    ? entryRows(
        role.value.entries,
        drafted.value && !drafted.value.closed ? drafted.value.entries : null,
      )
    : [],
);
/** The role as the draft would leave it: what « Modifier » and « Dupliquer » start from. */
const shown = computed<RoleItem | undefined>(() =>
  role.value && drafted.value && !drafted.value.closed
    ? { ...role.value, entries: drafted.value.entries }
    : role.value,
);
const entryLabel = (entry: string) => {
  const found = label("permission", entry);
  return found === entry ? null : found;
};

/** The refusals of the role under the gate `refused` (`null`: the lasting ones only). */
const refusalsAt = (refused: WriteRefusal | null) =>
  role.value
    ? roleRefusals({
        role: role.value,
        rights: session.value.rights,
        refused,
        bound: boundGroups.value,
        words: {
          ...draft.gate.words.value,
          builtin: t("admin.why.roleBuiltin"),
          bound: (n) => t("admin.why.bound", { n }, n),
        },
      })
    : {};
const refusals = computed(() => refusalsAt(draft.gate.refused.value));
/** The lasting refusals (the right, a role delivered): what `e` says first. */
const lasting = computed(() => refusalsAt(null));
const refusalOf = (refusal: Refusal | undefined) => refusal ?? {};
/**
 * The gate alone, for the action of a row the draft marks (its permission put back or taken
 * out of the definition waiting there): no right refuses it, the gate does.
 */
const gateRefusal = computed(() => (draft.gate.why.value ? { reason: draft.gate.why.value } : {}));
/** Why `e` is refused, said by the polite status of the panel. */
const { said, status, clear: clearSaid } = useStatus();
// Another role shown by the same panel: what was said of the last one no longer holds.
watch(() => props.id, clearSaid);
const closing = computed(() => drafted.value?.closed === true);

const mode = ref<"edit" | "copy">("edit");
const formOpen = ref(false);
const removeOpen = ref(false);
const adding = ref(false);
const actions = computed<ToolbarAction[]>(() => [
  {
    id: "edit",
    label: t("admin.roles.edit"),
    icon: "pencil",
    shortcut: ["e"],
    priority: 2,
    ...refusalOf(refusals.value.edit),
  },
  {
    id: "copy",
    label: t("admin.roles.copy"),
    icon: "copy",
    priority: 1,
    ...refusalOf(refusals.value.copy),
  },
  {
    id: "remove",
    label: t("admin.roles.remove"),
    icon: "trash",
    ...refusalOf(
      refusals.value.remove ?? (closing.value ? { reason: t("admin.why.drafted") } : undefined),
    ),
  },
]);
function onAction(id: string): void {
  if (id === "remove") removeOpen.value = true;
  else {
    mode.value = id === "copy" ? "copy" : "edit";
    formOpen.value = true;
  }
}
useViewAction("edit", {
  label: () => t("admin.roles.editTitle", { role: props.id }),
  run: () => onAction("edit"),
  // The lasting reason only: held by the gate alone, it stays in the palette and says the
  // gate when run (`play`); refused for a lasting reason, `e` says it, as the button does.
  allowed: () => !!role.value && !lasting.value.edit,
  why: () => refusalSentence(lasting.value.edit, t),
  say: status,
  lasting: true,
});
async function remove(): Promise<void> {
  await draft.add(
    [{ op: "close_role", key: props.id }],
    t("admin.roles.removed", { role: props.id }),
  );
}

/** The entries of the role as the draft would leave it, changed by `next`; whether written. */
async function setEntries(next: readonly string[]): Promise<boolean> {
  if (!role.value) return false;
  return draft.writeRole(
    role.value,
    props.id,
    role.value.labels,
    next,
    t("admin.roles.modified", { role: props.id }),
  );
}
const current = () => shown.value?.entries ?? [];
const removeEntry = (entry: string) => setEntries(current().filter((e) => e !== entry));
const restoreEntry = (entry: string) => setEntries([...current(), entry]);
// A permission added taken back: its row gone, the focus to « Ajouter une permission ».
const entriesAdd = ref<HTMLElement | null>(null);
async function onEntry(row: EntryRow): Promise<void> {
  if (row.pending === "removed") {
    await restoreEntry(row.entry);
    return;
  }
  if ((await removeEntry(row.entry)) && row.pending === "added") {
    entriesAdd.value?.querySelector<HTMLElement>("button")?.focus();
  }
}

// « Ajouter une permission »: a permission the dictionary names, or a pattern typed.
const dictionary = useQuery(() => labelDictionary(locale.value));
const permissionOptions = computed<ListOption[]>(() => {
  const space = (dictionary.data.value as Entry<Dictionary> | undefined)?.data.spaces.permission;
  return Object.entries(space ?? {})
    .filter(([key]) => !current().includes(key))
    .map(([key, words]) => ({
      value: key,
      label: typeof words === "string" ? words : key,
      detail: key,
      mono: true,
    }));
});
const suggest = async (query: string) => filterOptions(permissionOptions.value, query);
const text = ref("");
const chosen = ref<string | undefined>(undefined);
const tried = ref(false);
watch(adding, (now) => {
  if (!now) return;
  text.value = "";
  chosen.value = undefined;
  tried.value = false;
});
const wanted = computed(() => chosen.value ?? text.value.trim());
const addError = computed(() =>
  !tried.value
    ? undefined
    : !wanted.value
      ? t("admin.required", { field: t("admin.roles.permission") })
      : current().includes(wanted.value)
        ? t("admin.keyTaken")
        : undefined,
);
/** Why nothing is sent now: the window narrowed (or the period moved) since it opened. */
const held = computed(() => draft.gate.why.value);
async function addEntry(): Promise<void> {
  // Refused by the gate, read when sending: said in the dialog, the input kept.
  if (held.value) return;
  tried.value = true;
  if (addError.value) return;
  if (await setEntries([...current(), wanted.value])) adding.value = false;
}
const read = computed(() => roles.query);
const entryRefusal = computed(() => refusals.value.edit);
const labelOfRole = computed(() =>
  role.value ? (role.value.labels[locale.value] ?? keyLabel("role", role.value.key)) : null,
);
</script>

<template>
  <ProblemState
    v-if="read.isError.value"
    :title="t('admin.roles.error')"
    :error="read.error.value"
    :retry="read.refetch"
  />
  <Skeleton v-else-if="read.isPending.value" variant="rows" :rows="6" />
  <EmptyState v-else-if="!role" :title="t('admin.roles.unknown')">
    {{ t("admin.roles.unknownText") }}
  </EmptyState>
  <Stack v-else gap="4" class="k-admin-detail">
    <span class="k-visually-hidden" role="status">{{ said }}</span>
    <Stack gap="2">
      <h2 class="k-admin-heading">
        <template v-if="labelOfRole">{{ `${labelOfRole} ` }}</template>
        <span class="k-mono k-admin-muted">{{ role.key }}</span>
      </h2>
      <span v-if="role.builtin"
        ><StatusBadge state="info" :label="t('admin.field.integrated')"
      /></span>
      <InlineAlert v-if="closing" tone="info" :title="t('admin.roles.removedInDraft')" />
      <Toolbar
        :label="t('admin.roles.detailActions')"
        :actions="actions"
        align="start"
        @action="onAction"
      />
      <RefusalAlert v-if="draft.refusal.value" :refusal="draft.refusal.value" :note="null" />
    </Stack>

    <section aria-labelledby="k-admin-permissions">
      <Stack gap="2">
        <h3 id="k-admin-permissions" class="k-admin-heading">
          {{ t("admin.roles.permissions", { n: role.entries.length }) }}
        </h3>
        <p v-if="entries.length === 0" class="k-admin-muted">
          {{ t("admin.roles.noPermissions") }}
        </p>
        <AdminLines v-else :label="t('admin.roles.permissionList', { role: role.key })">
          <li v-for="row in entries" :key="row.entry" :data-pending="row.pending">
            <span class="k-admin-lines__text">
              <Icon
                v-if="row.pending"
                :name="row.pending === 'removed' ? 'minus' : 'plus'"
                meaningful
                :label="t(`admin.pending.${row.pending}`)"
                class="k-admin-lines__mark"
              />
              <span class="k-admin-lines__label">{{ entryLabel(row.entry) ?? row.entry }}</span>
            </span>
            <CodeBadge :code="row.entry" />
            <span class="k-admin-lines__actions">
              <!-- The same button in every state of the row: it keeps the focus (2.4.3). -->
              <IconButton
                data-k-row-action
                :icon="
                  row.pending === 'removed' ? 'undo-2' : row.pending === 'added' ? 'x' : 'trash'
                "
                size="sm"
                :label="
                  t(
                    row.pending === 'removed'
                      ? 'admin.roles.undoPermission'
                      : row.pending === 'added'
                        ? 'admin.roles.dropPermission'
                        : 'admin.roles.removePermission',
                    { code: row.entry },
                  )
                "
                :loading="draft.busy.value"
                v-bind="row.pending ? gateRefusal : refusalOf(entryRefusal)"
                @click="onEntry(row)"
              />
            </span>
          </li>
        </AdminLines>
        <span ref="entriesAdd">
          <Button
            size="sm"
            variant="ghost"
            icon="plus"
            v-bind="refusalOf(entryRefusal)"
            @click="adding = true"
            >{{ t("admin.roles.addPermission") }}</Button
          >
        </span>
      </Stack>
    </section>

    <section aria-labelledby="k-admin-bound">
      <Stack gap="2">
        <h3 id="k-admin-bound" class="k-admin-heading">
          {{ t("admin.roles.groups", { n: boundGroups }) }}
        </h3>
        <ProblemState
          v-if="bindings.query.isError.value"
          :title="t('admin.bindings.error')"
          :error="bindings.query.error.value"
          :retry="bindings.query.refetch"
        />
        <Skeleton v-else-if="bindings.query.isPending.value" variant="rows" :rows="2" />
        <p v-else-if="bound.length === 0" class="k-admin-muted">{{ t("admin.roles.noGroups") }}</p>
        <AdminLines v-else :label="t('admin.roles.groupList', { role: role.key })" read>
          <li v-for="b in bound" :key="b.id">
            <KeyLabel :value="b.group" :label="groupLabel(b.group)" stacked />
            <span class="k-admin-lines__value">{{ scopeWords(b.scope) }}</span>
          </li>
        </AdminLines>
      </Stack>
    </section>
    <RoleForm
      v-model:open="formOpen"
      :mode="mode"
      :role="shown"
      :taken="roles.rows.value.map((r) => r.key)"
    />
    <AlertDialog
      v-model:open="removeOpen"
      :title="t('admin.roles.removeTitle', { role: role.key })"
      :description="t('admin.roles.removeText')"
      :confirm="t('admin.roles.removeConfirm')"
      danger
      @confirm="remove"
    />
    <Dialog
      v-model:open="adding"
      :title="t('admin.roles.addPermissionTitle', { role: role.key })"
      size="md"
    >
      <form :id="formId" novalidate @submit.prevent="addEntry">
        <Stack gap="3">
          <Combobox
            v-model="text"
            v-model:value="chosen"
            :label="t('admin.roles.permission')"
            :hint="t('admin.roles.permissionHint')"
            :error="addError"
            :suggest="suggest"
            mono
            required
          />
          <InlineAlert v-if="held" tone="warn" icon="lock" :title="held" />
          <RefusalAlert v-if="draft.refusal.value" :refusal="draft.refusal.value" :note="null" />
        </Stack>
      </form>
      <template #actions>
        <Button variant="ghost" @click="adding = false">{{ t("admin.cancel") }}</Button>
        <Button
          variant="primary"
          icon="draft"
          type="submit"
          :form="formId"
          :loading="draft.busy.value"
          v-bind="held ? { reason: held } : {}"
          >{{ t("admin.toDraft") }}</Button
        >
      </template>
    </Dialog>
  </Stack>
</template>
