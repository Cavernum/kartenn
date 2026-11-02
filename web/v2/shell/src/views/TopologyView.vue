<script setup lang="ts">
// The topography (REQ-021-9; design 021 « Écrans » : Topographie; maquette E1): the tree of
// the prefixes of a domain in a `TreeGrid` (parents deduced, no move), its first level and
// the children of each node read page by page through the cache at the dates of the page,
// the children of a closed node read on intent (REQ-021-22, `childrenIntent`), each prefix
// with its occupancy as its page serves it (021/A11, no read for a row); beside it,
// the content of the chosen node (`?node=<id>`), its record one link away. The domain is
// chosen in the URL (`?domain=<id>`), the first one by default. The « + » of a prefix, `c`
// and `e` open the edit panel of the kind (task 12). No move (task 24, REQ-021-17): a
// prefix changes place by its value, through the draft, and the tree is read again.
// Edited in place (task 12a, REQ-021-15, 16): the nature of a prefix, the usage of a range,
// `expected` and `If-Match` the `version` its page served with it; Ctrl+Enter asks for its
// motive at the foot of the tree, which says too why a cell has no editor. Enter on a node
// opens its record, Tab goes into its « + », `c` adds in the node of the focus (021/A20);
// the « Ajouter » of the chosen node stays visible when refused, its permission said
// (REQ-021-37) — the `TreeGrid` has no refused « + » of its own.
import {
  drafts,
  queryClient,
  reads,
  type components,
  type Draft,
  type Entry,
  type TimeArg,
} from "@kartenn/sdk";
import {
  EmptyState,
  InlineAlert,
  Select,
  Split,
  Toolbar,
  TreeGrid,
  type CellRef,
  type GridColumn,
  type GridCommit,
  type ToolbarAction,
  type IconName,
  type ListOption,
  type TreeNode,
  type TreeNodeState,
} from "@kartenn/ui";
import {
  InfiniteQueryObserver,
  type InfiniteData,
  type InfiniteQueryObserverResult,
} from "@tanstack/query-core";
import {
  computed,
  nextTick,
  onBeforeUnmount,
  onMounted,
  ref,
  shallowReactive,
  useTemplateRef,
  watch,
  watchEffect,
} from "vue";
import { useI18n } from "vue-i18n";
import { useRoute, useRouter } from "vue-router";
import { useInfiniteQuery, useQuery } from "../data/query.ts";
import KLink from "../prefetch/KLink.vue";
import { useProblems } from "../problems/index.ts";
import { childrenIntent } from "../prefetch/tree.ts";
import { useSession } from "../rights/index.ts";
import { useViewAction } from "../shortcuts/view.ts";
import { useTime } from "../time/index.ts";
import { collectionById, pageOfObject, type CollectionId, type Item } from "./collections.ts";
import { applyRight, draftedValues, editRight } from "./edit/fields.ts";
import EditHint from "./edit/EditHint.vue";
import { whyWords } from "./edit/inline.ts";
import { NODE_COLLECTIONS, useNodeEdit, useNodeWords } from "./edit/nodes.ts";
import { openEdit } from "./edit/panel.ts";
import QuickApply from "./edit/QuickApply.vue";
import { useSay } from "./edit/say.ts";
import ProblemState from "../problems/ProblemState.vue";
import { rowsOf, useKeyLabel } from "./data.ts";
import ViewPanel from "./parts/ViewPanel.vue";
import { shareOf } from "./occupancy.ts";
import { nodesOf } from "./topology.ts";
import ContentTab from "./record/ContentTab.vue";

type Node = components["schemas"]["TreeNode"];
/**
 * A node as the view names it: a listed one, or a prefix read alone (no version nor
 * occupancy; not in the tree until its level is read, its occupancy is shown nowhere and
 * never read for it).
 */
type Shown = Node extends infer N
  ? N extends unknown
    ? Omit<N, "version" | "occupancy">
    : never
  : never;
type Pages = InfiniteData<{ data: { items: Node[] } }, string | null>;
type Result = InfiniteQueryObserverResult<Pages>;

const { t, te } = useI18n();
const route = useRoute();
const router = useRouter();
const time = useTime();
const keyLabel = useKeyLabel();
const dated = (): TimeArg => ({ validAt: time.value.validAt, asOf: time.value.asOf });

// The domains, for the selector: their first page (a few, in practice); the domain of the
// URL is looked for in the pages after it (ten at most) before the tree falls back to the
// first one, which it then says.
const DOMAIN_PAGES = 10;
const domains = useInfiniteQuery(() => reads.list("/v1/domains", null, dated()));
const domainItems = computed(
  () => rowsOf(domains.data.value) as unknown as { id: string; slug: string }[],
);
const domainOptions = computed<ListOption[]>(() =>
  domainItems.value.map((d) => ({ value: d.id, label: d.slug, mono: true })),
);
const asked = computed(() =>
  typeof route.query.domain === "string" ? route.query.domain : undefined,
);
const found = computed(
  () => asked.value !== undefined && domainItems.value.some((d) => d.id === asked.value),
);
const searching = computed(
  () =>
    asked.value !== undefined &&
    !found.value &&
    domains.hasNextPage.value &&
    !domains.isFetchNextPageError.value &&
    (domains.data.value?.pages.length ?? 0) < DOMAIN_PAGES,
);
watchEffect(() => {
  if (searching.value && !domains.isFetching.value) void domains.fetchNextPage();
});
const domain = computed(() =>
  found.value ? asked.value : searching.value ? undefined : domainItems.value[0]?.id,
);
/** The domain of the URL was not found: the tree shows the first one, and says so. */
const fellBack = computed(
  () => asked.value !== undefined && !found.value && !searching.value && domains.isSuccess.value,
);
const domainSlug = computed(() => domainItems.value.find((d) => d.id === domain.value)?.slug ?? "");
function chooseDomain(id: string | undefined): void {
  if (!id || id === domain.value) return;
  void router.replace({ query: { ...route.query, domain: id, node: undefined } });
}

// The pages of the tree: one observer of the cache per level read (the first level, then
// each node opened), its result kept here; all of them start again with the domain or the
// dates.
const results = shallowReactive(new Map<string | null, Result>());
const observers = new Map<
  string | null,
  { observer: InfiniteQueryObserver<Pages>; stop: () => void }
>();
const optionsOf = (parent: string | null) =>
  parent === null
    ? reads.subPages<Node>("domain", domain.value!, "roots", null, dated())
    : reads.subPages<Node>("prefix", parent, "children", null, dated());
function observe(parent: string | null): void {
  if (observers.has(parent)) return;
  const options = queryClient.defaultQueryOptions(optionsOf(parent) as never);
  const observer = new InfiniteQueryObserver<Pages>(queryClient, options as never);
  results.set(parent, observer.getOptimisticResult(options as never) as Result);
  const stop = observer.subscribe((result) => results.set(parent, result as Result));
  observers.set(parent, { observer, stop });
}
function reset(): void {
  for (const { stop } of observers.values()) stop();
  observers.clear();
  results.clear();
  if (domain.value) observe(null);
}
watch([domain, () => time.value.validAt, () => time.value.asOf], reset, { immediate: true });
onBeforeUnmount(() => {
  for (const { stop } of observers.values()) stop();
});
/** The tree asks for the children of a node, or the next page of a level. */
function onLoad(parent: string | null): void {
  const known = observers.get(parent);
  if (!known) observe(parent);
  else if (results.get(parent)?.hasNextPage) void known.observer.fetchNextPage();
}

const ICONS: Record<Node["kind"], IconName> = {
  prefix: "prefix",
  range: "range",
  address: "address",
};
const valueOf = (node: Shown) =>
  node.kind === "range" ? `${node.first} – ${node.last}` : node.value;
const detailOf = (node: Shown) => {
  const value = node.kind === "prefix" ? node.nature : node.kind === "range" ? node.usage : null;
  if (!value) return "";
  const key = `views.value.${node.kind === "prefix" ? "nature" : "usage"}.${value}`;
  return te(key) ? t(key) : value;
};
const itemsOf = (parent: string | null) =>
  rowsOf(results.get(parent)?.data as never) as unknown as Node[];
/** A level read, as the tree takes it: `undefined` until its first page is read. */
const levelOf = (parent: string | null) => {
  const result = results.get(parent);
  return result?.data ? { items: itemsOf(parent), more: result.hasNextPage } : undefined;
};
// In-place editing (task 12a): the nodes of every level read, each at the version its page
// served; after a 412, the level that holds the node is read again.
const items = computed(() => [...results.keys()].flatMap(itemsOf));
const itemById = computed(() => new Map(items.value.map((item) => [item.id, item])));
const said = ref("");
// Emptied first: the same sentence twice is said twice.
const say = useSay((text) => {
  said.value = "";
  void nextTick(() => (said.value = text));
});
const words = useNodeWords();
const edit = useNodeEdit({
  rows: () => items.value as unknown as Item[],
  words,
  reread: (id) => {
    for (const [parent, result] of results) {
      if (itemsOf(parent).some((item) => item.id === id)) void result.refetch();
    }
  },
  t: (key, values) => t(key, values ?? {}),
  sentence: useProblems().sentence,
  say,
});
/** The nodes as the cells show them: with the values of the draft and of this session. */
const shownById = computed(
  () => new Map(edit.shownRows.value.map((row) => [String(row.id), row as unknown as Node])),
);
const rowOf = (id: string) => itemById.value.get(id) as Item | undefined;
const editable = (node: TreeNode, column: GridColumn) => {
  const row = rowOf(node.id);
  return row !== undefined && edit.editable(row, column);
};
const validate = (node: TreeNode, column: GridColumn, value: string) => {
  const row = rowOf(node.id);
  return row ? edit.validate(row, column, value) : undefined;
};
const cellState = (node: TreeNode, column: GridColumn) => {
  const row = rowOf(node.id);
  return row ? edit.cellState(row, column) : undefined;
};
const onCommit = (commit: GridCommit) => edit.onCommit(commit);

// The occupancy of a prefix comes with its page (021/A11): no read for a row. The node is
// named by its value read (the tree places it by the present); its detail is the one shown.
const nodeOf = (item: Node): TreeNode => ({
  id: item.id,
  label: valueOf(item),
  icon: ICONS[item.kind],
  cells: {
    occupancy: (item.kind === "prefix" ? shareOf(item.occupancy) : undefined) ?? "",
    detail: detailOf(shownById.value.get(item.id) ?? item),
  },
  detail: keyLabel("kind", `core.${item.kind}`) ?? item.kind,
});
// Each node placed once, even while its levels are read again (`topology.ts`).
const nodes = computed(() => nodesOf(levelOf, nodeOf, (item) => item.kind === "prefix"));
const roots = computed(() => results.get(null));

// Parents deduced (D34, REQ-021-17): no move is offered; a node whose value the active
// draft changes stays where the present has it, marked « en brouillon » — the tree is never
// retouched by the client, only read again once a change is applied (X24).
const active = ref<string | null>(drafts.active());
const stopActive = drafts.onActive((id) => (active.value = id));
onBeforeUnmount(stopActive);
const activeDraft = useQuery(() => ({
  ...drafts.get(active.value ?? "-"),
  enabled: active.value !== null,
}));
const inDraft = computed(() => {
  const operations = (activeDraft.data.value as Entry<Draft> | undefined)?.data.operations;
  return new Set(
    (["prefixes", "ranges", "addresses"] as const).flatMap((c) => [
      ...draftedValues(operations, c).keys(),
    ]),
  );
});
const nodeState = (node: TreeNode): TreeNodeState | undefined =>
  results.get(node.id)?.isFetching
    ? "loading-children"
    : time.value.present && inDraft.value.has(node.id)
      ? "draft"
      : undefined;
// The occupancy first, as the mock-up E1; the nature (not in the mock-up, kept from task
// 11) beside it, the first to hide.
const columns = computed<GridColumn[]>(() => [
  { id: "occupancy", label: t("views.record.occupancy"), width: "sm", align: "end" },
  { id: "detail", label: t("views.record.nature"), width: "sm", priority: 2 },
]);

/** The open nodes: closed again with the domain or the dates (their pages start again). */
const expanded = ref<string[]>([]);
watch([domain, () => time.value.validAt, () => time.value.asOf], () => (expanded.value = []));

const selected = computed({
  get: () => (typeof route.query.node === "string" ? route.query.node : undefined),
  set: (id: string | undefined) => void router.replace({ query: { ...route.query, node: id } }),
});
/** A node read, by identifier, in any level read so far. */
function nodeById(id: string | undefined): Node | undefined {
  if (id === undefined) return undefined;
  for (const parent of results.keys()) {
    const found = itemsOf(parent).find((item) => item.id === id);
    if (found) return found;
  }
  return undefined;
}
/**
 * A node of the URL not read yet (a link, a reload): read as a prefix — the tree's nodes
 * that hold content are prefixes — until its level is read.
 */
const unread = computed(() =>
  selected.value !== undefined && nodeById(selected.value) === undefined
    ? selected.value
    : undefined,
);
const fallback = useQuery(() => ({
  ...reads.object<{
    id: string;
    value: string;
    nature: "container" | "network";
    links: { self: string };
  }>("prefix", unread.value ?? "", dated()),
  enabled: unread.value !== undefined,
}));
const chosen = computed<Shown | undefined>(() => {
  const read = nodeById(selected.value);
  if (read) return read;
  const prefix = fallback.data.value?.data;
  return prefix && prefix.id === selected.value
    ? {
        kind: "prefix",
        id: prefix.id,
        value: prefix.value,
        nature: prefix.nature,
        href: prefix.links.self,
      }
    : undefined;
});
function onActivate(id: string): void {
  const node = nodeById(id);
  const to = node ? pageOfObject(node.kind, node.id) : null;
  if (to) void router.push(to);
}

// Direct addition (024/M10, REQ-021-36): the « + » of a prefix, and `c`, open the edit
// panel of the kinds it may hold — a network its addresses and ranges, a container its
// prefixes — the domain given; `e` modifies the chosen node. At the present only; the
// panel refuses, visibly, what the rights do not allow. `c` adds in the node of the focus
// in the tree, else in the chosen one (021/A20).
const session = useSession();
/** The kinds a node may hold: a network its addresses and ranges, else prefixes. */
const kindsIn = (node: Shown | undefined): CollectionId[] =>
  node?.kind === "prefix" && node.nature === "network" ? ["addresses", "ranges"] : ["prefixes"];
/**
 * Why adding in `node` (the first level without one) is refused: off the present, or
 * without the right to write any kind it may hold — the first one named, as served.
 */
function addRefusal(node: Shown | undefined): { forbidden: string } | { reason: string } | null {
  if (!time.value.present) return { reason: t("edit.why.past") };
  const types = kindsIn(node).map((id) => collectionById(id)!.type);
  if (types.some((type) => session.value.rights.has(editRight(type)))) return null;
  return { forbidden: editRight(types[0]!) };
}
// The « + » of a row only for whom may add: the `TreeGrid` has no refused « + » (the one
// of the content, beside, stays visible and says the permission).
const addLabel = (node: TreeNode) => {
  const item = nodeById(node.id);
  return item?.kind === "prefix" && addRefusal(item) === null
    ? t("edit.addIn", { within: node.label })
    : undefined;
};
function addIn(id: string | undefined): void {
  const node = nodeById(id) ?? chosen.value;
  const inside = node?.kind === "prefix" ? node : undefined;
  const kinds = kindsIn(inside);
  openEdit({
    mode: "create",
    collection: kinds[0]!,
    ...(inside ? { kinds, within: valueOf(inside) } : {}),
    ...(domain.value ? { preset: { domain: domain.value } } : {}),
  });
}
/** The node of the focus in the tree, if any: where `c` adds. */
const focused = ref<string | undefined>();
const target = computed<Shown | undefined>(() => nodeById(focused.value) ?? chosen.value);
const targetPrefix = computed(() => (target.value?.kind === "prefix" ? target.value : undefined));
useViewAction("create", {
  label: () =>
    targetPrefix.value
      ? t("edit.addIn", { within: valueOf(targetPrefix.value) })
      : t("edit.createAction", { kind: t("labels.kind.core.prefix") }),
  allowed: () => addRefusal(targetPrefix.value) === null && domain.value !== undefined,
  run: () => addIn(targetPrefix.value?.id),
});
/** « Ajouter dans » of the chosen prefix: visible, refused with its permission, `c`. */
const contentActions = computed<ToolbarAction[]>(() => {
  const node = chosen.value;
  if (node?.kind !== "prefix") return [];
  return [
    {
      id: "add",
      label: t("edit.addIn", { within: valueOf(node) }),
      icon: "plus",
      shortcut: ["c"],
      ...(addRefusal(node) ?? {}),
    },
  ];
});
useViewAction("edit", {
  label: () =>
    t("edit.editThis", { kind: detailKind.value, name: chosen.value ? valueOf(chosen.value) : "" }),
  allowed: () =>
    chosen.value !== undefined && session.value.rights.has(editRight(`core.${chosen.value.kind}`)),
  run: () => {
    if (chosen.value) {
      openEdit({
        mode: "edit",
        collection: NODE_COLLECTIONS[chosen.value.kind],
        id: chosen.value.id,
      });
    }
  },
});

// The keys of a node cell (021/A20), before the tree's own: Enter opens the record of the
// node (the tree would go into its « + »), Tab goes into its « + », if any; from there Tab
// leaves the tree, Shift+Tab, Escape or F2 come back. The `TreeGrid` does not offer them yet.
function onTreeKeydown(event: KeyboardEvent): void {
  if (event.altKey || event.ctrlKey || event.metaKey || event.shiftKey) return;
  const cell = event.target as HTMLElement;
  if (cell.getAttribute("role") !== "gridcell" || cell.dataset.column !== "") return;
  const node = nodeById(cell.dataset.row);
  if (!node) return;
  if (event.key === "Enter") {
    event.preventDefault();
    event.stopPropagation();
    onActivate(node.id);
  } else if (event.key === "Tab") {
    const add = cell.querySelector<HTMLElement>("button");
    if (!add) return;
    event.preventDefault();
    event.stopPropagation();
    add.focus();
  }
}
/** The cell of the focus in the tree: where `c` adds, what the foot of the tree says. */
const focusCell = ref<CellRef | undefined>();
function onTreeFocusin(event: FocusEvent): void {
  const cell = (event.target as HTMLElement).closest<HTMLElement>("[data-row][data-column]");
  focusCell.value = cell ? { row: cell.dataset.row!, column: cell.dataset.column! } : undefined;
  focused.value = cell && nodeById(cell.dataset.row) ? cell.dataset.row : undefined;
}
// The cell is kept while the focus goes into the foot that speaks of it (its code to
// copy), and goes once the focus is elsewhere: the foot never says keys that do nothing
// there; `c` then adds in the chosen node.
const inTreeOrFoot = (to: EventTarget | null) =>
  to instanceof HTMLElement && (!!tree.value?.contains(to) || !!to.closest("[data-k-tree-foot]"));
function onTreeFocusout(event: FocusEvent): void {
  if (!tree.value?.contains(event.relatedTarget as HTMLElement | null)) focused.value = undefined;
  if (!inTreeOrFoot(event.relatedTarget)) focusCell.value = undefined;
}
function onFootFocusout(event: FocusEvent): void {
  if (!inTreeOrFoot(event.relatedTarget)) focusCell.value = undefined;
}
const focusRow = computed(() => (focusCell.value ? rowOf(focusCell.value.row) : undefined));
/** Why the cell of the focus has no editor (the tree has no description for it), or none. */
const focusWhy = computed(() =>
  focusCell.value && focusRow.value ? edit.why(focusCell.value.column, focusRow.value) : null,
);
const focusEditable = computed(() => focusRow.value !== undefined && focusWhy.value === null);
const focusSaysWhy = computed(() => focusWhy.value !== null && focusWhy.value.kind !== "none");
/** The permission Ctrl+Enter lacks in the cell of the focus, `null` when held. */
const focusApplyLacks = computed(() =>
  focusRow.value && !edit.mayApply(focusRow.value)
    ? applyRight(collectionById(NODE_COLLECTIONS[focusRow.value.kind as Node["kind"]])!.type)
    : null,
);
// The tree has no description for a cell without an editor (`readonlyReason`): its reason
// is said by the status of the view when the focus comes to it (4.1.3).
watch(
  () => whyWords(focusWhy.value, t),
  (words) => {
    if (!words) return;
    said.value = "";
    void nextTick(() => (said.value = words));
  },
);

/** The line of the motive closed: the focus back to its cell. */
async function afterAsk(cell: CellRef): Promise<void> {
  await nextTick();
  document
    .querySelector<HTMLElement>(
      `[data-k-tree] [data-row="${CSS.escape(cell.row)}"][data-column="${CSS.escape(cell.column)}"]`,
    )
    ?.focus();
}
async function onConfirm(motive: string, value: string): Promise<void> {
  const ask = edit.asking.value;
  if (ask && (await edit.confirm(motive, value))) void afterAsk(edit.cellOf(ask));
}
function onCancelAsk(): void {
  const ask = edit.asking.value;
  edit.cancel();
  if (ask) void afterAsk(edit.cellOf(ask));
}
const detailKind = computed(() =>
  chosen.value ? (keyLabel("kind", `core.${chosen.value.kind}`) ?? chosen.value.kind) : "",
);

// The children of a closed node, read on intent.
const tree = useTemplateRef<HTMLElement>("tree");
let stopIntent = () => {};
onMounted(() => {
  if (tree.value)
    stopIntent = childrenIntent(tree.value, (id) => [
      reads.subPages<Node>("prefix", id, "children", null, dated()),
    ]);
});
onBeforeUnmount(() => stopIntent());
</script>

<template>
  <h1 class="k-visually-hidden">{{ t("nav.topology") }}</h1>
  <InlineAlert v-if="fellBack" tone="warn" :title="t('views.topology.domainMissing')">
    {{ t("views.topology.domainMissingText", { name: domainSlug }) }}
  </InlineAlert>
  <ProblemState
    v-if="domains.isError.value"
    :title="t('views.topology.domainsError')"
    :error="domains.error.value"
    :retry="domains.refetch"
  />
  <EmptyState
    v-else-if="domains.isSuccess.value && domainItems.length === 0"
    icon="domain"
    :title="t('views.topology.noDomain')"
  >
    {{ t("views.topology.noDomainText") }}
  </EmptyState>
  <Split v-else side-size="lg" gap="3">
    <template #side>
      <ViewPanel :title="t('nav.prefixes')" :detail="domainSlug" mono flush>
        <template #actions>
          <Select
            :model-value="domain"
            :label="t('views.field.domain')"
            :options="domainOptions"
            inline-label
            mono
            size="sm"
            @update:model-value="chooseDomain"
          />
        </template>
        <div
          ref="tree"
          class="k-topology__tree"
          @keydown.capture="onTreeKeydown"
          @focusin="onTreeFocusin"
          @focusout="onTreeFocusout"
        >
          <ProblemState
            v-if="roots?.isError && !roots.isFetchNextPageError"
            :title="t('views.topology.treeError')"
            :error="roots.error"
            :retry="roots.refetch"
          />
          <EmptyState
            v-else-if="roots?.isSuccess && nodes.length === 0"
            :title="t('views.topology.empty')"
          />
          <TreeGrid
            v-else
            v-model:expanded="expanded"
            v-model:selected="selected"
            data-k-tree
            :label="t('views.topology.tree', { name: domainSlug })"
            :tree-label="t('views.topology.prefix')"
            :nodes="nodes"
            :columns="columns"
            :more="roots?.hasNextPage ?? false"
            :more-loading="roots?.isFetchingNextPage ?? false"
            :loading="!roots || roots.isPending"
            :loading-rows="8"
            :node-state="nodeState"
            :add-label="addLabel"
            :editable="editable"
            :validate="validate"
            :cell-state="cellState"
            parent-mode="derived"
            @load="onLoad"
            @activate="onActivate"
            @add="addIn"
            @commit="onCommit"
          />
        </div>
        <span class="k-visually-hidden" role="status">{{ said }}</span>
        <!-- Once a cell has had the focus: what Enter does there, or why it has no editor. -->
        <template v-if="edit.asking.value || focusEditable || focusSaysWhy" #footer>
          <QuickApply
            v-if="edit.asking.value"
            :key="edit.asking.value.row + edit.asking.value.column"
            :ask="edit.asking.value"
            :words="words"
            @confirm="onConfirm"
            @cancel="onCancelAsk"
          />
          <EditHint
            v-else
            data-k-tree-foot
            :why="focusWhy"
            :apply-lacks="focusApplyLacks"
            @focusout="onFootFocusout"
          />
        </template>
      </ViewPanel>
    </template>
    <ViewPanel
      v-if="chosen"
      :title="valueOf(chosen)"
      :detail="detailOf(chosen)"
      main
      :flush="chosen.kind === 'prefix'"
    >
      <template #actions>
        <Toolbar
          v-if="contentActions.length > 0"
          :label="t('views.topology.actions', { name: valueOf(chosen) })"
          :actions="contentActions"
          align="end"
          @action="(id: string) => id === 'add' && addIn(chosen!.id)"
        />
        <KLink
          v-if="pageOfObject(chosen.kind, chosen.id)"
          :to="pageOfObject(chosen.kind, chosen.id)!"
          >{{ t("views.topology.open") }}</KLink
        >
      </template>
      <ContentTab
        v-if="chosen.kind === 'prefix'"
        :id="chosen.id"
        kind="prefix"
        :name="valueOf(chosen)"
      />
      <p v-else>{{ t("views.topology.leaf") }}</p>
    </ViewPanel>
    <ViewPanel v-else :title="t('views.topology.content')" main>
      <EmptyState icon="folder-tree" :title="t('views.topology.choose')">
        {{ t("views.topology.chooseText") }}
      </EmptyState>
    </ViewPanel>
  </Split>
</template>

<style>
@layer components {
  .k-topology__tree {
    display: grid;
    min-inline-size: 0;
  }
}
</style>
