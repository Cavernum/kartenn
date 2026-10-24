<script setup lang="ts">
// E1 — Référentiel (design « Maquettes »; 021, « Catalogue des écrans », Accueil et
// référentiel): the prefix tree of a domain (parent deduced, no move), the content of the
// chosen node filtered by environment, one row in a draft and one in review.
import {
  Button,
  Cluster,
  DataGrid,
  EnvChip,
  FilterChip,
  Icon,
  KeyLabel,
  Menu,
  Split,
  TreeGrid,
  type GridColumn,
  type IconName,
  type MenuItem,
  type RowState,
  type TreeNode,
} from "@kartenn/ui";
import Cadre from "./EcranCadre.vue";
import Panneau from "./EcranPanneau.vue";
import { DOMAIN, ENV, usageLabel, type EnvKey } from "./donnees.ts";

const props = defineProps<{ vue: string }>();

const nodes: TreeNode[] = [
  {
    id: "p1",
    label: "Salle A",
    objectKey: "192.0.2.0/24",
    icon: "prefix",
    cells: { occupancy: "72 %", vlan: "—" },
    children: [
      {
        id: "p11",
        label: "Serveurs",
        objectKey: "192.0.2.0/26",
        icon: "prefix",
        cells: { occupancy: "91 %", vlan: "120" },
      },
      {
        id: "p12",
        label: "Supervision",
        objectKey: "192.0.2.64/26",
        icon: "prefix",
        cells: { occupancy: "45 %", vlan: "121" },
      },
      {
        id: "p13",
        label: "Réserve",
        objectKey: "192.0.2.128/25",
        icon: "prefix",
        hasChildren: true,
        cells: { occupancy: "12 %", vlan: "—" },
      },
    ],
  },
  {
    id: "p2",
    label: "Salle B",
    objectKey: "198.51.100.0/24",
    icon: "prefix",
    hasChildren: true,
    cells: { occupancy: "38 %", vlan: "—" },
  },
  {
    id: "p3",
    label: "Invités",
    objectKey: "203.0.113.0/24",
    icon: "prefix",
    cells: { occupancy: "100 %", vlan: "140" },
  },
  {
    id: "p4",
    label: "Cœur IPv6",
    objectKey: "2001:db8::/32",
    icon: "prefix",
    cells: { occupancy: "< 1 %", vlan: "—" },
    children: [
      {
        id: "p41",
        label: "Salle A",
        objectKey: "2001:db8:10::/48",
        icon: "prefix",
        cells: { occupancy: "3 %", vlan: "121" },
      },
    ],
  },
];
const treeColumns: GridColumn[] = [
  { id: "occupancy", label: "Occupation", width: "sm", align: "end" },
];

interface Row extends Record<string, unknown> {
  id: string;
  kind: "range" | "address";
  value: string;
  usage: string;
  env: EnvKey;
  owner: string;
  machine: string;
  vlan: string;
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
    vlan: "121",
  },
  {
    id: "r1",
    kind: "range",
    value: "192.0.2.66–192.0.2.90",
    usage: "dhcp",
    env: "prod",
    owner: "Atelier Nord",
    machine: "—",
    vlan: "121",
  },
  {
    id: "a2",
    kind: "address",
    value: "192.0.2.100",
    usage: "vip",
    env: "prod",
    owner: "Service Pilote",
    machine: "—",
    vlan: "121",
  },
  {
    id: "a3",
    kind: "address",
    value: "192.0.2.101",
    usage: "host",
    env: "prod",
    owner: "Service Pilote",
    machine: "srv-app-01",
    vlan: "121",
  },
  {
    id: "a4",
    kind: "address",
    value: "192.0.2.102",
    usage: "host",
    env: "prod",
    owner: "Service Pilote",
    machine: "srv-app-02",
    vlan: "121",
  },
  {
    id: "a5",
    kind: "address",
    value: "192.0.2.120",
    usage: "reserved",
    env: "prod",
    owner: "Atelier Nord",
    machine: "—",
    vlan: "121",
  },
];
const columns: GridColumn[] = [
  { id: "value", label: "Objet", width: "fill", mono: true, sortable: true },
  { id: "usage", label: "Usage", width: "sm", filterable: true },
  { id: "env", label: "Environnement", width: "md", filterable: true },
  { id: "owner", label: "Propriétaire", width: "md", sortable: true },
];
const KIND: Record<Row["kind"], { label: string; icon: IconName }> = {
  range: { label: "Plage", icon: "range" },
  address: { label: "Adresse", icon: "address" },
};
const rowState = (row: Row): RowState | undefined =>
  row.id === "a3" ? "draft" : row.id === "a2" ? "review" : undefined;
const actions: MenuItem[] = [
  { id: "open", label: "Ouvrir la fiche", icon: "external-link", shortcut: ["Enter"] },
  { id: "edit", label: "Modifier", icon: "pencil", shortcut: ["e"] },
  { id: "history", label: "Historique", icon: "clock" },
  {
    id: "move",
    label: "Changer de domaine…",
    icon: "move",
    disabled: true,
    reason: "Le domaine est immuable (core.domain.immutable)",
  },
  { kind: "separator" },
  { kind: "label", label: "Plugins" },
  { id: "vlan", label: "VLAN porté…", icon: "vlan" },
];
</script>

<template>
  <Cadre
    title="Référentiel : 192.0.2.64/26"
    active="home"
    space="referential"
    :collapsed="props.vue === 'replie'"
    :crumbs="[
      { label: 'Référentiel', to: '/' },
      { label: DOMAIN.label, to: `/domains/${DOMAIN.key}` },
      { label: '192.0.2.0/24', to: '/?node=p1', mono: true },
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
          />
        </Panneau>
      </template>
      <Panneau title="192.0.2.64/26" detail="réseau · Supervision" main flush>
        <template #actions>
          <Button icon="search">Prochain libre</Button>
          <Button variant="primary" icon="plus" :shortcut="['c']">Créer</Button>
          <Menu label="Actions sur 192.0.2.64/26" icon="ellipsis" icon-only :items="actions" />
        </template>
        <template #toolbar>
          <Cluster gap="2">
            <FilterChip property="Inclus dans" operator="=" value="192.0.2.64/26" mono />
            <FilterChip
              property="Environnement"
              operator="="
              :value="ENV.prod.label"
              remove-label="Retirer le filtre Environnement"
            />
            <Button variant="ghost" size="sm" icon="funnel" :shortcut="['/']">Filtre</Button>
          </Cluster>
        </template>
        <DataGrid
          label="Contenu de 192.0.2.64/26, environnement Production"
          :columns="columns"
          :rows="rows"
          row-key="id"
          :sort="{ column: 'value', direction: 'ascending' }"
          :row-state="rowState"
          :active="{ row: 'a3', column: 'usage' }"
        >
          <template #cell="{ row, column, value }">
            <Cluster v-if="column.id === 'value'" gap="1" :wrap="false">
              <Icon :name="KIND[row.kind].icon" meaningful :label="KIND[row.kind].label" />{{
                value
              }}
            </Cluster>
            <KeyLabel
              v-else-if="column.id === 'usage'"
              :value="row.usage"
              :label="usageLabel(row.usage)"
            />
            <EnvChip v-else-if="column.id === 'env'" v-bind="ENV[row.env]" />
            <template v-else>{{ value }}</template>
          </template>
        </DataGrid>
        <template #footer>
          <span class="k-num">6 objets sur 31 · 1 en brouillon · 1 en revue</span>
          <span>Au présent</span>
        </template>
      </Panneau>
    </Split>
  </Cadre>
</template>
