/**
 * Fictitious data shared by the mock-ups: documentation blocks (RFC 5737, RFC 3849),
 * reserved names (RFC 2606), invented teams, sites and machines, problem codes of the specs
 * of kartenn. No link with any real information system.
 */

/** Environments (REQ-020-8): key, label, hue of the closed palette. */
export const ENV = {
  prod: { value: "prod", label: "Production", hue: 1 },
  rece: { value: "rece", label: "Recette", hue: 2 },
  deve: { value: "deve", label: "Développement", hue: 3 },
} as const;
export type EnvKey = keyof typeof ENV;

/** Usages of an address or a range (closed lists of the contract), with their labels. */
export const USAGE: Record<string, string> = {
  host: "Hôte",
  gateway: "Passerelle",
  vip: "VIP",
  reserved: "Réservée",
  dhcp: "DHCP",
  static: "Statique",
};

/** The label of a usage, or null (the key is then shown, REQ-020-21). */
export const usageLabel = (key: string): string | null => USAGE[key] ?? null;

/** The domain read by the mock-ups. */
export const DOMAIN = { key: "dc-nord", label: "Centre de données nord" } as const;

/** The fixed instant of the past readings (the captures freeze the clock at 2026-01-15). */
export const PAST = "2025-11-03T09:30:00+01:00";

/** Identifiers in the shape of the API (ULID), invented. */
export const ID = {
  changeset: "01JD4K8B2C3D4E5F6G7H8J9K0M",
  request: "req_7f3a9c2e5b1d4068",
  prefix: "01JD4K7Q9V2M8X3N5P6R7S8T9W",
} as const;
