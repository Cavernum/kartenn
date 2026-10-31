import { queryClient, reads, type ListPath } from "@kartenn/sdk";
import type { IconName } from "@kartenn/ui";
import type { FoundObject } from "./recent.ts";

// The search of the palette by value and by name (REQ-021-3; after the global search of the
// 007, web/shell SearchView.vue): the text is the start of a value (`q`, « 10.1. ») for the
// prefixes and the addresses when it looks like one, of a slug or a name for the other
// collections; each collection the user reads, through the cache of the SDK (a text typed
// again is read from it), at the dates of the page. A collection that fails is left out:
// the palette offers what it found.

interface Searched {
  type: string;
  path: ListPath;
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

/** Reads the first page of a collection for `q` (the cache of the SDK, the page's dates). */
export type SearchRead = (path: ListPath, q: string) => Promise<readonly unknown[]>;

const cacheRead: SearchRead = async (path, q) => {
  const data = await queryClient.fetchInfiniteQuery(reads.list(path, { q } as never));
  return (data.pages[0]?.data.items ?? []) as readonly unknown[];
};

/** The objects found for `text`, in the order of the collections, for whom holds `has`. */
export async function searchObjects(
  input: string,
  has: (permission: string) => boolean,
  read: SearchRead = cacheRead,
): Promise<FoundObject[]> {
  const q = input.trim().slice(0, MAX_TEXT);
  if (q === "") return [];
  const value = looksLikeValue(q);
  const searched = SEARCHED.filter((s) => has(s.read) && (value || !s.byValue));
  const results = await Promise.all(
    searched.map(async (s) => {
      const items = await read(s.path, q).catch(() => [] as readonly unknown[]);
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
