<script setup lang="ts">
// E15 — Matrice au travail (spec 017, task 1; view `matrix` of the plugin `policy`,
// REQ-017-6, 7, 8, 26; Q-017-1 amended, Q-017-7; 010/S35), after E10 (validated
// 2026-10-29), which shows the matrix and its draft tab: here the work on it. A cell
// reached with the arrows (MatrixGrid: crosshair, direction source → destination), its
// state chosen in the right panel (a conditional cell needs its pattern, a denied one its
// text, « Aucune » closes its rule); the cells changed by the draft of the shell; the
// simulated effect of the whole draft under the matrix (sections `policy.combinations` and
// `flow.flows` of the last simulation: counts by nature, first rows, the decisive element
// before → after, « voir tout »); Ctrl+Entrée applies one cell at once (PUT under
// If-Match, motive asked); the matrix read again after a 412 (`policy.matrix.stale`), the
// draft kept; a submission held (202) and its link; a flow type without a living
// environment; the matrix read in the past, without any write (a past system date,
// `as_of`: read only, REQ-021-5).
import {
  Button,
  CodeBadge,
  Cluster,
  CommitForm,
  Dialog,
  DiffValue,
  EmptyState,
  EnvChip,
  ExpandableGrid,
  FormField,
  Icon,
  IconButton,
  InlineAlert,
  MatrixGrid,
  Select,
  Stack,
  StatusBadge,
  TextField,
  type GridColumn,
  type ListOption,
  type MatrixAxis,
  type MatrixCell,
  type MatrixCellState,
  type ToolbarAction,
} from "@kartenn/ui";
import { computed, ref } from "vue";
import { RouterLink, useRoute } from "vue-router";
import Attributs from "./EcranAttributs.vue";
import Barre from "./EcranBarre.vue";
import Cadre from "./EcranCadre.vue";
import Panneau from "./EcranPanneau.vue";
import Soumission from "./EcranSoumission.vue";
import { PAST } from "./donnees.ts";
import { EFFECT, PATTERNS, PEOPLE, SPACES_017, lien, type Effect } from "./donnees017.ts";

const props = defineProps<{ vue: string }>();
const route = useRoute();
const past = props.vue === "passe";
const held = props.vue === "retenue";
const stale = props.vue === "relue";
const empty = props.vue === "vide";

/** The environments, both axes (source in rows, destination in columns), as in E10. */
const ENVS: MatrixAxis[] = [
  { id: "prod", label: "Production" },
  { id: "preprod", label: "Préproduction" },
  { id: "rece", label: "Recette" },
  { id: "inte", label: "Intégration" },
  { id: "deve", label: "Développement" },
];
const HUE = { prod: 1, preprod: 4, rece: 2, inte: 5, deve: 3 } as const;
type EnvId = keyof typeof HUE;
const chip = (id: EnvId) => ({
  value: id,
  label: ENVS.find((e) => e.id === id)!.label,
  hue: HUE[id],
});
/** The matrix of `application` read at the date (in the past, before two rules). */
const MATRIX: Record<string, Effect[]> = {
  prod: ["allow", "deny", "deny", "deny", "deny"],
  preprod: ["conditional", "allow", "default", "deny", "deny"],
  rece: [held ? "conditional" : "default", "default", "allow", "allow", "default"],
  inte: ["deny", "default", stale ? "deny" : "allow", "allow", "allow"],
  deve: ["deny", "deny", "default", "allow", past ? "default" : "allow"],
};
const effectOf = (row: MatrixAxis, column: MatrixAxis): Effect =>
  MATRIX[row.id]![ENVS.indexOf(column)]!;
/** The cells of the draft (two), or in review once the draft is submitted and held. */
const DRAFTED = ["rece:prod", "inte:preprod"];
const cellState = (row: MatrixAxis, column: MatrixAxis): MatrixCellState | undefined => {
  if (past || empty) return undefined;
  const key = `${row.id}:${column.id}`;
  if (DRAFTED.includes(key)) return held ? "review" : "draft";
  return undefined;
};
/** The cell another reader wrote between the read and the submission (412). */
const isChanged = (row: MatrixAxis, column: MatrixAxis) =>
  stale && row.id === "inte" && column.id === "rece";
const active = ref<MatrixCell>({ row: "rece", column: "prod" });

const matrixActions = computed<ToolbarAction[]>(() => [
  { id: "outside", label: "2 règles hors vue", icon: "external-link", priority: 2 },
  {
    id: "simulate",
    label: "Simuler le brouillon",
    icon: "flow",
    priority: 1,
    ...(past ? { reason: "Lecture au passé : aucune écriture ni simulation." } : {}),
  },
  { id: "export", label: "Exporter" },
]);
type FlowTypeId = "application" | "admin" | "database" | "file" | "backup";
const FLOW_TYPES: Record<FlowTypeId, string> = {
  application: "Applicatif",
  admin: "Administratif",
  database: "Base de données",
  file: "Fichier",
  backup: "Sauvegarde",
};
const flowType = ref<string | undefined>(empty ? "backup" : "application");
const flowTypes: ListOption[] = Object.entries(FLOW_TYPES).map(([value, label]) => ({
  value,
  label,
  detail: value,
}));
const shown = computed(() => FLOW_TYPES[(flowType.value ?? "application") as FlowTypeId]);

// ——— The chosen cell, in the right panel ———

const state = ref<Effect>("conditional");
const STATES: { value: Effect | "none"; label: string }[] = [
  { value: "allow", label: "Autoriser" },
  { value: "conditional", label: "Sous motif" },
  { value: "deny", label: "Interdire" },
  { value: "default", label: "Aucune règle" },
];
const PATTERN_CHOICES: ListOption[] = PATTERNS.map((p) => ({
  value: p.key,
  label: p.label,
  detail: p.key,
}));
const pattern = ref<string | undefined>("api-lecture");
const cellLabel = ref("Lecture des API de production depuis la recette");
const text = ref("");

// ——— The simulated effect of the draft, under the matrix ———

interface Combination extends Record<string, unknown> {
  key: string;
  source: string;
  destination: string;
  service: string;
  before: Effect;
  after: Effect;
  rule: string;
}
const COMBINATIONS: Combination[] = [
  {
    key: "c1",
    source: "Recette",
    destination: "Production",
    service: "https",
    before: "default",
    after: "conditional",
    rule: "rece-vers-prod",
  },
  {
    key: "c2",
    source: "Recette",
    destination: "Production",
    service: "api",
    before: "default",
    after: "conditional",
    rule: "rece-vers-prod",
  },
  {
    key: "c3",
    source: "Intégration",
    destination: "Préproduction",
    service: "ssh",
    before: "allow",
    after: "conditional",
    rule: "admin-par-bastion",
  },
];
const combinationColumns: GridColumn[] = [
  { id: "source", label: "Source", width: "md" },
  { id: "destination", label: "Destination", width: "md" },
  { id: "service", label: "Service", width: "sm", mono: true },
  { id: "change", label: "Avant → après", width: "lg" },
  { id: "rule", label: "Élément décisif", width: "md", mono: true, priority: 2 },
];
interface Flow extends Record<string, unknown> {
  key: string;
  flow: string;
  ends: string;
  service: string;
  change: "nonconforming" | "conforming" | "outcome";
  rule: string;
}
const FLOWS: Flow[] = [
  {
    key: "f1",
    flow: "flux-1050",
    ends: "srv-rec-03 · eth0 → srv-app-01 · eth0",
    service: "https",
    change: "conforming",
    rule: "rece-vers-prod",
  },
  {
    key: "f2",
    flow: "flux-0912",
    ends: "int-ci-01 · eth0 → pre-app-02 · eth0",
    service: "ssh",
    change: "nonconforming",
    rule: "flow.verdict.default",
  },
  {
    key: "f3",
    flow: "flux-0913",
    ends: "int-ci-01 · eth0 → pre-db-01 · eth0",
    service: "ssh",
    change: "nonconforming",
    rule: "flow.verdict.default",
  },
  {
    key: "f4",
    flow: "flux-0977",
    ends: "198.51.100.12 → 192.0.2.10",
    service: "api",
    change: "outcome",
    rule: "rece-vers-prod",
  },
];
const CHANGE: Record<Flow["change"], { state: "ok" | "ko" | "warn"; label: string }> = {
  nonconforming: { state: "ko", label: "Devient non conforme" },
  conforming: { state: "ok", label: "Redevient conforme" },
  outcome: { state: "warn", label: "Issue : à instruire" },
};
const flowColumns: GridColumn[] = [
  { id: "flow", label: "Flux", width: "sm", mono: true },
  { id: "ends", label: "Extrémités", width: "lg", mono: true, wrap: true },
  { id: "service", label: "Service", width: "sm", mono: true, priority: 2 },
  { id: "change", label: "Changement", width: "md" },
  { id: "rule", label: "Règle décisive après", width: "md", mono: true, priority: 3 },
];
const effectLink = lien("E16", "combinaisons", route.query);
</script>

<template>
  <Cadre
    :title="`Matrice de politique ${shown.toLowerCase()}`"
    active="matrix"
    space="policy"
    :spaces="SPACES_017"
    :as-of="past ? PAST : undefined"
    :draft="held || past ? 0 : 2"
    :panel="held || empty ? undefined : stale ? 'draft' : 'detail'"
    :crumbs="[
      { label: 'Politique', to: '/x/policy' },
      { label: 'Matrice', to: '/x/policy/matrix' },
      { label: flowType ?? 'application', mono: true },
    ]"
  >
    <Stack gap="3">
      <InlineAlert
        v-if="stale"
        tone="ko"
        title="La matrice a changé depuis votre lecture : elle est relue, votre brouillon est gardé."
        :items="[
          {
            message: `${PEOPLE.lou} a écrit Intégration → Recette à 09:52. Vos deux cellules reprendront la nouvelle version à la soumission.`,
            code: 'policy.matrix.stale',
          },
        ]"
      />
      <InlineAlert
        v-if="held"
        tone="info"
        title="Soumis, retenu en revue : la politique « Ouverture de flux » attend un approbateur."
        :items="[
          {
            message:
              'Une combinaison s\'ouvre sous réserve ; le brouillon est vidé, la matrice relue montre l\'état appliqué.',
            code: 'policy.opens',
          },
        ]"
      >
        <RouterLink :to="effectLink">Voir l'effet du changeset</RouterLink>
      </InlineAlert>

      <Panneau title="Matrice" :detail="shown" main flush>
        <template #toolbar>
          <Barre label="Actions de la matrice" :actions="matrixActions" align="start">
            <Select
              v-model="flowType"
              label="Type de flux"
              :options="flowTypes"
              inline-label
              icon="flow"
            />
          </Barre>
        </template>
        <EmptyState
          v-if="empty"
          icon="environment"
          title="Aucun environnement vivant au 2026-01-15 pour la sauvegarde"
        >
          La matrice d'un type de flux croise les environnements vivants à la date : aucun n'est
          ouvert à la sauvegarde. Les environnements s'ouvrent dans les Paramètres de la politique.
          <template #action>
            <Button icon="settings">Ouvrir les Paramètres de la politique</Button>
          </template>
        </EmptyState>
        <MatrixGrid
          v-else
          v-model:active="active"
          :label="`Matrice ${shown} : source en ligne, destination en colonne`"
          corner="Source → destination"
          :axes="{ rows: 'Source', columns: 'Destination' }"
          :rows="ENVS"
          :columns="ENVS"
          :cell-state="cellState"
        >
          <template #cell="{ row, column }">
            <span class="k-e15-cell">
              <StatusBadge
                :state="EFFECT[effectOf(row, column)].state"
                :label="EFFECT[effectOf(row, column)].label"
              />
              <Icon
                v-if="isChanged(row, column)"
                name="refresh-cw"
                meaningful
                :label="`Changée par ${PEOPLE.lou} depuis votre lecture`"
                class="k-e15-changed"
              />
            </span>
          </template>
        </MatrixGrid>
        <template v-if="!empty" #footer>
          <ul class="k-e15-legend" aria-label="Légende de la matrice">
            <li><Icon name="draft" class="k-e15-legend__draft" />dans le brouillon</li>
            <li><Icon name="eye" class="k-e15-legend__review" />en revue</li>
            <li><span class="k-e15-legend__active"></span>cellule choisie</li>
            <li v-if="stale"><Icon name="refresh-cw" />changée depuis votre lecture</li>
          </ul>
          <span class="k-ecran-muted"
            >Flèches : de cellule en cellule · Entrée : choisir · Ctrl+Entrée : appliquer la seule
            cellule</span
          >
        </template>
      </Panneau>

      <!-- The simulated effect of the whole draft, under the matrix (pied de matrice). -->
      <Panneau
        v-if="!past && !empty && !held"
        title="Effet simulé du brouillon"
        detail="simulation de 09:58 · 2 cellules, 2 types de flux"
      >
        <Stack gap="4">
          <Stack as="section" gap="2" aria-labelledby="k-e15-combinations">
            <Cluster justify="between" :wrap="false" align="baseline">
              <h3 id="k-e15-combinations" class="k-ecran-subtitle">Combinaisons</h3>
              <span class="k-ecran-muted k-num">policy.combinations</span>
            </Cluster>
            <p class="k-num">0 s'ouvre · 3 passent sous réserve · 0 se ferme</p>
            <ExpandableGrid
              label="Combinaisons changées par le brouillon"
              :columns="combinationColumns"
              :rows="COMBINATIONS"
              row-key="key"
              :compact-columns="['source', 'destination', 'change']"
              :visible-rows="3"
            >
              <template #cell="{ row, column, value }">
                <DiffValue v-if="column.id === 'change'" kind="changed">
                  <template #before>
                    <StatusBadge
                      :state="EFFECT[row.before].state"
                      :label="EFFECT[row.before].label"
                    />
                  </template>
                  <StatusBadge :state="EFFECT[row.after].state" :label="EFFECT[row.after].label" />
                </DiffValue>
                <template v-else>{{ value }}</template>
              </template>
            </ExpandableGrid>
          </Stack>
          <Stack as="section" gap="2" aria-labelledby="k-e15-flows">
            <Cluster justify="between" :wrap="false" align="baseline">
              <h3 id="k-e15-flows" class="k-ecran-subtitle">Flux déclarés</h3>
              <span class="k-ecran-muted k-num">flow.flows</span>
            </Cluster>
            <p class="k-num">
              2 deviennent non conformes · 1 redevient conforme · 1 change d'issue
            </p>
            <ExpandableGrid
              label="Flux déclarés dont la conformité change avec le brouillon"
              :columns="flowColumns"
              :rows="FLOWS"
              row-key="key"
              :compact-columns="['flow', 'change']"
              :visible-rows="3"
            >
              <template #cell="{ row, column, value }">
                <StatusBadge
                  v-if="column.id === 'change'"
                  :state="CHANGE[row.change].state"
                  :label="CHANGE[row.change].label"
                />
                <template v-else>{{ value }}</template>
              </template>
            </ExpandableGrid>
          </Stack>
        </Stack>
        <template #footer>
          <span class="k-ecran-muted"
            >Tout le brouillon simulé, matrices des deux types de flux comprises ; le détail complet
            se lit dans l'effet du changeset une fois soumis.</span
          >
        </template>
      </Panneau>
    </Stack>

    <!-- The chosen cell: its state, read or written. -->
    <template #detail>
      <Stack gap="3">
        <h2 class="k-ecran-subtitle k-e15-title">
          <EnvChip v-bind="chip('rece')" />
          <Icon name="arrow-right" class="k-ecran-muted" />
          <span class="k-visually-hidden">vers</span>
          <EnvChip v-bind="chip('prod')" />
        </h2>
        <template v-if="past">
          <Attributs
            :items="[
              { key: 'flowType', label: 'Type de flux', value: shown },
              { key: 'state', label: 'État' },
              { key: 'rule', label: 'Règle' },
              { key: 'at', label: 'Lue au', value: '2025-11-03 09:30', mono: true },
            ]"
          >
            <template #state><StatusBadge state="neutral" label="Refus par défaut" /></template>
            <template #rule><span class="k-ecran-muted">aucune à cette date</span></template>
          </Attributs>
          <p class="k-ecran-muted">
            Lecture au passé : la cellule se lit telle qu'elle était ; on ne l'écrit qu'au présent.
          </p>
        </template>
        <template v-else>
          <Attributs
            :items="[
              { key: 'flowType', label: 'Type de flux', value: shown },
              { key: 'before', label: 'Lue', value: 'Refus par défaut, aucune règle' },
              { key: 'rule', label: 'Règle écrite' },
            ]"
          >
            <template #rule><CodeBadge code="rece-vers-prod" /></template>
          </Attributs>
          <FormField label="État" group required>
            <Cluster gap="1">
              <Button
                v-for="s in STATES"
                :key="s.value"
                size="sm"
                :pressed="state === s.value"
                @click="state = s.value as Effect"
                >{{ s.label }}</Button
              >
            </Cluster>
          </FormField>
          <Select
            v-model="pattern"
            label="Motif exigé"
            :options="PATTERN_CHOICES"
            required
            hint="Une cellule sous motif n'autorise que les flux qui le portent."
          />
          <TextField
            v-model="text"
            label="Texte pédagogique (fr)"
            hint="Exigé pour Interdire ; facultatif ici, le texte du motif s'affiche."
          />
          <TextField v-model="cellLabel" label="Libellé" />
          <p class="k-ecran-muted">
            « Aucune règle » clôt la règle de la cellule : elle redevient un refus par défaut.
          </p>
        </template>
      </Stack>
    </template>
    <template #detail-footer>
      <Cluster justify="end">
        <template v-if="past">
          <Button variant="ghost" reason="Lecture au passé : aucune écriture."
            >Appliquer seule</Button
          >
          <Button variant="primary" icon="draft" reason="Lecture au passé : aucune écriture."
            >Ajouter au brouillon</Button
          >
        </template>
        <template v-else>
          <Button variant="ghost" aria-keyshortcuts="Control+Enter">Appliquer seule</Button>
          <Button variant="primary" icon="draft" :shortcut="['Enter']">Ajouter au brouillon</Button>
        </template>
      </Cluster>
    </template>

    <!-- The draft kept after the 412: its cells will take the new version. -->
    <template #draft>
      <Stack gap="3">
        <InlineAlert
          v-if="stale"
          tone="info"
          title="Vos cellules sont gardées : elles reprennent la matrice relue (version m-44) à la soumission."
        />
        <Stack as="ol" gap="2" class="k-ecran-list" aria-label="Cellules du brouillon">
          <li class="k-ecran-draft-item k-e15-op">
            <Cluster justify="between" :wrap="false" gap="2">
              <span class="k-e15-title">
                <span class="k-ecran-muted">Applicatif</span>
                <EnvChip v-bind="chip('rece')" />
                <Icon name="arrow-right" class="k-ecran-muted" />
                <EnvChip v-bind="chip('prod')" />
              </span>
              <IconButton icon="x" size="sm" label="Retirer la cellule Recette → Production" />
            </Cluster>
            <DiffValue kind="changed">
              <template #before><StatusBadge state="neutral" label="Refus par défaut" /></template>
              <StatusBadge state="warn" label="Sous motif" />
            </DiffValue>
          </li>
          <li class="k-ecran-draft-item k-e15-op">
            <Cluster justify="between" :wrap="false" gap="2">
              <span class="k-e15-title">
                <span class="k-ecran-muted">Administratif</span>
                <EnvChip v-bind="chip('inte')" />
                <Icon name="arrow-right" class="k-ecran-muted" />
                <EnvChip v-bind="chip('preprod')" />
              </span>
              <IconButton
                icon="x"
                size="sm"
                label="Retirer la cellule Intégration → Préproduction"
              />
            </Cluster>
            <DiffValue kind="changed">
              <template #before><StatusBadge state="ok" label="Autoriser" /></template>
              <StatusBadge state="warn" label="Sous motif" />
            </DiffValue>
          </li>
        </Stack>
      </Stack>
    </template>
    <template #draft-footer>
      <Soumission held policy="Ouverture de flux" summary="Recette vers production sous motif" />
    </template>

    <Dialog
      v-if="props.vue === 'appliquer'"
      :open="true"
      title="Appliquer la cellule Recette → Production"
      description="La seule cellule s'écrit tout de suite, sous la version lue de la matrice ; les autres opérations du brouillon y restent."
    >
      <Stack gap="3">
        <Attributs
          :items="[
            { key: 'flowType', label: 'Type de flux', value: 'Applicatif' },
            { key: 'state', label: 'État' },
            { key: 'version', label: 'Version lue', value: 'm-43', mono: true },
          ]"
        >
          <template #state>
            <DiffValue kind="changed">
              <template #before><StatusBadge state="neutral" label="Refus par défaut" /></template>
              <StatusBadge state="warn" label="Sous motif api-lecture" />
            </DiffValue>
          </template>
        </Attributs>
        <CommitForm summary="Lecture des API de production depuis la recette" required />
      </Stack>
      <template #actions>
        <Button variant="ghost">Annuler</Button>
        <Button variant="primary" icon="circle-check" :shortcut="['Control', 'Enter']"
          >Appliquer</Button
        >
      </template>
    </Dialog>
  </Cadre>
</template>

<style>
@layer components {
  .k-ecran-panel__body[data-flush] > .k-matrix {
    border: 0;
    border-radius: 0;
  }

  .k-e15-cell {
    display: inline-flex;
    gap: var(--k-space-1);
    align-items: center;
  }

  .k-e15-changed {
    color: var(--k-state-warn-fg);
  }

  .k-e15-title {
    display: inline-flex;
    flex-wrap: wrap;
    gap: var(--k-space-1);
    align-items: center;
    min-inline-size: 0;
  }

  .k-e15-op {
    display: grid;
    gap: var(--k-space-1);
  }

  .k-e15-legend {
    display: flex;
    flex-wrap: wrap;
    gap: var(--k-space-1) var(--k-space-3);
    font-size: var(--k-text-sm);
    line-height: var(--k-text-sm-line);
    color: var(--k-text-muted);
    list-style: none;
  }

  .k-e15-legend > li {
    display: inline-flex;
    gap: var(--k-space-1);
    align-items: center;
  }

  .k-e15-legend__draft {
    color: var(--k-accent);
  }

  /* A sample of the edge of the chosen cell, as in E10. */
  .k-e15-legend__active {
    inline-size: var(--k-space-4);
    block-size: var(--k-space-3);
    background-color: var(--k-surface);
    border: var(--k-border-width) solid var(--k-border);
    box-shadow: var(--k-shadow-inset-accent);
  }

  .k-e15-legend__review {
    color: var(--k-state-review-fg);
  }
}
</style>
