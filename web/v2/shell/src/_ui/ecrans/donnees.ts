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
    /** The occupation of each group, in percent of its range (as E10 shows them). */
    vlan: {
      reserved: 37,
      deprecated: 5,
      groups: [
        { key: "campus-ouest", used: 87 },
        { key: "campus-est", used: 41 },
        { key: "labo", used: 6 },
      ],
    },
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

// Administration (E7; spec 025): rights go to kartenn groups only; a group holds members
// (identities, groups served by an IdP) and roles on scopes. The user of the mock-ups
// administers the entity « Atelier Nord », not the whole referential.

/** The issuer of the identities of the mock-ups (an invented IdP). */
export const ISSUER = "idp-interne";

/** A role (005): key, label (or null: the key is shown), counts. */
export interface AdminRole extends Record<string, unknown> {
  key: string;
  label: string | null;
  builtin: boolean;
  permissions: number;
  groups: number;
  about: string;
}
const adminRole = (
  key: string,
  label: string | null,
  builtin: boolean,
  permissions: number,
  groups: number,
  about: string,
): AdminRole => ({ key, label, builtin, permissions, groups, about });
export const ROLES: AdminRole[] = [
  adminRole("reader", "Lecteur", true, 14, 6, "Lecture de tout le référentiel"),
  adminRole("editor", "Éditeur", true, 31, 3, "Édition du référentiel, sans l'administration"),
  adminRole("admin", "Administrateur", true, 61, 2, "Tous les droits"),
  adminRole("lecteur-reseau", "Lecteur réseau", false, 8, 4, "Lecture du réseau et des machines"),
  adminRole("instructeur-flux", "Instructeur", false, 3, 2, "Instruction des demandes de flux"),
  adminRole("import-ipam", null, false, 8, 1, "Compte d'import"),
];

/** The permissions of the role `lecteur-reseau`: served name (spec 018) and label. */
export const ROLE_PERMISSIONS = [
  { code: "core.prefix.read", label: "Lire les préfixes" },
  { code: "core.address.read", label: "Lire les adresses" },
  { code: "core.range.read", label: "Lire les plages" },
  { code: "core.site.read", label: "Lire les sites" },
  { code: "vlan.read", label: "Lire les VLAN" },
  { code: "vlan.group.read", label: "Lire les groupes de VLAN" },
  { code: "machines.read", label: "Lire les machines" },
  { code: "machines.interface.read", label: "Lire les interfaces" },
];
/** The groups bound to the role `lecteur-reseau`, with their scope (read in the role). */
export const ROLE_GROUPS = [
  { key: "astreinte", label: "Astreinte", scope: "Globale" },
  { key: "reseau", label: "Équipe réseau", scope: "Entité : Atelier Nord" },
  { key: "sauvegarde", label: "Équipe sauvegarde", scope: "Domaine : dc-nord" },
  { key: "supervision", label: "Équipe supervision", scope: "Entité : Atelier Nord" },
];

/** A kartenn group (025, G2): key, label, builtin (`secours`), counts. */
export interface AdminGroup extends Record<string, unknown> {
  key: string;
  label: string | null;
  builtin: boolean;
  members: number;
  roles: number;
  about: string;
}
const adminGroup = (
  key: string,
  label: string | null,
  builtin: boolean,
  members: number,
  roles: number,
  about: string,
): AdminGroup => ({ key, label, builtin, members, roles, about });
export const GROUPS: AdminGroup[] = [
  adminGroup("secours", "Secours", true, 1, 1, "Compte de secours, administrateur global"),
  adminGroup("supervision", "Équipe supervision", false, 5, 3, "Lecture du réseau, instruction"),
  adminGroup("reseau", "Équipe réseau", false, 12, 2, "Édition du réseau de l'atelier nord"),
  adminGroup("astreinte", "Astreinte", false, 3, 1, "Lecture de tout le référentiel"),
  adminGroup("sauvegarde", "Équipe sauvegarde", false, 4, 1, "Lecture des machines du domaine"),
  adminGroup("compte-sync-ipam", null, false, 1, 1, "Reporté par la migration, à regrouper"),
];

/** A member of a group: an identity (person, service account) or a group of an IdP. */
export type MemberKind = "person" | "service" | "breakglass" | "idp";
export const MEMBER_KIND: Record<MemberKind, string> = {
  person: "personne",
  service: "compte de service",
  breakglass: "compte de secours",
  idp: "groupe de l'IdP",
};
export interface GroupMember {
  kind: MemberKind;
  /** The name of the identity, or `émetteur / groupe` for a group of an IdP. */
  name: string;
  /** The end of a temporary membership (025, G2). */
  until?: string;
}
export const SUPERVISION_MEMBERS: GroupMember[] = [
  { kind: "idp", name: `${ISSUER} / supervision-n1` },
  { kind: "person", name: "Alex Morel" },
  { kind: "person", name: "Lou Bernard" },
  { kind: "person", name: "Sam Leroy", until: "2026-03-31" },
  { kind: "service", name: "export-dns" },
];
/** The member added in the draft of the view `groupe-edition`. */
export const SUPERVISION_NEW_MEMBER: GroupMember = {
  kind: "idp",
  name: `${ISSUER} / supervision-n2`,
};
export const ASTREINTE_MEMBERS: GroupMember[] = [
  { kind: "idp", name: `${ISSUER} / astreinte` },
  { kind: "person", name: "Camille Martin" },
  { kind: "service", name: "sync-ipam" },
];
export const SECOURS_MEMBERS: GroupMember[] = [
  { kind: "breakglass", name: "secours", until: "2026-04-15" },
];

/** A binding of a group: a role on a scope (005, K4 amended by 025, G1). */
export interface GroupBinding {
  role: string;
  roleLabel: string | null;
  scope: string;
  until?: string;
}
export const SUPERVISION_BINDINGS: GroupBinding[] = [
  { role: "lecteur-reseau", roleLabel: "Lecteur réseau", scope: "Entité : Atelier Nord" },
  { role: "instructeur-flux", roleLabel: "Instructeur", scope: "Site : site-ouest" },
  { role: "reader", roleLabel: "Lecteur", scope: "Domaine : dc-nord", until: "2026-06-30" },
];
/** The binding added in the draft of the view `groupe-edition`. */
export const SUPERVISION_NEW_BINDING: GroupBinding = {
  role: "editor",
  roleLabel: "Éditeur",
  scope: "Site : site-ouest",
};
export const ASTREINTE_BINDINGS: GroupBinding[] = [
  { role: "reader", roleLabel: "Lecteur", scope: "Globale" },
];
export const SECOURS_BINDINGS: GroupBinding[] = [
  { role: "admin", roleLabel: "Administrateur", scope: "Globale" },
];

/** The service accounts (005): state, live secrets, last use; their kartenn groups. */
export interface AdminAccount extends Record<string, unknown> {
  name: string;
  state: "active" | "disabled";
  groups: number;
  secrets: number;
  lastUse: string;
}
export const ACCOUNTS: AdminAccount[] = [
  { name: "sync-ipam", state: "active", groups: 2, secrets: 2, lastUse: "2026-01-15 09:58" },
  { name: "export-dns", state: "active", groups: 1, secrets: 1, lastUse: "2026-01-15 06:00" },
  { name: "audit-annuel", state: "disabled", groups: 0, secrets: 0, lastUse: "2025-12-31 23:10" },
];

/** An identity (005): a person known by an IdP, or the break-glass account. */
export interface AdminIdentity extends Record<string, unknown> {
  key: string;
  name: string;
  issuer: string;
  state: "active" | "disabled";
  groups: number;
  lastSession: string;
}
const identity = (
  key: string,
  name: string,
  issuer: string,
  state: "active" | "disabled",
  groups: number,
  lastSession: string,
): AdminIdentity => ({ key, name, issuer, state, groups, lastSession });
export const IDENTITIES: AdminIdentity[] = [
  identity("camille.martin", "Camille Martin", ISSUER, "active", 2, "2026-01-15 08:47"),
  identity("lou.bernard", "Lou Bernard", ISSUER, "active", 3, "2026-01-15 09:12"),
  identity("alex.morel", "Alex Morel", ISSUER, "active", 1, "2026-01-14 17:30"),
  identity("sam.leroy", "Sam Leroy", ISSUER, "disabled", 1, "2025-12-19 11:02"),
  identity("noa.petit", "Noa Petit", ISSUER, "active", 0, "—"),
  identity("secours", "secours", "local", "active", 1, "—"),
];

/** The groups of the chosen identity, and by which group of its token (null: directly). */
export const IDENTITY_GROUPS = [
  { key: "astreinte", label: "Astreinte", via: `${ISSUER} / astreinte` },
  { key: "reseau", label: "Équipe réseau", via: null },
  { key: "supervision", label: "Équipe supervision", via: `${ISSUER} / supervision-n1` },
];
/** Its effective rights: the bindings of its groups (computed, read only). */
export const IDENTITY_RIGHTS = [
  { role: "editor", roleLabel: "Éditeur", scope: "Entité : Atelier Nord", group: "Équipe réseau" },
  {
    role: "lecteur-reseau",
    roleLabel: "Lecteur réseau",
    scope: "Entité : Atelier Nord",
    group: "Équipe supervision",
  },
  {
    role: "instructeur-flux",
    roleLabel: "Instructeur",
    scope: "Site : site-ouest",
    group: "Équipe supervision",
  },
  { role: "reader", roleLabel: "Lecteur", scope: "Globale", group: "Astreinte" },
];
/** The groups served by the IdP in its last session (025, Q-025-1), and their kartenn group. */
export const IDENTITY_SESSION = {
  at: "2026-01-15 09:12",
  groups: [
    { name: `${ISSUER} / astreinte`, group: "Astreinte" },
    { name: `${ISSUER} / supervision-n1`, group: "Équipe supervision" },
    { name: `${ISSUER} / tous-agents`, group: null },
  ],
};
