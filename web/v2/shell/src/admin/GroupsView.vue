<script setup lang="ts">
// The groups of kartenn (spec 025, REQ-025-12; REQ-021-12; design 021 « Administration »;
// maquette E7, vue `groupes`): one `DataGrid` of `GET /v1/groups`, by key; a row chosen
// opens its group in the Détail tab of the right panel (`/admin/groups/:id`); a group the
// active draft touches is marked. « Créer » (`c`) asks its key and label. Three states.
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
import { revealDetail } from "../frame/detail.ts";
import ProblemState from "../problems/ProblemState.vue";
import { useSession } from "../rights/index.ts";
import { useViewAction } from "../shortcuts/view.ts";
import { useKeyLabel } from "../views/data.ts";
import { useStatus } from "../views/edit/say.ts";
import ViewPanel from "../views/parts/ViewPanel.vue";
import { useAdminDraft } from "./draft.ts";
import { useAdminGate } from "./gate.ts";
import GroupForm from "./GroupForm.vue";
import { RBAC_ADMIN, touchesGroup, type GroupItem } from "./model.ts";
import { useList } from "./reads.ts";

const props = defineProps<{ selected?: string | undefined }>();
const { t, locale } = useI18n();
const router = useRouter();
const session = useSession();
const gate = useAdminGate();
const keyLabel = useKeyLabel();
const draft = useAdminDraft();
/** Why `c` is refused, said politely (the key has no button to carry it). */
const { said, status } = useStatus();

const title = computed(() => t("nav.groups"));
const { query, rows } = useList("/v1/groups");
const sort = ref<GridSort>({ column: "key", direction: "ascending" });
const columns = computed<GridColumn[]>(() => [
  { id: "key", label: t("nav.groups"), width: "lg", sortable: true },
  { id: "builtin", label: t("admin.field.builtin"), width: "sm", sortable: true, priority: 3 },
  { id: "members", label: t("admin.field.members"), width: "sm", align: "end", sortable: true },
  { id: "bindings", label: t("admin.field.roles"), width: "sm", align: "end", sortable: true },
]);
const sorted = computed(() => {
  const { column, direction } = sort.value;
  const sign = direction === "descending" ? -1 : 1;
  const value = (g: GroupItem) => g[column as keyof GroupItem];
  return [...rows.value].sort((a, b) => {
    const x = value(a);
    const y = value(b);
    return sign * (typeof x === "number" ? x - (y as number) : String(x).localeCompare(String(y)));
  });
});
const labelOf = (g: GroupItem) => g.labels[locale.value] ?? keyLabel("group", g.key);
const rowState = (row: GroupItem) =>
  draft.operations.value.some((op) => touchesGroup(op, row.key)) ? ("draft" as const) : undefined;

/** Opens a group; the one shown already, the panel reopened on it. */
function open(id: string): void {
  if (id === props.selected) revealDetail();
  else void router.push(`/admin/groups/${encodeURIComponent(id)}`);
}
// The row chosen again (its selection taken off by the grid): the panel shows it again.
function onSelected(keys: string[]): void {
  const id = keys[0] ?? props.selected;
  if (id) open(id);
}
const onActivate = (cell: CellRef) => open(cell.row);

// « Créer »: the right, then the gate of writes (the present, 1024 px wide).
const creating = ref(false);
const createRefusal = computed(() =>
  !session.value.rights.has(RBAC_ADMIN)
    ? { forbidden: RBAC_ADMIN }
    : gate.why.value
      ? { reason: gate.why.value }
      : {},
);
const actions = computed<ToolbarAction[]>(() => [
  {
    id: "create",
    label: t("admin.groups.create"),
    icon: "plus",
    variant: "primary",
    shortcut: ["c"],
    ...createRefusal.value,
  },
]);
useViewAction("create", {
  label: () => t("admin.groups.createAction"),
  run: () => (creating.value = true),
  allowed: () => session.value.rights.has(RBAC_ADMIN),
  say: status,
});
const total = computed(() =>
  query.isSuccess.value ? new Intl.NumberFormat(locale.value).format(rows.value.length) : undefined,
);
</script>

<template>
  <h1 class="k-visually-hidden">{{ title }}</h1>
  <ViewPanel :title="title" :detail="total" main flush>
    <!-- At the start: the right panel may cover the end of the surface. -->
    <template #actions>
      <Toolbar
        :label="t('admin.groups.actions')"
        :actions="actions"
        align="start"
        @action="creating = true"
      />
    </template>
    <ProblemState
      v-if="query.isError.value"
      :title="t('admin.groups.error')"
      :error="query.error.value"
      :retry="query.refetch"
    >
      {{ t("views.errorText") }}
    </ProblemState>
    <EmptyState
      v-else-if="query.isSuccess.value && rows.length === 0"
      :title="t('admin.groups.none')"
    >
      {{ t("admin.groups.noneText") }}
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
        <span v-else class="k-num">{{ value }}</span>
      </template>
    </DataGrid>
  </ViewPanel>
  <span class="k-visually-hidden" role="status">{{ said }}</span>
  <GroupForm v-model:open="creating" :taken="rows.map((g) => g.key)" />
</template>
