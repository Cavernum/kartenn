import type { IconName, SidebarGroup } from "@kartenn/ui";
import type { Rights } from "../rights/index.ts";

// The spaces of the sidebar (REQ-021-2; design 021 « Cadre », « Catalogue des écrans »),
// in their closed order: `referential`, `changes`, `network`, `hosts`, `policy`, `flows`,
// `admin`. The core fills four; `hosts`, `policy` and `flows` come from the manifests of
// the plugins (task 16). An entry shows when one of `anyOf` is held: a hint only, the
// server stays the judge; a space left without entries is hidden (SidebarNav omits it).

/** The order of the spaces (REQ-021-2), the plugins' ones included. */
export const SPACE_ORDER = [
  "referential",
  "changes",
  "network",
  "hosts",
  "policy",
  "flows",
  "admin",
] as const;
export type SpaceId = (typeof SPACE_ORDER)[number];

export interface SpaceEntry {
  id: string;
  /** The key of its label in the catalogs of the shell (`nav.<id>`). */
  label: string;
  icon: IconName;
  to: string;
  /** Shown when one of these is held; always when unset. */
  anyOf?: readonly string[];
}

export interface Space {
  id: SpaceId;
  /** The key of its label (`space.<id>`). */
  label: string;
  entries: readonly SpaceEntry[];
}

/** The reads of the referential (the reader's role, 005). */
const REFERENTIAL_READS = [
  "core.domain.read",
  "core.prefix.read",
  "core.range.read",
  "core.address.read",
  "core.entity.read",
  "core.site.read",
] as const;

/**
 * What gives drafts (crates/kartenn-core/src/authz.rs): the edition of an object of the
 * core or of a property (`prop.<key>.edit`, any key), or the administration of the drafts
 * of others.
 */
export const EDITS = [
  "core.entity.edit",
  "core.site.edit",
  "core.domain.edit",
  "core.prefix.edit",
  "core.range.edit",
  "core.address.edit",
  "core.vlan_group.edit",
  "core.vlan.edit",
  "prop.*.edit",
  "core.draft.admin",
] as const;

const RBAC = ["core.rbac.read", "core.rbac.admin"] as const;

/** The spaces of the core and their entries. */
export const CORE_SPACES: readonly Space[] = [
  {
    id: "referential",
    label: "space.referential",
    entries: [
      { id: "home", label: "nav.home", icon: "house", to: "/", anyOf: REFERENTIAL_READS },
      {
        id: "topology",
        label: "nav.topology",
        icon: "folder-tree",
        to: "/topology",
        anyOf: ["core.prefix.read"],
      },
      {
        id: "domains",
        label: "nav.domains",
        icon: "domain",
        to: "/domains",
        anyOf: ["core.domain.read"],
      },
      {
        id: "prefixes",
        label: "nav.prefixes",
        icon: "prefix",
        to: "/prefixes",
        anyOf: ["core.prefix.read"],
      },
      {
        id: "ranges",
        label: "nav.ranges",
        icon: "range",
        to: "/ranges",
        anyOf: ["core.range.read"],
      },
      {
        id: "addresses",
        label: "nav.addresses",
        icon: "address",
        to: "/addresses",
        anyOf: ["core.address.read"],
      },
      {
        id: "entities",
        label: "nav.entities",
        icon: "users",
        to: "/entities",
        anyOf: ["core.entity.read"],
      },
      { id: "sites", label: "nav.sites", icon: "globe", to: "/sites", anyOf: ["core.site.read"] },
      {
        id: "anomalies",
        label: "nav.anomalies",
        icon: "triangle-alert",
        to: "/anomalies",
        anyOf: REFERENTIAL_READS,
      },
      {
        id: "properties",
        label: "nav.properties",
        icon: "tag",
        to: "/property-declarations",
        anyOf: REFERENTIAL_READS,
      },
    ],
  },
  {
    id: "changes",
    label: "space.changes",
    entries: [
      {
        id: "changesets",
        label: "nav.changesets",
        icon: "changeset",
        to: "/changesets",
        anyOf: ["core.changeset.read"],
      },
      {
        id: "proposals",
        label: "nav.proposals",
        icon: "git-pull-request",
        to: "/proposals",
        anyOf: ["core.changeset.read"],
      },
      { id: "drafts", label: "nav.drafts", icon: "draft", to: "/drafts", anyOf: EDITS },
      {
        id: "imports",
        label: "nav.imports",
        icon: "clipboard-paste",
        to: "/imports",
        anyOf: EDITS,
      },
    ],
  },
  {
    id: "network",
    label: "space.network",
    entries: [
      { id: "vlans", label: "nav.vlans", icon: "vlan", to: "/vlans", anyOf: ["core.vlan.read"] },
      {
        id: "vlan-groups",
        label: "nav.vlanGroups",
        icon: "layers",
        to: "/vlan-groups",
        anyOf: ["core.vlan_group.read"],
      },
    ],
  },
  {
    id: "admin",
    label: "space.admin",
    entries: [
      { id: "groups", label: "nav.groups", icon: "users", to: "/admin/groups", anyOf: RBAC },
      {
        id: "roles",
        label: "nav.roles",
        icon: "key-round",
        to: "/admin/roles",
        anyOf: [...RBAC, "core.role.admin"],
      },
      {
        id: "identities",
        label: "nav.identities",
        icon: "user",
        to: "/admin/identities",
        anyOf: [...RBAC, "core.account.admin"],
      },
      {
        id: "accounts",
        label: "nav.accounts",
        icon: "server",
        to: "/admin/accounts",
        anyOf: ["core.account.admin", "core.rbac.read"],
      },
      {
        id: "issuers",
        label: "nav.issuers",
        icon: "globe",
        to: "/admin/issuers",
        anyOf: ["core.issuer.admin", "core.rbac.read"],
      },
      {
        id: "settings",
        label: "nav.settings",
        icon: "settings",
        to: "/admin/settings",
        anyOf: ["core.settings.edit"],
      },
      {
        id: "audit",
        label: "nav.audit",
        icon: "rotate-ccw-clock",
        to: "/admin/audit",
        anyOf: ["core.audit.read"],
      },
      {
        id: "observability",
        label: "nav.observability",
        icon: "eye",
        to: "/admin/observability",
        anyOf: ["core.observability.admin"],
      },
    ],
  },
];

/** Whether `rights` may see `entry`. */
export function visible(entry: SpaceEntry, rights: Rights): boolean {
  return !entry.anyOf || rights.hasAny(entry.anyOf);
}

/**
 * Where a space leads (its crumb, `/admin`): its first entry `rights` may see, else its
 * first entry (the server answers there).
 */
export function spaceEntry(space: Space, rights: Rights): SpaceEntry {
  return space.entries.find((entry) => visible(entry, rights)) ?? space.entries[0]!;
}

/**
 * The groups of the sidebar for `rights`, labels in the language of `t`, in the order of
 * the spaces; a space left without entries is dropped.
 */
export function sidebarGroups(
  rights: Rights,
  t: (key: string) => string,
  spaces: readonly Space[] = CORE_SPACES,
): SidebarGroup[] {
  return [...spaces]
    .sort((a, b) => SPACE_ORDER.indexOf(a.id) - SPACE_ORDER.indexOf(b.id))
    .map((space) => ({
      id: space.id,
      label: t(space.label),
      entries: space.entries
        .filter((entry) => visible(entry, rights))
        .map((entry) => ({ id: entry.id, label: t(entry.label), icon: entry.icon, to: entry.to })),
    }))
    .filter((group) => group.entries.length > 0);
}

/** The space and the entry of a path: the entry whose `to` is the longest prefix of it. */
export function locate(
  path: string,
  spaces: readonly Space[] = CORE_SPACES,
): { space: Space; entry: SpaceEntry } | null {
  let found: { space: Space; entry: SpaceEntry } | null = null;
  for (const space of spaces) {
    for (const entry of space.entries) {
      const hit =
        entry.to === "/" ? path === "/" : path === entry.to || path.startsWith(`${entry.to}/`);
      if (hit && (!found || entry.to.length > found.entry.to.length)) found = { space, entry };
    }
  }
  return found;
}
