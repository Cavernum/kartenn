<script setup lang="ts">
// E10 — Hôte de plugin (design « Maquettes »; 021, « Plugin Politique »; 024/M8, M12): a
// full-page view of a plugin in the frame of the shell (the VLAN screens are core screens
// since the 026: E12), the policy matrix of the 017 between
// environments for one flow type (the matrix is per flow type, 017) in its two-way grid
// (MatrixGrid: row and column headers of the same make, both sticky; a crosshair on the
// hovered or active cell, the direction source → destination in the corner and in
// arrowheads on the borders of the crosshair), the chosen cell in the right panel of the
// frame with its simulated effect. Its changes go to the draft of the shell, which here
// changes the matrices of two flow types; its tab lists the changed cells with their flow
// type, source → destination, state before and after, the simulated effect (counts, then
// the flows that become nonconforming grouped by flow type, one compact grid per type that
// opens large: ExpandableGrid; only the types whose matrix the draft changes), the other
// operations of the draft, and the common foot.
import {
  Button,
  CodeBadge,
  Cluster,
  DiffValue,
  EnvChip,
  ExpandableGrid,
  Icon,
  IconButton,
  InlineAlert,
  MatrixGrid,
  Stack,
  StatusBadge,
  TextField,
  type GridColumn,
  type GridSort,
  type MatrixAxis,
  type MatrixCell,
  type MatrixCellState,
  type State,
  type ToolbarAction,
} from "@kartenn/ui";
import { computed, ref } from "vue";
import AVenir from "./AVenir.vue";
import Attributs from "./EcranAttributs.vue";
import Barre from "./EcranBarre.vue";
import Brouillon from "./EcranBrouillon.vue";
import Cadre from "./EcranCadre.vue";
import Panneau from "./EcranPanneau.vue";
import Soumission from "./EcranSoumission.vue";

const props = defineProps<{ vue: string }>();

type Verdict = "allow" | "deny" | "conditional" | "default";
/** The environments, both axes of the matrix (source in rows, destination in columns). */
const ENVS: MatrixAxis[] = [
  { id: "prod", label: "Production" },
  { id: "preprod", label: "Préproduction" },
  { id: "rece", label: "Recette" },
  { id: "inte", label: "Intégration" },
  { id: "deve", label: "Développement" },
];
/** The hue of each environment (REQ-020-8), for its chip in the draft tab. */
const HUE = { prod: 1, preprod: 4, rece: 2, inte: 5, deve: 3 } as const;
const env = (id: keyof typeof HUE) => ({
  value: id,
  label: ENVS.find((e) => e.id === id)!.label,
  hue: HUE[id],
});
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

/**
 * The flow types (010, REQ-010-4: the example set, key and label). The matrix is one per
 * flow type (`/x/policy/matrix?flow_type=<clé>`, 017): the page shows the one of
 * `application`.
 */
type FlowType = "application" | "admin" | "database" | "file";
const FLOW_TYPES: Record<FlowType, string> = {
  application: "Applicatif",
  admin: "Administratif",
  database: "Base de données",
  file: "Fichier",
};
const SHOWN: FlowType = "application";

/**
 * The cells changed by the draft, in the matrices of two flow types: flow type, source,
 * destination, before and after, the pattern and the label added.
 */
const CHANGED = [
  {
    key: "application:rece:prod",
    flowType: "application" as FlowType,
    source: env("rece"),
    destination: env("prod"),
    before: "default" as Verdict,
    after: "conditional" as Verdict,
    pattern: "api-lecture",
    label: "Lecture des API de production depuis la recette",
  },
  {
    key: "admin:inte:preprod",
    flowType: "admin" as FlowType,
    source: env("inte"),
    destination: env("preprod"),
    before: "allow" as Verdict,
    after: "conditional" as Verdict,
    pattern: "bastion-admin",
    label: "Administration de la préproduction par le bastion",
  },
];
const SHOWN_CELL = CHANGED.find((c) => c.flowType === SHOWN)!;

/**
 * The flows that the draft makes nonconforming, each against the matrix of its own flow
 * type: only the types whose matrix the draft changes have some (a flow of an untouched
 * type keeps its verdict). Each with its service (by its key alone: transport and ports
 * belong to the service, read on its card) and the code of the rule that refuses it.
 */
interface Nonconforming extends Record<string, unknown> {
  key: string;
  flowType: FlowType;
  source: string;
  destination: string;
  service: string;
  rule: string;
}
const flows = (
  flowType: FlowType,
  sources: string[],
  targets: [destination: string, service: string][],
): Nonconforming[] =>
  targets.flatMap(([destination, service], t) =>
    sources.map((source, s) => ({
      key: `${flowType}-${t}-${s}`,
      flowType,
      source,
      destination,
      service,
      rule: (t + s) % 3 === 0 ? "flow.verdict.default" : "flow.verdict.denied",
    })),
  );
const NONCONFORMING: Nonconforming[] = [
  ...flows(
    "application",
    ["rece-batch", "rece-ci", "rece-web", "rece-etl", "rece-mon"],
    [
      ["prod-api", "https"],
      ["prod-registry", "registry"],
    ],
  ),
  ...flows(
    "admin",
    ["inte-ci", "inte-web", "inte-mon"],
    [
      ["preprod-app", "ssh"],
      ["preprod-db", "ssh"],
    ],
  ),
];
/**
 * In the panel, the source and the destination; all columns, enlarged. The flow type is
 * the one of the group (its title), not a column.
 */
const COMPACT = ["source", "destination"];
const nonconformingColumns: GridColumn[] = [
  { id: "source", label: "Source", width: "md", mono: true, sortable: true },
  { id: "destination", label: "Destination", width: "fill", mono: true, sortable: true },
  { id: "service", label: "Service", width: "sm", mono: true, sortable: true },
  { id: "rule", label: "Règle", width: "lg", sortable: true },
];
/** The nonconforming flows grouped by flow type, in the order of the changed cells. */
const GROUPS = [...new Set(CHANGED.map((c) => c.flowType))].map((type) => ({
  type,
  label: FLOW_TYPES[type],
  flows: NONCONFORMING.filter((f) => f.flowType === type),
}));
/** The sort of each group's grid. */
const bySource = (): GridSort => ({ column: "source", direction: "ascending" });
const sorts = ref<Record<FlowType, GridSort>>({
  application: bySource(),
  admin: bySource(),
  database: bySource(),
  file: bySource(),
});
const sorted = (type: FlowType, rows: Nonconforming[]): Nonconforming[] => {
  const sort = sorts.value[type];
  const way = sort.direction === "ascending" ? 1 : -1;
  return [...rows].sort(
    (a, b) => way * String(a[sort.column]).localeCompare(String(b[sort.column])),
  );
};
const shownFlows = NONCONFORMING.filter((f) => f.flowType === SHOWN);
const ofShown = computed(() => sorted(SHOWN, shownFlows));
/** The simulated effect of the whole draft, each flow against the matrix of its type. */
const EFFECT = [
  { key: "opens", label: "S'ouvrent sous réserve", value: String(CHANGED.length), mono: true },
  {
    key: "nonconforming",
    label: "Non conformes",
    value: String(NONCONFORMING.length),
    mono: true,
  },
  { key: "conforming", label: "Conformes de nouveau", value: "0", mono: true },
];
</script>

<template>
  <Cadre
    title="Matrice de politique applicatif"
    active="matrix"
    space="policy"
    :draft="3"
    :panel="props.vue === 'brouillon' ? 'draft' : 'detail'"
    :crumbs="[
      { label: 'Politique', to: '/x/policy' },
      { label: 'Matrice', to: '/x/policy/matrix' },
      { label: SHOWN, mono: true },
    ]"
  >
    <Panneau title="Matrice" :detail="FLOW_TYPES[SHOWN]" main flush>
      <template #toolbar>
        <Barre label="Actions de la matrice" :actions="matrixActions" align="start">
          <AVenir component="Select" task="18" inline>
            <Button size="sm" icon="flow">Type de flux : {{ FLOW_TYPES[SHOWN] }}</Button>
          </AVenir>
        </Barre>
      </template>
      <MatrixGrid
        v-model:active="active"
        :label="`Matrice ${FLOW_TYPES[SHOWN]} : source en ligne, destination en colonne`"
        corner="Source → destination"
        :axes="{ rows: 'Source', columns: 'Destination' }"
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
        <!-- What the marks of a cell say, in words (relecture du 2026-10-26). -->
        <ul class="k-ecran-legende" aria-label="Légende de la matrice">
          <li><Icon name="draft" class="k-ecran-legende__draft" />dans le brouillon</li>
          <li><Icon name="eye" class="k-ecran-legende__review" />en revue</li>
          <li><span class="k-ecran-legende__active"></span>cellule choisie</li>
        </ul>
        <span>Brouillon · 1 cellule de cette matrice sur {{ CHANGED.length }} · 1 en revue</span>
        <span class="k-num"
          >1 s'ouvre sous réserve · {{ shownFlows.length }} non conformes à cette matrice · 0
          conforme de nouveau</span
        >
      </template>
    </Panneau>
    <template #detail>
      <Stack gap="3">
        <h2 class="k-ecran-subtitle">
          {{ SHOWN_CELL.source.label }} → {{ SHOWN_CELL.destination.label }}
        </h2>
        <Attributs
          :items="[
            { key: 'flowType', label: 'Type de flux', value: FLOW_TYPES[SHOWN] },
            { key: 'state', label: 'État' },
            { key: 'patterns', label: 'Motifs' },
          ]"
        >
          <template #state><StatusBadge state="warn" label="Sous motif" /></template>
          <template #patterns><CodeBadge :code="SHOWN_CELL.pattern" /></template>
        </Attributs>
        <TextField label="Libellé" :model-value="SHOWN_CELL.label" />
        <InlineAlert
          tone="warn"
          :title="`Effet simulé : ${shownFlows.length} flux ${FLOW_TYPES[SHOWN].toLowerCase()}s deviennent non conformes à la matrice`"
        />
        <ExpandableGrid
          v-model:sort="sorts[SHOWN]"
          :label="`Flux ${FLOW_TYPES[SHOWN].toLowerCase()}s non conformes à la matrice par ${SHOWN_CELL.source.label} → ${SHOWN_CELL.destination.label}`"
          :columns="nonconformingColumns"
          :rows="ofShown"
          row-key="key"
          :compact-columns="COMPACT"
          :visible-rows="3"
        >
          <template #cell="{ column, value }">
            <CodeBadge v-if="column.id === 'rule'" :code="String(value)" />
            <template v-else>{{ value }}</template>
          </template>
        </ExpandableGrid>
      </Stack>
    </template>
    <template #detail-footer>
      <Cluster justify="end">
        <Button variant="primary" icon="draft" :shortcut="['Enter']">Ajouter au brouillon</Button>
      </Cluster>
    </template>
    <template #draft>
      <Stack gap="4">
        <Stack as="section" gap="2" aria-labelledby="k-e10-cells">
          <Cluster justify="between" :wrap="false">
            <h2 id="k-e10-cells" class="k-ecran-subtitle">Cellules changées</h2>
            <span class="k-ecran-muted k-num">{{ CHANGED.length }} cellules</span>
          </Cluster>
          <Stack as="ol" gap="3" class="k-ecran-list">
            <li v-for="cell in CHANGED" :key="cell.key" class="k-ecran-cell k-ecran-draft-item">
              <Stack gap="2">
                <Cluster justify="between" :wrap="false" gap="2">
                  <span class="k-ecran-cell__flow">
                    <span class="k-visually-hidden">De</span>
                    <EnvChip v-bind="cell.source" />
                    <Icon name="arrow-right" class="k-ecran-cell__arrow" />
                    <span class="k-visually-hidden">vers</span>
                    <EnvChip v-bind="cell.destination" />
                  </span>
                  <IconButton
                    icon="x"
                    size="sm"
                    :label="`Retirer la cellule ${cell.source.label} → ${cell.destination.label} de la matrice ${FLOW_TYPES[cell.flowType]}`"
                  />
                </Cluster>
                <Attributs
                  :items="[
                    { key: 'flowType', label: 'Type de flux', value: FLOW_TYPES[cell.flowType] },
                    { key: 'state', label: 'État' },
                    { key: 'pattern', label: 'Motif' },
                    { key: 'label', label: 'Libellé' },
                  ]"
                >
                  <template #state>
                    <DiffValue kind="changed">
                      <template #before>
                        <StatusBadge
                          :state="VERDICT[cell.before].state"
                          :label="VERDICT[cell.before].label"
                        />
                      </template>
                      <StatusBadge
                        :state="VERDICT[cell.after].state"
                        :label="VERDICT[cell.after].label"
                      />
                    </DiffValue>
                  </template>
                  <template #pattern>
                    <DiffValue kind="added"><CodeBadge :code="cell.pattern" /></DiffValue>
                  </template>
                  <template #label>
                    <DiffValue kind="added">{{ cell.label }}</DiffValue>
                  </template>
                </Attributs>
              </Stack>
            </li>
          </Stack>
        </Stack>
        <Stack as="section" gap="2" aria-labelledby="k-e10-effect">
          <h2 id="k-e10-effect" class="k-ecran-subtitle">Effet simulé</h2>
          <Attributs :items="EFFECT" />
          <InlineAlert
            tone="warn"
            :title="`${NONCONFORMING.length} flux deviennent non conformes à la matrice de leur type`"
          >
            Ils étaient conformes avant ce brouillon. Seuls les types de flux dont il change la
            matrice sont touchés ; la règle qui refuse chaque flux est dans sa liste.
          </InlineAlert>
          <Stack
            v-for="group in GROUPS"
            :key="group.type"
            as="section"
            gap="2"
            :aria-labelledby="`k-e10-effect-${group.type}`"
          >
            <Cluster justify="between" :wrap="false" align="baseline">
              <h3 :id="`k-e10-effect-${group.type}`" class="k-ecran-subtitle">
                {{ group.label }}
              </h3>
              <span class="k-ecran-muted k-num">{{ group.flows.length }} non conformes</span>
            </Cluster>
            <ExpandableGrid
              v-model:sort="sorts[group.type]"
              :label="`Flux de type ${group.label} non conformes à leur matrice après ce brouillon`"
              :columns="nonconformingColumns"
              :rows="sorted(group.type, group.flows)"
              row-key="key"
              :compact-columns="COMPACT"
            >
              <template #cell="{ column, value }">
                <CodeBadge v-if="column.id === 'rule'" :code="String(value)" />
                <template v-else>{{ value }}</template>
              </template>
            </ExpandableGrid>
          </Stack>
        </Stack>
        <Stack as="section" gap="2" aria-labelledby="k-e10-others">
          <Cluster justify="between" :wrap="false">
            <h2 id="k-e10-others" class="k-ecran-subtitle">Autres opérations</h2>
            <span class="k-ecran-muted k-num">1</span>
          </Cluster>
          <Brouillon :count="1" />
        </Stack>
      </Stack>
    </template>
    <template #draft-footer>
      <Soumission
        held
        policy="Changements de matrice"
        summary="Accès sous motif, matrices applicatif et administratif"
      />
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

  /* A changed cell of the draft: source → destination, then its changes. */
  .k-ecran-cell {
    padding-block-end: var(--k-space-3);
    border-block-end: var(--k-border-width) solid var(--k-border);
  }

  .k-ecran-cell__flow {
    display: inline-flex;
    flex-wrap: wrap;
    gap: var(--k-space-1);
    align-items: center;
    min-inline-size: 0;
  }

  .k-ecran-cell__arrow {
    flex: none;
    color: var(--k-text-muted);
  }

  /* The legend of the matrix: each mark of a cell and its word, on one wrapping line. */
  .k-ecran-legende {
    display: flex;
    flex-basis: 100%;
    flex-wrap: wrap;
    gap: var(--k-space-1) var(--k-space-3);
    font-size: var(--k-text-sm);
    line-height: var(--k-text-sm-line);
    color: var(--k-text-muted);
    list-style: none;
  }

  .k-ecran-legende > li {
    display: inline-flex;
    gap: var(--k-space-1);
    align-items: center;
  }

  .k-ecran-legende__draft {
    color: var(--k-accent);
  }

  .k-ecran-legende__review {
    color: var(--k-state-review-fg);
  }

  /* A sample of the edge of the chosen cell. */
  .k-ecran-legende__active {
    inline-size: var(--k-space-4);
    block-size: var(--k-space-3);
    background-color: var(--k-surface);
    border: var(--k-border-width) solid var(--k-border);
    box-shadow: var(--k-shadow-inset-accent);
  }
}
</style>
