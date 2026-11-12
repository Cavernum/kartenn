import type { components } from "@kartenn/sdk";
import { computed, getCurrentScope, onScopeDispose, shallowReactive, watch } from "vue";
import { useI18n } from "vue-i18n";
import { collectionById, type Collection, type CollectionId, type Item } from "../collections.ts";
import { applyRight } from "./fields.ts";
import { rereadLists, useInlineEdit, type InlineDeps } from "./inline.ts";

// In-place editing of the nodes of the prefixes (task 12a; REQ-021-15, 16): the tree of the
// topography and the grid of the content of a node mix prefixes, ranges and addresses, each
// row edited by its kind — the column `detail` stands for the nature of a prefix and the
// usage of a range (an address of the tree has none). The node itself (its value, its
// bounds) is never edited in place: Enter on it opens its record (021/A20), `e` the edit
// panel. `expected` and `If-Match` are the `version` of the page the node came with (one
// snapshot, 11a): never a version read apart from the value shown. One editing for the tree
// and the content of its node chosen beside it (task 12f2): `share` joins the rows of the
// content, so a cell held, under way or refused says it in both, and one line of the motive
// at a time stands for both, under the grid it was asked from. A row of no known kind (the
// kind read under a type guard) has no editor; it counts as a prefix for the rest.

type Kind = components["schemas"]["TreeNode"]["kind"];

/** The collection of each kind of node. */
export const NODE_COLLECTIONS: Readonly<Record<Kind, CollectionId>> = {
  prefix: "prefixes",
  range: "ranges",
  address: "addresses",
};

/** The field the column `detail` of a node edits, by its kind. */
export const DETAIL_FIELD: Readonly<Partial<Record<Kind, string>>> = {
  prefix: "nature",
  range: "usage",
};

/** Whether `value` is a kind of node (`TreeNode.kind`). */
export const isKind = (value: unknown): value is Kind =>
  typeof value === "string" && Object.hasOwn(NODE_COLLECTIONS, value);

/** The kind of a row, `undefined` for a row of no known kind (it has no editor). */
const kindOf = (row: Item | undefined): Kind | undefined => {
  const kind = row?.kind;
  return isKind(kind) ? kind : undefined;
};
/** The collection of a row; without a row (or a kind), the prefixes. */
const collectionOf = (row?: Item): Collection =>
  collectionById(NODE_COLLECTIONS[kindOf(row) ?? "prefix"])!;
/** The field the column `detail` of `row` edits, if any. */
const detailField = (row: Item): string | undefined => {
  const kind = kindOf(row);
  return kind ? DETAIL_FIELD[kind] : undefined;
};

/** The permission that applies at once a change of the node `row` (`X.approval.self`). */
export const nodeApplyRight = (row: Item): string => applyRight(collectionOf(row).type);

/** The detail of a node as its row holds it: the nature of a prefix, the usage of a range. */
export function detailOf(row: Item): string | null {
  const field = detailField(row);
  const value = field ? row[field] : undefined;
  return typeof value === "string" ? value : null;
}

/** The words of a key of the nature or a usage (`views.value.<field>.<key>`), if any. */
export function useNodeWords(): (field: string, key: string) => string | null {
  const { t, te } = useI18n();
  return (field, key) =>
    te(`views.value.${field}.${key}`) ? t(`views.value.${field}.${key}`) : null;
}

/** The grids of the topography that edit nodes (`QuickAsk.grid`): the tree, the content. */
export const TREE_GRID = "tree";
export const CONTENT_GRID = "content";

/** Another grid of the same nodes: its name, its rows, how it reads them again after a 412. */
export interface SharedRows {
  grid: string;
  rows: () => readonly Item[];
  reread: (row: string) => void;
}

/** The in-place editing of rows of nodes (`TreeNode` as their pages serve them). */
export function useNodeEdit(deps: Omit<InlineDeps, "collection" | "field" | "rowOf">) {
  const scope = getCurrentScope();
  const shared = shallowReactive(new Set<SharedRows>());
  // The rows of the host, then those of the grids shared, each node once: what the cells
  // say (their states, the draft) is of the object, whichever grid shows it. A cell is
  // written at the version of the grid it was typed in (`rowOf`).
  const rows = computed(() => {
    const byId = new Map<string, Item>();
    for (const row of deps.rows()) byId.set(String(row.id), row);
    for (const other of shared) {
      for (const row of other.rows()) if (!byId.has(String(row.id))) byId.set(String(row.id), row);
    }
    return [...byId.values()];
  });
  const edit = useInlineEdit({
    ...deps,
    rows: () => rows.value,
    rowOf: (id, grid) => {
      for (const other of shared) {
        if (other.grid === grid) return other.rows().find((row) => String(row.id) === id);
      }
      return deps.rows().find((row) => String(row.id) === id);
    },
    collection: collectionOf,
    field: (row, column) => (column === "detail" ? detailField(row) : undefined),
    // After a 412, every grid that shows the node reads it again.
    reread: (row, of) => {
      (deps.reread ?? rereadLists)(row, of);
      for (const other of shared) {
        if (other.rows().some((item) => String(item.id) === row)) other.reread(row);
      }
    },
  });
  /**
   * The grid `other` edited by this editing (the content of the node chosen beside the
   * tree): its cells share their states and the line of the motive; gone with its scope,
   * its line of the motive with it (no grid would show it, nor let it be closed), once its
   * write under way is answered.
   */
  function share(other: SharedRows): void {
    shared.add(other);
    onScopeDispose(() => {
      shared.delete(other);
      const ask = edit.askOf(other.grid);
      if (!ask) return;
      if (!ask.busy) return edit.cancel();
      // In the host's scope: the grid's is being stopped.
      scope?.run(() => {
        const stop = watch(edit.asking, (now) => {
          if (now?.busy) return;
          stop();
          if (now && now.grid === other.grid) edit.cancel();
        });
      });
    });
  }
  return { ...edit, share };
}

export type NodeEdit = ReturnType<typeof useNodeEdit>;
