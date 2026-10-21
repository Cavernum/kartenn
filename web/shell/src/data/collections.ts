// The collections of the core the shell lists and opens (REQ-007-17, design « Écrans et
// routes du client »): their filters, their columns, how a row is named.

export const COLLECTIONS = [
  "entities",
  "sites",
  "domains",
  "prefixes",
  "ranges",
  "addresses",
] as const;
export type Collection = (typeof COLLECTIONS)[number];

/** `/:collection` and `/:collection/:id` (route pattern). */
export const COLLECTION_PATTERN = COLLECTIONS.join("|");

export function isCollection(value: unknown): value is Collection {
  return (COLLECTIONS as readonly unknown[]).includes(value);
}

/** The type of an object of each collection (contract 006, `kind`). */
export const TYPES: Record<Collection, string> = {
  entities: "entity",
  sites: "site",
  domains: "domain",
  prefixes: "prefix",
  ranges: "range",
  addresses: "address",
};

/** The collection of a type, else `null`. */
export function collectionOf(type: string | null | undefined): Collection | null {
  const found = COLLECTIONS.find((c) => TYPES[c] === type);
  return found ?? null;
}

/** The filters of each list, those of the contract's `GET` (in the URL of the shell). */
export const FILTERS: Record<Collection, readonly string[]> = {
  entities: ["slug", "parent"],
  sites: ["slug", "parent"],
  domains: ["slug"],
  prefixes: ["domain", "value"],
  ranges: ["domain", "first"],
  addresses: ["domain", "value"],
};

/** The columns of each list, the first one naming the row. */
export const COLUMNS: Record<Collection, readonly string[]> = {
  entities: ["slug", "parent"],
  sites: ["slug", "parent"],
  domains: ["slug"],
  prefixes: ["value", "nature", "domain", "public"],
  ranges: ["first", "last", "usage", "domain"],
  addresses: ["value", "usage", "domain", "public"],
};

type Item = Record<string, unknown>;

function isSummary(value: unknown): value is { id: string; slug?: string | null; href?: string } {
  return typeof value === "object" && value !== null && "id" in value;
}

/** The text of a field of an object: a summary by its slug, nothing as `—`. */
export function cell(item: Item, key: string): string {
  const value = item[key];
  if (value === null || value === undefined) return "—";
  if (isSummary(value)) return value.slug ?? value.id;
  if (typeof value === "object") return JSON.stringify(value);
  return String(value);
}

/** The text of a value of a property: lists joined, references by their name. */
export function valueText(value: unknown): string {
  if (value === null || value === undefined) return "—";
  if (Array.isArray(value)) return value.length === 0 ? "—" : value.map(valueText).join(", ");
  if (typeof value === "object") {
    const object = value as Item;
    if ("value" in object || "slug" in object || "id" in object) return nameOf(object);
    return JSON.stringify(value);
  }
  return String(value);
}

/** The name of an object: its value, slug or bounds, else its identifier. */
export function nameOf(item: Item): string {
  if (typeof item.value === "string") return item.value;
  if (typeof item.slug === "string") return item.slug;
  if (typeof item.first === "string" && typeof item.last === "string") {
    return `${item.first} – ${item.last}`;
  }
  return String(item.id ?? "—");
}

/** The page of the shell for a URL of the API (`/v1/prefixes/<id>` → `/prefixes/<id>`). */
export function pageOf(href: unknown): string | null {
  if (typeof href !== "string") return null;
  const match = /^(?:https?:\/\/[^/]+)?\/v1\/([a-z-]+)\/([^/?#]+)$/.exec(href);
  return match && isCollection(match[1]) ? `/${match[1]}/${match[2]}` : null;
}

/** The link of a summary in a field, if it names an object of the core. */
export function linkOf(item: Item, key: string): string | null {
  const value = item[key];
  return isSummary(value) ? pageOf(value.href) : null;
}
