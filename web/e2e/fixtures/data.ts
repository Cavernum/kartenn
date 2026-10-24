/**
 * A role of the journeys, defined by global-setup.ts: editing addresses without applying
 * them (spec 024) and reading the changesets (a proposal of Bob validated by Alice,
 * REQ-007-18).
 */
export const CONTRIBUTOR = {
  key: "contributor",
  label: "Contributeur",
  entries: ["core.address.edit", "core.prefix.edit", "core.changeset.read"],
};

/**
 * The path of a write of the journeys: applied by its author when allowed (`apply=own`,
 * spec 024, REQ-024-13), pending otherwise.
 */
export function appliedByAuthor(method: string, path: string): string {
  if (["GET", "HEAD"].includes(method.toUpperCase()) || /[?&]apply=/.test(path)) return path;
  return `${path}${path.includes("?") ? "&" : "?"}apply=own`;
}

/** Roles bound to the groups of users.toml, on the whole organisation. */
export const GROUPS: Record<string, readonly string[]> = {
  "kartenn-admins": ["administrator"],
  lecteurs: ["reader", CONTRIBUTOR.key],
};

/** The data set of the journeys, loaded by global-setup.ts. */
export const DATA = {
  entity: "exemple",
  /** The entity the group `redacteurs` may edit, and nothing else (task 19). */
  confined: "redaction",
  domain: "production",
  prefixes: [
    ["10.0.0.0/8", "container"],
    ["10.1.0.0/24", "network"],
  ] as const,
  addresses: ["10.1.0.10"],
  /** A network with 1 000 addresses: a long list (REQ-007-31). */
  bulk: { prefix: "10.2.0.0/22", count: 1000 },
  /** A container with 10⁴ direct children: the virtualised tree (REQ-007-31). */
  wide: { prefix: "10.3.0.0/16", count: 10_000 },
};

/** The children of the wide container: /30 networks, 10.3.0.0/30 onwards. */
export function widePrefixes(): string[] {
  return Array.from({ length: DATA.wide.count }, (_, i) => {
    const n = i * 4;
    return `10.3.${Math.floor(n / 256)}.${n % 256}/30`;
  });
}

/** The addresses of the bulk network: 10.2.0.1 onwards. */
export function bulkAddresses(): string[] {
  return Array.from({ length: DATA.bulk.count }, (_, i) => {
    const n = i + 1;
    return `10.2.${Math.floor(n / 256)}.${n % 256}`;
  });
}
