<script setup lang="ts">
// E20 — Rapports d'audit (spec 017, task 1; view `reports` of the plugin `policy`,
// REQ-017-14; 010, REQ-010-22, 23): the dated reports (`GET /v1/policy-reports`, task
// 021-30), their state (`pending`, `running`, `done`), the request of a report at a date
// (`POST`), whose 409 leads to the report under way; a report read every 5 s while it
// runs; a finished report, its count per rubric (a rubric the build does not serve is
// « indisponible », never 0), the detail of a rubric page by page (100 rows), each row
// linked to its card, and the export of a rubric as JSON (pages concatenated by the
// browser). All under `policy.rule.admin`.
import {
  Button,
  CodeBadge,
  Cluster,
  DataGrid,
  InlineAlert,
  KeyLabel,
  Skeleton,
  Stack,
  StatusBadge,
  TextField,
  type GridColumn,
  type GridSort,
  type State,
} from "@kartenn/ui";
import { ref } from "vue";
import AVenir from "./AVenir.vue";
import Attributs from "./EcranAttributs.vue";
import Cadre from "./EcranCadre.vue";
import Panneau from "./EcranPanneau.vue";
import { ME, PEOPLE, SPACES_017 } from "./donnees017.ts";

const props = defineProps<{ vue: string }>();
const report = props.vue === "rapport";

type ReportState = "pending" | "running" | "done";
const STATE: Record<ReportState, { state: State; label: string }> = {
  pending: { state: "neutral", label: "En attente" },
  running: { state: "info", label: "En cours" },
  done: { state: "ok", label: "Fait" },
};
interface Report extends Record<string, unknown> {
  key: string;
  validAt: string;
  state: ReportState;
  by: string;
  requested: string;
}
const reports: Report[] = [
  {
    key: "rpt-0042",
    validAt: "2026-01-15",
    state: "running",
    by: PEOPLE.sacha,
    requested: "2026-01-15 09:57",
  },
  { key: "rpt-0041", validAt: "2026-01-01", state: "done", by: ME, requested: "2026-01-02 08:12" },
  { key: "rpt-0040", validAt: "2025-12-01", state: "done", by: ME, requested: "2025-12-01 08:05" },
  {
    key: "rpt-0039",
    validAt: "2025-11-01",
    state: "done",
    by: PEOPLE.sacha,
    requested: "2025-11-03 10:40",
  },
];
const reportColumns: GridColumn[] = [
  { id: "key", label: "Rapport", width: "md", mono: true, sortable: true },
  { id: "validAt", label: "À la date du", width: "md", mono: true, sortable: true },
  { id: "state", label: "État", width: "md", sortable: true },
  { id: "by", label: "Demandé par", width: "fill" },
  { id: "requested", label: "Demandé le", width: "md", mono: true, priority: 3 },
];
const reportSort = ref<GridSort | undefined>({ column: "validAt", direction: "descending" });

interface Rubric extends Record<string, unknown> {
  key: string;
  label: string;
  count: number | null;
}
const rubrics: Rubric[] = [
  { key: "unclassified", label: "Non classifiés", count: 12 },
  { key: "noncompliant_flows", label: "Flux non conformes", count: 4 },
  { key: "outside_rules", label: "Règles hors matrice", count: 1 },
  { key: "inference_mismatches", label: "Écarts d'inférence", count: 5 },
  { key: "expired_waivers", label: "Dérogations échues", count: 0 },
  { key: "unused_rules", label: "Règles sans effet", count: null },
];
const rubricColumns: GridColumn[] = [
  { id: "label", label: "Rubrique", width: "fill" },
  { id: "count", label: "Décompte", width: "md", align: "end", sortable: true },
];
interface Node extends Record<string, unknown> {
  node: string;
  kind: string;
  owner: string;
  missing: string;
}
const nodes: Node[] = [
  "192.0.2.200",
  "192.0.2.201",
  "192.0.2.202",
  "192.0.2.208/29",
  "198.51.100.96/28",
  "198.51.100.100",
  "198.51.100.101",
  "2001:db8:30::/64",
  "2001:db8:30::10",
  "2001:db8:30::11",
  "203.0.113.64/27",
  "203.0.113.70",
].map((node) => ({
  node,
  kind: node.includes("/") ? "Préfixe" : "Adresse",
  owner: node.startsWith("198") ? "Atelier Sud" : "Atelier Nord",
  missing: "policy.environment",
}));
const nodeColumns: GridColumn[] = [
  { id: "node", label: "Nœud", width: "md", mono: true, sortable: true },
  { id: "kind", label: "Sorte", width: "sm", sortable: true },
  { id: "owner", label: "Propriétaire", width: "md", priority: 2 },
  { id: "missing", label: "Axe sans valeur", width: "fill", mono: true, priority: 3 },
];
</script>

<template>
  <Cadre
    title="Rapports d'audit"
    active="reports"
    space="policy"
    :spaces="SPACES_017"
    :draft="0"
    :panel="report ? undefined : 'detail'"
    :crumbs="[
      { label: 'Politique', to: '/x/policy' },
      { label: 'Rapports d\'audit', to: '/x/policy/reports' },
      ...(report ? [{ label: 'rpt-0041', mono: true }] : []),
    ]"
  >
    <Panneau v-if="!report" title="Rapports d'audit" detail="politique au présent" main flush>
      <template #toolbar>
        <Stack gap="2">
          <Cluster gap="2" align="end">
            <AVenir component="DateTimeField" task="020-20" inline>
              <TextField label="À la date du" model-value="2026-01-15" mono size="sm" />
            </AVenir>
            <Button variant="primary" icon="plus">Demander un rapport</Button>
          </Cluster>
          <InlineAlert
            tone="info"
            title="Un rapport au 2026-01-15 est déjà en cours : il est ouvert ci-contre."
            :items="[
              {
                message: `Demandé par ${PEOPLE.sacha} à 09:57 ; un seul rapport tourne à la fois.`,
                code: 'urn:kartenn:problem:conflict',
              },
            ]"
          />
        </Stack>
      </template>
      <DataGrid
        v-model:sort="reportSort"
        label="Rapports d'audit"
        :columns="reportColumns"
        :rows="reports"
        row-key="key"
        selectable="single"
        :selected="['rpt-0042']"
      >
        <template #cell="{ row, column, value }">
          <StatusBadge
            v-if="column.id === 'state'"
            :state="STATE[row.state].state"
            :label="STATE[row.state].label"
          />
          <template v-else>{{ value }}</template>
        </template>
      </DataGrid>
      <template #footer>
        <span class="k-num">4 rapports</span>
        <span class="k-ecran-muted">Un rapport en cours est relu toutes les 5 s</span>
      </template>
    </Panneau>

    <Stack v-else gap="3">
      <Panneau title="rpt-0041" detail="au 2026-01-01 · fait le 2026-01-02 à 08:14" main flush>
        <template #actions>
          <Button icon="copy">Copier l'identifiant</Button>
        </template>
        <DataGrid
          label="Décompte par rubrique"
          :columns="rubricColumns"
          :rows="rubrics"
          row-key="key"
          selectable="single"
          :selected="['unclassified']"
        >
          <template #cell="{ row, column }">
            <KeyLabel v-if="column.id === 'label'" :value="row.key" :label="row.label" show-key />
            <StatusBadge v-else-if="row.count === null" state="neutral" label="Indisponible" />
            <span v-else class="k-num">{{ row.count }}</span>
          </template>
        </DataGrid>
        <template #footer>
          <span class="k-ecran-muted"
            >« Indisponible » : rubrique que ce serveur ne calcule pas, à ne pas lire comme
            zéro.</span
          >
        </template>
      </Panneau>
      <Panneau title="Non classifiés" detail="12 nœuds sans environnement" flush>
        <template #actions>
          <Button icon="external-link">Exporter en JSON</Button>
        </template>
        <DataGrid
          label="Nœuds non classifiés au 2026-01-01"
          :columns="nodeColumns"
          :rows="nodes"
          row-key="node"
        >
          <template #cell="{ row, column, value }">
            <a
              v-if="column.id === 'node'"
              :href="`/${row.kind === 'Préfixe' ? 'prefixes' : 'addresses'}/${row.node}`"
              >{{ row.node }}</a
            >
            <template v-else>{{ value }}</template>
          </template>
        </DataGrid>
        <template #footer>
          <span class="k-num">12 sur 12 · une page</span>
          <span class="k-ecran-muted">Pages de 100, l'export les met bout à bout</span>
        </template>
      </Panneau>
    </Stack>

    <template #detail>
      <Stack gap="3">
        <h2 class="k-ecran-subtitle"><code>rpt-0042</code></h2>
        <Attributs
          :items="[
            { key: 'state', label: 'État' },
            { key: 'at', label: 'À la date du', value: '2026-01-15', mono: true },
            { key: 'by', label: 'Demandé par', value: PEOPLE.sacha },
            { key: 'id', label: 'Lien' },
          ]"
        >
          <template #state><StatusBadge state="info" label="En cours" /></template>
          <template #id><CodeBadge code="/v1/policy-reports/rpt-0042" /></template>
        </Attributs>
        <p class="k-ecran-muted" role="status">
          Relu à 10:00:05 · prochaine lecture dans 5 s. Les décomptes viennent quand il est fait.
        </p>
        <Skeleton variant="text" :rows="6" />
        <p class="k-ecran-muted">
          Pour comparer, le rapport précédent : <a href="#rpt-0041" class="k-mono">rpt-0041</a>,
          avec 12 non classifiés.
        </p>
      </Stack>
    </template>
  </Cadre>
</template>
