<script setup lang="ts">
// The service accounts (005, K8; REQ-007-19, REQ-021-12; design 021 « Administration »;
// maquette E7, vue `secret`): one `DataGrid` of `GET /v1/service-accounts`, by name, its
// state and its active secrets; a row chosen opens its account in the Détail tab of the
// right panel (`/admin/accounts/:id`); one whose secret the draft revokes is marked.
// « Créer » (`c`) asks its name, the expiry of its first secret and the motive, then shows
// the secret once. Three states.
import {
  DataGrid,
  EmptyState,
  StatusBadge,
  Toolbar,
  type CellRef,
  type GridColumn,
  type GridSort,
  type ToolbarAction,
} from "@kartenn/ui";
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import { useRouter } from "vue-router";
import { revealDetail } from "../frame/detail.ts";
import ProblemState from "../problems/ProblemState.vue";
import { useSession } from "../rights/index.ts";
import { useViewAction } from "../shortcuts/view.ts";
import { useTime } from "../time/index.ts";
import ViewPanel from "../views/parts/ViewPanel.vue";
import { ACCOUNT_ADMIN, activeSecrets, globalRefusal, type AccountItem } from "./access.ts";
import { useAdminDraft } from "./draft.ts";
import { useList } from "./reads.ts";
import SecretDialog from "./SecretDialog.vue";

const props = defineProps<{ selected?: string | undefined }>();
const { t, locale } = useI18n();
const router = useRouter();
const session = useSession();
const time = useTime();
const draft = useAdminDraft();

const title = computed(() => t("nav.accounts"));
const { query, rows } = useList("/v1/service-accounts");
type Row = AccountItem & { active: number; state: string };
const shown = computed<Row[]>(() =>
  rows.value.map((a) => ({ ...a, active: activeSecrets(a), state: a.disabled ? "1" : "0" })),
);
const sort = ref<GridSort>({ column: "name", direction: "ascending" });
const columns = computed<GridColumn[]>(() => [
  { id: "name", label: t("admin.accounts.account"), width: "fill", mono: true, sortable: true },
  { id: "state", label: t("admin.field.state"), width: "md", sortable: true },
  {
    id: "active",
    label: t("admin.accounts.secrets"),
    width: "sm",
    align: "end",
    sortable: true,
    priority: 2,
  },
]);
const sorted = computed(() => {
  const { column, direction } = sort.value;
  const sign = direction === "descending" ? -1 : 1;
  const value = (a: Row) => a[column as keyof Row];
  return [...shown.value].sort((a, b) => {
    const x = value(a);
    const y = value(b);
    return (
      sign *
      (typeof x === "number" ? x - (y as number) : String(x).localeCompare(String(y), locale.value))
    );
  });
});
const rowState = (row: Row) =>
  draft.operations.value.some(
    (op) =>
      (op as { op: string; account?: string }).op === "revoke_service_secret" &&
      (op as { account?: string }).account === row.id,
  )
    ? ("draft" as const)
    : undefined;

/** Opens an account; the one shown already, the panel reopened on it. */
function open(id: string): void {
  if (id === props.selected) revealDetail();
  else void router.push(`/admin/accounts/${encodeURIComponent(id)}`);
}
function onSelected(keys: string[]): void {
  const id = keys[0] ?? props.selected;
  if (id) open(id);
}
const onActivate = (cell: CellRef) => open(cell.row);

// « Créer »: the global right and the present (nothing is written in the past).
const creating = ref(false);
const createRefusal = computed(
  () =>
    globalRefusal({
      me: session.value.me,
      permission: ACCOUNT_ADMIN,
      present: time.value.present,
      past: t("edit.why.past"),
    }) ?? {},
);
const actions = computed<ToolbarAction[]>(() => [
  {
    id: "create",
    label: t("admin.accounts.create"),
    icon: "plus",
    variant: "primary",
    shortcut: ["c"],
    ...createRefusal.value,
  },
]);
useViewAction("create", {
  label: () => t("admin.accounts.createTitle"),
  run: () => (creating.value = true),
  allowed: () => Object.keys(createRefusal.value).length === 0,
});
const total = computed(() =>
  query.isSuccess.value ? new Intl.NumberFormat(locale.value).format(rows.value.length) : undefined,
);
</script>

<template>
  <h1 class="k-visually-hidden">{{ title }}</h1>
  <ViewPanel :title="title" :detail="total" main flush>
    <template #actions>
      <Toolbar
        :label="t('admin.accounts.actions')"
        :actions="actions"
        align="start"
        @action="creating = true"
      />
    </template>
    <ProblemState
      v-if="query.isError.value"
      :title="t('admin.accounts.error')"
      :error="query.error.value"
      :retry="query.refetch"
    >
      {{ t("views.errorText") }}
    </ProblemState>
    <EmptyState
      v-else-if="query.isSuccess.value && rows.length === 0"
      :title="t('admin.accounts.none')"
    >
      {{ t("admin.accounts.noneText") }}
    </EmptyState>
    <DataGrid
      v-else
      v-model:sort="sort"
      :label="title"
      :columns="columns"
      :rows="sorted"
      row-key="id"
      selectable="single"
      :selected="props.selected ? [props.selected] : []"
      :row-state="rowState"
      :loading="query.isPending.value"
      :loading-rows="6"
      @update:selected="onSelected"
      @activate="onActivate"
    >
      <template #cell="{ row, column, value }">
        <StatusBadge
          v-if="column.id === 'state'"
          :state="row.disabled ? 'neutral' : 'ok'"
          :label="row.disabled ? t('admin.accounts.disabled') : t('admin.accounts.enabled')"
        />
        <span v-else-if="column.id === 'active'" class="k-num">{{ value }}</span>
        <template v-else>{{ value }}</template>
      </template>
    </DataGrid>
  </ViewPanel>
  <SecretDialog v-model:open="creating" :taken="rows.map((a) => a.name)" />
</template>
