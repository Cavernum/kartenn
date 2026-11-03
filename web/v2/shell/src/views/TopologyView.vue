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
// opens its record, Tab goes into its « + » (the tree's keys), `c` adds in the node of the
// focus (021/A20); a refused « + », and the « Ajouter » of the chosen node, stay visible
// and say their permission (REQ-021-37). The preview of the active draft (task 24b,
// 021/A12): the places its simulation gives laid over the tree served (`topology.ts`).
import { drafts, queryClient, reads, type components, type TimeArg } from "@kartenn/sdk";
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
  shallowRef,
  useTemplateRef,
  watch,
  watchEffect,
} from "vue";
import { useI18n } from "vue-i18n";
import { useRoute, useRouter } from "vue-router";
import { useInfiniteQuery, useQuery } from "../data/query.ts";
import { useDraftQueries } from "../draft/state.ts";
import KLink from "../prefetch/KLink.vue";
import { useProblems } from "../problems/index.ts";
import { childrenIntent } from "../prefetch/tree.ts";
import { useSession } from "../rights/index.ts";
import { useViewAction } from "../shortcuts/view.ts";
import { useTime } from "../time/index.ts";
import { collectionById, pageOfObject, type CollectionId, type Item } from "./collections.ts";
import { draftedValues, editRight } from "./edit/fields.ts";
import EditHint from "./edit/EditHint.vue";
import { whyWords } from "./edit/inline.ts";
import { NODE_COLLECTIONS, nodeApplyRight, useNodeEdit, useNodeWords } from "./edit/nodes.ts";
import { openEdit } from "./edit/panel.ts";
import QuickApply from "./edit/QuickApply.vue";
import { useSay } from "./edit/say.ts";
import ProblemState from "../problems/ProblemState.vue";
import { rowsOf, useKeyLabel } from "./data.ts";
import ViewPanel from "./parts/ViewPanel.vue";
import { shareOf } from "./occupancy.ts";
import {
  cappedKey,
  cappedParent,
  draftStateOf,
  isCapped,
  isPreview,
  levelOfPages,
  nodesOf,
  overlay,
  PREVIEW,
  previewOf,
  type Preview,
  type Shift,
} from "./topology.ts";
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

const { t, te, locale } = useI18n();
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
  // A row of the preview has its children given (`topology.ts`): nothing to read.
  if (parent !== null && isPreview(parent)) return;
  const known = observers.get(parent);
  if (!known) observe(parent);
  // Within the cap only (021/A34): past it, the cache would drop the first page.
  else if (levelOf(parent)?.more) void known.observer.fetchNextPage();
}

const ICONS: Record<Node["kind"], IconName> = {
  prefix: "prefix",
  range: "range",
  address: "address",
};
const valueOf = (node: Shown) =>
  node.kind === "range" ? `${node.first} – ${node.last}` : node.value;
/** The nature of a prefix, the usage of a range, in words. */
const detailWords = (kind: Node["kind"], value: string | null | undefined) => {
  if (!value || kind === "address") return "";
  const key = `views.value.${kind === "prefix" ? "nature" : "usage"}.${value}`;
  return te(key) ? t(key) : value;
};
const detailOf = (node: Shown) =>
  detailWords(
    node.kind,
    node.kind === "prefix" ? node.nature : node.kind === "range" ? node.usage : null,
  );
const itemsOf = (parent: string | null) =>
  rowsOf(results.get(parent)?.data as never) as unknown as Node[];
/**
 * A level read, as the tree takes it: `undefined` until its first page is read; its pages
 * read up to the cap of the cache (`CHILDREN_PAGES`), a notice under it past the cap.
 */
const levelOf = (parent: string | null) =>
  levelOfPages(results.get(parent) as Parameters<typeof levelOfPages<Node>>[0]);
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
const roots = computed(() => results.get(null));

// Past the pages the cache keeps of a level (021/A34, 10⁴ children), the row under its
// last child says so and leads to the list « dans ce préfixe » — the kind a node holds
// (`kindsIn`), filtered by `within` —, or for the first level to the prefixes of the domain.
/** Where the rest of a level capped is listed: a list of the core, filtered. */
function restOf(parent: string | null): { name: CollectionId; query: Record<string, string> } {
  if (parent === null)
    return { name: "prefixes", query: domain.value ? { domain: domain.value } : {} };
  const node = nodeById(parent);
  return { name: kindsIn(node)[0]!, query: { within: node ? valueOf(node) : "" } };
}
function noticeOf(parent: string | null): TreeNode {
  const count = new Intl.NumberFormat(locale.value).format(levelOf(parent)?.items.length ?? 0);
  const rest = restOf(parent);
  return {
    id: cappedKey(parent),
    label:
      parent === null
        ? t("views.topology.capped.roots", { n: count, name: domainSlug.value })
        : t(`views.topology.capped.${rest.name}`, { n: count, within: rest.query.within }),
    icon: "info",
  };
}
/** The row of a notice chosen or activated: its list. */
function openRest(key: string): void {
  void router.push(restOf(cappedParent(key)));
}

// Parents deduced (D34, REQ-021-17): no move is offered; a node whose value the active
// draft changes stays where the present has it, marked « en brouillon » (« déplacé dans le
// brouillon » when the draft moves it, task 24d) — the tree is never
// retouched by the client, only read again once a change is applied (X24).
const active = ref<string | null>(drafts.active());
const stopActive = drafts.onActive((id) => (active.value = id));
onBeforeUnmount(stopActive);
const activeDraft = useDraftQueries(active);
const inDraft = computed(() => {
  const operations = activeDraft.operations.value;
  return new Set(
    (["prefixes", "ranges", "addresses"] as const).flatMap((c) => [
      ...draftedValues(operations, c).keys(),
    ]),
  );
});
// The preview of the draft (task 24b, 021/A12): from its valid simulation at the present,
// the one last answered kept while the next is to come (the operations it simulated with
// it), gone with the draft. A simulation with errors shows none: it cannot be applied.
const preview = shallowRef<Preview>();
watchEffect(() => {
  const operations = activeDraft.operations.value;
  if (!time.value.present || domain.value === undefined || operations.length === 0) {
    preview.value = undefined;
    return;
  }
  if (activeDraft.simulating.value) return;
  const simulated = activeDraft.simulated.value;
  preview.value = simulated?.valid
    ? previewOf(simulated, operations, (id) => itemById.value.has(id), domain.value)
    : undefined;
});
/** A row of the preview: the object at its place after, in its value after. */
function shiftOf(shift: Shift, children: TreeNode[] | undefined): TreeNode {
  const row = shift.object ? (shownById.value.get(shift.object) ?? nodeById(shift.object)) : null;
  const read = shift.object ? itemById.value.get(shift.object) : undefined;
  return {
    id: shift.key,
    label: shift.creation?.label ?? (row ? valueOf(row) : ""),
    icon: ICONS[shift.kind],
    ...(children ? { children } : {}),
    cells: {
      // An object moved keeps its occupancy; one created has none read yet.
      occupancy: (read?.kind === "prefix" ? shareOf(read.occupancy) : undefined) ?? "",
      detail: shift.creation
        ? detailWords(shift.kind, shift.creation.detail)
        : row
          ? detailOf(row)
          : "",
    },
    detail: keyLabel("kind", `core.${shift.kind}`) ?? shift.kind,
  };
}
// Each node placed once, even while its levels are read again (`topology.ts`); the preview
// over them.
const laid = computed(() => {
  const served = nodesOf(levelOf, nodeOf, (item) => item.kind === "prefix", noticeOf);
  return preview.value ? overlay(served, preview.value, shiftOf) : { nodes: served, shown: 0 };
});
const nodes = computed(() => laid.value.nodes);
/** The keys of the rows of the preview laid. */
const previewKeys = computed(() => {
  const keys = new Set<string>();
  const walk = (list: readonly TreeNode[]) =>
    list.forEach((n) => {
      if (isPreview(n.id)) keys.add(n.id);
      walk(n.children ?? []);
    });
  walk(nodes.value);
  return keys;
});
/** The preview says what it shows, what waits for its parent to be read, and if partial. */
const previewNote = computed(() => {
  const shown = preview.value;
  if (!shown || (shown.shifts.length === 0 && !shown.truncated)) return undefined;
  const waiting = shown.shifts.length - laid.value.shown;
  return {
    partial: shown.truncated,
    text: [
      t("views.topology.preview.shown", { n: laid.value.shown }, laid.value.shown),
      ...(waiting > 0 ? [t("views.topology.preview.waiting", { n: waiting }, waiting)] : []),
      ...(shown.truncated ? [t("views.topology.preview.partialText")] : []),
    ].join(" "),
  };
});
const nodeState = (node: TreeNode): TreeNodeState | undefined =>
  results.get(node.id)?.isFetching
    ? "loading-children"
    : time.value.present
      ? draftStateOf(node.id, inDraft.value, preview.value)
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

/** The node chosen, in the URL: an object of the tree served. */
const node = computed(() => (typeof route.query.node === "string" ? route.query.node : undefined));
/** The row of the preview chosen, while it stands for the node chosen. */
const previewChoice = ref<string>();
// The row chosen: one of the preview stays selected where it was chosen, its object in the
// URL (none for an object the draft creates: nothing to show, nothing chosen).
const selected = computed({
  get: () =>
    previewChoice.value !== undefined &&
    previewKeys.value.has(previewChoice.value) &&
    objectOf(previewChoice.value) === node.value
      ? previewChoice.value
      : node.value,
  set: (id: string | undefined) => {
    if (id !== undefined && isCapped(id)) return openRest(id);
    const object = id !== undefined && isPreview(id) ? objectOf(id) : id;
    if (object === null) return;
    previewChoice.value = id !== undefined && isPreview(id) ? id : undefined;
    void router.replace({ query: { ...route.query, node: object } });
  },
});
/**
 * The object of a row of the preview (a shift, or a child it takes along); `null` for one
 * the draft creates.
 */
function objectOf(key: string): string | null {
  const shift = preview.value?.shifts.find((s) => s.key === key);
  if (shift) return shift.object ?? null;
  const id = key.slice(PREVIEW.length);
  return itemById.value.has(id) ? id : null;
}
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
  node.value !== undefined && nodeById(node.value) === undefined ? node.value : undefined,
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
  const read = nodeById(node.value);
  if (read) return read;
  const prefix = fallback.data.value?.data;
  return prefix && prefix.id === node.value
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
  if (isCapped(id)) return openRest(id);
  // A row of the preview opens the record of its object, if it exists already.
  const node = nodeById(isPreview(id) ? (objectOf(id) ?? undefined) : id);
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
// The « + » of every prefix; refused, it stays visible and says why (REQ-021-37).
const addLabel = (node: TreeNode) =>
  nodeById(node.id)?.kind === "prefix" ? t("edit.addIn", { within: node.label }) : undefined;
const addBlocked = (node: TreeNode) => addRefusal(nodeById(node.id)) ?? undefined;
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
/** The cell of the focus in the tree (kept on leaving it, as a grid's): where `c` adds. */
const activeCell = ref<CellRef | undefined>();
const target = computed<Shown | undefined>(() => nodeById(activeCell.value?.row) ?? chosen.value);
const targetPrefix = computed(() => (target.value?.kind === "prefix" ? target.value : undefined));
useViewAction("create", {
  label: () =>
    targetPrefix.value
      ? t("edit.addIn", { within: valueOf(targetPrefix.value) })
      : t("edit.createAction", { kind: t("labels.kind.core.prefix") }),
  allowed: () => addRefusal(targetPrefix.value) === null && domain.value !== undefined,
  run: () => addIn(targetPrefix.value?.id),
});
/**
 * « Ajouter dans » of the chosen prefix: visible, refused with its permission; `c` shown
 * on it only when `c` adds there (the node of the focus is the chosen one, or none).
 */
const contentActions = computed<ToolbarAction[]>(() => {
  const node = chosen.value;
  if (node?.kind !== "prefix") return [];
  return [
    {
      id: "add",
      label: t("edit.addIn", { within: valueOf(node) }),
      icon: "plus",
      ...(targetPrefix.value?.id === node.id ? { shortcut: ["c"] } : {}),
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

// The foot of the tree speaks of the cell of the focus (the tree's `active`, kept when the
// focus leaves it, as at the foot of a list): Enter opens the record of a node and Tab its
// « + » by the tree itself (021/A20).
const focusRow = computed(() => (activeCell.value ? rowOf(activeCell.value.row) : undefined));
/** Why the cell of the focus has no editor (the tree has no description for it), or none. */
const focusWhy = computed(() =>
  activeCell.value && focusRow.value ? edit.why(activeCell.value.column, focusRow.value) : null,
);
const focusEditable = computed(() => focusRow.value !== undefined && focusWhy.value === null);
const focusSaysWhy = computed(() => focusWhy.value !== null && focusWhy.value.kind !== "none");
/** The permission Ctrl+Enter lacks in the cell of the focus, `null` when held. */
const focusApplyLacks = computed(() =>
  focusRow.value && !edit.mayApply(focusRow.value) ? nodeApplyRight(focusRow.value) : null,
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
    stopIntent = childrenIntent(tree.value, (id) =>
      // A row of the preview has its children given: nothing to read.
      isPreview(id) ? [] : [reads.subPages<Node>("prefix", id, "children", null, dated())],
    );
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
        <!-- The preview of the draft: what it shows, what waits, partial (never silent);
             in the band above the tree, which has its margin (the body is flush). -->
        <template v-if="previewNote" #toolbar>
          <InlineAlert
            :tone="previewNote.partial ? 'warn' : 'info'"
            icon="draft"
            :title="
              t(
                previewNote.partial
                  ? 'views.topology.preview.partial'
                  : 'views.topology.preview.title',
              )
            "
          >
            {{ previewNote.text }}
          </InlineAlert>
        </template>
        <div ref="tree" class="k-topology__tree">
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
            v-model:active="activeCell"
            data-k-tree
            :label="t('views.topology.tree', { name: domainSlug })"
            :tree-label="t('views.topology.prefix')"
            :nodes="nodes"
            :columns="columns"
            :more="levelOf(null)?.more ?? false"
            :more-loading="roots?.isFetchingNextPage ?? false"
            :loading="!roots || roots.isPending"
            :loading-rows="8"
            :node-state="nodeState"
            :add-label="addLabel"
            :add-blocked="addBlocked"
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
          <EditHint v-else :why="focusWhy" :apply-lacks="focusApplyLacks" />
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
