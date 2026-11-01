<script setup lang="ts">
// The roles (005, REQ-005-17, 25; REQ-021-12; design 021 « Administration »; maquette E7,
// vue `roles`): one `DataGrid` of `GET /v1/roles`, the built-in ones and those of the
// organisation, with the count of their permissions and of the groups bound to them; a row
// chosen opens its role in the Détail tab (`/admin/roles/:key`); a role the active draft
// defines or deletes is marked. « Créer » (`c`). Three states.
import {
  DataGrid,
  EmptyState,
  KeyLabel,
  Toolbar,
  type CellRef,
  type GridColumn,
  type GridSort,
  type ToolbarAction,
} from "@kartenn/ui";
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import { useRouter } from "vue-router";
import ProblemState from "../problems/ProblemState.vue";
import { useSession } from "../rights/index.ts";
import { useViewAction } from "../shortcuts/view.ts";
import { useTime } from "../time/index.ts";
import { useKeyLabel } from "../views/data.ts";
import ViewPanel from "../views/parts/ViewPanel.vue";
import { useAdminDraft } from "./draft.ts";
import { ROLE_ADMIN, touchesRole, type RoleItem } from "./model.ts";
import { useList } from "./reads.ts";
import RoleForm from "./RoleForm.vue";

const props = defineProps<{ selected?: string | undefined }>();
const { t, locale } = useI18n();
const router = useRouter();
const session = useSession();
const time = useTime();
const keyLabel = useKeyLabel();
const draft = useAdminDraft();

const title = computed(() => t("nav.roles"));
const { query, rows } = useList("/v1/roles");
const bindings = useList("/v1/bindings");
/** The groups bound to each role, now. */
const groupsOf = computed(() => {
  const by = new Map<string, Set<string>>();
  for (const b of bindings.rows.value) by.set(b.role, (by.get(b.role) ?? new Set()).add(b.group));
  return by;
});
type Row = RoleItem & { permissions: number; groups: number | null };
const shown = computed<Row[]>(() =>
  rows.value.map((r) => ({
    ...r,
    permissions: r.entries.length,
    groups: bindings.query.isSuccess.value ? (groupsOf.value.get(r.key)?.size ?? 0) : null,
  })),
);
const sort = ref<GridSort>({ column: "key", direction: "ascending" });
const columns = computed<GridColumn[]>(() => [
  { id: "key", label: t("admin.field.role"), width: "lg", sortable: true },
  { id: "builtin", label: t("admin.field.integrated"), width: "sm", sortable: true, priority: 3 },
  {
    id: "permissions",
    label: t("admin.field.permissions"),
    width: "sm",
    align: "end",
    sortable: true,
  },
  {
    id: "groups",
    label: t("admin.field.groups"),
    width: "sm",
    align: "end",
    sortable: true,
    priority: 2,
  },
]);
const sorted = computed(() => {
  const { column, direction } = sort.value;
  const sign = direction === "descending" ? -1 : 1;
  const value = (r: Row) => r[column as keyof Row];
  return [...shown.value].sort((a, b) => {
    const x = value(a);
    const y = value(b);
    return (
      sign * (typeof x === "number" ? x - ((y as number) ?? 0) : String(x).localeCompare(String(y)))
    );
  });
});
const labelOf = (r: RoleItem) => r.labels[locale.value] ?? keyLabel("role", r.key);
const rowState = (row: Row) =>
  draft.operations.value.some((op) => touchesRole(op, row.key)) ? ("draft" as const) : undefined;

const open = (key: string) => void router.push(`/admin/roles/${encodeURIComponent(key)}`);
function onSelected(keys: string[]): void {
  if (keys[0] && keys[0] !== props.selected) open(keys[0]);
}
const onActivate = (cell: CellRef) => open(cell.row);

const creating = ref(false);
const createRefusal = computed(() =>
  !session.value.rights.has(ROLE_ADMIN)
    ? { forbidden: ROLE_ADMIN }
    : !time.value.present
      ? { reason: t("edit.why.past") }
      : {},
);
const actions = computed<ToolbarAction[]>(() => [
  {
    id: "create",
    label: t("admin.roles.create"),
    icon: "plus",
    variant: "primary",
    shortcut: ["c"],
    ...createRefusal.value,
  },
]);
useViewAction("create", {
  label: () => t("admin.roles.createAction"),
  run: () => (creating.value = true),
  allowed: () => session.value.rights.has(ROLE_ADMIN),
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
        :label="t('admin.roles.actions')"
        :actions="actions"
        align="start"
        @action="creating = true"
      />
    </template>
    <ProblemState
      v-if="query.isError.value"
      :title="t('admin.roles.error')"
      :error="query.error.value"
      :retry="query.refetch"
    >
      {{ t("views.errorText") }}
    </ProblemState>
    <EmptyState
      v-else-if="query.isSuccess.value && rows.length === 0"
      :title="t('admin.roles.none')"
    >
      {{ t("admin.roles.noneText") }}
    </EmptyState>
    <DataGrid
      v-else
      v-model:sort="sort"
      :label="title"
      :columns="columns"
      :rows="sorted"
      row-key="key"
      selectable="single"
      :selected="props.selected ? [props.selected] : []"
      :row-state="rowState"
      :loading="query.isPending.value"
      :loading-rows="8"
      @update:selected="onSelected"
      @activate="onActivate"
    >
      <template #cell="{ row, column, value }">
        <KeyLabel
          v-if="column.id === 'key'"
          :value="row.key"
          :label="labelOf(row)"
          show-key
          plain
        />
        <template v-else-if="column.id === 'builtin'">{{
          row.builtin ? t("admin.yes") : t("admin.no")
        }}</template>
        <span v-else class="k-num">{{ value ?? "—" }}</span>
      </template>
    </DataGrid>
  </ViewPanel>
  <RoleForm v-model:open="creating" mode="create" :taken="rows.map((r) => r.key)" />
</template>
