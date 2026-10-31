import { queryClient, reads, type FiltersOf, type ListPath } from "@kartenn/sdk";
import type { IconName } from "@kartenn/ui";
import type { FoundObject } from "./recent.ts";

// The search of the palette by value and by name (REQ-021-3; after the global search of the
// 007, web/shell SearchView.vue): the text is the start of a value (`q`, « 10.1. ») for the
// prefixes and the addresses when it looks like one, of a slug or a name for the other
// collections; each collection the user reads, through the cache of the SDK (a text typed
// again is read from it), at the dates of the page. A collection that fails is left out:
// the palette offers what it found.

/** The lists whose contract takes `q` (the start of a value, a slug or a name). */
export type SearchPath = {
  [P in ListPath]: "q" extends keyof FiltersOf<P> ? P : never;
}[ListPath];

interface Searched {
  type: string;
  path: SearchPath;
  /** The read without which the collection is not searched. */
  read: string;
  icon: IconName;
  /** Searched by value (prefixes, addresses), else by slug or name. */
  byValue?: boolean;
  /** What the palette shows of an object: its value (fixed pitch), and where it lies. */
  show: (item: Record<string, unknown>) => { value: string; context?: string };
}

const slugOf = (summary: unknown): string | undefined => {
  const slug = (summary as { slug?: unknown } | null)?.slug;
  return typeof slug === "string" ? slug : undefined;
};
const text = (value: unknown): string => (typeof value === "string" ? value : String(value));
const shown = (value: string, context: string | undefined) =>
  context === undefined ? { value } : { value, context };

/** The collections searched, in the order of the palette. */
export const SEARCHED: readonly Searched[] = [
  {
    type: "prefix",
    path: "/v1/prefixes",
    read: "core.prefix.read",
    icon: "prefix",
    byValue: true,
    show: (item) => shown(text(item.value), slugOf(item.domain)),
  },
  {
    type: "address",
    path: "/v1/addresses",
    read: "core.address.read",
    icon: "address",
    byValue: true,
    show: (item) => shown(text(item.value), slugOf(item.domain)),
  },
  {
    type: "domain",
    path: "/v1/domains",
    read: "core.domain.read",
    icon: "domain",
    show: (item) => ({ value: text(item.slug) }),
  },
  {
    type: "entity",
    path: "/v1/entities",
    read: "core.entity.read",
    icon: "users",
    show: (item) => shown(text(item.slug), slugOf(item.parent)),
  },
  {
    type: "site",
    path: "/v1/sites",
    read: "core.site.read",
    icon: "globe",
    show: (item) => shown(text(item.slug), slugOf(item.parent)),
  },
  {
    type: "vlan",
    path: "/v1/vlans",
    read: "core.vlan.read",
    icon: "vlan",
    show: (item) => ({ value: `VID ${text(item.vid)}`, context: text(item.name) }),
  },
  {
    type: "vlan-group",
    path: "/v1/vlan-groups",
    read: "core.vlan_group.read",
    icon: "layers",
    show: (item) => ({ value: text(item.slug) }),
  },
];

/** How many objects of a collection the palette takes, and of all collections. */
export const FOUND_PER_TYPE = 5;
export const FOUND_SHOWN = 8;
/** The longest text sent (the contract caps `q` at 100 characters). */
const MAX_TEXT = 100;

/** Whether `text` starts an address or a prefix: hexadecimal, `.`, `:`, `/`, a digit or `:`. */
export function looksLikeValue(text: string): boolean {
  return /^[0-9a-f.:/]+$/i.test(text) && /[0-9:]/.test(text);
}

/** The page of a record from its link in the API: `/v1/prefixes/<id>` → `/prefixes/<id>`. */
export function pageOf(link: unknown): string | null {
  return typeof link === "string" && link.startsWith("/v1/") ? link.slice(3) : null;
}

/** Whether the objects of `type` are searched, and shown among the recent ones, for `has`. */
export function readableType(type: string, has: (permission: string) => boolean): boolean {
  const searched = SEARCHED.find((s) => s.type === type);
  return !!searched && has(searched.read);
}

/**
 * Reads the first page of a collection for `q`; `signal` aborts it (a new text, the palette
 * closed).
 */
export type SearchRead = (
  path: SearchPath,
  q: string,
  signal: AbortSignal,
) => Promise<readonly unknown[]>;

/**
 * Through the cache of the SDK, at the page's dates. The size of the page stays the
 * contract's for a suggestion (20): the SDK's list filters leave `page_size` out, the cache
 * keeping one key per filter; the palette takes the first ones.
 */
const cacheRead: SearchRead = async (path, q, signal) => {
  const options = reads.list(path, { q });
  const cancel = () => void queryClient.cancelQueries({ queryKey: options.queryKey, exact: true });
  signal.addEventListener("abort", cancel, { once: true });
  try {
    const data = await queryClient.fetchInfiniteQuery(options);
    return (data.pages[0]?.data.items ?? []) as readonly unknown[];
  } finally {
    signal.removeEventListener("abort", cancel);
  }
};

/**
 * The objects found for `text`, in the order of the collections, for whom holds `has`;
 * aborted by `signal` (then the answer is not read).
 */
export async function searchObjects(
  input: string,
  has: (permission: string) => boolean,
  signal: AbortSignal,
  read: SearchRead = cacheRead,
): Promise<FoundObject[]> {
  const q = input.trim().slice(0, MAX_TEXT);
  if (q === "") return [];
  const value = looksLikeValue(q);
  const searched = SEARCHED.filter((s) => has(s.read) && (value || !s.byValue));
  const results = await Promise.all(
    searched.map(async (s) => {
      const items = await read(s.path, q, signal).catch(() => [] as readonly unknown[]);
      return items.slice(0, FOUND_PER_TYPE).flatMap((raw): FoundObject[] => {
        const item = raw as Record<string, unknown>;
        const links = item.links as { self?: unknown } | undefined;
        const to = pageOf(links?.self ?? item.href);
        if (to === null || typeof item.id !== "string") return [];
        return [{ type: s.type, id: item.id, icon: s.icon, to, ...s.show(item) }];
      });
    }),
  );
  return results.flat().slice(0, FOUND_SHOWN);
}
