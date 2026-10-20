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
};
