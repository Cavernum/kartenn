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

/**
 * The counters of the home dashboard, in the shape of `GET /v1/stats` (docs/spec
 * « Statistiques »): readable objects by qualified kind, changesets applied over 7 and 30
 * calendar days and by UTC day (oldest first, today last), pending changesets, open
 * anomalies, counters of the modules, and the instant of the snapshot.
 */
export const STATS = {
  objects: {
    "core.domain": 4,
    "core.prefix": 1208,
    "core.range": 342,
    "core.address": 18455,
    "core.entity": 64,
    "core.site": 9,
  },
  changesets: {
    applied_7d: 38,
    applied_30d: 141,
    by_day: [
      4, 6, 3, 0, 0, 5, 7, 4, 6, 2, 0, 0, 8, 5, 3, 6, 4, 0, 0, 7, 9, 5, 3, 6, 0, 0, 8, 6, 11, 7,
    ],
  },
  pending: { to_apply: 3, mine: 2 },
  anomalies: { open: 12 },
  modules: {
    vlan: { reserved: 37, deprecated: 5 },
    flow: { noncompliant: 4, waivers_expiring: 2 },
  },
  /** Two minutes before the frozen clock of the captures. */
  computed_at: "2026-01-15T08:58:00Z",
} as const;

/** The labels of the qualified kinds counted on the dashboard. */
export const KIND_LABELS: Record<keyof typeof STATS.objects, string> = {
  "core.domain": "Domaines",
  "core.prefix": "Préfixes",
  "core.range": "Plages",
  "core.address": "Adresses",
  "core.entity": "Entités",
  "core.site": "Sites",
};

/** An operation of the active draft, as its tab lists it. */
export interface DraftOperation {
  key: string;
  what: string;
  /** The value before, empty for a creation (`after` is then its summary). */
  before: string;
  after: string;
}

/** The operations of the active draft (a frame that counts n shows the first n). */
export const DRAFT_OPERATIONS: DraftOperation[] = [
  { key: "192.0.2.11", what: "Usage", before: "Hôte", after: "VIP" },
  { key: "192.0.2.12", what: "Propriétaire", before: "Atelier Nord", after: "Atelier Sud" },
  { key: "192.0.2.40", what: "Créer l'adresse", before: "", after: "Hôte, Service Pilote" },
];

/** The drafts of the user (`GET /v1/drafts`), most recent first. */
export const DRAFTS = [
  {
    id: "01JF2A7C4D5E6F7G8H9J0K1M2N",
    title: "Bascule des VIP de la salle B",
    operations: 3,
    updated: "2026-01-15 09:41",
    expires: "",
  },
  {
    id: "01JE8R3T5V7W9X1Y3Z5A7B9C1D",
    title: "Nettoyage des réservations",
    operations: 12,
    updated: "2026-01-12 16:05",
    expires: "",
  },
  {
    id: "01JC1M4P7Q2R5S8T1U4V7W0X3Y",
    title: "Plan d'adressage de la salle C",
    operations: 41,
    updated: "2025-10-22 11:30",
    expires: "2026-01-20",
  },
];
