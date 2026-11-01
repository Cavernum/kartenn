<script setup lang="ts">
// The issuers attached (005, ADR 0021; REQ-007-19, REQ-021-12; design 021
// « Administration »): one `DataGrid` of `GET /v1/issuer-attachments`, the issuer (`iss`)
// and the entity it serves, or the whole organisation. « Rattacher » (`c`) asks the issuer
// and its entity; « Détacher » detaches the row chosen; both to the draft
// (`attach_issuer`, `detach_issuer`), a detachment marked in place, an attachment at the
// end, the action of a marked row taking it out of the draft. Under `core.issuer.admin`,
// held on the whole organisation (005). Three states.
import type { Operation } from "@kartenn/sdk";
import {
  Button,
  Combobox,
  DataGrid,
  Dialog,
  EmptyState,
  Select,
  Stack,
  TextField,
  Toolbar,
  filterOptions,
  type GridColumn,
  type GridSort,
  type ListOption,
  type ToolbarAction,
} from "@kartenn/ui";
import { computed, ref, useId, watch } from "vue";
import { useI18n } from "vue-i18n";
import ProblemState from "../problems/ProblemState.vue";
import { useSession } from "../rights/index.ts";
import { useViewAction } from "../shortcuts/view.ts";
import { useTime } from "../time/index.ts";
import { nameOf, type Item } from "../views/collections.ts";
import RefusalAlert from "../views/edit/RefusalAlert.vue";
import ViewPanel from "../views/parts/ViewPanel.vue";
import {
  attachmentKey,
  attachmentOp,
  attachmentRows,
  globalRefusal,
  ISSUER_ADMIN,
  type AttachmentRow,
} from "./access.ts";
import { useAdminDraft } from "./draft.ts";
import { useList, useScopeNames } from "./reads.ts";

const { t, locale } = useI18n();
const session = useSession();
const time = useTime();
const draft = useAdminDraft();

const title = computed(() => t("nav.issuers"));
const { query, rows } = useList("/v1/issuer-attachments");
const attachments = computed(() => attachmentRows(rows.value, draft.operations.value));
const entityName = useScopeNames(() =>
  attachments.value.map((a) => ({ kind: "entity", id: a.entity ?? null })),
);
const entityWords = (row: AttachmentRow) =>
  !row.entity ? t("admin.bindings.scopes.global") : (entityName(row.entity) ?? row.entity);
type Row = AttachmentRow & { scope: string };
const shown = computed<Row[]>(() =>
  attachments.value.map((a) => ({ ...a, scope: entityWords(a) })),
);
const sort = ref<GridSort>({ column: "issuer", direction: "ascending" });
const columns = computed<GridColumn[]>(() => [
  { id: "issuer", label: t("admin.members.issuer"), width: "fill", mono: true, sortable: true },
  { id: "scope", label: t("admin.issuers.entity"), width: "lg", sortable: true },
]);
// The served rows sorted, those the draft adds after them, in their order.
const sorted = computed(() => {
  const { column, direction } = sort.value;
  const sign = direction === "descending" ? -1 : 1;
  const value = (a: Row) => String(a[column as keyof Row] ?? "");
  const served = shown.value.filter((a) => a.pending !== "added");
  const added = shown.value.filter((a) => a.pending === "added");
  return [
    ...served.sort((a, b) => sign * value(a).localeCompare(value(b), locale.value)),
    ...added,
  ];
});
const rowState = (row: Row) =>
  row.pending === "removed" ? ("removed" as const) : row.pending ? ("draft" as const) : undefined;

const selected = ref<string | undefined>(undefined);
const chosen = computed(() => shown.value.find((a) => a.key === selected.value));
const onSelected = (keys: string[]) => (selected.value = keys[0]);

const refusal = computed(
  () =>
    globalRefusal({
      me: session.value.me,
      permission: ISSUER_ADMIN,
      present: time.value.present,
      past: t("edit.why.past"),
    }) ?? {},
);
const detachAction = computed<ToolbarAction>(() => {
  const row = chosen.value;
  const base = { id: "detach", icon: row?.pending ? "undo-2" : "trash" } as const;
  if (!row) return { ...base, label: t("admin.issuers.detach"), reason: t("admin.issuers.choose") };
  if (row.pending === "removed") return { ...base, label: t("admin.issuers.undoDetach") };
  if (row.pending === "added") return { ...base, label: t("admin.issuers.dropAttach") };
  return { ...base, label: t("admin.issuers.detach"), ...refusal.value };
});
const actions = computed<ToolbarAction[]>(() => [
  {
    id: "attach",
    label: t("admin.issuers.attach"),
    icon: "plus",
    variant: "primary",
    shortcut: ["c"],
    ...refusal.value,
  },
  detachAction.value,
]);
const attaching = ref(false);
async function onAction(id: string): Promise<void> {
  if (id === "attach") {
    attaching.value = true;
    return;
  }
  const row = chosen.value;
  if (!row) return;
  const what = t("admin.issuers.detached", { issuer: row.issuer, entity: entityWords(row) });
  if (row.index !== undefined) await draft.drop(row.index, what);
  else await draft.add([attachmentOp("detach_issuer", row.issuer, row.entity)], what);
}
useViewAction("create", {
  label: () => t("admin.issuers.attachTitle"),
  run: () => (attaching.value = true),
  allowed: () => Object.keys(refusal.value).length === 0,
});
const total = computed(() =>
  query.isSuccess.value ? new Intl.NumberFormat(locale.value).format(rows.value.length) : undefined,
);

// « Rattacher »: an issuer, on the whole organisation or an entity among the first ones.
const formId = useId();
const issuer = ref("");
const scope = ref<string | undefined>("global");
const entityText = ref("");
const entity = ref<string | undefined>(undefined);
const tried = ref(false);
watch(attaching, (now) => {
  if (!now) return;
  issuer.value = "";
  scope.value = "global";
  entityText.value = "";
  entity.value = undefined;
  tried.value = false;
  draft.refusal.value = null;
});
const entities = useList("/v1/entities", null, undefined, attaching);
const scopeOptions = computed<ListOption[]>(() => [
  { value: "global", label: t("admin.bindings.scopes.global") },
  { value: "entity", label: t("admin.bindings.scopes.entity") },
]);
const entityOptions = computed<ListOption[]>(() =>
  entities.rows.value.map((e) => ({
    value: String(e.id),
    label: nameOf(e as Item),
    detail: String((e as Item).slug ?? ""),
  })),
);
const suggest = async (text: string) => filterOptions(entityOptions.value, text);
const issuerError = computed(() => {
  if (!tried.value) return undefined;
  const text = issuer.value.trim();
  if (!text) return t("admin.required", { field: t("admin.members.issuer") });
  const target = scope.value === "entity" ? (entity.value ?? null) : null;
  return attachments.value.some((a) => a.key === attachmentKey(text, target))
    ? t("admin.issuers.already")
    : undefined;
});
const entityError = computed(() =>
  tried.value && scope.value === "entity" && !entity.value
    ? t("admin.required", { field: t("admin.issuers.entity") })
    : undefined,
);
async function attach(): Promise<void> {
  tried.value = true;
  if (issuerError.value || entityError.value) return;
  const target = scope.value === "entity" ? (entity.value ?? null) : null;
  const text = issuer.value.trim();
  const words = target
    ? (entityOptions.value.find((o) => o.value === target)?.label ?? target)
    : t("admin.bindings.scopes.global");
  const op: Operation = attachmentOp("attach_issuer", text, target);
  if (await draft.add([op], t("admin.issuers.attached", { issuer: text, entity: words }))) {
    attaching.value = false;
  }
}
</script>

<template>
  <h1 class="k-visually-hidden">{{ title }}</h1>
  <ViewPanel :title="title" :detail="total" main flush>
    <template #actions>
      <Toolbar
        :label="t('admin.issuers.actions')"
        :actions="actions"
        align="start"
        @action="onAction"
      />
    </template>
    <ProblemState
      v-if="query.isError.value"
      :title="t('admin.issuers.error')"
      :error="query.error.value"
      :retry="query.refetch"
    >
      {{ t("views.errorText") }}
    </ProblemState>
    <EmptyState
      v-else-if="query.isSuccess.value && attachments.length === 0"
      :title="t('admin.issuers.none')"
    >
      {{ t("admin.issuers.noneText") }}
    </EmptyState>
    <DataGrid
      v-else
      v-model:sort="sort"
      :label="title"
      :columns="columns"
      :rows="sorted"
      row-key="key"
      selectable="single"
      :selected="selected ? [selected] : []"
      :row-state="rowState"
      :loading="query.isPending.value"
      :loading-rows="3"
      @update:selected="onSelected"
    />
    <RefusalAlert
      v-if="draft.refusal.value && !attaching"
      :refusal="draft.refusal.value"
      :note="null"
    />
  </ViewPanel>
  <Dialog v-model:open="attaching" :title="t('admin.issuers.attachTitle')" size="md">
    <form :id="formId" novalidate @submit.prevent="attach">
      <Stack gap="3">
        <TextField
          v-model="issuer"
          :label="t('admin.members.issuer')"
          :hint="t('admin.issuers.issuerHint')"
          :error="issuerError"
          mono
          required
          autocomplete="off"
        />
        <Select v-model="scope" :label="t('admin.bindings.scope')" :options="scopeOptions" />
        <Combobox
          v-if="scope === 'entity'"
          v-model="entityText"
          v-model:value="entity"
          :label="t('admin.issuers.entity')"
          :hint="t('admin.bindings.objectHint')"
          :error="entityError"
          :suggest="suggest"
          required
        />
        <RefusalAlert v-if="draft.refusal.value" :refusal="draft.refusal.value" :note="null" />
      </Stack>
    </form>
    <template #actions>
      <Button variant="ghost" @click="attaching = false">{{ t("admin.cancel") }}</Button>
      <Button
        variant="primary"
        icon="draft"
        type="submit"
        :form="formId"
        :loading="draft.busy.value"
        >{{ t("admin.toDraft") }}</Button
      >
    </template>
  </Dialog>
</template>
