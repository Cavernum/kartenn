<script setup lang="ts">
// E10 — Hôte de plugin (design « Maquettes »; 021, « Plugin VLAN », « Plugin Politique »;
// 024/M8, M12): a full-page view of a plugin in the frame of the shell (VLAN groups, the VLAN
// of the chosen group, the next free VID), and the policy matrix of the 017 between
// environments in its two-way grid (MatrixGrid: row and column headers of the same make,
// both sticky), the chosen cell in the right panel with its simulated effect; its changes
// go to the draft of the shell.
import {
  Button,
  CodeBadge,
  DataGrid,
  InlineAlert,
  MatrixGrid,
  SidePanel,
  Stack,
  StatusBadge,
  TextField,
  type GridColumn,
  type GridSort,
  type MatrixAxis,
  type MatrixCell,
  type MatrixCellState,
  type RowState,
  type State,
  type ToolbarAction,
} from "@kartenn/ui";
import { ref } from "vue";
import AVenir from "./AVenir.vue";
import Attributs from "./EcranAttributs.vue";
import Barre from "./EcranBarre.vue";
import Cadre from "./EcranCadre.vue";
import Panneau from "./EcranPanneau.vue";

const props = defineProps<{ vue: string }>();

interface Group extends Record<string, unknown> {
  key: string;
  anchor: string;
  range: string;
  used: string;
  free: string;
}
const groups: Group[] = [
  { key: "campus-ouest", anchor: "site-ouest", range: "100–199", used: "87 %", free: "12" },
  { key: "campus-est", anchor: "site-est", range: "200–299", used: "41 %", free: "59" },
  { key: "labo", anchor: "dmz-labo", range: "900–999", used: "6 %", free: "94" },
];
const groupColumns: GridColumn[] = [
  { id: "key", label: "Groupe", width: "fill", mono: true, sortable: true },
  { id: "anchor", label: "Ancre", width: "md", mono: true, filterable: true },
  { id: "range", label: "VID", width: "sm", mono: true },
  { id: "used", label: "Occupation", width: "sm", align: "end", sortable: true },
  { id: "free", label: "Libres", width: "sm", align: "end", sortable: true },
];
const groupSort = ref<GridSort>({ column: "key", direction: "ascending" });

interface Vlan extends Record<string, unknown> {
  vid: string;
  name: string;
  state: "active" | "deprecated" | "reserved";
  networks: string;
  interfaces: string;
}
const vlans: Vlan[] = [
  { vid: "100", name: "admin", state: "active", networks: "192.0.2.0/26", interfaces: "14" },
  { vid: "101", name: "serveurs", state: "active", networks: "2 réseaux", interfaces: "38" },
  { vid: "102", name: "voix", state: "deprecated", networks: "—", interfaces: "0" },
  { vid: "104", name: "sauvegarde", state: "active", networks: "—", interfaces: "0" },
  { vid: "110", name: "reserve-wifi", state: "reserved", networks: "—", interfaces: "0" },
  { vid: "120", name: "serveurs-b", state: "active", networks: "192.0.2.0/26", interfaces: "21" },
  { vid: "121", name: "supervision", state: "active", networks: "192.0.2.64/26", interfaces: "6" },
];
const vlanColumns: GridColumn[] = [
  { id: "vid", label: "VID", width: "sm", mono: true, sortable: true },
  { id: "name", label: "Nom", width: "fill", mono: true, sortable: true },
  { id: "state", label: "Statut", width: "md", filterable: true },
  { id: "networks", label: "Réseaux", width: "md", mono: true, filterable: true },
  { id: "interfaces", label: "Interfaces", width: "sm", align: "end", sortable: true },
];
const vlanSort = ref<GridSort>({ column: "vid", direction: "ascending" });
const VLAN_STATE: Record<Vlan["state"], { state: State; label: string }> = {
  active: { state: "ok", label: "Actif" },
  deprecated: { state: "warn", label: "Déprécié" },
  reserved: { state: "neutral", label: "Réservé" },
};
const vlanRow = (row: Vlan): RowState | undefined => (row.vid === "104" ? "draft" : undefined);
const vlanActions: ToolbarAction[] = [
  { id: "free", label: "Prochain libre : 105", icon: "search", priority: 2 },
  { id: "create", label: "Créer", icon: "plus", variant: "primary", shortcut: ["c"], priority: 3 },
  { id: "export", label: "Exporter", priority: 1 },
  { id: "renumber", label: "Renuméroter…", icon: "pencil", forbidden: "vlan.edit" },
];

type Verdict = "allow" | "deny" | "conditional" | "default";
/** The environments, both axes of the matrix (source in rows, destination in columns). */
const ENVS: MatrixAxis[] = [
  { id: "prod", label: "Production" },
  { id: "preprod", label: "Préproduction" },
  { id: "rece", label: "Recette" },
  { id: "inte", label: "Intégration" },
  { id: "deve", label: "Développement" },
];
const MATRIX: Record<string, Verdict[]> = {
  prod: ["allow", "deny", "deny", "deny", "deny"],
  preprod: ["conditional", "allow", "default", "deny", "deny"],
  rece: ["conditional", "default", "allow", "allow", "default"],
  inte: ["deny", "default", "allow", "allow", "allow"],
  deve: ["deny", "deny", "default", "allow", "allow"],
};
const verdict = (row: MatrixAxis, column: MatrixAxis): Verdict =>
  MATRIX[row.id]![ENVS.indexOf(column)]!;
const VERDICT: Record<Verdict, { state: State; label: string }> = {
  allow: { state: "ok", label: "Autoriser" },
  deny: { state: "ko", label: "Interdire" },
  conditional: { state: "warn", label: "Sous motif" },
  default: { state: "neutral", label: "Refus par défaut" },
};
const cellState = (row: MatrixAxis, column: MatrixAxis): MatrixCellState | undefined =>
  row.id === "rece" && column.id === "prod"
    ? "draft"
    : row.id === "preprod" && column.id === "prod"
      ? "review"
      : undefined;
const active = ref<MatrixCell>({ row: "rece", column: "prod" });
const matrixActions: ToolbarAction[] = [
  { id: "hidden", label: "3 règles hors vue", icon: "external-link", priority: 2 },
  { id: "simulate", label: "Simuler le brouillon", icon: "flow", priority: 1 },
  { id: "export", label: "Exporter" },
];
const panelTabs = [
  { value: "detail", label: "Détail" },
  { value: "draft", label: "Brouillon", count: 3 },
];
</script>

<template>
  <Cadre
    v-if="props.vue === 'vlan'"
    title="Groupe de VLAN campus-ouest"
    active="vlan-groups"
    space="network"
    :crumbs="[
      { label: 'Réseau', to: '/x/vlan' },
      { label: 'Groupes de VLAN', to: '/x/vlan/groups' },
      { label: 'campus-ouest', mono: true },
    ]"
  >
    <Panneau title="Groupes de VLAN" detail="3" flush>
      <DataGrid
        v-model:sort="groupSort"
        label="Groupes de VLAN"
        :columns="groupColumns"
        :rows="groups"
        row-key="key"
        selectable
        :selected="['campus-ouest']"
      />
    </Panneau>
    <Panneau title="campus-ouest" detail="VID 100–199 · ancre site-ouest" main flush>
      <template #toolbar>
        <Barre label="Actions du groupe" :actions="vlanActions">
          <span class="k-num">87 sur 100 utilisés · 12 libres</span>
        </Barre>
      </template>
      <DataGrid
        v-model:sort="vlanSort"
        label="VLAN du groupe campus-ouest"
        :columns="vlanColumns"
        :rows="vlans"
        row-key="vid"
        :row-state="vlanRow"
        :filtered="['state']"
      >
        <template #cell="{ row, column, value }">
          <StatusBadge
            v-if="column.id === 'state'"
            :state="VLAN_STATE[row.state].state"
            :label="VLAN_STATE[row.state].label"
          />
          <template v-else>{{ value }}</template>
        </template>
      </DataGrid>
      <template #footer>
        <span class="k-num">7 VLAN sur 87 · 1 en brouillon</span>
        <span class="k-ecran-muted">Colonne « Interfaces » : plugin Machines</span>
      </template>
    </Panneau>
  </Cadre>

  <Cadre
    v-else
    title="Matrice de politique applicatif"
    active="matrix"
    space="policy"
    :draft="3"
    :crumbs="[
      { label: 'Politique', to: '/x/policy' },
      { label: 'Matrice', to: '/x/policy/matrix' },
      { label: 'applicatif', mono: true },
    ]"
  >
    <Panneau title="Matrice" detail="applicatif" main flush>
      <template #toolbar>
        <Barre label="Actions de la matrice" :actions="matrixActions" align="start">
          <AVenir component="Select" task="18" inline>
            <Button size="sm" icon="flow">Type de flux : applicatif</Button>
          </AVenir>
        </Barre>
      </template>
      <MatrixGrid
        v-model:active="active"
        label="Matrice applicatif : source en ligne, destination en colonne"
        corner="Source → destination"
        :rows="ENVS"
        :columns="ENVS"
        :cell-state="cellState"
      >
        <template #cell="{ row, column }">
          <StatusBadge
            :state="VERDICT[verdict(row, column)].state"
            :label="VERDICT[verdict(row, column)].label"
          />
        </template>
      </MatrixGrid>
      <template #footer>
        <span>Brouillon · 1 cellule · 1 en revue</span>
        <span class="k-num">1 s'ouvre sous réserve · 2 flux cassent · 0 guérit</span>
      </template>
    </Panneau>
    <template #drawer>
      <SidePanel label="Panneau droit" :tabs="panelTabs" tab="detail">
        <template #detail>
          <Stack gap="3">
            <h2 class="k-ecran-subtitle">Recette → Production</h2>
            <Attributs
              :items="[
                { key: 'state', label: 'État' },
                { key: 'patterns', label: 'Motifs' },
              ]"
            >
              <template #state><StatusBadge state="warn" label="Sous motif" /></template>
              <template #patterns><CodeBadge code="bastion-admin" /></template>
            </Attributs>
            <TextField label="Libellé" model-value="Accès recette vers production" />
            <InlineAlert
              tone="warn"
              title="Effet simulé : 2 flux cassent"
              :items="[
                { message: 'rece-batch → prod-api (tcp/443)', code: 'flow.verdict.denied' },
                { message: 'rece-ci → prod-registry (tcp/5000)', code: 'flow.verdict.denied' },
              ]"
            />
          </Stack>
        </template>
        <template #draft>
          <p class="k-ecran-muted">3 opérations, dont la cellule Recette → Production.</p>
        </template>
        <template #footer>
          <Button variant="primary" icon="draft" :shortcut="['Enter']">Ajouter au brouillon</Button>
        </template>
      </SidePanel>
    </template>
  </Cadre>
</template>

<style>
@layer components {
  /* The matrix in a flush panel: the panel draws the outer border, as for a DataGrid. */
  .k-ecran-panel__body[data-flush] > .k-matrix {
    border: 0;
    border-radius: 0;
  }
}
</style>
