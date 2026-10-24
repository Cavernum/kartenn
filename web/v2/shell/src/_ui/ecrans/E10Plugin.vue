<script setup lang="ts">
// E10 — Hôte de plugin (design « Maquettes »; 021, « Plugin VLAN », « Plugin Politique »):
// a full-page view of a plugin in the frame of the shell (VLAN groups and the VLAN of the
// chosen group), and the policy matrix of the 017 between environments, a chosen cell, the
// simulated effect; its changes go to the draft of the shell.
import {
  Button,
  CodeBadge,
  Cluster,
  DataGrid,
  EnvChip,
  FilterChip,
  InlineAlert,
  Split,
  Stack,
  StatusBadge,
  TextField,
  type CellState,
  type GridColumn,
  type RowState,
  type State,
} from "@kartenn/ui";
import AVenir from "./AVenir.vue";
import Attributs from "./EcranAttributs.vue";
import Cadre from "./EcranCadre.vue";
import Panneau from "./EcranPanneau.vue";
import { ENV, type EnvKey } from "./donnees.ts";

const props = defineProps<{ vue: string }>();

interface Group extends Record<string, unknown> {
  key: string;
  anchor: string;
  range: string;
  used: string;
}
const groups: Group[] = [
  { key: "campus-ouest", anchor: "site-ouest", range: "100–199", used: "87 %" },
  { key: "campus-est", anchor: "site-est", range: "200–299", used: "41 %" },
  { key: "labo", anchor: "dmz-labo", range: "900–999", used: "6 %" },
];
const groupColumns: GridColumn[] = [
  { id: "key", label: "Groupe", width: "fill", mono: true },
  { id: "range", label: "VID", width: "sm", mono: true },
  { id: "used", label: "Occ.", width: "xs", align: "end" },
];

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
  { id: "name", label: "Nom", width: "fill", mono: true },
  { id: "state", label: "Statut", width: "md", filterable: true },
  { id: "networks", label: "Réseaux", width: "md", mono: true },
  { id: "interfaces", label: "Interfaces", width: "sm", align: "end" },
];
const VLAN_STATE: Record<Vlan["state"], { state: State; label: string }> = {
  active: { state: "ok", label: "Actif" },
  deprecated: { state: "warn", label: "Déprécié" },
  reserved: { state: "neutral", label: "Réservé" },
};

type Verdict = "allow" | "deny" | "conditional" | "default";
interface MatrixRow extends Record<string, unknown> {
  source: EnvKey;
  prod: Verdict;
  rece: Verdict;
  deve: Verdict;
}
const matrix: MatrixRow[] = [
  { source: "prod", prod: "allow", rece: "deny", deve: "deny" },
  { source: "rece", prod: "conditional", rece: "allow", deve: "default" },
  { source: "deve", prod: "deny", rece: "default", deve: "allow" },
];
const matrixColumns: GridColumn[] = [
  { id: "source", label: "Source ↓ · destination →", width: "md" },
  { id: "prod", label: "Production", width: "fill" },
  { id: "rece", label: "Recette", width: "fill" },
  { id: "deve", label: "Développement", width: "fill" },
];
const VERDICT: Record<Verdict, { state: State; label: string }> = {
  allow: { state: "ok", label: "Autoriser" },
  deny: { state: "ko", label: "Interdire" },
  conditional: { state: "warn", label: "Sous motif" },
  default: { state: "neutral", label: "Refus par défaut" },
};
const matrixCell = (row: MatrixRow, column: GridColumn): CellState | undefined =>
  row.source === "rece" && column.id === "prod" ? "draft" : undefined;
const vlanRow = (row: Vlan): RowState | undefined => (row.vid === "104" ? "draft" : undefined);
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
    <Split gap="3">
      <template #side>
        <Panneau title="Groupes" detail="3" flush>
          <DataGrid
            label="Groupes de VLAN"
            :columns="groupColumns"
            :rows="groups"
            row-key="key"
            selectable
            :selected="['campus-ouest']"
          />
        </Panneau>
      </template>
      <Panneau title="campus-ouest" detail="VID 100–199 · ancre site-ouest" main flush>
        <template #actions>
          <Button icon="search">Prochain libre : 105</Button>
          <Button variant="primary" icon="plus" :shortcut="['c']">Créer</Button>
        </template>
        <template #toolbar>
          <Stack gap="2">
            <Cluster gap="2">
              <FilterChip property="Statut" operator="∈" value="Actif, Réservé, Déprécié" />
              <FilterChip property="VID" operator="entre" value="100..149" mono />
            </Cluster>
            <Attributs
              :items="[
                { key: 'used', label: 'Utilisés', value: '87 sur 100' },
                { key: 'free', label: 'Libres', value: '12 (105–110, 140–145)' },
                { key: 'reserved', label: 'Réservés', value: '1' },
              ]"
            />
          </Stack>
        </template>
        <DataGrid
          label="VLAN du groupe campus-ouest"
          :columns="vlanColumns"
          :rows="vlans"
          row-key="vid"
          :sort="{ column: 'vid', direction: 'ascending' }"
          :row-state="vlanRow"
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
    </Split>
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
    <Split side="end" gap="3">
      <Panneau title="Matrice" detail="applicatif" main flush>
        <template #actions>
          <Button variant="ghost" icon="external-link">3 règles hors vue</Button>
        </template>
        <template #toolbar>
          <AVenir component="Select" task="18" inline>
            <Button size="sm" icon="flow">Type de flux : applicatif</Button>
          </AVenir>
        </template>
        <DataGrid
          label="Matrice applicatif : source en ligne, destination en colonne"
          :columns="matrixColumns"
          :rows="matrix"
          row-key="source"
          :cell-state="matrixCell"
          :active="{ row: 'rece', column: 'prod' }"
        >
          <template #cell="{ row, column }">
            <EnvChip v-if="column.id === 'source'" v-bind="ENV[row.source]" />
            <StatusBadge
              v-else
              :state="VERDICT[row[column.id as EnvKey]].state"
              :label="VERDICT[row[column.id as EnvKey]].label"
            />
          </template>
        </DataGrid>
        <template #footer>
          <span>Brouillon · 1 cellule</span>
          <span class="k-num">1 s'ouvre sous réserve · 2 flux cassent · 0 guérit</span>
        </template>
      </Panneau>
      <template #side>
        <Panneau title="Recette → Production">
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
          <template #footer>
            <Button variant="primary" icon="draft" :shortcut="['Enter']"
              >Ajouter au brouillon</Button
            >
          </template>
        </Panneau>
      </template>
    </Split>
  </Cadre>
</template>
