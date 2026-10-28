<script setup lang="ts">
// E13 — Paramètres (027, REQ-027-5, 6, 8; P4 to P8): one screen of the Administration
// space, one tab per declarant (Cœur, Machines, Politique) that lists its settings. The
// core: its fields and the `custom.*` ones (key, labels, type, carriers, inheritance,
// sensitivity) and its closed lists, `core.external_system` opened as a table-form (key
// frozen after creation, labels FR/EN, validity period, a cell being edited). Machines:
// its fields, synchronised from the code of the module (003/P12), only their labels
// change here, and its closed lists (interface media and roles). Politique: environments,
// services with their transport/port pairs (REQ-010-5), flow types, axes and their values.
// Each tab is written under `<declarant>.settings.edit` (P4, P5): without it, the tab is
// read only, said at its top with the code of the permission, each write refused by it
// (024/M14). The sensitivity of any field stays with the core (P6). Every change goes to
// the draft, grouped by element, then to a changeset (E5). Closing a value still cited is
// refused with the list of the objects that cite it (D33).
import {
  Button,
  CodeBadge,
  DataGrid,
  Icon,
  IconButton,
  InlineAlert,
  Kbd,
  KeyLabel,
  Stack,
  Tabs,
  Toolbar,
  type CellState,
  type GridColumn,
  type RowState,
  type TabItem,
  type ToolbarAction,
} from "@kartenn/ui";
import { computed, ref } from "vue";
import Attributs from "./EcranAttributs.vue";
import Cadre from "./EcranCadre.vue";
import Panneau from "./EcranPanneau.vue";

const props = defineProps<{ vue: string }>();

// ——— Declarants and the rights of the reader ———

type Declarant = "core" | "machines" | "policy";
const DECLARANT: Record<Declarant, string> = {
  core: "Cœur",
  machines: "Machines",
  policy: "Politique",
};
/** « les paramètres … » of each declarant. */
const OF: Record<Declarant, string> = {
  core: "du cœur",
  machines: "du module Machines",
  policy: "de la Politique",
};
const DECLARANTS: Declarant[] = ["core", "machines", "policy"];
const TABS: TabItem[] = DECLARANTS.map((d) => ({
  value: d,
  label: DECLARANT[d],
}));
const TAB_OF: Record<string, Declarant> = {
  coeur: "core",
  machines: "machines",
  politique: "policy",
  "politique-lecture": "policy",
  cloture: "policy",
};
const tab = ref<string>(TAB_OF[props.vue] ?? "core");
const shownDeclarant = computed(() => DECLARANT[tab.value as Declarant] ?? DECLARANT.core);

/**
 * The global permissions of the reader of each view (P5): an administrator of the core
 * (`coeur`), the team that keeps the Machines settings (`machines`), a security team
 * (`politique`, `cloture`), a reader (`politique-lecture`).
 */
const GRANTS: Record<string, string[]> = {
  coeur: ["core.settings.edit", "machines.settings.edit", "policy.settings.edit"],
  machines: ["machines.settings.edit", "machines.settings.approval.self"],
  politique: ["policy.settings.edit", "policy.settings.approval.self"],
  "politique-lecture": [],
  cloture: ["policy.settings.edit", "policy.settings.approval.self"],
};
const grants = GRANTS[props.vue] ?? [];
const editOf = (d: Declarant): string => `${d}.settings.edit`;
const can = (d: Declarant): boolean => grants.includes(editOf(d));
/** The permission a write of `d` lacks, for `forbidden` (« Il faut … »); none when held. */
const lacks = (d: Declarant): string | undefined => (can(d) ? undefined : editOf(d));
const editing = props.vue === "coeur";
const draftMachines = props.vue === "machines";
const draftPolicy = props.vue === "politique";

// ——— Fields (REQ-027-6; 003, REQ-003-1 to 4) ———

interface FieldRow extends Record<string, unknown> {
  key: string;
  label: string;
  labelEn: string;
  type: string;
  carriers: string;
  heritable: boolean;
  sensitive: boolean;
}
const FIELDS: Record<"core" | "machines", FieldRow[]> = {
  core: [
    {
      key: "core.owner",
      label: "Propriétaire",
      labelEn: "Owner",
      type: "Référence : entité",
      carriers: "Préfixe, plage, adresse",
      heritable: true,
      sensitive: false,
    },
    {
      key: "core.site",
      label: "Site",
      labelEn: "Site",
      type: "Référence : site",
      carriers: "Préfixe, plage, adresse",
      heritable: true,
      sensitive: false,
    },
    {
      key: "core.vlan",
      label: "VLAN",
      labelEn: "VLAN",
      type: "Référence : VLAN",
      carriers: "Réseau",
      heritable: false,
      sensitive: false,
    },
    {
      key: "core.external_system",
      label: "Système externe",
      labelEn: "External system",
      type: "Liste fermée",
      carriers: "Tout objet",
      heritable: false,
      sensitive: false,
    },
    {
      key: "custom.contrat",
      label: "Contrat de maintenance",
      labelEn: "Maintenance contract",
      type: "Texte",
      carriers: "Préfixe, adresse",
      heritable: true,
      sensitive: true,
    },
    ...(editing
      ? [
          {
            key: "custom.astreinte",
            label: "Astreinte",
            labelEn: "On-call",
            type: "Texte",
            carriers: "Préfixe",
            heritable: true,
            sensitive: false,
          },
        ]
      : []),
  ],
  machines: [
    {
      key: "machines.fqdn",
      label: "Nom complet",
      labelEn: "Fully qualified name",
      type: "Texte",
      carriers: "Machine",
      heritable: false,
      sensitive: false,
    },
    {
      key: "machines.interface_media",
      label: "Média d'interface",
      labelEn: "Interface medium",
      type: "Liste fermée",
      carriers: "Interface",
      heritable: false,
      sensitive: false,
    },
    {
      key: "machines.interface_role",
      label: "Rôle d'interface",
      labelEn: "Interface role",
      type: "Liste fermée",
      carriers: "Interface",
      heritable: false,
      sensitive: false,
    },
  ],
};
// The key first and of fixed width; the right panel covers the end of the grid (024/M8).
const fieldColumns: GridColumn[] = [
  { id: "key", label: "Clé", width: "lg", mono: true, sortable: true, filterable: true },
  { id: "label", label: "Libellé", width: "fill", wrap: true, sortable: true, filterable: true },
  { id: "labelEn", label: "Libellé (en)", width: "md", filterable: true, priority: 3 },
  { id: "type", label: "Type", width: "md", filterable: true, priority: 2 },
  { id: "carriers", label: "Porteurs", width: "lg", wrap: true, filterable: true, priority: 2 },
  { id: "heritable", label: "Héritable", width: "sm", filterable: true, priority: 3 },
  { id: "sensitive", label: "Sensible", width: "sm", filterable: true },
];
const fieldState = (row: FieldRow): RowState | undefined =>
  row.key === "custom.astreinte" ? "draft" : undefined;

// ——— Closed lists (003, REQ-003-5, 7; D33) ———

interface ValueRow extends Record<string, unknown> {
  key: string;
  label: string;
  labelEn: string;
  from: string;
  until: string;
  cited: number;
}
interface ClosedList {
  key: string;
  label: string;
  values: ValueRow[];
  /** The cells of the draft: `key|column` → state. */
  cells?: Record<string, CellState>;
}
const v = (
  key: string,
  label: string,
  labelEn: string,
  from: string,
  cited: number,
  until = "",
): ValueRow => ({ key, label, labelEn, from, until, cited });
const LISTS: Record<"core" | "machines", ClosedList[]> = {
  core: [
    {
      key: "core.external_system",
      label: "Système externe",
      values: [
        v("ticketing", "Gestion des demandes", "Ticketing", "2024-01-15", 412),
        v("inventaire", "Inventaire matériel", "Inventory", "2024-01-15", 1284),
        v("supervision", "Supervision", "Monitoring", "2024-03-01", 233),
        v("annuaire", "Annuaire", "Directory", "2024-01-15", 18, "2026-12-31"),
        ...(editing ? [v("sauvegarde", "Sauvegarde", "Backup", "2026-10-27", 0)] : []),
      ],
      ...(editing ? { cells: { "inventaire|labelEn": "draft" as const } } : {}),
    },
  ],
  machines: [
    {
      key: "machines.interface_media",
      label: "Média d'interface",
      values: [
        v("ethernet", "Ethernet", "Ethernet", "2025-06-02", 1890),
        v("sfp", "SFP", "SFP", "2025-06-02", 640),
        ...(draftMachines ? [v("sfp28", "SFP28", "SFP28", "2026-10-27", 0)] : []),
        v("qsfp", "QSFP", "QSFP", "2025-06-02", 96),
        v("wifi", draftMachines ? "Wi-Fi (802.11)" : "Wi-Fi", "Wi-Fi", "2025-06-02", 57),
        v("aggregate", "Agrégat", "Aggregate", "2025-06-02", 212),
        v("virtual", "Virtuelle", "Virtual", "2025-06-02", 3315),
        v("loopback", "Bouclage", "Loopback", "2025-06-02", 402),
      ],
      ...(draftMachines ? { cells: { "wifi|label": "draft" as const } } : {}),
    },
    {
      key: "machines.interface_role",
      label: "Rôle d'interface",
      values: [
        v("administration", "Administration", "Administration", "2025-06-02", 480),
        v("backup", "Sauvegarde", "Backup", "2025-06-02", 155),
        v("monitoring", "Supervision", "Monitoring", "2025-06-02", 92),
        v("storage", "Stockage", "Storage", "2025-06-02", 310),
        v("cluster", "Grappe", "Cluster", "2025-06-02", 64),
        v("service", "Service", "Service", "2025-06-02", 2902),
      ],
    },
  ],
};
const ADDED = new Set(["sauvegarde", "sfp28", "custom.astreinte", "preprod", "ldaps"]);
const valueColumns: GridColumn[] = [
  { id: "key", label: "Clé", width: "md", mono: true, sortable: true, filterable: true },
  { id: "label", label: "Libellé", width: "fill", sortable: true, filterable: true },
  { id: "labelEn", label: "Libellé (en)", width: "md", filterable: true, priority: 2 },
  { id: "from", label: "Valide depuis", width: "md", mono: true, sortable: true, priority: 3 },
  { id: "until", label: "Jusqu'au", width: "md", mono: true, sortable: true, priority: 3 },
  { id: "cited", label: "Citations", width: "sm", align: "end", sortable: true, priority: 2 },
];
const addedState = (row: Record<string, unknown>): RowState | undefined =>
  ADDED.has(String(row.key)) ? "draft" : undefined;
const valueCell =
  (list: ClosedList) =>
  (row: ValueRow, column: GridColumn): CellState | undefined =>
    list.cells?.[`${row.key}|${column.id}`];

// ——— The catalogue of the policy (010, REQ-010-1, 2, 4, 5, 29) ———

interface EnvRow extends Record<string, unknown> {
  key: string;
  label: string;
  description: string;
  from: string;
  cited: number;
}
const ENVIRONMENTS: EnvRow[] = [
  {
    key: "prod",
    label: "Production",
    description: "Ce qui sert les utilisateurs.",
    from: "2024-01-15",
    cited: 1904,
  },
  {
    key: "rece",
    label: "Recette",
    description: "Validation avant la production, sur des jeux de contrôle.",
    from: "2024-01-15",
    cited: 388,
  },
  {
    key: "deve",
    label: "Développement",
    description: "Postes et services des équipes de développement.",
    from: "2024-01-15",
    cited: 241,
  },
  {
    key: "admin",
    label: "Administration",
    description: "Bastions et interfaces d'administration.",
    from: "2024-01-15",
    cited: 57,
  },
  {
    key: "sandbox",
    label: "Bac à sable",
    description: "Essais isolés, sans donnée réelle.",
    from: "2025-02-01",
    cited: 4,
  },
  ...(draftPolicy
    ? [
        {
          key: "preprod",
          label: "Préproduction",
          description: "Répétition des mises en production, données anonymisées.",
          from: "2026-10-27",
          cited: 0,
        },
      ]
    : []),
];
const envColumns: GridColumn[] = [
  { id: "key", label: "Clé", width: "sm", mono: true, sortable: true, filterable: true },
  { id: "label", label: "Libellé", width: "fill", sortable: true, filterable: true },
  { id: "description", label: "Description", width: "fill", wrap: true, priority: 2 },
  { id: "from", label: "Valide depuis", width: "md", mono: true, sortable: true, priority: 3 },
  { id: "cited", label: "Citations", width: "sm", align: "end", sortable: true, priority: 3 },
];

/** A flow type (REQ-010-4): key, labels. */
const FLOW_TYPES = [
  { key: "application", label: "Applicatif", labelEn: "Application", services: 3 },
  { key: "admin", label: "Administratif", labelEn: "Administration", services: 3 },
  { key: "database", label: "Base de données", labelEn: "Database", services: 1 },
  { key: "file", label: "Fichier", labelEn: "File", services: 1 },
];
const FLOW_TYPE_LABEL = Object.fromEntries(FLOW_TYPES.map((t) => [t.key, t.label]));
const flowTypeColumns: GridColumn[] = [
  { id: "key", label: "Clé", width: "md", mono: true, sortable: true, filterable: true },
  { id: "label", label: "Libellé", width: "fill", sortable: true, filterable: true },
  { id: "labelEn", label: "Libellé (en)", width: "md", filterable: true, priority: 2 },
  { id: "services", label: "Services", width: "sm", align: "end", sortable: true, priority: 3 },
];

/**
 * A service (REQ-010-5, D29): exactly one flow type and its pairs — `tcp`, `udp`, `sctp`,
 * `dccp` with a port or a closed range, `icmp`, `icmpv6` with a type and an optional code,
 * `ip` with a protocol number.
 */
interface ServiceRow extends Record<string, unknown> {
  key: string;
  label: string;
  flowType: string;
  entries: string[];
}
const SERVICES: ServiceRow[] = [
  { key: "https", label: "Web chiffré", flowType: "application", entries: ["tcp/443"] },
  { key: "api", label: "API interne", flowType: "application", entries: ["tcp/8443"] },
  {
    key: "web-essais",
    label: "Web des essais",
    flowType: "application",
    entries: ["tcp/8000-8099"],
  },
  { key: "ssh", label: "Shell sécurisé", flowType: "admin", entries: ["tcp/22"] },
  {
    key: "ping",
    label: "Écho ICMP",
    flowType: "admin",
    entries: ["icmp type 8 code 0", "icmpv6 type 128"],
  },
  {
    key: "ipsec",
    label: "Tunnel IPsec",
    flowType: "admin",
    entries: ["ip proto 50", "udp/500", "udp/4500"],
  },
  { key: "postgres", label: "PostgreSQL", flowType: "database", entries: ["tcp/5432"] },
  { key: "nfs", label: "Partage NFS", flowType: "file", entries: ["tcp/2049", "udp/2049"] },
  { key: "signalisation", label: "Signalisation", flowType: "application", entries: ["sctp/3868"] },
  ...(draftPolicy
    ? [{ key: "ldaps", label: "Annuaire chiffré", flowType: "application", entries: ["tcp/636"] }]
    : []),
];
const serviceColumns: GridColumn[] = [
  { id: "key", label: "Service", width: "lg", sortable: true, filterable: true },
  { id: "flowType", label: "Type de flux", width: "md", sortable: true, filterable: true },
  {
    id: "entries",
    label: "Couples transport et port",
    width: "fill",
    wrap: true,
    filterable: true,
  },
];

/** An axis (REQ-010-29): a closed list or external reference property, and its values. */
interface AxisRow extends Record<string, unknown> {
  key: string;
  label: string;
  carrier: string;
  values: string[];
  imposed: boolean;
}
const AXES: AxisRow[] = [
  {
    key: "policy.environment",
    label: "Environnement",
    carrier: "Arbre d'adressage",
    values: ["les environnements"],
    imposed: true,
  },
  {
    key: "policy.sensitivity",
    label: "Sensibilité",
    carrier: "Arbre d'adressage",
    values: ["public", "internal", "confidential"],
    imposed: false,
  },
  {
    key: "policy.regulatory",
    label: "Cadre réglementaire",
    carrier: "Arbre d'adressage",
    values: draftPolicy ? ["none", "health", "pci"] : ["none", "health"],
    imposed: false,
  },
  {
    key: "policy.criticality",
    label: "Criticité",
    carrier: "Machine",
    values: ["low", "medium", "high"],
    imposed: false,
  },
];
const axisColumns: GridColumn[] = [
  { id: "key", label: "Axe", width: "lg", sortable: true, filterable: true },
  { id: "carrier", label: "Porteur", width: "md", filterable: true, priority: 2 },
  { id: "values", label: "Valeurs", width: "fill", wrap: true, filterable: true },
];
const axisCell = (row: AxisRow, column: GridColumn): CellState | undefined =>
  draftPolicy && row.key === "policy.regulatory" && column.id === "values" ? "draft" : undefined;

// ——— The draft, grouped by element (024-12) ———

interface Operation {
  what: string;
  detail: string;
}
const OPERATIONS: Record<string, { key: string; operations: Operation[] }[]> = {
  coeur: [
    {
      key: "core.external_system",
      operations: [
        { what: "Ajouter la valeur", detail: "sauvegarde · Sauvegarde" },
        { what: "Changer le libellé (en)", detail: "inventaire : Inventory → Hardware inventory" },
      ],
    },
    {
      key: "custom.astreinte",
      operations: [{ what: "Déclarer le champ", detail: "Astreinte · texte · préfixe" }],
    },
  ],
  machines: [
    {
      key: "machines.interface_media",
      operations: [
        { what: "Ajouter la valeur", detail: "sfp28 · SFP28" },
        { what: "Changer le libellé", detail: "wifi : Wi-Fi → Wi-Fi (802.11)" },
      ],
    },
  ],
  politique: [
    {
      key: "preprod",
      operations: [{ what: "Créer l'environnement", detail: "Préproduction" }],
    },
    { key: "ldaps", operations: [{ what: "Créer le service", detail: "tcp/636 · applicatif" }] },
    {
      key: "policy.regulatory",
      operations: [{ what: "Ajouter la valeur d'axe", detail: "pci" }],
    },
  ],
};
const draft = OPERATIONS[props.vue] ?? [];
const draftCount = draft.reduce((n, element) => n + element.operations.length, 0);
const PANEL: Record<string, "detail" | "draft" | undefined> = {
  machines: "detail",
  politique: "draft",
  cloture: "detail",
};

// ——— The chosen field (machines) and the refused closing (cloture) ———

const fqdnActions = computed<ToolbarAction[]>(() => [
  {
    id: "labels",
    label: "Modifier les libellés",
    icon: "pencil",
    ...(lacks("machines") ? { forbidden: editOf("machines") } : {}),
  },
  {
    id: "sensitive",
    label: "Rendre sensible",
    icon: "lock",
    ...(lacks("core") ? { forbidden: editOf("core") } : {}),
  },
]);
const sandboxActions: ToolbarAction[] = [
  { id: "edit", label: "Modifier", icon: "pencil", shortcut: ["e"], priority: 2 },
  { id: "close", label: "Clore…", icon: "trash" },
];
/** The objects that cite `sandbox` (D33): the refusal lists them, readable or not. */
const CITING = [
  { name: "192.0.2.128/26", kind: "Réseau · policy.environment posée" },
  { name: "198.51.100.128/26", kind: "Réseau · policy.environment posée" },
  { name: "203.0.113.60", kind: "Adresse · policy.environment posée" },
  { name: "sandbox-isole", kind: "Règle de la politique" },
];
</script>

<template>
  <Cadre
    title="Paramètres"
    active="settings"
    space="admin"
    :draft="draftCount"
    :panel="PANEL[props.vue]"
    :crumbs="[
      { label: 'Administration', to: '/admin' },
      { label: 'Paramètres', to: '/admin/settings' },
      { label: shownDeclarant },
    ]"
  >
    <Panneau title="Paramètres" detail="un onglet par déclarant" main>
      <Tabs v-model="tab" :tabs="TABS" label="Déclarants des paramètres">
        <template v-for="d in DECLARANTS" :key="d" #[d]>
          <Stack gap="4">
            <p class="k-ecran-muted">
              Écriture par changeset, sous <code>{{ editOf(d) }}</code>
              <template v-if="d === 'machines'">
                ; la sensibilité d'un champ, sous <code>core.settings.edit</code></template
              >.
            </p>
            <InlineAlert
              v-if="!can(d)"
              tone="warn"
              icon="lock"
              :title="`Lecture seule : il vous faut ${editOf(d)} pour modifier les paramètres ${OF[d]}.`"
            >
              <CodeBadge :code="editOf(d)" />
            </InlineAlert>

            <template v-if="d !== 'policy'">
              <section class="k-e13-section" :aria-labelledby="`k-e13-${d}-fields`">
                <div class="k-e13-section__head">
                  <h3 :id="`k-e13-${d}-fields`" class="k-ecran-subtitle">
                    Champs <span class="k-ecran-muted k-num">{{ FIELDS[d].length }}</span>
                  </h3>
                  <Button
                    v-if="d === 'core'"
                    size="sm"
                    variant="ghost"
                    icon="plus"
                    :forbidden="lacks(d)"
                    >Déclarer un champ custom.*</Button
                  >
                </div>
                <DataGrid
                  :label="`Champs de ${DECLARANT[d]}`"
                  :columns="fieldColumns"
                  :rows="FIELDS[d]"
                  row-key="key"
                  :row-state="fieldState"
                  :selectable="d === 'machines' && props.vue === 'machines' ? 'single' : false"
                  :selected="props.vue === 'machines' ? ['machines.fqdn'] : []"
                  stack
                >
                  <template #cell="{ row, column, value }">
                    <template v-if="column.id === 'heritable'">{{
                      row.heritable ? "oui" : "non"
                    }}</template>
                    <span v-else-if="column.id === 'sensitive'" class="k-e13-flag">
                      <Icon v-if="row.sensitive" name="lock" />{{ row.sensitive ? "oui" : "non" }}
                    </span>
                    <template v-else>{{ value }}</template>
                  </template>
                </DataGrid>
                <p v-if="d === 'machines'" class="k-ecran-muted">
                  Déclarés par le code du module et synchronisés : seuls leurs libellés se modifient
                  ici.
                </p>
              </section>

              <section
                v-for="list in LISTS[d]"
                :key="list.key"
                class="k-e13-section"
                :aria-labelledby="`k-e13-${list.key}`"
              >
                <div class="k-e13-section__head">
                  <h3 :id="`k-e13-${list.key}`" class="k-ecran-subtitle">
                    {{ list.label }} <code class="k-ecran-muted">{{ list.key }}</code>
                    <span class="k-ecran-muted k-num"> · {{ list.values.length }}</span>
                  </h3>
                  <Button size="sm" variant="ghost" icon="plus" :forbidden="lacks(d)"
                    >Ajouter une valeur à {{ list.key }}</Button
                  >
                </div>
                <DataGrid
                  :label="`Valeurs de ${list.key}`"
                  :columns="valueColumns"
                  :rows="list.values"
                  row-key="key"
                  :row-state="addedState"
                  :cell-state="valueCell(list)"
                  :active="
                    editing && list.key === 'core.external_system'
                      ? { row: 'supervision', column: 'labelEn' }
                      : undefined
                  "
                  :editing="
                    editing && list.key === 'core.external_system'
                      ? { row: 'supervision', column: 'labelEn' }
                      : undefined
                  "
                  stack
                >
                  <template #cell="{ row, column, value }">
                    <span v-if="column.id === 'until' && !row.until" class="k-ecran-muted">—</span>
                    <template v-else>{{ value }}</template>
                  </template>
                </DataGrid>
                <p v-if="can(d)" class="k-keyboard-hint">
                  Clé figée après création · <Kbd :keys="['Enter']" /> vers le brouillon ·
                  <Kbd :keys="['Escape']" /> annuler
                </p>
              </section>
            </template>

            <template v-else>
              <section class="k-e13-section" aria-labelledby="k-e13-environments">
                <div class="k-e13-section__head">
                  <h3 id="k-e13-environments" class="k-ecran-subtitle">
                    Environnements
                    <span class="k-ecran-muted k-num">{{ ENVIRONMENTS.length }}</span>
                  </h3>
                  <Button size="sm" variant="ghost" icon="plus" :forbidden="lacks(d)"
                    >Ajouter un environnement</Button
                  >
                </div>
                <DataGrid
                  label="Environnements"
                  :columns="envColumns"
                  :rows="ENVIRONMENTS"
                  row-key="key"
                  :row-state="addedState"
                  :selectable="props.vue === 'cloture' ? 'single' : false"
                  :selected="props.vue === 'cloture' ? ['sandbox'] : []"
                  stack
                />
              </section>

              <section class="k-e13-section" aria-labelledby="k-e13-services">
                <div class="k-e13-section__head">
                  <h3 id="k-e13-services" class="k-ecran-subtitle">
                    Services <span class="k-ecran-muted k-num">{{ SERVICES.length }}</span>
                  </h3>
                  <Button size="sm" variant="ghost" icon="plus" :forbidden="lacks(d)"
                    >Ajouter un service</Button
                  >
                </div>
                <DataGrid
                  label="Services"
                  :columns="serviceColumns"
                  :rows="SERVICES"
                  row-key="key"
                  :row-state="addedState"
                  stack
                >
                  <template #cell="{ row, column, value }">
                    <KeyLabel
                      v-if="column.id === 'key'"
                      :value="row.key"
                      :label="row.label"
                      show-key
                    />
                    <template v-else-if="column.id === 'flowType'">{{
                      FLOW_TYPE_LABEL[row.flowType]
                    }}</template>
                    <span v-else-if="column.id === 'entries'" class="k-e13-codes">
                      <span v-for="(entry, i) in row.entries" :key="entry"
                        ><code>{{ entry }}</code
                        ><template v-if="i < row.entries.length - 1">,</template></span
                      >
                    </span>
                    <template v-else>{{ value }}</template>
                  </template>
                </DataGrid>
                <p class="k-ecran-muted">
                  Un seul type de flux par service ; ports ou plages fermées pour tcp, udp, sctp et
                  dccp, type et code pour icmp et icmpv6, numéro de protocole pour ip.
                </p>
              </section>

              <section class="k-e13-section" aria-labelledby="k-e13-flow-types">
                <div class="k-e13-section__head">
                  <h3 id="k-e13-flow-types" class="k-ecran-subtitle">
                    Types de flux <span class="k-ecran-muted k-num">{{ FLOW_TYPES.length }}</span>
                  </h3>
                  <Button size="sm" variant="ghost" icon="plus" :forbidden="lacks(d)"
                    >Ajouter un type de flux</Button
                  >
                </div>
                <DataGrid
                  label="Types de flux"
                  :columns="flowTypeColumns"
                  :rows="FLOW_TYPES"
                  row-key="key"
                  stack
                />
              </section>

              <section class="k-e13-section" aria-labelledby="k-e13-axes">
                <div class="k-e13-section__head">
                  <h3 id="k-e13-axes" class="k-ecran-subtitle">
                    Axes <span class="k-ecran-muted k-num">{{ AXES.length }}</span>
                  </h3>
                  <Button size="sm" variant="ghost" icon="plus" :forbidden="lacks(d)"
                    >Ajouter une valeur d'axe</Button
                  >
                </div>
                <DataGrid
                  label="Axes et leurs valeurs"
                  :columns="axisColumns"
                  :rows="AXES"
                  row-key="key"
                  :cell-state="axisCell"
                  stack
                >
                  <template #cell="{ row, column }">
                    <KeyLabel
                      v-if="column.id === 'key'"
                      :value="row.key"
                      :label="row.label"
                      show-key
                    />
                    <span v-else-if="column.id === 'values' && row.imposed" class="k-ecran-muted"
                      >imposé : {{ row.values[0] }}</span
                    >
                    <span v-else-if="column.id === 'values'" class="k-e13-codes">
                      <span v-for="(value, i) in row.values" :key="value"
                        ><code>{{ value }}</code
                        ><template v-if="i < row.values.length - 1">,</template></span
                      >
                    </span>
                    <template v-else>{{ row[column.id] }}</template>
                  </template>
                </DataGrid>
              </section>
            </template>
          </Stack>
        </template>
      </Tabs>
      <template #footer>
        <span>Au présent</span>
        <span class="k-ecran-muted">Toute modification va au brouillon, puis à un changeset</span>
      </template>
    </Panneau>

    <template #detail>
      <!-- A field of a module: its labels are the module's, its sensitivity the core's. -->
      <Stack v-if="props.vue === 'machines'" gap="4">
        <Stack gap="2">
          <h2 class="k-ecran-subtitle">
            Nom complet <span class="k-mono k-ecran-muted">machines.fqdn</span>
          </h2>
          <Toolbar label="Actions du champ" :actions="fqdnActions" align="start" />
        </Stack>
        <Attributs
          :items="[
            { key: 'declarant', label: 'Déclarant', value: 'Machines' },
            { key: 'labelEn', label: 'Libellé (en)', value: 'Fully qualified name' },
            { key: 'type', label: 'Type', value: 'Texte' },
            { key: 'carriers', label: 'Porteurs', value: 'Machine' },
            { key: 'heritable', label: 'Héritable', value: 'non' },
            { key: 'sensitive', label: 'Sensible', value: 'non' },
          ]"
        />
        <InlineAlert tone="info" icon="lock" title="La sensibilité relève du cœur">
          Rendre un champ sensible décide qui en lit la valeur
          (<code>prop.machines.fqdn.read</code>), quel que soit son déclarant : il faut
          <code>core.settings.edit</code>.
        </InlineAlert>
      </Stack>

      <!-- A value still cited: its closing refused, with the objects that cite it (D33). -->
      <Stack v-else-if="props.vue === 'cloture'" gap="4">
        <Stack gap="2">
          <h2 class="k-ecran-subtitle">
            Bac à sable <span class="k-mono k-ecran-muted">sandbox</span>
          </h2>
          <Toolbar label="Actions de l'environnement" :actions="sandboxActions" align="start" />
        </Stack>
        <InlineAlert
          tone="ko"
          title="Clôture refusée : 4 objets citent encore sandbox"
          :items="[
            {
              message: 'Un objet ne cite que des valeurs vivantes sur sa période.',
              code: 'policy.reference.closed',
            },
          ]"
        />
        <section aria-labelledby="k-e13-citing">
          <Stack gap="2">
            <h3 id="k-e13-citing" class="k-ecran-subtitle">
              Objets qui la citent · {{ CITING.length }}
            </h3>
            <ul class="k-ecran-list k-e13-lines">
              <li v-for="c in CITING" :key="c.name">
                <code>{{ c.name }}</code>
                <span class="k-ecran-muted">{{ c.kind }}</span>
              </li>
            </ul>
            <p class="k-ecran-muted">
              Changez leur environnement ou retirez la règle dans le même changeset, puis closez la
              valeur.
            </p>
          </Stack>
        </section>
        <Attributs
          :items="[
            { key: 'label', label: 'Libellé', value: 'Bac à sable' },
            {
              key: 'description',
              label: 'Description',
              value: 'Essais isolés, sans donnée réelle.',
            },
            { key: 'from', label: 'Valide depuis', value: '2025-02-01', mono: true },
            { key: 'close', label: 'Clôture demandée au', value: '2026-11-01', mono: true },
          ]"
        />
      </Stack>
    </template>

    <template v-if="draft.length" #draft>
      <Stack as="ol" gap="3" class="k-ecran-list" aria-label="Opérations du brouillon">
        <li v-for="element in draft" :key="element.key">
          <Stack gap="1">
            <code>{{ element.key }}</code>
            <ul class="k-ecran-list k-e13-ops">
              <li
                v-for="op in element.operations"
                :key="op.what"
                class="k-e13-op k-ecran-draft-item"
              >
                <Icon name="draft" />
                <span class="k-e13-op__change"
                  >{{ op.what }} <span class="k-ecran-muted">({{ op.detail }})</span></span
                >
                <IconButton
                  icon="x"
                  size="sm"
                  :label="`Retirer l'opération « ${op.what} » sur ${element.key}`"
                />
              </li>
            </ul>
          </Stack>
        </li>
      </Stack>
    </template>
  </Cadre>
</template>

<style>
@layer components {
  .k-e13-section {
    display: grid;
    grid-template-columns: minmax(0, 1fr);
    gap: var(--k-space-2);
    min-inline-size: 0;
  }

  /* The title of a section and its action on one line; the action wraps under it. */
  .k-e13-section__head {
    display: flex;
    flex-wrap: wrap;
    gap: var(--k-space-2) var(--k-space-3);
    align-items: center;
    justify-content: space-between;
    min-inline-size: 0;
  }

  .k-e13-flag {
    display: inline-flex;
    gap: var(--k-space-1);
    align-items: center;
  }

  /* The pairs of a service, the values of an axis: whole codes, wrapping between them. */
  .k-e13-codes {
    display: inline-flex;
    flex-wrap: wrap;
    gap: var(--k-space-1) var(--k-space-2);
  }

  .k-e13-codes > span {
    white-space: nowrap;
  }

  /* An object that cites a value: its name left, its kind right, wrapping when narrow. */
  .k-e13-lines {
    display: grid;
    grid-template-columns: minmax(0, auto) minmax(0, 1fr);
    gap: var(--k-space-1) var(--k-space-3);
  }

  .k-e13-lines > li {
    display: grid;
    grid-column: 1 / -1;
    grid-template-columns: subgrid;
    align-items: baseline;
  }

  .k-e13-lines > li > * {
    min-inline-size: 0;
    overflow-wrap: anywhere;
  }

  /* An operation of the draft under its element, its remove button at the end. */
  .k-e13-op {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr) auto;
    gap: var(--k-space-2);
    align-items: center;
  }

  .k-e13-op__change {
    min-inline-size: 0;
    overflow-wrap: anywhere;
  }
}
</style>
