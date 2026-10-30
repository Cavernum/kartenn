<script setup lang="ts">
// E1 — Topographie (design « Maquettes »; 021, « Catalogue des écrans » : Topographie;
// 024/M10, M13): the prefix tree of a domain (parent deduced, no move) with a « + » on its
// nodes, the content of the chosen node filtered by environment, one row in a draft and
// one in review. The bottom panel shows the chosen row, its attributes in several columns
// (wide content: the tree and the table keep their whole width; 020, « Panneau du bas »);
// the « + » of a node opens the edit panel of the kind in the right panel, with the node as
// parent — the same panel as from the table, the palette or the menu « Créer » (a narrow
// form: its fields one under the other).
import {
  Button,
  Cluster,
  DataGrid,
  EnvChip,
  FilterChip,
  Icon,
  KeyLabel,
  Select,
  Split,
  SplitButton,
  Stack,
  StatusBadge,
  TextField,
  Toolbar,
  TreeGrid,
  type GridColumn,
  type GridSort,
  type IconName,
  type ListOption,
  type MenuItem,
  type RowState,
  type SplitAction,
  type TabItem,
  type ToolbarAction,
  type TreeNode,
} from "@kartenn/ui";
import { ref } from "vue";
import Attributs from "./EcranAttributs.vue";
import Cadre from "./EcranCadre.vue";
import Panneau from "./EcranPanneau.vue";
import { DOMAIN, ENV, USAGE, usageLabel, type EnvKey } from "./donnees.ts";

const props = defineProps<{ vue: string }>();
const adding = props.vue === "ajout";
/** The bottom panel of the node view: the detail of the chosen row (wide content). */
const BOTTOM: TabItem[] = [{ value: "detail", label: "Détail" }];

const nodes: TreeNode[] = [
  {
    id: "p1",
    label: "Salle A",
    objectKey: "192.0.2.0/24",
    icon: "prefix",
    cells: { occupancy: "72 %" },
    children: [
      {
        id: "p11",
        label: "Serveurs",
        objectKey: "192.0.2.0/26",
        icon: "prefix",
        cells: { occupancy: "91 %" },
      },
      {
        id: "p12",
        label: "Supervision",
        objectKey: "192.0.2.64/26",
        icon: "prefix",
        cells: { occupancy: "45 %" },
      },
      {
        id: "p13",
        label: "Réserve",
        objectKey: "192.0.2.128/25",
        icon: "prefix",
        hasChildren: true,
        cells: { occupancy: "12 %" },
      },
    ],
  },
  {
    id: "p2",
    label: "Salle B",
    objectKey: "198.51.100.0/24",
    icon: "prefix",
    hasChildren: true,
    cells: { occupancy: "38 %" },
  },
  {
    id: "p3",
    label: "Invités",
    objectKey: "203.0.113.0/24",
    icon: "prefix",
    cells: { occupancy: "100 %" },
  },
  {
    id: "p4",
    label: "Cœur IPv6",
    objectKey: "2001:db8::/32",
    icon: "prefix",
    cells: { occupancy: "< 1 %" },
    children: [
      {
        id: "p41",
        label: "Salle A",
        objectKey: "2001:db8:10::/48",
        icon: "prefix",
        cells: { occupancy: "3 %" },
      },
    ],
  },
];
const treeColumns: GridColumn[] = [
  { id: "occupancy", label: "Occupation", width: "sm", align: "end" },
];
const addLabel = (node: TreeNode) =>
  node.objectKey ? `Ajouter dans ${node.objectKey}` : undefined;

interface Row extends Record<string, unknown> {
  id: string;
  kind: "range" | "address";
  value: string;
  usage: string;
  env: EnvKey;
  owner: string;
  machine: string;
}
const rows: Row[] = [
  {
    id: "a1",
    kind: "address",
    value: "192.0.2.65",
    usage: "gateway",
    env: "prod",
    owner: "Atelier Nord",
    machine: "rt-nord-1",
  },
  {
    id: "r1",
    kind: "range",
    value: "192.0.2.66–192.0.2.90",
    usage: "dhcp",
    env: "prod",
    owner: "Atelier Nord",
    machine: "—",
  },
  {
    id: "a2",
    kind: "address",
    value: "192.0.2.100",
    usage: "vip",
    env: "prod",
    owner: "Service Pilote",
    machine: "—",
  },
  {
    id: "a3",
    kind: "address",
    value: "192.0.2.101",
    usage: "host",
    env: "prod",
    owner: "Service Pilote",
    machine: "srv-app-01",
  },
  {
    id: "a4",
    kind: "address",
    value: "192.0.2.102",
    usage: "host",
    env: "prod",
    owner: "Service Pilote",
    machine: "srv-app-02",
  },
  {
    id: "a5",
    kind: "address",
    value: "192.0.2.120",
    usage: "reserved",
    env: "prod",
    owner: "Atelier Nord",
    machine: "—",
  },
];
const columns: GridColumn[] = [
  { id: "value", label: "Objet", width: "fill", mono: true, sortable: true },
  { id: "usage", label: "Usage", width: "sm", sortable: true, filterable: true, priority: 2 },
  { id: "env", label: "Environnement", width: "md", sortable: true, filterable: true, priority: 2 },
  {
    id: "owner",
    label: "Propriétaire",
    width: "md",
    sortable: true,
    filterable: true,
    priority: 2,
  },
];
const sort = ref<GridSort | undefined>({ column: "value", direction: "ascending" });
const KIND: Record<Row["kind"], { label: string; icon: IconName }> = {
  range: { label: "Plage", icon: "range" },
  address: { label: "Adresse", icon: "address" },
};
const rowState = (row: Row): RowState | undefined =>
  row.id === "a3" ? "draft" : row.id === "a2" ? "review" : undefined;

const actions: ToolbarAction[] = [
  { id: "create", label: "Créer", icon: "plus", variant: "primary", shortcut: ["c"], priority: 3 },
  { id: "free", label: "Prochain libre", icon: "search", priority: 2 },
  { id: "copy", label: "Copier les valeurs", icon: "copy" },
];
const more: MenuItem[] = [
  { id: "open", label: "Ouvrir la fiche", icon: "external-link", shortcut: ["Enter"] },
  { id: "history", label: "Historique", icon: "clock" },
  { id: "vlan", label: "VLAN porté…", icon: "vlan" },
  {
    id: "move",
    label: "Changer de domaine…",
    icon: "move",
    reason: "Le domaine d'un objet ne change pas après sa création (core.domain.immutable).",
  },
];

// The edit panel of the kind (« + » on 192.0.2.64/26): an address by default in a network.
const value = ref("192.0.2.103");
const usage = ref<string | undefined>("host");
const usages: ListOption[] = Object.entries(USAGE).map(([key, label]) => ({ value: key, label }));
const owner = ref("Service Pilote");
const addActions: SplitAction[] = [
  { id: "draft", label: "Ajouter au brouillon", icon: "draft" },
  { id: "apply", label: "Appliquer", icon: "circle-check" },
  { id: "review", label: "Soumettre en revue", icon: "send" },
];
const addAction = ref("draft");
</script>

<template>
  <Cadre
    title="Topographie : 192.0.2.64/26"
    active="topology"
    space="referential"
    :panel="adding ? 'detail' : undefined"
    :bottom="props.vue === 'noeud' ? BOTTOM : undefined"
    bottom-label="Détail de l'objet choisi"
    :collapsed="props.vue === 'replie'"
    :crumbs="[
      { label: 'Topographie', to: '/topology' },
      { label: DOMAIN.label, to: `/domains/${DOMAIN.key}` },
      { label: '192.0.2.0/24', to: '/topology?node=p1', mono: true },
      { label: '192.0.2.64/26', mono: true },
    ]"
  >
    <Split side-size="lg" gap="3">
      <template #side>
        <Panneau title="Préfixes" :detail="DOMAIN.label" flush>
          <TreeGrid
            :label="`Arbre des préfixes du domaine ${DOMAIN.label}`"
            tree-label="Préfixe"
            :nodes="nodes"
            :columns="treeColumns"
            :expanded="['p1', 'p4']"
            selected="p12"
            :add-label="addLabel"
          />
        </Panneau>
      </template>
      <Panneau title="192.0.2.64/26" detail="réseau · Supervision" main flush>
        <template #actions>
          <Toolbar label="Actions sur 192.0.2.64/26" align="end" :actions="actions" :more="more" />
        </template>
        <template #toolbar>
          <Cluster gap="2">
            <!-- The words of FilterPicker (« dans », « est »), its bar without « Ajouter ». -->
            <FilterChip property="Valeur" operator="dans" value="192.0.2.64/26" mono />
            <FilterChip property="Environnement" operator="est" :value="ENV.prod.label" />
          </Cluster>
        </template>
        <DataGrid
          v-model:sort="sort"
          label="Contenu de 192.0.2.64/26, environnement Production"
          :columns="columns"
          :rows="rows"
          row-key="id"
          :row-state="rowState"
          :filtered="['env']"
          :active="{ row: 'a3', column: 'value' }"
        >
          <template #cell="{ row, column, value: cell }">
            <Cluster v-if="column.id === 'value'" gap="1" :wrap="false">
              <Icon :name="KIND[row.kind].icon" meaningful :label="KIND[row.kind].label" />{{
                cell
              }}
            </Cluster>
            <KeyLabel
              v-else-if="column.id === 'usage'"
              :value="row.usage"
              :label="usageLabel(row.usage)"
            />
            <EnvChip v-else-if="column.id === 'env'" v-bind="ENV[row.env]" />
            <template v-else>{{ cell }}</template>
          </template>
        </DataGrid>
        <template #footer>
          <span class="k-num">6 objets sur 31 · 1 en brouillon · 1 en revue</span>
          <span>Au présent</span>
        </template>
      </Panneau>
    </Split>
    <template v-if="adding" #detail>
      <Stack gap="3">
        <h2 class="k-ecran-subtitle">Ajouter dans <code>192.0.2.64/26</code></h2>
        <Cluster gap="1" role="group" aria-label="Sorte d'objet">
          <Button size="sm" icon="address" :pressed="true">Adresse</Button>
          <Button size="sm" icon="range" :pressed="false">Plage</Button>
          <Button
            size="sm"
            icon="prefix"
            :pressed="false"
            reason="Un réseau ne contient pas de préfixe (core.prefix.network_not_leaf)."
            >Préfixe</Button
          >
        </Cluster>
        <TextField
          v-model="value"
          label="Valeur"
          mono
          required
          hint="Prochain libre de 192.0.2.64/26."
        />
        <Select v-model="usage" label="Usage" :options="usages" />
        <TextField v-model="owner" label="Propriétaire" />
        <Attributs
          :items="[
            { key: 'parent', label: 'Parent', value: '192.0.2.64/26', mono: true },
            { key: 'domain', label: 'Domaine', value: DOMAIN.label },
          ]"
        />
      </Stack>
    </template>
    <template v-if="adding" #detail-footer>
      <Cluster justify="end">
        <SplitButton
          v-model:action="addAction"
          :actions="addActions"
          :shortcut="['Control', 'Enter']"
        />
      </Cluster>
    </template>
    <!-- The chosen row in the bottom panel: its attributes in several columns, the tree and
         the table keep their whole width. -->
    <template #bottom-detail>
      <Stack gap="2">
        <Cluster gap="2">
          <h2 class="k-ecran-subtitle"><code>192.0.2.101</code></h2>
          <StatusBadge state="neutral" label="En brouillon" />
          <span class="k-ecran-muted">Créée par le brouillon actif ; pas encore appliquée.</span>
        </Cluster>
        <Attributs
          columns
          :items="[
            { key: 'kind', label: 'Sorte', value: 'Adresse' },
            { key: 'usage', label: 'Usage' },
            { key: 'env', label: 'Environnement' },
            { key: 'owner', label: 'Propriétaire', value: 'Service Pilote' },
            { key: 'machine', label: 'Machine', value: 'srv-app-01', mono: true },
            { key: 'vlan', label: 'VLAN', value: '121', mono: true },
          ]"
        >
          <template #usage><KeyLabel value="host" :label="usageLabel('host')" /></template>
          <template #env><EnvChip v-bind="ENV.prod" /></template>
        </Attributs>
      </Stack>
    </template>
    <template #bottom-detail-footer>
      <Button variant="ghost" icon="external-link">Ouvrir la fiche</Button>
      <Button icon="pencil" :shortcut="['e']">Modifier</Button>
    </template>
  </Cadre>
</template>
