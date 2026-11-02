import type { components } from "@kartenn/sdk";
import { useI18n } from "vue-i18n";
import { collectionById, type Collection, type CollectionId, type Item } from "../collections.ts";
import { useInlineEdit, type InlineDeps } from "./inline.ts";

// In-place editing of the nodes of the prefixes (task 12a; REQ-021-15, 16): the tree of the
// topography and the grid of the content of a node mix prefixes, ranges and addresses, each
// row edited by its kind — the column `detail` stands for the nature of a prefix and the
// usage of a range (an address of the tree has none). The node itself (its value, its
// bounds) is never edited in place: Enter on it opens its record (021/A20), `e` the edit
// panel. `expected` and `If-Match` are the `version` of the page the node came with (one
// snapshot, 11a): never a version read apart from the value shown.

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

const kindOf = (row: Item | undefined): Kind =>
  row?.kind === "range" || row?.kind === "address" ? row.kind : "prefix";
const collectionOf = (row?: Item): Collection => collectionById(NODE_COLLECTIONS[kindOf(row)])!;

/** The detail of a node as its row holds it: the nature of a prefix, the usage of a range. */
export function detailOf(row: Item): string | null {
  const field = DETAIL_FIELD[kindOf(row)];
  const value = field ? row[field] : undefined;
  return typeof value === "string" ? value : null;
}

/** The words of a key of the nature or a usage (`views.value.<field>.<key>`), if any. */
export function useNodeWords(): (field: string, key: string) => string | null {
  const { t, te } = useI18n();
  return (field, key) =>
    te(`views.value.${field}.${key}`) ? t(`views.value.${field}.${key}`) : null;
}

/** The in-place editing of rows of nodes (`TreeNode` as their pages serve them). */
export function useNodeEdit(deps: Omit<InlineDeps, "collection" | "field">) {
  return useInlineEdit({
    ...deps,
    collection: collectionOf,
    field: (row, column) => (column === "detail" ? DETAIL_FIELD[kindOf(row)] : undefined),
  });
}
