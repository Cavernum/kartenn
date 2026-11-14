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
// motive at the foot of the tree, which says too why a cell has no editor or is held in
// review, as the cell does (its description, task 12f2); one editing for the tree and the
// content of the node chosen (`ContentTab`, its line under its own grid). Enter on a node
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
  Stack,
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
import { useTime, writesOf } from "../time/index.ts";
import { collectionById, pageOfObject, type Item } from "./collections.ts";
import { editRight } from "./edit/fields.ts";
import EditHint from "./edit/EditHint.vue";
import { whyWords } from "./edit/inline.ts";
import {
  NODE_COLLECTIONS,
  nodeApplyRight,
  TREE_GRID,
  useNodeEdit,
  useNodeWords,
} from "./edit/nodes.ts";
import { openEdit } from "./edit/panel.ts";
import QuickApply from "./edit/QuickApply.vue";
import { useSay, useStatus } from "./edit/say.ts";
import ExtensionElement from "../plugins/ExtensionElement.vue";
import { cellExtension, gridColumns, stableObjectRefs, useExtensions } from "../plugins/slots.ts";
import NextPageAlert from "../problems/NextPageAlert.vue";
import ProblemState from "../problems/ProblemState.vue";
import { rowsOf, useKeyLabel } from "./data.ts";
import ViewPanel from "./parts/ViewPanel.vue";
import { shareOf } from "./occupancy.ts";
import {
  askNextPage,
  cappedKey,
  cappedParent,
  kindsIn,
  restOf,
  draftStateOf,
  inDraftOf,
  isCapped,
  isPreview,
  levelOfPages,
  nodesOf,
  overlay,
  PREVIEW,
  previewOf,
  type LevelReader,
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
/**
 * The tree asks for the children of a node, or the next page of a level: read from the
 * observer itself (its result here comes a moment later), within the cap only (021/A34:
 * past it, the cache would drop the first page), never twice under way, and asked again
 * after a read of the level an invalidation made meanwhile (`askNextPage`, task 22d).
 */
function onLoad(parent: string | null): void {
  // A row of the preview has its children given (`topology.ts`): nothing to read.
  if (parent !== null && isPreview(parent)) return;
  const known = observers.get(parent);
  if (!known) {
    observe(parent);
    return;
  }
  const { observer } = known;
  askNextPage(
    observer as unknown as LevelReader<Node>,
    () => observers.get(parent)?.observer === observer,
  );
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
// The status of the view (`useStatus`: emptied at another period, other rights, another node).
const { said, status, clear: clearSaid } = useStatus();
const say = useSay(status);
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
// The tree's own nodes only (the editing holds the content's rows too).
const shownById = computed(
  () =>
    new Map(
      items.value.map((item) => [
        item.id,
        edit.shownRow(item as unknown as Item) as unknown as Node,
      ]),
    ),
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
/** The nature of a prefix, the usage of a range: chosen in their list (task 12g). */
const options = (node: TreeNode, column: GridColumn) => {
  const row = rowOf(node.id);
  return row ? edit.choices(row, column) : undefined;
};
// Why a cell of the tree has no editor, or is held in review: its description and its
// tooltip, as in the grid of the content (task 12f2). The node's own cell has neither: its
// value is not edited in place (021/A20).
const readonlyReason = (node: TreeNode, column: GridColumn) => {
  const row = rowOf(node.id);
  return row ? whyWords(edit.why(column.id, row), t) : undefined;
};
const stateReason = (node: TreeNode, column: GridColumn) => {
  const row = rowOf(node.id);
  return row ? edit.stateReason(row, column) : undefined;
};
/** The line of the motive asked in the tree goes under it, that of the content under its grid. */
const onCommit = (commit: GridCommit) => edit.onCommit(commit, TREE_GRID);
const treeAsk = computed(() => edit.askOf(TREE_GRID));

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
// (`kindsIn`), filtered by `within` —, or for the first level to the prefixes of the domain
// (`restOf`); a parent or a domain not known, to none. A notice of the tree (020-32): out of
// the set of its siblings and of the count said, never selected, Enter or a click activates it.
const restAt = (parent: string | null) =>
  restOf(parent, parent === null ? undefined : nodeById(parent), domain.value);
function noticeOf(parent: string | null): TreeNode {
  const count = new Intl.NumberFormat(locale.value).format(levelOf(parent)?.items.length ?? 0);
  const rest = restAt(parent);
  return {
    id: cappedKey(parent),
    label:
      rest === null
        ? t("views.topology.capped.nowhere", { n: count })
        : parent === null
          ? t("views.topology.capped.roots", { n: count, name: domainSlug.value })
          : t(`views.topology.capped.${rest.name}`, { n: count, within: rest.query.within }),
    icon: "info",
    notice: true,
  };
}
/** The row of a notice activated: its list, when it has one. */
function openRest(key: string): void {
  const rest = restAt(cappedParent(key));
  if (rest !== null) void router.push(rest);
}

// A level whose rest could not be read (task 021-13b, REQ-021-20): its next page refused
// (`failed`), or the first page of a node opened — the first page of the first level
// refused is the tree's `ProblemState`. Its rows read stay; above the tree, an alert per
// level names it, says why with its codes copyable, and « Réessayer » asks the page again,
// as « Afficher la suite » activated does. Out of the tree: no row of its own, so none in
// the rows `TreeGrid` recycles (task 22e).
const refused = computed(() =>
  [...results].flatMap(([parent, result]) => {
    const error =
      levelOf(parent)?.failed ??
      (parent !== null && result.isError && result.data === undefined ? result.error : undefined);
    // A node closed again hides its alert with its rows: opened, its level says it again.
    if (error === undefined || error === null) return [];
    if (parent !== null && !expanded.value.includes(parent)) return [];
    const holder = parent === null ? undefined : nodeById(parent);
    const title =
      parent === null
        ? t("views.topology.refused.roots", { name: domainSlug.value })
        : holder
          ? t("views.topology.refused.node", { name: valueOf(holder) })
          : t("problem.nextPage");
    return [{ key: parent ?? "", parent, error, title }];
  }),
);
/** The level whose « Réessayer » was pressed, until its alert goes. */
let retried: string | null | undefined;
function retry(parent: string | null): void {
  retried = parent;
  onLoad(parent);
}
// Its page read, the alert goes with the button that had the focus (2.4.3): the focus back
// to the tree, its cell of the focus — not taken from anywhere else.
watch(refused, async (levels) => {
  if (retried === undefined || levels.some((level) => level.parent === retried)) return;
  retried = undefined;
  await nextTick();
  if (document.activeElement !== null && document.activeElement !== document.body) return;
  tree.value?.querySelector<HTMLElement>("[role='gridcell'][tabindex='0']")?.focus();
});

// Parents deduced (D34, REQ-021-17): no move is offered; a node whose value the active
// draft changes stays where the present has it, marked « en brouillon » (« déplacé dans le
// brouillon » when the draft moves it, task 24d) — the tree is never
// retouched by the client, only read again once a change is applied (X24).
const active = ref<string | null>(drafts.active());
const stopActive = drafts.onActive((id) => (active.value = id));
onBeforeUnmount(stopActive);
const activeDraft = useDraftQueries(active);
const inDraft = computed(() => inDraftOf(activeDraft.operations.value));
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
// 11) beside it, the first to hide; then the columns of the plugins for the prefixes
// (REQ-021-13, task 11o): each cell of a prefix an element of the plugin, given the prefix
// as its page serves it and the object it stands for; none in a row of another kind, nor
// in a row of the preview.
const extensions = useExtensions("column", "prefix");
const columns = computed<GridColumn[]>(() => [
  { id: "occupancy", label: t("views.record.occupancy"), width: "sm", align: "end" },
  { id: "detail", label: t("views.record.nature"), width: "sm", priority: 2 },
  ...gridColumns(extensions.value, (key) => t(key)),
]);
// The same `object` from one render to the next, as in the lists (`ExtensionElement`).
const objectRef = stableObjectRefs();
const elementProps = (node: TreeNode) => {
  const read = nodeById(node.id);
  return read?.kind === "prefix"
    ? { object: objectRef("prefix", read.id, read.href), row: read }
    : undefined;
};

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
/**
 * The nodes read, by identifier: the first level that holds one wins. Indexed once per
 * read, not searched per row (the rows of a level of 10⁴ asked each render, task 021-22e).
 */
const readById = computed(() => {
  const found = new Map<string, Node>();
  for (const parent of results.keys())
    for (const item of itemsOf(parent)) if (!found.has(item.id)) found.set(item.id, item);
  return found;
});
/** A node read, by identifier, in any level read so far. */
function nodeById(id: string | undefined): Node | undefined {
  return id === undefined ? undefined : readById.value.get(id);
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
// prefixes — the domain given; `e` modifies the chosen node. Under `valid_at` alone too, to
// the dated draft only (REQ-021-5; `action.ts`), never under `as_of`; the panel refuses,
// visibly, what the rights do not allow. `c` adds in the node of the focus in the tree,
// else in the chosen one (021/A20).
const session = useSession();
/**
 * Why adding in `node` (the first level without one) is refused: under `as_of`, or
 * without the right to write any kind it may hold — the first one named, as served.
 */
function addRefusal(node: Shown | undefined): { forbidden: string } | { reason: string } | null {
  if (!writesOf(time.value).draft) return { reason: t("edit.why.past") };
  const types = kindsIn(node).map((id) => collectionById(id)!.type);
  if (types.some((type) => session.value.rights.has(editRight(type)))) return null;
  return { forbidden: editRight(types[0]!) };
}
// The « + » of every prefix; refused, it stays visible and says why (REQ-021-37).
/** The names of the « + », by label, made once per language: asked of each row at each render. */
const addLabels = computed(() => {
  void locale.value;
  return new Map<string, string>();
});
function addLabel(node: TreeNode): string | undefined {
  if (nodeById(node.id)?.kind !== "prefix") return undefined;
  const made = addLabels.value;
  const label = made.get(node.label) ?? t("edit.addIn", { within: node.label });
  made.set(node.label, label);
  return label;
}
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
  // Refused, `c` says why by the status of the view (off the present first, `view.ts`): no
  // domain to add in, once the domains are read; else what the « + » says.
  why: () => {
    if (domain.value === undefined) {
      return domains.isSuccess.value && domainItems.value.length === 0
        ? t("views.topology.addNoDomain")
        : undefined;
    }
    const refusal = addRefusal(targetPrefix.value);
    if (!refusal) return undefined;
    return "forbidden" in refusal
      ? whyWords({ kind: "forbidden", permission: refusal.forbidden }, t)
      : refusal.reason;
  },
  say: say.status,
  run: () => addIn(targetPrefix.value?.id),
  dated: true,
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
  why: () =>
    chosen.value
      ? whyWords({ kind: "forbidden", permission: editRight(`core.${chosen.value.kind}`) }, t)
      : undefined,
  say: say.status,
  run: () => {
    if (chosen.value) {
      openEdit({
        mode: "edit",
        collection: NODE_COLLECTIONS[chosen.value.kind],
        id: chosen.value.id,
      });
    }
  },
  dated: true,
});

// The foot of the tree speaks of the cell of the focus (the tree's `active`, kept when the
// focus leaves it, as at the foot of a list): Enter opens the record of a node and Tab its
// « + » by the tree itself (021/A20).
const focusRow = computed(() => (activeCell.value ? rowOf(activeCell.value.row) : undefined));
/** Why the cell of the focus has no editor (its description says it too), or none. */
const focusWhy = computed(() =>
  activeCell.value && focusRow.value ? edit.why(activeCell.value.column, focusRow.value) : null,
);
const focusEditable = computed(() => focusRow.value !== undefined && focusWhy.value === null);
const focusSaysWhy = computed(() => focusWhy.value !== null && focusWhy.value.kind !== "none");
/** The permission Ctrl+Enter lacks in the cell of the focus, `null` when held. */
const focusApplyLacks = computed(() =>
  focusRow.value && !edit.mayApply(focusRow.value) ? nodeApplyRight(focusRow.value) : null,
);
/** Why the cell of the focus is held in review, said at the foot too (seen, not only read). */
const focusHeld = computed(() =>
  activeCell.value && focusRow.value
    ? edit.heldOf(focusRow.value, activeCell.value.column)
    : undefined,
);

// The cell says why it has no editor (its description, read when the focus comes to it);
// a reason that changes under the focus (the date, the rights) is said by the status,
// a description changed in place being read by few screen readers.
watch(
  () => [activeCell.value?.row, activeCell.value?.column, whyWords(focusWhy.value, t)] as const,
  ([row, column, words], [oldRow, oldColumn, oldWords]) => {
    if (!words || words === oldWords || row !== oldRow || column !== oldColumn) return;
    status(words);
  },
);
// What `c` or `e` said of the node chosen no longer holds for another.
watch(() => chosen.value?.id, clearSaid);

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
  const ask = treeAsk.value;
  if (ask && (await edit.confirm(motive, value))) void afterAsk(edit.cellOf(ask));
}
function onCancelAsk(): void {
  const ask = treeAsk.value;
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
  <template v-else-if="domains.isSuccess.value && domainItems.length === 0">
    <EmptyState icon="domain" :title="t('views.topology.noDomain')">
      {{ t("views.topology.noDomainText") }}
    </EmptyState>
    <!-- The tree's status is not rendered here: `c` says here why it adds nothing. -->
    <span class="k-visually-hidden" role="status">{{ said }}</span>
  </template>
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
             the levels whose rest could not be read; in the band above the tree, which has
             its margin (the body is flush). -->
        <template v-if="previewNote || refused.length > 0" #toolbar>
          <Stack class="k-topology__notes">
            <InlineAlert
              v-if="previewNote"
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
            <NextPageAlert
              v-for="level in refused"
              :key="level.key"
              :title="level.title"
              :error="level.error"
              :retry="() => retry(level.parent)"
            />
          </Stack>
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
            :options="options"
            :cell-state="cellState"
            :readonly-reason="readonlyReason"
            :state-reason="stateReason"
            :element-props="elementProps"
            parent-mode="derived"
            @load="onLoad"
            @activate="onActivate"
            @add="addIn"
            @commit="onCommit"
          >
            <!-- A column of a plugin: its element hosted, an error kept in its cell. -->
            <template #extension="given">
              <ExtensionElement cell v-bind="cellExtension(extensions, given)" />
            </template>
          </TreeGrid>
        </div>
        <span class="k-visually-hidden" role="status">{{ said }}</span>
        <!-- Once a cell has had the focus: what Enter does there, or why it has no editor. -->
        <template v-if="treeAsk || focusEditable || focusSaysWhy" #footer>
          <QuickApply
            v-if="treeAsk"
            :key="treeAsk.row + treeAsk.column"
            :ask="treeAsk"
            :words="words"
            @confirm="onConfirm"
            @cancel="onCancelAsk"
          />
          <EditHint v-else :why="focusWhy" :apply-lacks="focusApplyLacks" :held="focusHeld" />
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
        :node-edit="edit"
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

  /* Several levels refused never push the tree out of the panel: past about two alerts,
   * the band scrolls. */
  .k-topology__notes {
    max-block-size: calc(var(--k-row-height) * 12);
    overflow-y: auto;
  }
}
</style>
