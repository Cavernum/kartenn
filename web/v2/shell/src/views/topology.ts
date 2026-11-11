import { CHILDREN_PAGES, type Operation, type Simulated } from "@kartenn/sdk";
import type { TreeNode, TreeNodeState } from "@kartenn/ui";
import type { CollectionId } from "./collections.ts";
import { draftedClosures, draftedValues } from "./edit/fields.ts";

// The nodes of the tree of the topography from its levels read (task 24, REQ-021-34): the
// children of a prefix are those its own list gives, read again after a change level by
// level. Until every level is read again, one list may still show a node another already
// shows at its new place — even a node under its own former child, a cycle: each node is
// placed once, the first list that shows it, depth first, and waits there for the others
// to be read again, rather than show twice or loop.

/**
 * A level read: its items, whether pages remain to read; `capped`, pages remain beyond
 * those the tree reads (021/A34); `failed`, the error of its next page refused (task
 * 021-13b, REQ-021-20: the items read stay, its rest is still to read); `undefined` when
 * not read yet.
 */
export interface Level<I> {
  items: readonly I[];
  more: boolean;
  capped?: boolean;
  failed?: unknown;
}

/** The pages of a level as the cache keeps them (`InfiniteQueryObserver`). */
export interface LevelPages<I> {
  data?: { pages: readonly { data: { items: readonly I[] } }[] } | undefined;
  hasNextPage: boolean;
  /** Its next page refused, and why (`error`). */
  isFetchNextPageError?: boolean;
  error?: unknown;
}

/**
 * A level from its pages: the next page read while pages remain and fewer than `cap` are
 * kept (`CHILDREN_PAGES`, the cache's for the children of a node); past the cap, no page
 * more — the cache would drop the first one to keep the next — and the level is capped.
 */
export function levelOfPages<I>(
  result: LevelPages<I> | undefined,
  cap: number = CHILDREN_PAGES,
): Level<I> | undefined {
  const pages = result?.data?.pages;
  if (!pages) return undefined;
  const capped = result.hasNextPage && pages.length >= cap;
  return {
    items: pages.flatMap((page) => page.data.items),
    more: result.hasNextPage && !capped,
    capped,
    ...(result.isFetchNextPageError ? { failed: result.error } : {}),
  };
}

/** The observer of a level's pages as the tree asks it (`InfiniteQueryObserver`). */
export interface LevelReader<I> {
  getCurrentResult(): LevelPages<I> & { isFetching: boolean; isError: boolean };
  fetchNextPage(options?: {
    cancelRefetch?: boolean;
  }): Promise<LevelPages<I> & { isFetching: boolean; isError: boolean }>;
  refetch(): Promise<unknown>;
}

/**
 * The tree asks for the next page of a level (`onLoad`, task 021-22d). The tree asks once
 * per number of children read (`treePages.ts` of `@kartenn/ui`): the ask holds until its
 * page comes, or until the level fails or says it has no page more. A read under way — the
 * page itself, or the level read again after an invalidation (`ready`, a change) — is never
 * cancelled nor doubled: the page is asked once it is done. An invalidation that comes
 * while the page is under way cancels it (`invalidateQueries`) and reads the level again
 * as it was: the page is asked again after that read. A level whose first page failed is
 * read again (its row activated). `live`: whether the level is still the tree's (a domain
 * or dates changed start every level again). The level counts its pages up to `cap`, the
 * `maxPages` of its query (`levelOfPages`). A page refused is not asked again: the level
 * says why (`failed`), until it is asked anew (« Réessayer », the row activated).
 */
export function askNextPage<I>(
  reader: LevelReader<I>,
  live: () => boolean = () => true,
  cap: number = CHILDREN_PAGES,
): void {
  const now = reader.getCurrentResult();
  const level = levelOfPages(now, cap);
  if (level === undefined) {
    if (!now.isFetching) void reader.refetch();
    return;
  }
  if (!level.more) return;
  const pages = now.data?.pages.length ?? 0;
  void reader.fetchNextPage({ cancelRefetch: false }).then((after) => {
    const came = (after.data?.pages.length ?? 0) > pages;
    if (!came && !after.isError && live()) askNextPage(reader, live, cap);
  });
}

/** The key of the notice under a level capped: its parent's (`""`, the first level). */
export const CAPPED = "capped:";
export const cappedKey = (parent: string | null): string => `${CAPPED}${parent ?? ""}`;
export const isCapped = (key: string): boolean => key.startsWith(CAPPED);
/** The parent of a notice (`null`: the first level). */
export const cappedParent = (key: string): string | null => key.slice(CAPPED.length) || null;

/** A node of the tree as `kindsIn` and `restOf` read it. */
export interface Holder {
  kind: string;
  nature?: string | null;
  value?: string;
}

/** The kinds a node may hold: a network its addresses and ranges, else prefixes. */
export const kindsIn = (node: Holder | undefined): CollectionId[] =>
  node?.kind === "prefix" && node.nature === "network" ? ["addresses", "ranges"] : ["prefixes"];

/** A list of the core, filtered: where the rest of a capped level is listed. */
export interface Rest {
  name: CollectionId;
  query: Record<string, string>;
}

/**
 * Where the rest of a capped level is listed (021/A34): under a prefix, the list of the
 * first kind it holds (`kindsIn`) `within` its value — the prefixes of a container, the
 * addresses of a network —; for the first level (`parent` `null`), the prefixes of the
 * domain. `null` when the prefix or the domain is not known: never a list without filter
 * (task 021-7a).
 */
export function restOf(
  parent: string | null,
  node: Holder | undefined,
  domain: string | undefined,
): Rest | null {
  if (parent === null) return domain ? { name: "prefixes", query: { domain } } : null;
  if (node?.kind !== "prefix" || !node.value) return null;
  return { name: kindsIn(node)[0]!, query: { within: node.value } };
}

/**
 * The nodes under `parent`, each placed once; after the children of a level capped, the
 * row of `notice`, when given (the rest of the list elsewhere).
 */
export function nodesOf<I extends { id: string }>(
  levelOf: (parent: string | null) => Level<I> | undefined,
  nodeOf: (item: I) => TreeNode,
  /** Whether an item holds children (a prefix). */
  holds: (item: I) => boolean,
  notice?: (parent: string | null) => TreeNode,
  parent: string | null = null,
  placed = new Set<string>(),
): TreeNode[] {
  const level = levelOf(parent);
  const items = (level?.items ?? []).filter((item) => !placed.has(item.id));
  for (const item of items) placed.add(item.id);
  const nodes = items.map((item) => {
    const node = nodeOf(item);
    if (!holds(item)) return node;
    const below = levelOf(item.id);
    if (below) {
      node.children = nodesOf(levelOf, nodeOf, holds, notice, item.id, placed);
      if (below.more) node.more = true;
    } else node.hasChildren = true;
    return node;
  });
  if (level?.capped && notice) nodes.push(notice(parent));
  return nodes;
}

// The preview of the active draft in the tree (task 24b; REQ-021-17; 021/A12, precised by
// 24a): the places its simulation gives (`placements`, at the present), laid over the tree
// served, never a deduction of the client. Each object whose parent the draft changes stays
// at its present place, marked, and shows again under its parent after; the tree served
// stays the truth, and the preview goes with the draft (applied or abandoned).

/** The kinds of the nodes of the tree of the topography. */
export type NodeKind = "prefix" | "range" | "address";
const NODE_KINDS: ReadonlySet<string> = new Set<NodeKind>(["prefix", "range", "address"]);

/** An object a creation of the draft makes, as the preview names it. */
export interface Creation {
  /** The index of its operation in the draft: its key, kept from one simulation to the next. */
  index: number;
  kind: NodeKind;
  /** Its value: a prefix or an address, a range « first – last ». */
  label: string;
  /** Its nature (a prefix) or its usage (a range), as the operation gives it. */
  detail: string | null;
  /** Its domain, when the operation names it by identifier. */
  domain: string | null;
}

/** An object of the tree shown under its parent after the draft. */
export interface Shift {
  /** Its key among the rows: `draft:` and its object, or the index of the creation. */
  key: string;
  /** The object, when it exists already; a creation otherwise. */
  object?: string;
  creation?: Creation;
  kind: NodeKind;
  /**
   * The row it shows under: a node served, another shift, `null` for the first level;
   * `undefined` while its parent after is not read in this tree (not shown).
   */
  parent: string | null | undefined;
}

/** The preview of a simulation in the tree of a domain. */
export interface Preview {
  /** The objects of the domain whose place changes, each to show under its parent after. */
  shifts: Shift[];
  /** The objects that exist already and change place: marked at their present place. */
  moved: ReadonlySet<string>;
  /** Some placements may be missing (`placements_truncated`): the preview is partial. */
  truncated: boolean;
}

/** The key of a row of the preview, never one of the tree served. */
export const PREVIEW = "draft:";
export const isPreview = (key: string): boolean => key.startsWith(PREVIEW);

/**
 * The objects of the tree the operations of a draft change, marked at their present place
 * (REQ-021-17): those whose value an `update_*` changes; a range whose bounds the draft
 * changes is closed then created anew (no bounds in `update_range`), the one closed marked
 * as well, the one created shown by the preview (task 24g).
 */
export function inDraftOf(operations: readonly Operation[] | undefined): Set<string> {
  return new Set([
    ...(["prefixes", "ranges", "addresses"] as const).flatMap((c) => [
      ...draftedValues(operations, c).keys(),
    ]),
    ...draftedClosures(operations, "ranges"),
  ]);
}

/**
 * What the draft does to the row `id` (task 24d; 021 design « Après un dépôt accepté »): a
 * row of the preview is the object at its place after (`preview`); a row served whose
 * object the draft moves stays at its present place, `moved`; one the draft changes without
 * moving it, `draft` (`inDraft`: the objects the draft changes); none otherwise.
 */
export function draftStateOf(
  id: string,
  inDraft: ReadonlySet<string>,
  preview: Preview | undefined,
): Exclude<TreeNodeState, "loading-children"> | undefined {
  if (isPreview(id)) return "preview";
  if (preview?.moved.has(id)) return "moved";
  return inDraft.has(id) ? "draft" : undefined;
}

/**
 * The node of the tree operation `index` of the draft creates, if any: under `within`, the
 * creation it wraps, with the index of its envelope (as the contract numbers them).
 */
function creationOf(operation: Operation, index: number): Creation | undefined {
  let op = operation as Record<string, unknown>;
  while (op.op === "within" && typeof op.operation === "object" && op.operation !== null) {
    op = op.operation as Record<string, unknown>;
  }
  const name = typeof op.op === "string" ? op.op : "";
  if (!name.startsWith("create_")) return undefined;
  const kind = name.slice("create_".length);
  const text = (member: string) => (typeof op[member] === "string" ? op[member] : null);
  const domain = (op.domain as { id?: unknown } | undefined)?.id;
  const label =
    kind === "prefix"
      ? text("prefix")
      : kind === "address"
        ? text("address")
        : kind === "range"
          ? `${text("first") ?? ""} – ${text("last") ?? ""}`
          : null;
  if (label === null || !NODE_KINDS.has(kind)) return undefined;
  return {
    index,
    kind: kind as NodeKind,
    label,
    detail: kind === "prefix" ? text("nature") : kind === "range" ? text("usage") : null,
    domain: typeof domain === "string" ? domain : null,
  };
}

/**
 * The preview of `simulated` in the tree of `domain`, `operations` the draft simulated and
 * `known` whether an object is read in this tree. An object the draft creates has an
 * identifier of the simulation alone: its placement, from no parent, names the index of its
 * operation (`operation`, 021/A26), which gives its creation. A parent the author may not read
 * comes `null`: shown at the first level, as the top of the tree it reads.
 */
export function previewOf(
  simulated: Pick<Simulated, "placements" | "placements_truncated">,
  operations: readonly Operation[],
  known: (id: string) => boolean,
  domain: string,
): Preview {
  const creations = new Map<number, Creation>();
  operations.forEach((operation, index) => {
    const creation = creationOf(operation, index);
    if (creation) creations.set(index, creation);
  });
  // The identifiers of the simulation of the objects created, by the index of their operation.
  const created = new Map<string, Creation>();
  for (const placement of simulated.placements) {
    const creation = placement.operation === null ? undefined : creations.get(placement.operation);
    if (creation?.kind === placement.kind && placement.before === null) {
      created.set(placement.object, creation);
    }
  }

  const keyOf = (object: string) => {
    const creation = created.get(object);
    return `${PREVIEW}${creation ? String(creation.index) : object}`;
  };
  const ofDomain = simulated.placements.filter((p) => {
    if (!NODE_KINDS.has(p.kind)) return false;
    const creation = created.get(p.object);
    return creation ? creation.domain === domain : known(p.object);
  });
  const shifting = new Set(ofDomain.map((p) => p.object));
  const moved = new Set(ofDomain.filter((p) => !created.has(p.object)).map((p) => p.object));
  const shifts = ofDomain.map((p): Shift => {
    const creation = created.get(p.object);
    const after = p.after;
    const parent =
      after === null
        ? null
        : shifting.has(after)
          ? keyOf(after)
          : !created.has(after) && known(after)
            ? after
            : undefined;
    return {
      key: keyOf(p.object),
      ...(creation ? { creation } : { object: p.object }),
      kind: creation?.kind ?? (p.kind as NodeKind),
      parent,
    };
  });
  return { shifts, moved, truncated: simulated.placements_truncated };
}

/**
 * The nodes served with the preview over them: each shift under its row after (a node
 * served whose children are read, another shift, the first level), after the children
 * served; the count shown, the others waiting for their parent to be read. An object that
 * moves takes along the children it keeps (not placed: their parent stays), copied from
 * the tree served as rows of the preview, as far as they are read.
 */
export function overlay(
  nodes: readonly TreeNode[],
  preview: Preview,
  nodeOf: (shift: Shift, children: TreeNode[] | undefined) => TreeNode,
): { nodes: TreeNode[]; shown: number } {
  const under = new Map<string | null, Shift[]>();
  for (const shift of preview.shifts) {
    if (shift.parent === undefined) continue;
    under.set(shift.parent, [...(under.get(shift.parent) ?? []), shift]);
  }
  const servedById = new Map<string, TreeNode>();
  const index = (list: readonly TreeNode[]) =>
    list.forEach((node) => {
      servedById.set(node.id, node);
      index(node.children ?? []);
    });
  index(nodes);
  const shifting = new Set(preview.shifts.flatMap((shift) => shift.object ?? []));
  // A row of the preview has its children given: one not read is shown without them; the
  // notice of a level capped stays under the row at its present place.
  const kept = (list: readonly TreeNode[]): TreeNode[] =>
    list
      .filter((node) => !shifting.has(node.id) && !node.notice)
      .map((node) => {
        const copied = kept(node.children ?? []);
        const copy: TreeNode = { ...node, id: `${PREVIEW}${node.id}` };
        delete copy.hasChildren;
        delete copy.more;
        delete copy.children;
        if (copied.length > 0) copy.children = copied;
        return copy;
      });
  const done = new Set<string>();
  const extra = (key: string | null): TreeNode[] =>
    (under.get(key) ?? [])
      .filter((shift) => !done.has(shift.key))
      .map((shift) => {
        done.add(shift.key);
        const children = [
          ...kept((shift.object && servedById.get(shift.object)?.children) || []),
          ...extra(shift.key),
        ];
        return nodeOf(shift, children.length > 0 ? children : undefined);
      });
  const walk = (list: readonly TreeNode[]): TreeNode[] =>
    list.map((node) =>
      node.children ? { ...node, children: [...walk(node.children), ...extra(node.id)] } : node,
    );
  const out = [...walk(nodes), ...extra(null)];
  return { nodes: out, shown: done.size };
}
