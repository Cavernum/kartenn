/** Roles bound to the groups of users.toml, on the whole organisation. */
export const GROUPS: Record<string, string> = {
  "kartenn-admins": "administrator",
  lecteurs: "reader",
};

/** The data set of the journeys, loaded by global-setup.ts. */
export const DATA = {
  entity: "exemple",
  domain: "production",
  prefixes: [
    ["10.0.0.0/8", "container"],
    ["10.1.0.0/24", "network"],
  ] as const,
  addresses: ["10.1.0.10"],
  /** A network with 1 000 addresses: a long list (REQ-007-31). */
  bulk: { prefix: "10.2.0.0/22", count: 1000 },
};

/** The addresses of the bulk network: 10.2.0.1 onwards. */
export function bulkAddresses(): string[] {
  return Array.from({ length: DATA.bulk.count }, (_, i) => {
    const n = i + 1;
    return `10.2.${Math.floor(n / 256)}.${n % 256}`;
  });
}
