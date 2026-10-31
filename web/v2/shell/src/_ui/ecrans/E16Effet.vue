<script setup lang="ts">
// E16 — Effet d'un changement de politique (spec 017, task 1; view `changes` of the plugin
// `policy` and action `effect` on a changeset, REQ-017-9; Q-017-1): what an approver of a
// policy of approval (`policy.opens`, `policy.closes`, `flow.nonconforming`) validates.
// The header of the changeset (motive, state, author, the policy that holds it), then its
// sections read page by page (`GET /v1/changesets/{id}/diff?section=`, `links.next`), one
// tab each: the combinations that open, close or become conditional, with the decisive
// element before and after; the nodes reclassified and their heirs; the declared flows
// whose conformity changes (only when the module `flow` is present), grouped by flow type
// as in E10. The validation itself happens on the card of the changeset (E4), where the
// action « Effet sur la politique » leads here.
import {
  Button,
  CodeBadge,
  DataGrid,
  DiffValue,
  EnvChip,
  Grid,
  KeyLabel,
  Stack,
  StatusBadge,
  Tabs,
  type GridColumn,
  type GridSort,
  type State,
  type TabItem,
  type ToolbarAction,
} from "@kartenn/ui";
import { ref } from "vue";
import Attributs from "./EcranAttributs.vue";
import Barre from "./EcranBarre.vue";
import Cadre from "./EcranCadre.vue";
import Panneau from "./EcranPanneau.vue";
import { ENV, FLOW_TYPE, SERVICES, type EnvKey, type FlowTypeKey } from "./donnees.ts";
import { CHANGESET, PEOPLE, SPACES_017 } from "./donnees017.ts";

const props = defineProps<{ vue: string }>();
const tab = ref(
  props.vue === "reclasses" ? "reclassified" : props.vue === "flux" ? "flows" : "combinations",
);
const tabs: TabItem[] = [
  { value: "combinations", label: "Combinaisons", count: 312 },
  { value: "reclassified", label: "Nœuds reclassés", count: 2 },
  { value: "flows", label: "Flux déclarés", count: 7 },
];
const actions: ToolbarAction[] = [
  {
    id: "open",
    label: "Ouvrir le changeset",
    icon: "changeset",
    variant: "primary",
    priority: 3,
  },
  { id: "copy", label: "Copier l'identifiant", icon: "copy", priority: 1 },
];

// ——— policy.combinations ———

type Nature = "opens" | "closes" | "pending";
const NATURE: Record<Nature, { state: State; label: string }> = {
  opens: { state: "ok", label: "S'ouvre" },
  closes: { state: "ko", label: "Se ferme" },
  pending: { state: "warn", label: "Sous réserve" },
};
interface Combination extends Record<string, unknown> {
  key: string;
  nature: Nature;
  source: EnvKey;
  destination: EnvKey;
  service: string;
  before: string;
  after: string;
}
const combinations: Combination[] = [
  ["pending", "rece", "prod", "https", "default", "rece-vers-prod"],
  ["pending", "rece", "prod", "api", "default", "rece-vers-prod"],
  ["opens", "admin", "rece", "ssh", "default", "admin-par-bastion"],
  ["opens", "admin", "deve", "ssh", "default", "admin-par-bastion"],
  ["closes", "deve", "rece", "postgres", "bdd-meme-env", "deny-deve-rece-bdd"],
  ["closes", "deve", "rece", "nfs", "fichier-deve", "deny-deve-rece-bdd"],
].map(([nature, source, destination, service, before, after], i) => ({
  key: `c${i}`,
  nature: nature as Nature,
  source: source as EnvKey,
  destination: destination as EnvKey,
  service: service!,
  before: before === "default" ? "flow.verdict.default" : before!,
  after: after!,
}));
const combinationColumns: GridColumn[] = [
  { id: "nature", label: "Nature", width: "md", sortable: true },
  { id: "source", label: "Source", width: "md", sortable: true },
  { id: "destination", label: "Destination", width: "md", sortable: true },
  { id: "service", label: "Service", width: "md", sortable: true },
  { id: "decisive", label: "Élément décisif, avant → après", width: "fill", wrap: true },
];

// ——— policy.reclassified ———

interface Reclassified extends Record<string, unknown> {
  key: string;
  node: string;
  axis: string;
  before: EnvKey;
  after: EnvKey;
  heirs: number;
}
const reclassified: Reclassified[] = [
  {
    key: "r1",
    node: "198.51.100.0/26",
    axis: "policy.environment",
    before: "rece",
    after: "prod",
    heirs: 41,
  },
  {
    key: "r2",
    node: "2001:db8:20::/64",
    axis: "policy.environment",
    before: "rece",
    after: "prod",
    heirs: 12,
  },
];
const reclassifiedColumns: GridColumn[] = [
  { id: "node", label: "Nœud", width: "md", mono: true, sortable: true },
  { id: "axis", label: "Axe", width: "md" },
  { id: "change", label: "Valeur, avant → après", width: "fill" },
  { id: "heirs", label: "Héritiers reclassés", width: "sm", align: "end", sortable: true },
];

// ——— flow.flows ———

type Change = "nonconforming" | "conforming" | "outcome";
const CHANGE: Record<Change, { state: State; label: string }> = {
  nonconforming: { state: "ko", label: "Devient non conforme" },
  conforming: { state: "ok", label: "Redevient conforme" },
  outcome: { state: "review", label: "Issue : à instruire" },
};
interface Flow extends Record<string, unknown> {
  key: string;
  type: FlowTypeKey;
  source: string;
  destination: string;
  service: string;
  change: Change;
  rule: string;
}
const flows: Flow[] = [
  [
    "application",
    "srv-rec-03 · eth0",
    "srv-app-01 · eth0",
    "https",
    "conforming",
    "rece-vers-prod",
  ],
  ["application", "198.51.100.12", "192.0.2.10", "api", "outcome", "rece-vers-prod"],
  ["application", "198.51.100.14", "192.0.2.10", "api", "outcome", "rece-vers-prod"],
  [
    "database",
    "dev-web-01 · eth0",
    "198.51.100.30",
    "postgres",
    "nonconforming",
    "deny-deve-rece-bdd",
  ],
  [
    "database",
    "dev-web-02 · eth0",
    "198.51.100.30",
    "postgres",
    "nonconforming",
    "deny-deve-rece-bdd",
  ],
  ["file", "dev-web-01 · eth0", "198.51.100.40", "nfs", "nonconforming", "deny-deve-rece-bdd"],
  ["file", "192.0.2.128/28", "198.51.100.40", "nfs", "nonconforming", "deny-deve-rece-bdd"],
].map(([type, source, destination, service, change, rule], i) => ({
  key: `f${i}`,
  type: type as FlowTypeKey,
  source: source!,
  destination: destination!,
  service: service!,
  change: change as Change,
  rule: rule!,
}));
/** One grid, the flow type a column of its own (sorted by it: grouped). */
const flowColumns: GridColumn[] = [
  { id: "type", label: "Type de flux", width: "md", sortable: true },
  { id: "source", label: "Source", width: "md", mono: true, wrap: true, sortable: true },
  { id: "destination", label: "Destination", width: "md", mono: true, sortable: true },
  { id: "service", label: "Service", width: "md", sortable: true, priority: 3 },
  { id: "change", label: "Changement", width: "lg", sortable: true },
  { id: "rule", label: "Règle décisive après", width: "fill", mono: true, priority: 2 },
];
const flowSort = ref<GridSort | undefined>({ column: "type", direction: "ascending" });
</script>

<template>
  <Cadre
    title="Effet sur la politique : Recette vers production sous motif"
    active="matrix"
    space="policy"
    :spaces="SPACES_017"
    :draft="0"
    :crumbs="[
      { label: 'Politique', to: '/x/policy' },
      { label: 'Effet d\'un changement' },
      { label: CHANGESET.policy.slice(0, 10), mono: true },
    ]"
  >
    <Panneau title="Recette vers production sous motif" detail="effet sur la politique" main>
      <template #toolbar>
        <Barre label="Actions sur le changeset" :actions="actions">
          <StatusBadge state="review" label="En revue" />
        </Barre>
      </template>
      <Grid columns="auto" gap="3" class="k-ecran-attrs-tiles">
        <Attributs
          :items="[
            { key: 'author', label: 'Auteur', value: PEOPLE.sacha },
            { key: 'submitted', label: 'Soumis', value: '2026-01-15 09:58', mono: true },
          ]"
        />
        <Attributs
          :items="[
            { key: 'policy', label: 'Retenu par', value: 'Ouverture de flux' },
            { key: 'rule', label: 'Déclencheur' },
          ]"
        >
          <template #rule><CodeBadge code="policy.opens" /></template>
        </Attributs>
        <Attributs
          :items="[
            { key: 'operations', label: 'Opérations', value: '2 cellules de matrice' },
            { key: 'id', label: 'Identifiant' },
          ]"
        >
          <template #id><CodeBadge :code="CHANGESET.policy" /></template>
        </Attributs>
      </Grid>
      <Tabs v-model="tab" :tabs="tabs" label="Sections du changeset">
        <template #combinations>
          <Stack gap="2">
            <p class="k-num">
              4 s'ouvrent · 2 passent sous réserve · 306 se ferment
              <span class="k-ecran-muted">· section policy.combinations</span>
            </p>
            <DataGrid
              label="Combinaisons changées par le changeset, page 1"
              :columns="combinationColumns"
              :rows="combinations"
              row-key="key"
            >
              <template #cell="{ row, column }">
                <StatusBadge
                  v-if="column.id === 'nature'"
                  :state="NATURE[row.nature].state"
                  :label="NATURE[row.nature].label"
                />
                <EnvChip v-else-if="column.id === 'source'" v-bind="ENV[row.source]" />
                <EnvChip v-else-if="column.id === 'destination'" v-bind="ENV[row.destination]" />
                <KeyLabel
                  v-else-if="column.id === 'service'"
                  :value="row.service"
                  :label="SERVICES[row.service]?.label ?? null"
                  show-key
                />
                <DiffValue v-else kind="changed">
                  <template #before
                    ><code>{{ row.before }}</code></template
                  >
                  <code>{{ row.after }}</code>
                </DiffValue>
              </template>
            </DataGrid>
          </Stack>
        </template>
        <template #reclassified>
          <Stack gap="2">
            <p class="k-num">
              2 nœuds reclassés, 53 héritiers avec eux
              <span class="k-ecran-muted">· section policy.reclassified</span>
            </p>
            <DataGrid
              label="Nœuds reclassés par le changeset"
              :columns="reclassifiedColumns"
              :rows="reclassified"
              row-key="key"
            >
              <template #cell="{ row, column, value }">
                <a v-if="column.id === 'node'" :href="`/prefixes/${row.node}`">{{ row.node }}</a>
                <KeyLabel
                  v-else-if="column.id === 'axis'"
                  :value="row.axis"
                  label="Environnement"
                />
                <DiffValue v-else-if="column.id === 'change'" kind="changed">
                  <template #before><EnvChip v-bind="ENV[row.before]" /></template>
                  <EnvChip v-bind="ENV[row.after]" />
                </DiffValue>
                <template v-else>{{ value }}</template>
              </template>
            </DataGrid>
          </Stack>
        </template>
        <template #flows>
          <Stack gap="3">
            <p class="k-num">
              5 deviennent non conformes · 1 redevient conforme · 2 changent d'issue
              <span class="k-ecran-muted">· section flow.flows</span>
            </p>
            <DataGrid
              v-model:sort="flowSort"
              label="Flux déclarés dont la conformité change, par type de flux"
              :columns="flowColumns"
              :rows="flows"
              row-key="key"
            >
              <template #cell="{ row, column, value }">
                <template v-if="column.id === 'type'">{{ FLOW_TYPE[row.type] }}</template>
                <KeyLabel
                  v-else-if="column.id === 'service'"
                  :value="row.service"
                  :label="SERVICES[row.service]?.label ?? null"
                  show-key
                />
                <StatusBadge
                  v-else-if="column.id === 'change'"
                  :state="CHANGE[row.change].state"
                  :label="CHANGE[row.change].label"
                />
                <template v-else>{{ value }}</template>
              </template>
            </DataGrid>
          </Stack>
        </template>
      </Tabs>
      <template #footer>
        <span v-if="tab === 'combinations'" class="k-num"
          >Page 1 sur 4 · 100 lignes par page (extrait)</span
        >
        <span v-else-if="tab === 'reclassified'" class="k-num">2 sur 2</span>
        <span v-else class="k-num">7 sur 7</span>
        <Button v-if="tab === 'combinations'" size="sm" icon="chevron-right" :shortcut="['n']"
          >Page suivante</Button
        >
      </template>
    </Panneau>
  </Cadre>
</template>
