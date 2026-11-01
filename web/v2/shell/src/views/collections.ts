import type { ListPath } from "@kartenn/sdk";
import type { GridColumn, IconName } from "@kartenn/ui";

// The collections of the referential the shell lists and opens (REQ-021-9; design 021
// « Catalogue des écrans » : listes des collections, fiche d'objet): their route of the
// API, their type, their columns and the filter each column stands for (020 decision A7:
// a column offers its filter only when the list's model serves that parameter), how a row
// is named. The view of a list and of a record read them; the routes declare their reads.

export type CollectionId = "domains" | "prefixes" | "ranges" | "addresses" | "entities" | "sites";

/** A column of a list: its field, how it shows, and the parameter of its filter. */
export interface CollectionColumn {
  id: string;
  width: NonNullable<GridColumn["width"]>;
  mono?: boolean;
  priority?: GridColumn["priority"];
  /** The filter of the model the column stands for (its funnel), if any. */
  filter?: string;
  /** The values are keys of a closed list of the core, shown by their label. */
  enumerated?: boolean;
  /** The value is a summary of an object (`{id, slug, href}`), shown by its slug. */
  summary?: boolean;
}

export interface Collection {
  id: CollectionId;
  /** The list of the contract. */
  path: ListPath;
  /** The kind of its objects (`prefix`) and their qualified type (`core.prefix`). */
  kind: string;
  type: string;
  /** The read without which the list is refused (403). */
  read: string;
  icon: IconName;
  /** The columns, the key first (it names the row). */
  columns: readonly CollectionColumn[];
}

export const COLLECTIONS: readonly Collection[] = [
  {
    id: "domains",
    path: "/v1/domains",
    kind: "domain",
    type: "core.domain",
    read: "core.domain.read",
    icon: "domain",
    columns: [{ id: "slug", width: "fill", mono: true, filter: "slug" }],
  },
  {
    id: "prefixes",
    path: "/v1/prefixes",
    kind: "prefix",
    type: "core.prefix",
    read: "core.prefix.read",
    icon: "prefix",
    columns: [
      { id: "value", width: "md", mono: true, filter: "value" },
      { id: "nature", width: "sm", filter: "nature", enumerated: true },
      { id: "domain", width: "fill", mono: true, filter: "domain", summary: true },
      { id: "public", width: "sm", priority: 3 },
    ],
  },
  {
    id: "ranges",
    path: "/v1/ranges",
    kind: "range",
    type: "core.range",
    read: "core.range.read",
    icon: "range",
    columns: [
      { id: "first", width: "md", mono: true, filter: "first" },
      { id: "last", width: "md", mono: true },
      { id: "usage", width: "sm", filter: "usage", enumerated: true, priority: 2 },
      { id: "domain", width: "fill", mono: true, filter: "domain", summary: true },
    ],
  },
  {
    id: "addresses",
    path: "/v1/addresses",
    kind: "address",
    type: "core.address",
    read: "core.address.read",
    icon: "address",
    columns: [
      { id: "value", width: "md", mono: true, filter: "value" },
      { id: "usage", width: "sm", filter: "usage", enumerated: true },
      { id: "domain", width: "fill", mono: true, filter: "domain", summary: true },
      { id: "public", width: "sm", priority: 3 },
    ],
  },
  {
    id: "entities",
    path: "/v1/entities",
    kind: "entity",
    type: "core.entity",
    read: "core.entity.read",
    icon: "users",
    columns: [
      { id: "slug", width: "md", mono: true, filter: "slug" },
      { id: "parent", width: "fill", mono: true, filter: "parent", summary: true },
    ],
  },
  {
    id: "sites",
    path: "/v1/sites",
    kind: "site",
    type: "core.site",
    read: "core.site.read",
    icon: "globe",
    columns: [
      { id: "slug", width: "md", mono: true, filter: "slug" },
      { id: "parent", width: "fill", mono: true, filter: "parent", summary: true },
    ],
  },
];

/** The collection of a segment of the URL (`prefixes`), `undefined` if none. */
export function collectionById(id: unknown): Collection | undefined {
  return COLLECTIONS.find((collection) => collection.id === id);
}

/** The collection of a kind of object (`prefix`, `core.prefix`), `undefined` if none. */
export function collectionOfKind(kind: string | null | undefined): Collection | undefined {
  return COLLECTIONS.find((collection) => collection.kind === kind || collection.type === kind);
}

export type Item = Record<string, unknown>;

/** A summary of an object in a field (`{id, slug?, href}`). */
export interface Summary {
  id: string;
  slug?: string | null;
  href?: string | null;
  kind?: string | null;
}

export function isSummary(value: unknown): value is Summary {
  return (
    typeof value === "object" &&
    value !== null &&
    typeof (value as { id?: unknown }).id === "string"
  );
}

/** The name of an object: its value, its bounds, its slug, else its identifier. */
export function nameOf(item: Item): string {
  if (typeof item.value === "string") return item.value;
  if (typeof item.first === "string" && typeof item.last === "string") {
    return `${item.first} – ${item.last}`;
  }
  if (typeof item.slug === "string") return item.slug;
  return String(item.id ?? "—");
}

/** The text of a field: a summary by its slug, a boolean, nothing as `—`. */
export function fieldText(value: unknown): string {
  if (value === null || value === undefined || value === "") return "—";
  if (isSummary(value)) return value.slug ?? value.id;
  if (typeof value === "string" || typeof value === "number") return String(value);
  if (typeof value === "boolean") return value ? "true" : "false";
  return JSON.stringify(value);
}

/**
 * The page of the shell of a URL of the API (`/v1/prefixes/<id>` → `/prefixes/<id>`), if it
 * names an object of a collection the shell opens.
 */
export function pageOfHref(href: unknown): string | null {
  if (typeof href !== "string") return null;
  const match = /^(?:https?:\/\/[^/]+)?\/v1\/([a-z-]+)\/([^/?#]+)$/.exec(href);
  return match && collectionById(match[1]) ? `/${match[1]}/${match[2]}` : null;
}

/** The page of an object known by its kind and identifier, if the shell opens it. */
export function pageOfObject(kind: string | null | undefined, id: string): string | null {
  const collection = collectionOfKind(kind);
  return collection ? `/${collection.id}/${id}` : null;
}
