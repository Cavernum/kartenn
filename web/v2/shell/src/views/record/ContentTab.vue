<script setup lang="ts">
// The content of a domain or of a prefix (REQ-021-9; maquette E2, onglet Contenu): the
// prefixes at the top of a domain (`roots`), the direct children of a prefix (prefixes,
// ranges, addresses: `children`), each a link to its record, read page by page at the
// dates of the page; three states. Edited in place (task 12a, REQ-021-15): the nature of a
// prefix and the usage of a range, at the version their page served (`edit/nodes.ts`);
// Ctrl+Enter asks for its motive under the grid. Beside the tree of the topography, the
// editing of the tree (`edit`, task 12f2): one for both, its line of the motive under the
// grid it was asked from.
import { reads, type components } from "@kartenn/sdk";
import {
  DataGrid,
  EmptyState,
  Icon,
  KeyLabel,
  type CellRef,
  type GridColumn,
  type GridCommit,
  type GridWindow,
  type IconName,
} from "@kartenn/ui";
import { computed, nextTick, ref } from "vue";
import { useI18n } from "vue-i18n";
import { useInfiniteQuery } from "../../data/query.ts";
import KLink from "../../prefetch/KLink.vue";
import { useNextPage, type ListWindow } from "../../prefetch/nextPage.ts";
import { useProblems } from "../../problems/index.ts";
import { useTime } from "../../time/index.ts";
import { pageOfHref, pageOfObject, type Item } from "../collections.ts";
import EditHint from "../edit/EditHint.vue";
import { whyWords } from "../edit/inline.ts";
import {
  CONTENT_GRID,
  detailOf,
  nodeApplyRight,
  useNodeEdit,
  useNodeWords,
  type NodeEdit,
} from "../edit/nodes.ts";
import QuickApply from "../edit/QuickApply.vue";
import { useSay } from "../edit/say.ts";
import NextPageAlert from "../../problems/NextPageAlert.vue";
import ProblemState from "../../problems/ProblemState.vue";
import { rowsOf, useKeyLabel } from "../data.ts";

type TreeNode = components["schemas"]["TreeNode"];
interface Row extends Record<string, unknown> {
  id: string;
  kind: "prefix" | "range" | "address";
  value: string;
  detail: string | null;
  to: string | null;
}

const props = defineProps<{
  kind: "domain" | "prefix";
  id: string;
  name: string;
  /** The editing of the host that shows these nodes too (the tree), else its own. */
  nodeEdit?: NodeEdit | undefined;
}>();
const { t, te } = useI18n();
const time = useTime();
const keyLabel = useKeyLabel();

const content = useInfiniteQuery(() =>
  reads.subPages<TreeNode>(
    props.kind,
    props.id,
    props.kind === "domain" ? "roots" : "children",
    null,
    {
      validAt: time.value.validAt,
      asOf: time.value.asOf,
    },
  ),
);
const view = ref<ListWindow>({ end: 0, size: 0 });
useNextPage(content, view);
const onWindow = (window: GridWindow) => (view.value = { end: window.end, size: window.size });

const ICONS: Record<Row["kind"], IconName> = {
  prefix: "prefix",
  range: "range",
  address: "address",
};
// The nodes as their page serves them (their version with them), the value as shown.
const read = computed<Item[]>(() =>
  rowsOf(content.data.value).map((node) => ({
    ...node,
    value: node.kind === "range" ? `${node.first} – ${node.last}` : node.value,
    to: pageOfHref(node.href) ?? pageOfObject(node.kind, node.id),
  })),
);

// In-place editing: the words of a closed list stand for its key in a cell.
const words = useNodeWords();
const said = ref("");
const reread = () => void content.refetch();
// Given once, by the host that mounts it (the tree beside it): never another one after.
// Shared, the host says the outcomes (its status, its toasts): none here.
const shared = props.nodeEdit;
const edit =
  shared ??
  useNodeEdit({
    rows: () => read.value,
    words,
    reread,
    t: (key, values) => t(key, values ?? {}),
    sentence: useProblems().sentence,
    say: useSay((text) => {
      said.value = "";
      void nextTick(() => (said.value = text));
    }),
  });
shared?.share({ grid: CONTENT_GRID, rows: () => read.value, reread });
/** The motive asked here: its line under this grid (the tree's goes under the tree). */
const asked = computed(() => edit.askOf(CONTENT_GRID));
const rows = computed<Row[]>(() =>
  read.value.map((row) => {
    const shown = edit.shownRow(row);
    return { ...(shown as Row), detail: detailOf(shown) };
  }),
);
const readonlyReason = (row: Row, column: GridColumn) => whyWords(edit.why(column.id, row), t);
/** The cell of the focus, and what its keys do or why it has no editor, under the grid. */
const active = ref<CellRef | undefined>();
const activeRow = computed(() =>
  active.value && active.value.row !== ""
    ? read.value.find((row) => row.id === active.value!.row)
    : undefined,
);
const activeWhy = computed(() =>
  active.value && activeRow.value ? edit.why(active.value.column, activeRow.value) : null,
);
const applyLacks = computed(() =>
  activeRow.value && !edit.mayApply(activeRow.value) ? nodeApplyRight(activeRow.value) : null,
);
/** Why the cell of the focus is held in review, said under the grid too (seen, not only read). */
const activeHeld = computed(() =>
  active.value && activeRow.value ? edit.heldOf(activeRow.value, active.value.column) : undefined,
);

const columns = computed<GridColumn[]>(() => [
  { id: "value", label: t("views.record.object"), width: "fill", mono: true },
  { id: "detail", label: t("views.record.nature"), width: "md", priority: 2 },
]);
const kindWords = (kind: string) => keyLabel("kind", `core.${kind}`) ?? kind;
const detailWords = (row: Row) => {
  const key = `views.value.${row.kind === "prefix" ? "nature" : "usage"}.${row.detail}`;
  return te(key) ? t(key) : null;
};

/** The line of the motive closed: the focus back to its cell. */
async function afterAsk(cell: CellRef): Promise<void> {
  await nextTick();
  document
    .querySelector<HTMLElement>(
      `[data-k-content] [data-row="${CSS.escape(cell.row)}"][data-column="${CSS.escape(cell.column)}"]`,
    )
    ?.focus();
}
async function onConfirm(motive: string, value: string): Promise<void> {
  const ask = asked.value;
  if (ask && (await edit.confirm(motive, value))) void afterAsk(edit.cellOf(ask));
}
function onCancelAsk(): void {
  const ask = asked.value;
  edit.cancel();
  if (ask) void afterAsk(edit.cellOf(ask));
}
</script>

<template>
  <NextPageAlert
    v-if="content.isFetchNextPageError.value"
    :error="content.error.value"
    :retry="content.fetchNextPage"
  />
  <ProblemState
    v-if="content.isError.value && !content.isFetchNextPageError.value"
    :title="t('views.record.contentError')"
    :error="content.error.value"
    :retry="content.refetch"
  />
  <EmptyState
    v-else-if="content.isSuccess.value && rows.length === 0"
    :title="t('views.record.noContent')"
  >
    {{ t("views.record.noContentText") }}
  </EmptyState>
  <template v-else>
    <DataGrid
      v-model:active="active"
      data-k-content
      :label="t('views.record.contentOf', { name: props.name })"
      :columns="columns"
      :rows="rows"
      row-key="id"
      :loading="content.isPending.value"
      :loading-rows="4"
      :editable="edit.editable"
      :readonly-reason="readonlyReason"
      :validate="edit.validate"
      :cell-state="edit.cellState"
      :state-reason="edit.stateReason"
      :row-state="edit.rowState"
      :row-reason="edit.rowReason"
      @commit="(commit: GridCommit) => edit.onCommit(commit, CONTENT_GRID)"
      @window="onWindow"
    >
      <template #cell="{ row, column }">
        <template v-if="column.id === 'value'">
          <Icon :name="ICONS[row.kind]" meaningful :label="kindWords(row.kind)" />
          <KLink v-if="row.to" :to="row.to">{{ row.value }}</KLink>
          <template v-else>{{ row.value }}</template>
        </template>
        <KeyLabel v-else-if="row.detail" :value="row.detail" :label="detailWords(row)" plain />
      </template>
    </DataGrid>
    <QuickApply
      v-if="asked"
      :key="asked.row + asked.column"
      :ask="asked"
      :words="words"
      @confirm="onConfirm"
      @cancel="onCancelAsk"
    />
    <EditHint v-else-if="activeRow" :why="activeWhy" :apply-lacks="applyLacks" :held="activeHeld" />
    <!-- Its own sentences; shared, the host's status says them. -->
    <span v-if="!shared" class="k-visually-hidden" role="status">{{ said }}</span>
  </template>
</template>
