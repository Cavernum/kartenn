<script setup lang="ts">
// « Mes brouillons » (REQ-021-11; spec 024 § A; design 021 « Écrans » : Brouillons; maquette
// E5, vue « liste »): the drafts of the user kept by the server (`GET /v1/drafts`, at most
// 50, the most recently modified first), each with its operations, its last modification
// and its deletion announced (`expires_at`); the active one of the tab marked. Resume one
// (the active draft of the tab), create another, delete one after a confirmation. The list
// follows the event `draft`.
import { drafts, type DraftList } from "@kartenn/sdk";
import {
  AlertDialog,
  Button,
  DataGrid,
  EmptyState,
  Menu,
  NoValue,
  StatusBadge,
  injectToasts,
  type CellRef,
  type GridColumn,
  type GridSort,
  type MenuItem,
  type RowState,
} from "@kartenn/ui";
import { computed, nextTick, onScopeDispose, ref, shallowRef, useTemplateRef } from "vue";
import { useI18n } from "vue-i18n";
import { useRouter } from "vue-router";
import { useCopyId } from "../changesets/copy.ts";
import { useChangesetWords } from "../changesets/words.ts";
import { useQuery } from "../data/query.ts";
import KLink from "../prefetch/KLink.vue";
import ProblemState from "../problems/ProblemState.vue";
import { useTime } from "../time/index.ts";
import RefusalAlert from "../views/edit/RefusalAlert.vue";
import type { Refusal } from "../views/edit/inline.ts";
import ViewPanel from "../views/parts/ViewPanel.vue";

type Row = DraftList["items"][number];

const { t, locale } = useI18n();
const router = useRouter();
const time = useTime();
const words = useChangesetWords();
const toasts = injectToasts();
const copyId = useCopyId();

const list = useQuery(() => drafts.list());
/** At most 50, all read: sorted here, the most recently modified first by default. */
const sort = ref<GridSort | undefined>({ column: "updated_at", direction: "descending" });
const rows = computed<Row[]>(() => {
  const items = [...(list.data.value?.data.items ?? [])];
  const by = sort.value;
  if (!by) return items;
  const key = by.column as keyof Row;
  const sign = by.direction === "ascending" ? 1 : -1;
  return items.sort((a, b) => {
    const [x, y] = [a[key] ?? "", b[key] ?? ""];
    return (x < y ? -1 : x > y ? 1 : 0) * sign;
  });
});
const active = ref<string | null>(drafts.active());
onScopeDispose(drafts.onActive((id) => (active.value = id)));

const title = computed(() => t("nav.drafts"));
const columns = computed<GridColumn[]>(() => [
  { id: "title", label: t("drafts.field.title"), width: "fill", wrap: true, sortable: true },
  {
    id: "operation_count",
    label: t("drafts.field.operations"),
    width: "md",
    align: "end",
    priority: 2,
    sortable: true,
  },
  {
    id: "updated_at",
    label: t("drafts.field.updated"),
    width: "md",
    mono: true,
    priority: 3,
    sortable: true,
  },
  {
    id: "expires_at",
    label: t("drafts.field.expires"),
    width: "md",
    priority: 3,
    sortable: true,
  },
  { id: "actions", label: t("views.actions"), width: "sm", actions: true },
]);
const rowState = (row: Row): RowState | undefined =>
  row.id === active.value ? "draft" : undefined;
const pageOf = (row: Row) => `/drafts/${encodeURIComponent(row.id)}`;
function onActivate(cell: CellRef): void {
  const row = rows.value.find((r) => r.id === cell.row);
  if (row) void router.push(pageOf(row));
}

const writable = computed(() => time.value.present);
const refusal = shallowRef<Refusal>();
const menu = (row: Row): MenuItem[] => [
  {
    id: "resume",
    label: t("drafts.resume"),
    icon: "undo-2",
    ...(row.id === active.value ? { reason: t("drafts.why.active") } : {}),
  },
  { id: "copy", label: t("changesets.copyId"), icon: "copy" },
  { kind: "separator" },
  {
    id: "delete",
    label: t("drafts.delete", { title: row.title }),
    icon: "trash",
    danger: true,
    ...(writable.value ? {} : { reason: t("drafts.why.past") }),
  },
];
const deleting = shallowRef<Row | null>(null);
const confirmOpen = ref(false);
async function onMenu(row: Row, id: string): Promise<void> {
  if (id === "resume" && row.id !== active.value) {
    drafts.setActive(row.id);
    toasts?.show({ tone: "ok", title: t("drafts.resumed", { title: row.title }) });
  } else if (id === "copy") await copyId(row.id);
  else if (id === "delete" && writable.value) {
    deleting.value = row;
    confirmOpen.value = true;
  }
}
async function remove(): Promise<void> {
  const row = deleting.value;
  if (!row) return;
  refusal.value = undefined;
  const outcome = await drafts.remove(row.id);
  if (outcome.kind !== "removed") {
    refusal.value = outcome;
    return;
  }
  toasts?.show({ tone: "ok", title: t("drafts.deleted", { title: row.title }) });
  // Its row is gone, and the menu that opened the dialog with it: the focus goes to the
  // grid, else to « Nouveau brouillon » (2.4.3).
  await list.refetch();
  await nextTick();
  const grid = view.value?.$el.querySelector<HTMLElement>('[role="grid"]');
  (grid?.querySelector<HTMLElement>('[tabindex="0"]') ?? newButton())?.focus();
}
const view = useTemplateRef<{ $el: HTMLElement }>("view");
const newButton = () => view.value?.$el.querySelector<HTMLElement>("[data-k-new-draft]") ?? null;

const stamp = computed(
  () => new Intl.DateTimeFormat(locale.value, { dateStyle: "short", timeStyle: "short" }),
);
const creating = ref(false);
/** A new draft, empty, made the active one of the tab; its page opens. */
async function create(): Promise<void> {
  if (creating.value || !writable.value) return;
  creating.value = true;
  refusal.value = undefined;
  const outcome = await drafts.create({
    title: t("drafts.newTitle", { when: stamp.value.format(new Date()) }),
  });
  creating.value = false;
  if (outcome.kind !== "saved") {
    refusal.value = outcome;
    return;
  }
  drafts.setActive(outcome.draft.id);
  void router.push(`/drafts/${encodeURIComponent(outcome.draft.id)}`);
}
</script>

<template>
  <h1 class="k-visually-hidden">{{ title }}</h1>
  <ViewPanel
    ref="view"
    :title="title"
    :detail="list.isSuccess.value ? t('drafts.count', { n: rows.length }) : undefined"
    main
    flush
  >
    <template #actions>
      <Button
        data-k-new-draft
        variant="primary"
        icon="plus"
        :loading="creating"
        v-bind="writable ? {} : { reason: t('drafts.why.past') }"
        @click="create"
        >{{ t("drafts.new") }}</Button
      >
    </template>
    <RefusalAlert v-if="refusal" :refusal="refusal" />
    <ProblemState
      v-if="list.isError.value"
      :title="t('drafts.listError')"
      :error="list.error.value"
      :retry="list.refetch"
    >
      {{ t("views.errorText") }}
    </ProblemState>
    <EmptyState v-else-if="list.isSuccess.value && rows.length === 0" :title="t('drafts.noneYet')">
      {{ t("drafts.noneYetText") }}
      <template v-if="writable" #action>
        <Button variant="ghost" icon="plus" :loading="creating" @click="create">{{
          t("drafts.new")
        }}</Button>
      </template>
    </EmptyState>
    <DataGrid
      v-else
      v-model:sort="sort"
      :label="title"
      :columns="columns"
      :rows="rows"
      row-key="id"
      stack
      :row-state="rowState"
      :loading="list.isPending.value"
      :loading-rows="4"
      @activate="onActivate"
    >
      <template #cell="{ row, column }">
        <template v-if="column.id === 'title'">
          <KLink :to="pageOf(row)">{{ row.title }}</KLink>
          <span v-if="row.id === active" class="k-drafts__muted">
            {{ t("drafts.activeMark") }}</span
          >
        </template>
        <span v-else-if="column.id === 'operation_count'" class="k-num">{{
          row.operation_count
        }}</span>
        <time v-else-if="column.id === 'updated_at'" :datetime="row.updated_at">{{
          words.when(row.updated_at)
        }}</time>
        <template v-else-if="column.id === 'expires_at'">
          <StatusBadge
            v-if="row.expires_at"
            state="warn"
            :label="t('drafts.expiresOn', { when: words.when(row.expires_at) })"
          />
          <NoValue v-else />
        </template>
        <Menu
          v-else-if="column.id === 'actions'"
          :label="t('drafts.actionsOf', { title: row.title })"
          icon="ellipsis"
          icon-only
          variant="ghost"
          size="sm"
          align="end"
          :items="menu(row)"
          @select="onMenu(row, $event)"
        />
      </template>
    </DataGrid>
    <template #footer>
      <span class="k-drafts__muted">{{ t("drafts.retention") }}</span>
    </template>
  </ViewPanel>
  <AlertDialog
    v-model:open="confirmOpen"
    :title="t('drafts.deleteTitle', { title: deleting?.title ?? '' })"
    :description="t('drafts.deleteText')"
    :confirm="t('drafts.deleteConfirm')"
    danger
    @confirm="remove"
  />
</template>

<style>
@layer components {
  .k-drafts__muted {
    color: var(--k-text-muted);
  }
}
</style>
