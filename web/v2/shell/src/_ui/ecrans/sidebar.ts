import type { SidebarGroup } from "@kartenn/ui";

/**
 * The spaces of the sidebar (spec 021, « Cadre » and « Catalogue des écrans »), in their
 * closed order; entries reached by the palette only are left out. Counters are fictitious.
 */
export const SPACES: SidebarGroup[] = [
  {
    id: "referential",
    label: "Référentiel",
    entries: [
      { id: "home", label: "Accueil", icon: "house", to: "/" },
      { id: "topology", label: "Topographie", icon: "folder-tree", to: "/topology" },
      { id: "vlans", label: "VLAN", icon: "vlan", to: "/vlans" },
      { id: "domains", label: "Domaines", icon: "domain", to: "/domains" },
      { id: "prefixes", label: "Préfixes", icon: "prefix", to: "/prefixes" },
      { id: "ranges", label: "Plages", icon: "range", to: "/ranges" },
      { id: "addresses", label: "Adresses", icon: "address", to: "/addresses" },
      { id: "entities", label: "Entités", icon: "users", to: "/entities" },
      { id: "sites", label: "Sites", icon: "globe", to: "/sites" },
      { id: "anomalies", label: "Anomalies", icon: "triangle-alert", to: "/anomalies", badge: 12 },
    ],
  },
  {
    id: "changes",
    label: "Changesets",
    entries: [
      { id: "changesets", label: "Changesets", icon: "changeset", to: "/changesets" },
      {
        id: "proposals",
        label: "À instruire",
        icon: "git-pull-request",
        to: "/proposals",
        badge: 3,
      },
      { id: "draft", label: "Mes brouillons", icon: "draft", to: "/drafts", badge: 3 },
    ],
  },
  {
    id: "hosts",
    label: "Machines",
    entries: [
      { id: "machines", label: "Machines", icon: "server", to: "/x/machines/machines" },
      { id: "clusters", label: "Hébergement", icon: "cluster", to: "/x/machines/clusters" },
      { id: "interfaces", label: "Interfaces", icon: "interface", to: "/x/machines/interfaces" },
    ],
  },
  {
    id: "policy",
    label: "Politique",
    entries: [
      { id: "matrix", label: "Matrice", icon: "matrix", to: "/x/policy/matrix" },
      { id: "rules", label: "Règles et motifs", icon: "shield", to: "/x/policy/rules" },
      { id: "inference", label: "Inférence", icon: "git-compare", to: "/x/policy/inference" },
      {
        id: "compliance",
        label: "Conformité",
        icon: "clipboard-check",
        to: "/x/policy/compliance",
      },
    ],
  },
  {
    id: "flows",
    label: "Flux",
    entries: [
      { id: "flows", label: "Flux", icon: "flow", to: "/x/flow/flows" },
      { id: "flow-review", label: "Instruction", icon: "send", to: "/x/flow/review", badge: 5 },
    ],
  },
  {
    id: "admin",
    label: "Administration",
    entries: [
      { id: "groups", label: "Groupes", icon: "users", to: "/admin/groups" },
      { id: "roles", label: "Rôles", icon: "key-round", to: "/admin/roles" },
      { id: "identities", label: "Identités", icon: "user", to: "/admin/identities" },
      { id: "accounts", label: "Comptes de service", icon: "server", to: "/admin/accounts" },
      { id: "issuers", label: "Émetteurs", icon: "globe", to: "/admin/issuers" },
      { id: "audit", label: "Audit", icon: "rotate-ccw-clock", to: "/admin/audit" },
    ],
  },
];

/** Every space but `open` starts closed: the sidebar stays short, the active space shows. */
export const closedBut = (open: string): string[] =>
  SPACES.map((space) => space.id).filter((id) => id !== open);
