<script setup lang="ts">
// E14 — Règles et motifs (spec 017, task 1; view `rules` of the plugin `policy`,
// REQ-017-4, 5; U5): the rules of the policy at the date of the context, filtered by key,
// effect, pattern, flow type and service (typed filters, as the 021 draws them); the chosen
// rule read in words in the right panel (« interdire, source {environment: deve} … »), its
// text by language and its history; the form of a rule (selectors as « axis → values »,
// « same value », flow types and services, effect, patterns for `allow` only, text by
// language, `fr` required for `deny`), refused by the client before it is sent and, once
// forced, by the server, each violation on its field; the 412 of a second tab, input kept;
// the patterns. A rule the matrix does not show (several axes) is marked « hors matrice »
// (REQ-017-6: a rule is in the view when a cell cites it). Writes go to the draft of the
// shell (amendment of 2026-10-23), the route of the resource with its `If-Match` at the
// submission.
import {
  Button,
  Checkbox,
  Cluster,
  DataGrid,
  Dialog,
  EnvChip,
  FilterPicker,
  FormField,
  IconButton,
  InlineAlert,
  KeyLabel,
  Select,
  SplitButton,
  Stack,
  StatusBadge,
  Tabs,
  TextField,
  Toolbar,
  type Filter,
  type FilterModel,
  type GridColumn,
  type GridSort,
  type ListOption,
  type SplitAction,
  type TabItem,
  type ToolbarAction,
} from "@kartenn/ui";
import { computed, ref } from "vue";
import AVenir from "./AVenir.vue";
import Attributs from "./EcranAttributs.vue";
import Cadre from "./EcranCadre.vue";
import Panneau from "./EcranPanneau.vue";
import { ENV, FLOW_TYPE, RULES, SERVICES, type EnvKey, type FlowTypeKey } from "./donnees.ts";
import { EFFECT, PATTERNS, PEOPLE, SPACES_017, type Effect } from "./donnees017.ts";

const props = defineProps<{ vue: string }>();
const form = props.vue === "formulaire" || props.vue === "violations";
const tab = ref(props.vue === "motifs" ? "patterns" : "rules");

// ——— The rules (GET /v1/policy-rules) ———

/** A selector of a rule (REQ-010-8): axis → values; empty destination = « même valeur ». */
interface Selector {
  axis: string;
  values: string[];
}
interface RuleRow extends Record<string, unknown> {
  key: string;
  effect: Effect;
  source: Selector[];
  destination: Selector[];
  same: boolean;
  flowTypes: FlowTypeKey[];
  pattern: string;
  from: string;
  /** Not in the matrix: it cites another axis than the environment. */
  outside: boolean;
  text: string;
}
const env = (values: EnvKey[]): Selector[] => [{ axis: "environment", values }];
const ruleRows: RuleRow[] = [
  ...RULES.map((rule, i) => ({
    key: rule.key,
    effect: rule.effect as Effect,
    source: env(rule.sources),
    destination: rule.destinations.length ? env(rule.destinations) : [],
    same: rule.destinations.length === 0,
    flowTypes: rule.flowTypes,
    pattern: rule.pattern ?? "",
    from: `2025-0${(i % 9) + 1}-1${i % 3}`,
    outside: false,
    text: rule.text,
  })),
  {
    key: "deny-restreint-hors-prod",
    effect: "deny",
    source: [
      { axis: "environment", values: ["prod"] },
      { axis: "sensitivity", values: ["restreint"] },
    ],
    destination: env(["rece", "deve"]),
    same: false,
    flowTypes: ["database", "file"],
    pattern: "",
    from: "2025-10-06",
    outside: true,
    text: "Une donnée restreinte ne quitte pas la production, même anonymisée à la main.",
  },
];
const SHOWN = ruleRows.find((r) => r.key === "deny-deve-prod")!;
/** The labels of the values of the other axes (closed lists of the core). */
const AXIS_VALUE: Record<string, string> = { restreint: "Restreinte" };
const flowTypesText = (types: FlowTypeKey[]): string =>
  types.length === 4 ? "Tous" : types.map((t) => FLOW_TYPE[t]).join(", ");

const RULE_FILTERS: FilterModel = [
  { key: "key", label: "Clé", type: "text", operators: ["contains", "eq"] },
  {
    key: "effect",
    label: "Effet",
    type: "enum",
    operators: ["eq"],
    options: [
      { value: "allow", label: "Autoriser" },
      { value: "deny", label: "Interdire" },
    ],
  },
  {
    key: "pattern",
    label: "Motif",
    type: "enum",
    operators: ["eq", "exists", "absent"],
    options: PATTERNS.map((p) => ({ value: p.key, label: p.label })),
  },
  {
    key: "flow_type",
    label: "Type de flux",
    type: "enum",
    operators: ["eq"],
    options: Object.entries(FLOW_TYPE).map(([value, label]) => ({ value, label })),
  },
  {
    key: "service",
    label: "Service",
    type: "enum",
    operators: ["eq"],
    options: Object.values(SERVICES).map((s) => ({ value: s.key, label: s.label })),
  },
];
const ruleFilters = ref<Filter[]>([{ key: "effect", operator: "eq", values: ["allow", "deny"] }]);
const ruleColumns: GridColumn[] = [
  { id: "key", label: "Règle", width: "lg", mono: true, sortable: true, filterable: true },
  { id: "effect", label: "Effet", width: "md", sortable: true, filterable: true },
  { id: "source", label: "Source", width: "lg", wrap: true },
  { id: "destination", label: "Destination", width: "lg", wrap: true },
  { id: "flowTypes", label: "Types de flux", width: "md", wrap: true, priority: 3 },
  { id: "pattern", label: "Motif", width: "md", mono: true, filterable: true, priority: 3 },
  { id: "from", label: "Depuis", width: "sm", mono: true, sortable: true, priority: 3 },
];
const ruleSort = ref<GridSort | undefined>({ column: "key", direction: "ascending" });
const rulesActions: ToolbarAction[] = [
  {
    id: "create",
    label: "Créer une règle",
    icon: "plus",
    variant: "primary",
    shortcut: ["c"],
    priority: 3,
  },
  { id: "matrix", label: "Voir dans la matrice", icon: "matrix", priority: 2 },
  { id: "export", label: "Exporter", variant: "ghost" },
];
const tabs: TabItem[] = [
  { value: "rules", label: "Règles", count: ruleRows.length },
  { value: "patterns", label: "Motifs", count: PATTERNS.length },
];

interface Version extends Record<string, unknown> {
  version: string;
  valid: string;
  motive: string;
  author: string;
}
const versions: Version[] = [
  {
    version: "v2",
    valid: "2025-09-12 →",
    motive: "Texte reformulé",
    author: PEOPLE.lou,
  },
  {
    version: "v1",
    valid: "2025-03-01 → 2025-09-12",
    motive: "Politique initiale",
    author: PEOPLE.sacha,
  },
];
const versionColumns: GridColumn[] = [
  { id: "version", label: "Version", width: "sm", mono: true },
  { id: "valid", label: "Valide", width: "lg", mono: true },
  { id: "motive", label: "Motif", width: "fill", wrap: true },
];

// ——— The form of a rule (POST /v1/policy-rules, through the draft) ———

const violations = props.vue === "violations";
const ruleKey = ref("deny-restreint-partenaires");
const effect = ref<"allow" | "deny">("deny");
const AXES: ListOption[] = [
  { value: "environment", label: "Environnement", detail: "policy.environment" },
  { value: "sensitivity", label: "Sensibilité", detail: "policy.sensitivity" },
  { value: "zone", label: "Zone", detail: "policy.zone" },
];
const sourceAxis = ref<string | undefined>("sensitivity");
const sourceAxis2 = ref<string | undefined>("environment");
const destinationAxis = ref<string | undefined>("zone");
const same = ref(false);
const types = ref<Record<FlowTypeKey, boolean>>({
  application: !violations,
  admin: false,
  database: !violations,
  file: !violations,
});
const SERVICE_CHOICES: ListOption[] = [
  { value: "all", label: "Tous les services des types choisis" },
  ...Object.values(SERVICES).map((s) => ({ value: s.key, label: s.label, detail: s.key })),
  { value: "ftp-ancien", label: "ftp-ancien", detail: "clos le 2025-12-31" },
];
const service = ref<string | undefined>(violations ? "ftp-ancien" : "all");
const textFr = ref("");
const textEn = ref("");
const label = ref("Restreint vers les partenaires");
const sendActions = computed<SplitAction[]>(() => {
  const reason = violations
    ? undefined
    : "Une règle Interdire exige un texte en français : il s'affiche à qui la rencontre.";
  return [
    {
      id: "draft",
      label: "Ajouter au brouillon",
      icon: "draft",
      ...(reason ? { disabled: true, reason } : {}),
    },
    {
      id: "review",
      label: "Soumettre en revue",
      icon: "send",
      ...(reason ? { disabled: true, reason } : {}),
    },
  ];
});
const sendAction = ref("draft");

// ——— The 412 of a second tab ———

const kept = ref(
  "La recette lit les API de production sous le motif api-lecture, jamais en masse.",
);

// ——— The patterns (GET /v1/policy-patterns) ———

const patternColumns: GridColumn[] = [
  { id: "key", label: "Motif", width: "md", mono: true, sortable: true },
  { id: "label", label: "Libellé", width: "md", sortable: true, priority: 2 },
  { id: "text", label: "Texte pédagogique", width: "fill", wrap: true },
  { id: "rules", label: "Règles", width: "xs", align: "end", sortable: true },
];
const patternSort = ref<GridSort | undefined>({ column: "key", direction: "ascending" });
const PATTERN = PATTERNS[0]!;
</script>

<template>
  <Cadre
    title="Règles et motifs"
    active="rules"
    space="policy"
    :spaces="SPACES_017"
    :draft="form ? 4 : 3"
    panel="detail"
    :crumbs="[
      { label: 'Politique', to: '/x/policy' },
      { label: 'Règles et motifs', to: '/x/policy/rules' },
      ...(form ? [{ label: 'Nouvelle règle' }] : []),
    ]"
  >
    <Panneau title="Règles et motifs" detail="au présent · politique r42" main>
      <template #actions>
        <Toolbar label="Actions des règles" align="end" :actions="rulesActions" />
      </template>
      <Tabs v-model="tab" :tabs="tabs" label="Règles et motifs">
        <template #rules>
          <Stack gap="2">
            <FilterPicker
              v-model="ruleFilters"
              :model="RULE_FILTERS"
              label="Filtres des règles"
              :shortcut="['/']"
            />
            <DataGrid
              v-model:sort="ruleSort"
              label="Règles de la politique"
              :columns="ruleColumns"
              :rows="ruleRows"
              row-key="key"
              selectable="single"
              :selected="form ? [] : [SHOWN.key]"
              :filtered="['effect']"
            >
              <template #cell="{ row, column, value }">
                <span v-if="column.id === 'key'" class="k-e14-key"
                  ><code>{{ row.key }}</code
                  ><StatusBadge v-if="row.outside" state="info" label="Hors matrice"
                /></span>
                <StatusBadge
                  v-else-if="column.id === 'effect'"
                  :state="EFFECT[row.effect].state"
                  :label="EFFECT[row.effect].label"
                />
                <span v-else-if="column.id === 'destination' && row.same" class="k-ecran-muted"
                  >même environnement</span
                >
                <Cluster
                  v-else-if="column.id === 'source' || column.id === 'destination'"
                  as="span"
                  gap="1"
                >
                  <template
                    v-for="s in column.id === 'source' ? row.source : row.destination"
                    :key="s.axis"
                  >
                    <EnvChip
                      v-for="v in s.axis === 'environment' ? s.values : []"
                      :key="v"
                      v-bind="ENV[v as EnvKey]"
                    />
                    <KeyLabel
                      v-for="v in s.axis === 'environment' ? [] : s.values"
                      :key="v"
                      :value="v"
                      :label="AXIS_VALUE[v] ?? null"
                    />
                  </template>
                </Cluster>
                <template v-else-if="column.id === 'flowTypes'">{{
                  flowTypesText(row.flowTypes)
                }}</template>
                <span v-else-if="!value" class="k-ecran-muted">—</span>
                <template v-else>{{ value }}</template>
              </template>
            </DataGrid>
          </Stack>
        </template>
        <template #patterns>
          <DataGrid
            v-model:sort="patternSort"
            label="Motifs de la politique"
            :columns="patternColumns"
            :rows="PATTERNS"
            row-key="key"
            selectable="single"
            :selected="[PATTERN.key]"
          />
        </template>
      </Tabs>
      <template #footer>
        <span v-if="tab === 'rules'" class="k-num"
          >{{ ruleRows.length }} règles · 1 hors matrice · {{ PATTERNS.length }} motifs</span
        >
        <span v-else class="k-num">{{ PATTERNS.length }} motifs · 4 règles les requièrent</span>
        <span v-if="tab === 'rules'" class="k-ecran-muted"
          >Hors matrice : une règle qui cite un autre axe que l'environnement</span
        >
      </template>
    </Panneau>

    <!-- The chosen rule, read in words. -->
    <template #detail>
      <Stack v-if="props.vue === 'regles' || props.vue === 'conflit'" gap="3">
        <h2 class="k-ecran-subtitle">
          <code>{{ SHOWN.key }}</code>
        </h2>
        <p class="k-e14-sentence">
          <StatusBadge state="ko" label="Interdire" />
          <span
            >source <code>{environment: deve}</code> → destination <code>{environment: prod}</code>,
            tous les types de flux, tous les services</span
          >
        </p>
        <Attributs
          :items="[
            { key: 'source', label: 'Source' },
            { key: 'destination', label: 'Destination' },
            { key: 'types', label: 'Types de flux', value: 'Tous (4)' },
            { key: 'patterns', label: 'Motifs requis', value: '— (règle Interdire)' },
            { key: 'from', label: 'Période', value: 'depuis 2025-03-01', mono: true },
            { key: 'cells', label: 'Dans la matrice', value: '4 cellules, une par type de flux' },
          ]"
        >
          <template #source><EnvChip v-bind="ENV.deve" /></template>
          <template #destination><EnvChip v-bind="ENV.prod" /></template>
        </Attributs>
        <Stack as="section" gap="1" aria-labelledby="k-e14-text">
          <h3 id="k-e14-text" class="k-ecran-subtitle">Texte pédagogique</h3>
          <p>{{ SHOWN.text }}</p>
          <p class="k-ecran-muted">
            Français · aucun texte anglais : repli sur le français, langue indiquée.
          </p>
        </Stack>
        <Stack as="section" gap="1" aria-labelledby="k-e14-history">
          <h3 id="k-e14-history" class="k-ecran-subtitle">Historique</h3>
          <DataGrid
            label="Versions de deny-deve-prod"
            :columns="versionColumns"
            :rows="versions"
            row-key="version"
            stack
          />
        </Stack>
      </Stack>

      <!-- The form of a new rule. -->
      <Stack v-else-if="form" gap="3">
        <h2 class="k-ecran-subtitle">Nouvelle règle</h2>
        <InlineAlert
          v-if="violations"
          tone="ko"
          title="La règle n'a pas été écrite : le serveur signale 3 violations."
        >
          Votre saisie est gardée ; chaque violation est sur son champ.
        </InlineAlert>
        <TextField v-model="ruleKey" label="Clé" mono required hint="Figée après la création." />
        <TextField v-model="label" label="Libellé (fr)" />
        <FormField label="Effet" group required>
          <Cluster gap="1">
            <Button size="sm" :pressed="effect === 'allow'" @click="effect = 'allow'"
              >Autoriser</Button
            >
            <Button size="sm" :pressed="effect === 'deny'" @click="effect = 'deny'"
              >Interdire</Button
            >
          </Cluster>
        </FormField>
        <FormField label="Source : axe → valeurs" group required>
          <Stack gap="1">
            <Cluster gap="2" :wrap="false" class="k-e14-selector">
              <Select
                v-model="sourceAxis"
                label="Axe de la source"
                hide-label
                :options="AXES"
                size="sm"
              />
              <span class="k-e14-values"><KeyLabel value="restreint" label="Restreint" /></span>
              <IconButton icon="plus" size="sm" label="Ajouter une valeur de sensibilité" />
              <IconButton icon="x" size="sm" label="Retirer l'axe Sensibilité de la source" />
            </Cluster>
            <Cluster gap="2" :wrap="false" class="k-e14-selector">
              <Select
                v-model="sourceAxis2"
                label="Second axe de la source"
                hide-label
                :options="AXES"
                size="sm"
              />
              <span class="k-e14-values"><EnvChip v-bind="ENV.prod" /></span>
              <IconButton icon="plus" size="sm" label="Ajouter un environnement" />
              <IconButton icon="x" size="sm" label="Retirer l'axe Environnement de la source" />
            </Cluster>
            <div><Button size="sm" variant="ghost" icon="plus">Ajouter un axe</Button></div>
          </Stack>
        </FormField>
        <FormField label="Destination : axe → valeurs" group required>
          <Stack gap="1">
            <Cluster gap="2" :wrap="false" class="k-e14-selector">
              <Select
                v-model="destinationAxis"
                label="Axe de la destination"
                hide-label
                :options="AXES"
                size="sm"
              />
              <span class="k-e14-values"><KeyLabel value="partenaires" label="Partenaires" /></span>
              <IconButton icon="plus" size="sm" label="Ajouter une zone" />
              <IconButton icon="x" size="sm" label="Retirer l'axe Zone de la destination" />
            </Cluster>
            <div class="k-e14-check">
              <Checkbox
                v-model="same"
                label="Même valeur à la source et à la destination"
                hint="Contrainte « même valeur » sur un axe (par exemple le même site)."
              />
            </div>
          </Stack>
        </FormField>
        <FormField
          label="Types de flux"
          group
          :error="
            violations
              ? 'Aucun type de flux ni service (policy.rule.flow_selector_empty)'
              : undefined
          "
        >
          <Cluster gap="3" class="k-e14-check">
            <Checkbox
              v-for="(name, key) in FLOW_TYPE"
              :key="key"
              v-model="types[key]"
              :label="name"
            />
          </Cluster>
        </FormField>
        <Select
          v-model="service"
          label="Services"
          :options="SERVICE_CHOICES"
          :error="
            violations ? 'Le service ftp-ancien est clos (policy.reference.closed)' : undefined
          "
        />
        <Select
          label="Motifs requis"
          placeholder="Aucun : la règle interdit"
          :options="PATTERNS.map((p) => ({ value: p.key, label: p.label, detail: p.key }))"
          readonly
          hint="Seulement pour une règle Autoriser."
        />
        <TextField
          v-model="textFr"
          label="Texte pédagogique (fr)"
          required
          :error="
            violations ? 'Texte exigé pour une règle Interdire (policy.text.missing)' : undefined
          "
          hint="Exigé pour une règle Interdire : il s'affiche à qui la rencontre."
        />
        <TextField
          v-model="textEn"
          label="Texte pédagogique (en)"
          hint="Facultatif : sinon le français, langue indiquée."
        />
        <AVenir component="DateTimeField" task="020-20">
          <TextField
            label="Début"
            model-value="au présent"
            readonly
            hint="Une règle datée s'écrit au présent ou dans le futur."
          />
        </AVenir>
      </Stack>

      <!-- The chosen pattern. -->
      <Stack v-else gap="3">
        <h2 class="k-ecran-subtitle">
          <KeyLabel :value="PATTERN.key" :label="PATTERN.label" show-key />
        </h2>
        <Attributs
          :items="[
            { key: 'from', label: 'Depuis', value: PATTERN.from, mono: true },
            {
              key: 'rules',
              label: 'Requis par',
              value: 'rece-vers-prod, api-partenaires',
              mono: true,
            },
          ]"
        />
        <Stack as="section" gap="1" aria-labelledby="k-e14-ptext">
          <h3 id="k-e14-ptext" class="k-ecran-subtitle">Texte pédagogique (fr)</h3>
          <p>{{ PATTERN.text }}</p>
        </Stack>
        <InlineAlert
          tone="info"
          title="Ce motif ne se clôt pas tant que deux règles le requièrent."
          :items="[
            {
              message: 'Retirez-le de rece-vers-prod et api-partenaires d\'abord.',
              code: 'policy.reference.closed',
            },
          ]"
        />
      </Stack>
    </template>
    <template #detail-footer>
      <Cluster v-if="form" justify="end">
        <Button variant="ghost">Annuler</Button>
        <SplitButton
          v-model:action="sendAction"
          :actions="sendActions"
          :shortcut="['Control', 'Enter']"
        />
      </Cluster>
      <Cluster v-else-if="props.vue === 'motifs'" justify="end">
        <Button
          variant="ghost"
          icon="trash"
          reason="Deux règles requièrent ce motif (policy.reference.closed)."
          >Clore…</Button
        >
        <Button icon="pencil" :shortcut="['e']">Modifier</Button>
      </Cluster>
      <Cluster v-else justify="end">
        <Button variant="ghost" icon="trash">Clore…</Button>
        <Button icon="pencil" :shortcut="['e']">Modifier</Button>
      </Cluster>
    </template>

    <Dialog
      v-if="props.vue === 'conflit'"
      :open="true"
      title="La règle a changé depuis votre lecture"
      description="Lou Bernard a modifié rece-vers-prod dans un autre onglet après que vous l'avez lue : rien n'a été écrit, votre saisie est gardée."
    >
      <Stack gap="3">
        <InlineAlert
          tone="ko"
          title="Vous avez lu la version v3, la version actuelle est la v4."
          :items="[
            {
              message: 'Relisez la règle actuelle, puis réappliquez ou abandonnez votre saisie.',
              code: 'urn:kartenn:problem:precondition-failed',
            },
          ]"
        />
        <Attributs
          :items="[
            { key: 'current', label: 'Texte actuel', value: RULES[1]!.text },
            { key: 'by', label: 'Modifiée par', value: `${PEOPLE.lou}, 09:52` },
          ]"
        />
        <TextField
          v-model="kept"
          label="Votre saisie, gardée"
          hint="Texte pédagogique (fr) de rece-vers-prod"
        />
      </Stack>
      <template #actions>
        <Button variant="secondary">Garder la règle actuelle</Button>
        <Button variant="primary" icon="redo-2">Réappliquer ma saisie</Button>
      </template>
    </Dialog>
  </Cadre>
</template>

<style>
@layer components {
  /* The key of a rule and its mark « hors matrice », on one wrapping line. */
  .k-e14-key {
    display: inline-flex;
    flex-wrap: wrap;
    gap: var(--k-space-1);
    align-items: center;
    min-inline-size: 0;
  }

  /* A rule read in words: its effect, then the sentence. */
  .k-e14-sentence {
    display: flex;
    gap: var(--k-space-2);
    align-items: baseline;
  }

  .k-e14-sentence > .k-status-badge {
    align-self: center;
  }

  /* A row of a selector: the axis, its values, then its buttons. */
  .k-e14-selector {
    min-inline-size: 0;
  }

  .k-e14-selector > .k-select {
    flex: 0 1 var(--k-size-col-md);
    min-inline-size: 0;
  }

  /* Boxes off the edge of the right panel: its resizing handle is a target too (2.5.8). */
  .k-e14-check {
    padding-inline-start: var(--k-space-2);
  }

  .k-e14-values {
    display: inline-flex;
    flex: 1 1 0;
    flex-wrap: wrap;
    gap: var(--k-space-1);
    min-inline-size: 0;
  }
}
</style>
