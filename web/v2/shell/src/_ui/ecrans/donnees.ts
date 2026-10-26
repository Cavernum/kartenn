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
  /** The environment of the administration interfaces (E11). */
  admin: { value: "admin", label: "Administration", hue: 6 },
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

/**
 * An identity (005): a person known by an IdP, or the break-glass account. Its card adds
 * `last_login_idp_groups` (006; 025, Q-025-1): the groups of the IdP at its last login,
 * overwritten at each login, without its date (not kept: it would track activity, 005/K3
 * amended); empty for an identity that never logged in through an IdP.
 */
export interface AdminIdentity extends Record<string, unknown> {
  key: string;
  name: string;
  issuer: string;
  state: "active" | "disabled";
  groups: number;
  lastLoginIdpGroups: string[];
}
const identity = (
  key: string,
  name: string,
  issuer: string,
  state: "active" | "disabled",
  groups: number,
  lastLoginIdpGroups: string[],
): AdminIdentity => ({ key, name, issuer, state, groups, lastLoginIdpGroups });
export const IDENTITIES: AdminIdentity[] = [
  identity("camille.martin", "Camille Martin", ISSUER, "active", 2, ["reseau", "tous-agents"]),
  identity("lou.bernard", "Lou Bernard", ISSUER, "active", 3, [
    "astreinte",
    "supervision-n1",
    "tous-agents",
  ]),
  identity("alex.morel", "Alex Morel", ISSUER, "active", 1, ["tous-agents"]),
  identity("sam.leroy", "Sam Leroy", ISSUER, "disabled", 1, ["tous-agents"]),
  identity("noa.petit", "Noa Petit", ISSUER, "active", 0, []),
  identity("secours", "secours", "local", "active", 1, []),
];
/** The kartenn group that an IdP group gives, if any (for the card of an identity). */
export const IDP_GROUP_TARGETS: Record<string, string> = {
  astreinte: "Astreinte",
  "supervision-n1": "Équipe supervision",
  reseau: "Équipe réseau",
};

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

// Flows (E11; specs 010 and 011): flow types, services, machines and their addresses with
// the values of the axes the policy cites, the rules of the policy (the matrix of each
// flow type is read from them), and the register of flows.

/** The flow types (010, REQ-010-4: the example set), key and label. */
export const FLOW_TYPE = {
  application: "Applicatif",
  admin: "Administratif",
  database: "Base de données",
  file: "Fichier",
} as const;
export type FlowTypeKey = keyof typeof FLOW_TYPE;

/** A service (D29): one flow type and its transport/port pairs. */
export interface FlowService {
  key: string;
  label: string;
  flowType: FlowTypeKey;
  ports: string[];
}
export const SERVICES: Record<string, FlowService> = {
  https: { key: "https", label: "Web chiffré", flowType: "application", ports: ["tcp/443"] },
  api: { key: "api", label: "API interne", flowType: "application", ports: ["tcp/8443"] },
  ssh: { key: "ssh", label: "Shell sécurisé", flowType: "admin", ports: ["tcp/22"] },
  postgres: { key: "postgres", label: "PostgreSQL", flowType: "database", ports: ["tcp/5432"] },
  nfs: { key: "nfs", label: "Partage NFS", flowType: "file", ports: ["tcp/2049", "udp/2049"] },
};

/**
 * An address and the values of the axes cited by the policy, with their origin
 * (REQ-010-12): the environment (`policy.environment`, null: none, the address is
 * unclassified) and the sensitivity.
 */
export interface FlowAddress {
  value: string;
  env: EnvKey | null;
  envFrom: string;
  sensitivity: string;
  sensitivityFrom: string;
}
/** An interface of a machine (module Machines) and its role. */
export interface FlowInterface {
  name: string;
  role: string;
  addresses: FlowAddress[];
}
/** A machine: declared environment, site, owner, interfaces. */
export interface FlowMachine {
  key: string;
  env: EnvKey | null;
  site: string;
  owner: string;
  system: string;
  interfaces: FlowInterface[];
}
const addr = (
  value: string,
  env: EnvKey | null,
  envFrom: string,
  sensitivity = "Interne",
  sensitivityFrom = "héritée de 192.0.2.0/24",
): FlowAddress => ({ value, env, envFrom, sensitivity, sensitivityFrom });
const iface = (name: string, role: string, ...addresses: FlowAddress[]): FlowInterface => ({
  name,
  role,
  addresses,
});
const machine = (
  key: string,
  env: EnvKey | null,
  site: string,
  system: string,
  ...interfaces: FlowInterface[]
): FlowMachine => ({ key, env, site, owner: "Atelier Nord", system, interfaces });
const FROM_SERVERS = "héritée de 192.0.2.0/26";
const FROM_ADMIN = "posée sur 198.51.100.0/28";
export const MACHINES: Record<string, FlowMachine> = Object.fromEntries(
  [
    machine(
      "srv-app-01",
      "prod",
      "site-ouest",
      "Linux",
      iface(
        "eth0",
        "Service",
        addr("192.0.2.10", "prod", FROM_SERVERS),
        addr("192.0.2.14", "prod", FROM_SERVERS),
      ),
      iface("eth1", "Administration", addr("198.51.100.5", "admin", FROM_ADMIN)),
    ),
    machine(
      "srv-web-02",
      "prod",
      "site-ouest",
      "Linux",
      iface(
        "eth0",
        "Service",
        addr("192.0.2.20", "prod", FROM_SERVERS),
        addr("192.0.2.21", "prod", FROM_SERVERS),
      ),
    ),
    machine(
      "srv-db-02",
      "prod",
      "site-ouest",
      "Linux",
      iface(
        "eth0",
        "Service",
        addr("192.0.2.30", "prod", FROM_SERVERS, "Confidentiel", "posée sur l'adresse"),
      ),
    ),
    machine(
      "bastion-01",
      "admin",
      "site-ouest",
      "Linux",
      iface("eth0", "Administration", addr("198.51.100.2", "admin", FROM_ADMIN)),
    ),
    machine(
      "srv-sauv-01",
      "prod",
      "site-est",
      "Linux",
      iface("eth0", "Sauvegarde", addr("198.51.100.40", "prod", "posée sur 198.51.100.32/28")),
    ),
    machine(
      "srv-app-07",
      "prod",
      "site-est",
      "Linux",
      iface("eth0", "Service", addr("192.0.2.72", "prod", "héritée de 192.0.2.64/26")),
    ),
    machine(
      "srv-rec-03",
      "rece",
      "site-ouest",
      "Linux",
      iface("eth0", "Service", addr("192.0.2.12", "rece", "posée sur l'adresse")),
    ),
    machine(
      "srv-rec-04",
      "rece",
      "site-ouest",
      "Linux",
      iface("eth0", "Service", addr("192.0.2.13", "rece", "posée sur l'adresse")),
    ),
    machine(
      "dev-web-01",
      "deve",
      "site-est",
      "Linux",
      iface("eth0", "Service", addr("192.0.2.70", "deve", "posée sur l'adresse")),
    ),
    machine(
      "srv-new-05",
      null,
      "site-est",
      "Linux",
      iface(
        "eth0",
        "Service",
        addr("203.0.113.50", null, "aucune valeur, ni posée ni héritée", "—", "aucune valeur"),
      ),
    ),
  ].map((m) => [m.key, m]),
);

/**
 * An end of a flow (REQ-011-2, 011/F21): an address, an interface, a prefix; never a
 * machine. The machine is only the context of an interface (or of an address it carries).
 */
export type FlowEnd =
  | { kind: "address"; value: string }
  | { kind: "interface"; machine: string; iface: string }
  | { kind: "prefix"; value: string; address: FlowAddress };

/** The prefix of the supervision (E2), an end of a flow evaluated on its own values. */
export const SUPERVISION_PREFIX: FlowEnd = {
  kind: "prefix",
  value: "192.0.2.64/26",
  address: addr("192.0.2.64/26", "prod", "posée sur le préfixe"),
};
/** The prefix of the web front ends, the source of a flow that covers their addresses. */
export const FRONT_PREFIX: FlowEnd = {
  kind: "prefix",
  value: "192.0.2.16/28",
  address: addr("192.0.2.16/28", "prod", "posée sur le préfixe"),
};

/**
 * A rule of the policy (010): effect, flow types, environments of source and destination,
 * the pattern a conditional `allow` waits for, its pedagogical text. The matrix of a flow
 * type is read from them: `deny` wins, then an `allow` without pattern, then one with.
 */
export interface FlowRule {
  key: string;
  effect: "allow" | "deny";
  flowTypes: FlowTypeKey[];
  sources: EnvKey[];
  /** Empty: the environment of the source (« même environnement »). */
  destinations: EnvKey[];
  pattern?: string;
  text: string;
}
const ALL_TYPES: FlowTypeKey[] = ["application", "admin", "database", "file"];
export const RULES: FlowRule[] = [
  {
    key: "app-meme-env",
    effect: "allow",
    flowTypes: ["application"],
    sources: ["prod", "rece", "deve"],
    destinations: [],
    text: "Les applications d'un même environnement se parlent par leurs services déclarés.",
  },
  {
    key: "rece-vers-prod",
    effect: "allow",
    flowTypes: ["application"],
    sources: ["rece"],
    destinations: ["prod"],
    pattern: "api-lecture",
    text: "La recette lit les API de production pour ses jeux de contrôle, sous le motif api-lecture.",
  },
  {
    key: "deny-deve-prod",
    effect: "deny",
    flowTypes: ALL_TYPES,
    sources: ["deve"],
    destinations: ["prod"],
    text: "Le développement n'atteint jamais la production : ses données n'en sortent pas, ses outils n'y entrent pas.",
  },
  {
    key: "deny-prod-hors-prod",
    effect: "deny",
    flowTypes: ["application", "database", "file"],
    sources: ["prod"],
    destinations: ["rece", "deve"],
    text: "La production ne pousse rien vers la recette ni le développement.",
  },
  {
    key: "admin-par-bastion",
    effect: "allow",
    flowTypes: ["admin"],
    sources: ["admin"],
    destinations: ["prod", "rece", "deve", "admin"],
    text: "L'administration part du bastion, vers tout environnement.",
  },
  {
    key: "deny-admin-hors-bastion",
    effect: "deny",
    flowTypes: ["admin"],
    sources: ["prod", "rece", "deve"],
    destinations: ["prod", "rece", "deve"],
    text: "L'administration ne part que du bastion : une machine n'en administre pas une autre.",
  },
  {
    key: "bdd-meme-env",
    effect: "allow",
    flowTypes: ["database"],
    sources: ["prod", "rece", "deve"],
    destinations: [],
    text: "Une base n'est lue que depuis son propre environnement.",
  },
  {
    key: "deny-rece-vers-prod-bdd",
    effect: "deny",
    flowTypes: ["database", "file"],
    sources: ["rece"],
    destinations: ["prod"],
    text: "La recette ne lit pas les données de production : elle passe par les API.",
  },
  {
    key: "fichier-prod",
    effect: "allow",
    flowTypes: ["file"],
    sources: ["prod"],
    destinations: ["prod"],
    text: "Les sauvegardes et partages de la production restent en production.",
  },
];

/** A condition of eligibility (REQ-010-36): key and pedagogical text. */
export const CONDITION_JUSTIFICATION = {
  key: "justification-requise",
  text: "Un flux entre deux environnements différents porte une justification dans son dossier.",
} as const;

/** The state of a flow (REQ-011-5) shown by the register. */
export type FlowState = "requested" | "active" | "noncompliant" | "waived" | "expired";

/** A flow of the register (REQ-011-1): its ends, its service, its state, its term. */
export interface RegisteredFlow {
  key: string;
  source: FlowEnd;
  destination: FlowEnd;
  service: string;
  state: FlowState;
  reference: string;
  /** The end of its waiver (REQ-011-9), if any. */
  waiver?: string;
  /** The end of the flow, if dated. */
  end?: string;
}
/** An interface of a machine as an end. */
export const ifaceEnd = (machine: string, iface: string): FlowEnd => ({
  kind: "interface",
  machine,
  iface,
});
/** An address as an end. */
export const addressEnd = (value: string): FlowEnd => ({ kind: "address", value });
const i = ifaceEnd;
export const FLOWS: RegisteredFlow[] = [
  {
    key: "flux-1041",
    source: i("srv-web-02", "eth0"),
    destination: i("srv-app-01", "eth0"),
    service: "api",
    state: "active",
    reference: "ticketing:CHG-1041",
  },
  {
    key: "flux-1046",
    source: FRONT_PREFIX,
    destination: i("srv-app-01", "eth0"),
    service: "https",
    state: "active",
    reference: "ticketing:CHG-1046",
  },
  {
    key: "flux-1042",
    source: i("srv-app-01", "eth0"),
    destination: addressEnd("192.0.2.30"),
    service: "postgres",
    state: "active",
    reference: "ticketing:CHG-1042",
  },
  {
    key: "flux-1043",
    source: i("bastion-01", "eth0"),
    destination: i("srv-app-01", "eth1"),
    service: "ssh",
    state: "active",
    reference: "ticketing:CHG-1043",
    end: "2026-06-30",
  },
  {
    key: "flux-1050",
    source: i("srv-rec-03", "eth0"),
    destination: i("srv-app-01", "eth0"),
    service: "https",
    state: "requested",
    reference: "ticketing:CHG-1050",
  },
  {
    key: "flux-1037",
    source: i("dev-web-01", "eth0"),
    destination: i("srv-db-02", "eth0"),
    service: "postgres",
    state: "waived",
    reference: "ticketing:CHG-1037",
    waiver: "2026-03-31",
    end: "2026-03-31",
  },
  {
    key: "flux-1044",
    source: i("srv-app-01", "eth0"),
    destination: addressEnd("198.51.100.40"),
    service: "nfs",
    state: "active",
    reference: "ticketing:CHG-1044",
  },
  {
    key: "flux-1029",
    source: i("srv-app-07", "eth0"),
    destination: SUPERVISION_PREFIX,
    service: "ssh",
    state: "noncompliant",
    reference: "ticketing:CHG-1029",
  },
  {
    key: "flux-0988",
    source: i("srv-rec-04", "eth0"),
    destination: i("srv-app-01", "eth0"),
    service: "https",
    state: "expired",
    reference: "ticketing:CHG-0988",
    end: "2025-12-31",
  },
];
