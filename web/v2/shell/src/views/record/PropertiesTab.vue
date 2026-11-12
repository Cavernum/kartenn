<script setup lang="ts">
// The resolved properties of an object (REQ-021-9, REQ-003-14; maquette E2): each its label
// and key, its value, and beside it the button of its explanation (`Popover`, read when it
// opens); a property the caller may not read is masked, said in words. Three states.
// Edited in place (task 12h, REQ-021-15, Q-021-19; `edit/properties.ts`): a value typed in
// its cell, checked by its declared type, to the draft (Enter) or applied with its motive
// (Ctrl+Enter, `properties` of the patch under `If-Match`); over an inherited value, the
// value of the object's own that overrides it, said under the grid (and when the editor
// opens) before it is sent; a cell without an editor says why.
import { reads, type components } from "@kartenn/sdk";
import {
  DataGrid,
  EmptyState,
  IconButton,
  KeyLabel,
  Popover,
  type CellRef,
  type GridColumn,
} from "@kartenn/ui";
import { computed, nextTick, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useQuery } from "../../data/query.ts";
import { useProblems } from "../../problems/index.ts";
import { useTime } from "../../time/index.ts";
import ProblemState from "../../problems/ProblemState.vue";
import type { Collection } from "../collections.ts";
import { useDeclarations, useKeyLabel } from "../data.ts";
import EditHint from "../edit/EditHint.vue";
import { whyWords } from "../edit/inline.ts";
import {
  propertyText,
  usePropertyEdit,
  type Declared,
  type PropertyRow,
} from "../edit/properties.ts";
import QuickApply from "../edit/QuickApply.vue";
import { useSay } from "../edit/say.ts";
import ExplanationBody from "./ExplanationBody.vue";
import PropertyValue from "./PropertyValue.vue";

type Resolved = components["schemas"]["Resolved"];

const props = defineProps<{
  collection: Collection;
  id: string;
  name: string;
  /** The version of the object read with its record: what a write expects (`If-Match`). */
  version: string | null;
}>();
const { t } = useI18n();
const time = useTime();
const keyLabel = useKeyLabel();
const { declarations } = useDeclarations();

const resolved = useQuery(() =>
  reads.sub<Resolved>(props.collection.kind, props.id, "properties", null, {
    validAt: time.value.validAt,
    asOf: time.value.asOf,
  }),
);
const read = computed<PropertyRow[]>(() => {
  const data = resolved.data.value?.data;
  if (!data) return [];
  const shown = Object.entries(data.properties as Record<string, unknown>).map(
    ([key, value]): PropertyRow => ({
      key,
      value: propertyText(value),
      read: value,
      masked: false,
    }),
  );
  const masked = data.masked.map((key): PropertyRow => ({
    key,
    value: "",
    read: null,
    masked: true,
  }));
  return [...shown, ...masked].sort((a, b) => a.key.localeCompare(b.key));
});
const declarationOf = (key: string) =>
  declarations.value.find((d) => d.key === key) as Declared | undefined;
const targetOf = (key: string) => {
  const target = declarationOf(key)?.params?.target;
  return typeof target === "string" ? target : undefined;
};
const labelOf = (key: string) => keyLabel("property", key);

const said = ref("");
const say = useSay((text) => {
  said.value = "";
  void nextTick(() => (said.value = text));
});
const edit = usePropertyEdit({
  collection: () => props.collection,
  id: () => props.id,
  name: () => props.name,
  version: () => props.version,
  rows: () => read.value,
  declaration: declarationOf,
  declarationById: (id) => (declarations.value as unknown as Declared[]).find((d) => d.id === id),
  label: (key) => labelOf(key) ?? key,
  valueLabel: (key, value) => keyLabel("value", `${key}/${value}`),
  reread: () => void resolved.refetch(),
  t: (key, values) => t(key, values ?? {}),
  sentence: useProblems().sentence,
  say,
});
const rows = computed(() => edit.shownRows.value);
const readonlyReason = (row: PropertyRow, column: GridColumn) =>
  column.id === "value" ? whyWords(edit.why(row), t) : undefined;

/** The cell of the focus, and what its keys do or why it has no editor, under the grid. */
const active = ref<CellRef | undefined>();
const activeKey = computed(() =>
  active.value?.column === "value" && active.value.row !== "" ? active.value.row : null,
);
watch(activeKey, (key) => (edit.focus.value = key), { immediate: true });
const activeWhy = computed(() => (activeKey.value ? edit.whyOf(activeKey.value) : null));
const applyLacks = computed(() =>
  activeKey.value && !edit.mayApply(activeKey.value) ? edit.applyRight(activeKey.value) : null,
);
// Said when the editor opens too, or once the origin is read if it comes later: heard
// before the value is sent, not only seen.
const editing = ref<CellRef | undefined>();
watch(
  () => (editing.value ? edit.noteOf(editing.value.row) : undefined),
  (note) => {
    if (note) say.status(note);
  },
);
/** What an edit of the cell of the focus writes, read with that cell (its name). */
const noteHere = (key: string) =>
  key === activeKey.value && activeWhy.value === null ? edit.noteOf(key) : undefined;

const columns = computed<GridColumn[]>(() => [
  { id: "key", label: t("views.record.property"), width: "fill" },
  { id: "value", label: t("views.record.value"), width: "lg" },
  { id: "explain", label: t("views.record.explanation"), width: "sm", actions: true },
]);

/** The line of the motive closed: the focus back to its cell. */
async function afterAsk(row: string): Promise<void> {
  await nextTick();
  document
    .querySelector<HTMLElement>(
      `[data-k-properties] [data-row="${CSS.escape(row)}"][data-column="value"]`,
    )
    ?.focus();
}
async function onConfirm(motive: string, value: string): Promise<void> {
  const ask = edit.asking.value;
  if (ask && (await edit.confirm(motive, value))) void afterAsk(ask.row);
}
function onCancelAsk(): void {
  const ask = edit.asking.value;
  edit.cancel();
  if (ask) void afterAsk(ask.row);
}
</script>

<template>
  <ProblemState
    v-if="resolved.isError.value"
    :title="t('views.record.propertiesError')"
    :error="resolved.error.value"
    :retry="resolved.refetch"
  />
  <EmptyState
    v-else-if="resolved.isSuccess.value && rows.length === 0"
    :title="t('views.record.noProperties')"
  >
    {{ t("views.record.noPropertiesText") }}
  </EmptyState>
  <template v-else>
    <DataGrid
      v-model:active="active"
      v-model:editing="editing"
      data-k-properties
      :label="t('views.record.propertiesOf', { name: props.name })"
      :columns="columns"
      :rows="rows"
      row-key="key"
      :loading="resolved.isPending.value"
      :loading-rows="4"
      :editable="edit.editable"
      :readonly-reason="readonlyReason"
      :validate="edit.validate"
      :cell-state="edit.cellState"
      :state-reason="edit.stateReason"
      :row-state="edit.rowState"
      :row-reason="edit.rowReason"
      @commit="edit.onCommit"
    >
      <template #cell="{ row, column }">
        <KeyLabel v-if="column.id === 'key'" :value="row.key" :label="labelOf(row.key)" show-key />
        <template v-else-if="column.id === 'value'">
          <span v-if="row.masked">{{ t("views.record.masked") }}</span>
          <PropertyValue
            v-else
            :property="row.key"
            :value="row.read"
            :type="declarationOf(row.key)?.type"
            :target="targetOf(row.key)"
          />
          <span v-if="noteHere(row.key)" class="k-visually-hidden">{{ noteHere(row.key) }}</span>
        </template>
        <Popover
          v-else-if="!row.masked"
          :title="t('views.record.explanationOf', { name: labelOf(row.key) ?? row.key })"
        >
          <template #trigger>
            <IconButton
              icon="info"
              :label="t('views.record.explain', { name: labelOf(row.key) ?? row.key })"
              size="sm"
              variant="ghost"
            />
          </template>
          <ExplanationBody :id="props.id" :kind="props.collection.kind" :property="row.key" />
        </Popover>
      </template>
    </DataGrid>
    <QuickApply
      v-if="edit.asking.value"
      :key="edit.asking.value.row"
      :ask="edit.asking.value"
      :words="(key, value) => keyLabel('value', `${key}/${value}`)"
      @confirm="onConfirm"
      @cancel="onCancelAsk"
    />
    <EditHint
      v-else-if="activeKey"
      :why="activeWhy"
      :apply-lacks="applyLacks"
      :held="edit.heldOf(activeKey)"
      :note="activeWhy === null ? edit.noteOf(activeKey) : undefined"
    />
    <span class="k-visually-hidden" role="status">{{ said }}</span>
  </template>
</template>
