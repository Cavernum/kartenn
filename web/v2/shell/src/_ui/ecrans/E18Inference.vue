<script setup lang="ts">
// E18 — Inférence (spec 017, task 1; view `inference` and action `expected` of the plugin
// `policy`, REQ-017-32 to 34; Q-017-5; 010/S36): the inference rules of the set (target
// axis, expected values, criteria from the closed list of the server, nested exceptions,
// explicit priority, cited domain) and their history; the form of a rule, its criteria and
// its exceptions edited as a tree of at most four levels (the server's
// `policy.inference.depth` decides), each violation on its criterion or its exception,
// with the preview of what it would match and the mismatches it would open or close,
// without writing; the open mismatches (`policy.inference.mismatch`), acknowledged one by
// one; the expected value of a node and its explanation, every candidate shown when
// several tie.
import {
  Button,
  CodeBadge,
  Cluster,
  DataGrid,
  Dialog,
  EnvChip,
  IconButton,
  InlineAlert,
  Select,
  Stack,
  StatusBadge,
  TextField,
  type GridColumn,
  type GridSort,
  type ListOption,
  type ToolbarAction,
} from "@kartenn/ui";
import { ref } from "vue";
import Attributs from "./EcranAttributs.vue";
import Barre from "./EcranBarre.vue";
import Cadre from "./EcranCadre.vue";
import Panneau from "./EcranPanneau.vue";
import { DOMAIN, ENV, type EnvKey } from "./donnees.ts";
import { PEOPLE, SPACES_017 } from "./donnees017.ts";

const props = defineProps<{ vue: string }>();

// ——— The rules of the set (GET /v1/inference-rules) ———

interface InferenceRule extends Record<string, unknown> {
  key: string;
  priority: number;
  axis: string;
  expected: EnvKey;
  criteria: string;
  exceptions: number;
  domain: string;
}
const rules: InferenceRule[] = [
  {
    key: "env-par-site",
    priority: 20,
    axis: "policy.environment",
    expected: "prod",
    criteria: "site = site-ouest · famille IPv4 ou IPv6",
    exceptions: 2,
    domain: "dc-nord",
  },
  {
    key: "env-par-vlan",
    priority: 20,
    axis: "policy.environment",
    expected: "rece",
    criteria: "core.vlan = 130–139",
    exceptions: 0,
    domain: "dc-nord",
  },
  {
    key: "env-par-nom",
    priority: 10,
    axis: "policy.environment",
    expected: "deve",
    criteria: "nom ~ ^dev-",
    exceptions: 1,
    domain: "global",
  },
  {
    key: "admin-par-prefixe",
    priority: 30,
    axis: "policy.environment",
    expected: "admin",
    criteria: "préfixe ⊂ 198.51.100.0/28",
    exceptions: 0,
    domain: "dc-nord",
  },
];
const ruleColumns: GridColumn[] = [
  { id: "priority", label: "Priorité", width: "sm", align: "end", sortable: true },
  { id: "key", label: "Règle", width: "md", mono: true, sortable: true },
  { id: "expected", label: "Attendu", width: "md" },
  { id: "criteria", label: "Critères", width: "fill", mono: true, wrap: true },
  { id: "exceptions", label: "Exceptions", width: "sm", align: "end", priority: 2 },
  { id: "domain", label: "Domaine", width: "sm", mono: true, priority: 3 },
];
const ruleSort = ref<GridSort | undefined>({ column: "priority", direction: "descending" });
const rulesActions: ToolbarAction[] = [
  {
    id: "create",
    label: "Créer une règle",
    icon: "plus",
    variant: "primary",
    shortcut: ["c"],
    priority: 3,
  },
  { id: "mismatches", label: "Écarts ouverts (5)", icon: "triangle-alert", priority: 2 },
  { id: "export", label: "Exporter", variant: "ghost" },
];

/** An exception of a rule, as a node of the tree (depth 1 to 4). */
interface Exception {
  id: string;
  depth: number;
  criterion: string;
  expected: EnvKey | "none";
  error?: string;
}
const SHOWN_EXCEPTIONS: Exception[] = [
  { id: "x1", depth: 1, criterion: "préfixe ⊂ 192.0.2.128/25 (salle B)", expected: "rece" },
  { id: "x2", depth: 2, criterion: "nom ~ ^srv-prod-", expected: "prod" },
];

// ——— The form of a rule and its preview ———

const KINDS: ListOption[] = [
  { value: "prefix", label: "Préfixe", detail: "prefix" },
  { value: "family", label: "Famille", detail: "family" },
  { value: "domain", label: "Domaine", detail: "domain" },
  { value: "kind", label: "Type d'objet", detail: "object_kind" },
  { value: "site", label: "Site et entité", detail: "site" },
  { value: "property", label: "Valeur d'une propriété", detail: "property" },
  { value: "name", label: "Nom par expression régulière", detail: "name_regex" },
];
const ENV_CHOICES: ListOption[] = (["prod", "rece", "deve", "admin"] as EnvKey[]).map((e) => ({
  value: e,
  label: ENV[e].label,
  detail: e,
}));
const formKey = ref("prod-salle-a");
const formAxis = ref<string | undefined>("environment");
const formExpected = ref<string | undefined>("prod");
const formPriority = ref("25");
const formDomain = ref<string | undefined>(DOMAIN.key);
const criteria = ref([
  { id: "c1", kind: "prefix", value: "192.0.2.0/26", error: undefined as string | undefined },
  {
    id: "c2",
    kind: "name",
    value: "^srv-(app|db)-[0-9+$",
    error: "Expression régulière invalide : crochet non fermé (policy.inference.regex_invalid)",
  },
]);
const FORM_EXCEPTIONS: Exception[] = [
  { id: "e1", depth: 1, criterion: "préfixe ⊂ 192.0.2.32/28", expected: "rece" },
  { id: "e2", depth: 2, criterion: "nom ~ ^srv-rec-0[1-4]$", expected: "prod" },
  { id: "e3", depth: 3, criterion: "type d'objet = adresse", expected: "rece" },
  { id: "e4", depth: 4, criterion: "core.vlan = 121", expected: "prod" },
];
const depthStyle = (e: Exception) => ({ "--k-e18-depth": String(e.depth - 1) });

interface PreviewNode extends Record<string, unknown> {
  node: string;
  declared: EnvKey | null;
  expected: EnvKey;
  change: "opens" | "none" | "closes";
}
const preview: PreviewNode[] = [
  { node: "192.0.2.0/26", declared: "prod", expected: "prod", change: "none" },
  { node: "192.0.2.10", declared: "prod", expected: "prod", change: "none" },
  { node: "192.0.2.14", declared: "prod", expected: "prod", change: "none" },
  { node: "192.0.2.21", declared: "deve", expected: "prod", change: "opens" },
  { node: "192.0.2.22", declared: "deve", expected: "prod", change: "opens" },
  { node: "192.0.2.30", declared: "prod", expected: "prod", change: "none" },
  { node: "192.0.2.33", declared: "deve", expected: "rece", change: "opens" },
];
const previewColumns: GridColumn[] = [
  { id: "node", label: "Nœud", width: "md", mono: true, sortable: true },
  { id: "declared", label: "Déclaré", width: "md" },
  { id: "expected", label: "Attendu", width: "md" },
  { id: "change", label: "Écart", width: "md", sortable: true },
];

// ——— Open mismatches (GET /v1/anomalies?rule=policy.inference.mismatch) ———

interface Mismatch extends Record<string, unknown> {
  key: string;
  node: string;
  declared: EnvKey;
  expected: EnvKey;
  heirs: number;
  rule: string;
  since: string;
}
const mismatches: Mismatch[] = [
  {
    key: "m1",
    node: "192.0.2.21",
    declared: "deve",
    expected: "prod",
    heirs: 0,
    rule: "env-par-site",
    since: "2026-01-12",
  },
  {
    key: "m2",
    node: "192.0.2.64/27",
    declared: "rece",
    expected: "prod",
    heirs: 14,
    rule: "env-par-site",
    since: "2026-01-09",
  },
  {
    key: "m3",
    node: "198.51.100.0/27",
    declared: "prod",
    expected: "rece",
    heirs: 22,
    rule: "env-par-vlan",
    since: "2025-12-18",
  },
  {
    key: "m4",
    node: "2001:db8:20::/64",
    declared: "prod",
    expected: "rece",
    heirs: 6,
    rule: "env-par-vlan",
    since: "2025-12-18",
  },
];
const mismatchColumns: GridColumn[] = [
  { id: "node", label: "Nœud d'origine", width: "md", mono: true, sortable: true },
  { id: "values", label: "Déclaré → attendu", width: "fill" },
  { id: "heirs", label: "Héritiers en écart", width: "sm", align: "end", sortable: true },
  { id: "rule", label: "Règle", width: "md", mono: true, priority: 2 },
  { id: "since", label: "Depuis", width: "sm", mono: true, sortable: true, priority: 3 },
  { id: "actions", label: "Actions", width: "md", actions: true },
];
const mismatchSort = ref<GridSort | undefined>({ column: "heirs", direction: "descending" });

// ——— The expected value of a node (action `expected`) ———

const CANDIDATES = [
  {
    rule: "env-par-site",
    expected: "prod" as EnvKey,
    node: "192.0.2.0/26",
    chain: "env-par-site → exception salle B non applicable (192.0.2.14 ∉ 192.0.2.128/25)",
    criteria: [
      { criterion: "site = site-ouest", observed: "site-ouest", met: true },
      { criterion: "famille IPv4 ou IPv6", observed: "IPv4", met: true },
    ],
  },
  {
    rule: "env-par-vlan",
    expected: "rece" as EnvKey,
    node: "192.0.2.0/26",
    chain: "env-par-vlan, sans exception",
    criteria: [
      { criterion: "core.vlan = 130–139", observed: "131 (posé sur 192.0.2.0/26)", met: true },
    ],
  },
];
const DISCARDED = [
  { rule: "env-par-nom", why: "priorité 10, inférieure" },
  { rule: "admin-par-prefixe", why: "critère non rempli : 192.0.2.14 ∉ 198.51.100.0/28" },
];
</script>

<template>
  <Cadre
    :title="props.vue === 'formulaire' ? 'Nouvelle règle d\'inférence' : 'Inférence'"
    :active="props.vue === 'attendu' ? 'addresses' : 'inference'"
    :space="props.vue === 'attendu' ? 'referential' : 'policy'"
    :spaces="SPACES_017"
    :draft="props.vue === 'formulaire' ? 1 : 0"
    :panel="props.vue === 'regles' || props.vue === 'formulaire' ? 'detail' : undefined"
    :crumbs="
      props.vue === 'attendu'
        ? [
            { label: 'Référentiel', to: '/' },
            { label: 'Adresses', to: '/addresses' },
            { label: '192.0.2.14', mono: true },
          ]
        : [
            { label: 'Politique', to: '/x/policy' },
            { label: 'Inférence', to: '/x/policy/inference' },
            ...(props.vue === 'formulaire'
              ? [{ label: 'Nouvelle règle' }]
              : props.vue === 'ecarts'
                ? [{ label: 'Écarts' }]
                : []),
          ]
    "
  >
    <!-- The rules of the set. -->
    <Panneau
      v-if="props.vue === 'regles' || props.vue === 'formulaire'"
      title="Règles d'inférence"
      detail="jeu r17 · axe Environnement"
      main
      flush
    >
      <template #toolbar>
        <Barre label="Actions des règles d'inférence" :actions="rulesActions">
          <Select
            label="Axe visé"
            :options="[
              { value: 'environment', label: 'Environnement', detail: 'policy.environment' },
            ]"
            model-value="environment"
            inline-label
          />
        </Barre>
      </template>
      <DataGrid
        v-model:sort="ruleSort"
        label="Règles d'inférence de l'axe Environnement"
        :columns="ruleColumns"
        :rows="rules"
        row-key="key"
        selectable="single"
        :selected="props.vue === 'regles' ? ['env-par-site'] : []"
      >
        <template #cell="{ row, column, value }">
          <EnvChip v-if="column.id === 'expected'" v-bind="ENV[row.expected]" />
          <template v-else>{{ value }}</template>
        </template>
      </DataGrid>
      <template #footer>
        <span class="k-num">4 règles · révision du jeu r17, 2026-01-12</span>
        <span class="k-ecran-muted"
          >À priorité égale, deux attendus différents : aucun n'est retenu</span
        >
      </template>
    </Panneau>

    <!-- The right panel: the form of a new rule and its preview, or the chosen rule. -->
    <template #detail>
      <Stack v-if="props.vue === 'formulaire'" gap="3">
        <h2 class="k-ecran-subtitle">Nouvelle règle d'inférence</h2>
        <Stack gap="3">
          <Cluster gap="3" align="start" class="k-e18-line">
            <TextField v-model="formKey" label="Clé" mono required />
            <TextField
              v-model="formPriority"
              label="Priorité"
              mono
              required
              hint="La plus haute l'emporte."
            />
          </Cluster>
          <Cluster gap="3" align="start" class="k-e18-line">
            <Select
              v-model="formAxis"
              label="Axe visé"
              :options="[
                { value: 'environment', label: 'Environnement', detail: 'policy.environment' },
              ]"
              required
            />
            <Select
              v-model="formExpected"
              label="Valeur attendue"
              :options="ENV_CHOICES"
              required
            />
            <Select
              v-model="formDomain"
              label="Domaine"
              :options="[
                { value: DOMAIN.key, label: DOMAIN.label, detail: DOMAIN.key },
                { value: 'global', label: 'Global' },
              ]"
              hint="Il faut policy.inference.admin sur ce domaine."
            />
          </Cluster>
          <Stack as="section" gap="2" aria-labelledby="k-e18-criteria">
            <h3 id="k-e18-criteria" class="k-ecran-subtitle">
              Critères <span class="k-ecran-muted">tous remplis</span>
            </h3>
            <Cluster
              v-for="c in criteria"
              :key="c.id"
              gap="2"
              align="start"
              class="k-e18-criterion"
            >
              <Select
                v-model="c.kind"
                :label="`Critère ${c.id === 'c1' ? 1 : 2}`"
                hide-label
                :options="KINDS"
              />
              <TextField
                v-model="c.value"
                :label="`Valeur du critère ${c.id === 'c1' ? 1 : 2}`"
                hide-label
                mono
                :error="c.error"
              />
              <IconButton icon="x" label="Retirer le critère" />
            </Cluster>
            <div><Button size="sm" variant="ghost" icon="plus">Ajouter un critère</Button></div>
          </Stack>
          <Stack as="section" gap="2" aria-labelledby="k-e18-exceptions">
            <h3 id="k-e18-exceptions" class="k-ecran-subtitle">
              Exceptions <span class="k-ecran-muted">4 niveaux au plus</span>
            </h3>
            <ol class="k-e18-tree" aria-label="Exceptions imbriquées">
              <li
                v-for="e in FORM_EXCEPTIONS"
                :key="e.id"
                class="k-e18-exception"
                :style="depthStyle(e)"
              >
                <span class="k-e18-depth k-num" :aria-label="`Niveau ${e.depth}`">{{
                  e.depth
                }}</span>
                <code class="k-e18-exception__criterion">{{ e.criterion }}</code>
                <span class="k-ecran-muted">attendu</span>
                <EnvChip v-bind="ENV[e.expected as EnvKey]" />
                <Button
                  size="sm"
                  variant="ghost"
                  icon="plus"
                  v-bind="
                    e.depth === 4
                      ? { reason: 'Quatre niveaux d\'exceptions au plus (policy.inference.depth).' }
                      : {}
                  "
                  >Exception</Button
                >
              </li>
            </ol>
            <div><Button size="sm" variant="ghost" icon="plus">Ajouter une exception</Button></div>
          </Stack>
        </Stack>
        <Stack as="section" gap="2" aria-labelledby="k-e18-preview">
          <h3 id="k-e18-preview" class="k-ecran-subtitle">
            Aperçu <span class="k-ecran-muted">sans écriture, relancé après une pause</span>
          </h3>
          <Attributs
            :items="[
              { key: 'nodes', label: 'Nœuds visés', value: '12', mono: true },
              { key: 'opens', label: 'Écarts ouverts', value: '3', mono: true },
              { key: 'closes', label: 'Écarts fermés', value: '0', mono: true },
            ]"
          />
          <InlineAlert
            tone="warn"
            title="Une autre règle de même priorité vise une partie de ces nœuds."
            :items="[
              {
                message:
                  'env-par-vlan (priorité 25) attend Recette sur 192.0.2.32/28 : à égalité, aucun attendu n\'y serait retenu.',
                code: 'policy.inference.conflict',
              },
            ]"
          />
          <DataGrid
            label="Nœuds que viserait la règle (7 sur 12)"
            :columns="previewColumns"
            :rows="preview"
            row-key="node"
            stack
          >
            <template #cell="{ row, column, value }">
              <EnvChip v-if="column.id === 'declared' && row.declared" v-bind="ENV[row.declared]" />
              <EnvChip v-else-if="column.id === 'expected'" v-bind="ENV[row.expected]" />
              <StatusBadge
                v-else-if="column.id === 'change' && row.change === 'opens'"
                state="warn"
                label="S'ouvre"
              />
              <span v-else-if="column.id === 'change'" class="k-ecran-muted">—</span>
              <template v-else>{{ value }}</template>
            </template>
          </DataGrid>
          <p class="k-ecran-muted">
            Lu par la route d'aperçu (010/S36) avec le critère invalide écarté.
          </p>
        </Stack>
      </Stack>
      <Stack v-else gap="3">
        <h2 class="k-ecran-subtitle"><code>env-par-site</code></h2>
        <Attributs
          :items="[
            { key: 'axis', label: 'Axe visé', value: 'Environnement' },
            { key: 'axisKey', label: 'Clé de l\'axe', value: 'policy.environment', mono: true },
            { key: 'expected', label: 'Attendu' },
            { key: 'priority', label: 'Priorité', value: '20', mono: true },
            { key: 'domain', label: 'Domaine', value: DOMAIN.label },
            { key: 'revision', label: 'Révision du jeu', value: 'r17 (2026-01-12)', mono: true },
          ]"
        >
          <template #expected><EnvChip v-bind="ENV.prod" /></template>
        </Attributs>
        <Stack as="section" gap="1" aria-labelledby="k-e18-rule-criteria">
          <h3 id="k-e18-rule-criteria" class="k-ecran-subtitle">Critères</h3>
          <ul class="k-ecran-list k-e18-plain">
            <li><code>site = site-ouest</code></li>
            <li><code>famille IPv4 ou IPv6</code></li>
          </ul>
        </Stack>
        <Stack as="section" gap="1" aria-labelledby="k-e18-rule-exceptions">
          <h3 id="k-e18-rule-exceptions" class="k-ecran-subtitle">Exceptions</h3>
          <ol class="k-e18-tree" aria-label="Exceptions de env-par-site">
            <li
              v-for="e in SHOWN_EXCEPTIONS"
              :key="e.id"
              class="k-e18-exception"
              :style="depthStyle(e)"
            >
              <span class="k-e18-depth k-num" :aria-label="`Niveau ${e.depth}`">{{ e.depth }}</span>
              <code class="k-e18-exception__criterion">{{ e.criterion }}</code>
              <EnvChip v-bind="ENV[e.expected as EnvKey]" />
            </li>
          </ol>
        </Stack>
        <p class="k-ecran-muted">
          Version v3, du 2026-01-12, par {{ PEOPLE.sacha }} : « Salle B en recette ».
        </p>
      </Stack>
    </template>

    <!-- The open mismatches. -->
    <Panneau
      v-if="props.vue === 'ecarts'"
      title="Écarts d'inférence"
      detail="ouverts · policy.inference.mismatch"
      main
      flush
    >
      <template #toolbar>
        <InlineAlert
          tone="ok"
          title="Écart de 192.0.2.33 acquitté : il quitte les écarts ouverts."
        />
      </template>
      <DataGrid
        v-model:sort="mismatchSort"
        label="Écarts d'inférence ouverts"
        :columns="mismatchColumns"
        :rows="mismatches"
        row-key="key"
      >
        <template #cell="{ row, column, value }">
          <a v-if="column.id === 'node'" :href="`/prefixes/${row.node}`">{{ row.node }}</a>
          <span v-else-if="column.id === 'values'" class="k-e18-values">
            <EnvChip v-bind="ENV[row.declared]" /><span class="k-ecran-muted">→</span
            ><EnvChip v-bind="ENV[row.expected]" />
          </span>
          <Button v-else-if="column.id === 'actions'" size="sm" icon="check">Acquitter</Button>
          <template v-else>{{ value }}</template>
        </template>
      </DataGrid>
      <template #footer>
        <span class="k-num">4 écarts ouverts · 42 héritiers en écart</span>
        <span class="k-ecran-muted">Un écart acquitté reste dans l'historique de l'anomalie</span>
      </template>
    </Panneau>

    <!-- The card of an address, under the dialog of its expected value. -->
    <Panneau
      v-if="props.vue === 'attendu'"
      title="192.0.2.14"
      detail="adresse · srv-app-01 · eth0"
      main
    >
      <Attributs
        :items="[
          { key: 'env', label: 'Environnement déclaré' },
          { key: 'owner', label: 'Propriétaire', value: 'Atelier Nord' },
        ]"
      >
        <template #env><EnvChip v-bind="ENV.prod" /></template>
      </Attributs>
    </Panneau>

    <!-- The chosen rule. -->
    <template #detail-footer>
      <Cluster v-if="props.vue === 'formulaire'" justify="end">
        <Button variant="ghost">Annuler</Button>
        <Button
          variant="primary"
          icon="draft"
          reason="Un critère est invalide : corrigez l'expression régulière."
          >Ajouter au brouillon</Button
        >
      </Cluster>
      <Cluster v-else justify="end">
        <Button variant="ghost" icon="trash">Clore…</Button>
        <Button icon="pencil" :shortcut="['e']">Modifier</Button>
      </Cluster>
    </template>

    <Dialog
      v-if="props.vue === 'attendu'"
      :open="true"
      size="lg"
      title="Attendu d'inférence de 192.0.2.14"
      description="Axe Environnement, jeu de règles r17. Lecture seule : rien n'est écrit."
    >
      <Stack gap="3">
        <InlineAlert
          tone="info"
          title="Deux candidats à égalité de priorité : aucun attendu n'est retenu, aucun écart n'est signalé."
        />
        <section
          v-for="(c, i) in CANDIDATES"
          :key="c.rule"
          class="k-e18-candidate"
          :aria-labelledby="`k-e18-cand-${i}`"
        >
          <h3 :id="`k-e18-cand-${i}`" class="k-ecran-subtitle k-e18-values">
            Candidat {{ i + 1 }} <EnvChip v-bind="ENV[c.expected]" /> <CodeBadge :code="c.rule" />
          </h3>
          <Attributs
            :items="[
              { key: 'node', label: 'Nœud de correspondance', value: c.node, mono: true },
              { key: 'chain', label: 'Règle et exceptions', value: c.chain },
            ]"
          />
          <ul class="k-ecran-list k-e18-plain" :aria-label="`Critères du candidat ${i + 1}`">
            <li v-for="cr in c.criteria" :key="cr.criterion" class="k-e18-values">
              <StatusBadge state="ok" label="Rempli" /><code>{{ cr.criterion }}</code>
              <span class="k-ecran-muted">observé : {{ cr.observed }}</span>
            </li>
          </ul>
        </section>
        <Stack as="section" gap="1" aria-labelledby="k-e18-discarded">
          <h3 id="k-e18-discarded" class="k-ecran-subtitle">Règles écartées</h3>
          <ul class="k-ecran-list k-e18-plain">
            <li v-for="d in DISCARDED" :key="d.rule" class="k-e18-values">
              <code>{{ d.rule }}</code
              ><span class="k-ecran-muted">{{ d.why }}</span>
            </li>
          </ul>
        </Stack>
      </Stack>
      <template #actions>
        <Button variant="primary">Fermer</Button>
      </template>
    </Dialog>
  </Cadre>
</template>

<style>
@layer components {
  .k-e18-line > * {
    flex: 1 1 var(--k-size-col-md);
    min-inline-size: 0;
  }

  /* The kind and the value share a line when there is room, else stack: the error under
   * the value keeps the width of the panel. */
  .k-e18-criterion > .k-select {
    flex: 1 1 var(--k-size-col-md);
    min-inline-size: 0;
  }

  .k-e18-criterion > .k-text-field {
    flex: 1 1 var(--k-size-col-md);
    min-inline-size: 0;
  }

  /* The exceptions, indented by their depth (CSSOM, REQ-020-14). */
  .k-e18-tree {
    display: grid;
    gap: var(--k-space-1);
    list-style: none;
  }

  .k-e18-exception {
    display: flex;
    flex-wrap: wrap;
    gap: var(--k-space-2);
    align-items: center;
    min-inline-size: 0;
    padding-block: var(--k-space-1);
    padding-inline-start: calc(var(--k-e18-depth, 0) * var(--k-space-6));
    border-block-end: var(--k-border-width) solid var(--k-border);
  }

  .k-e18-exception__criterion {
    min-inline-size: 0;
    overflow-wrap: anywhere;
  }

  .k-e18-depth {
    display: inline-grid;
    place-items: center;
    inline-size: var(--k-space-5);
    block-size: var(--k-space-5);
    font-size: var(--k-text-xs);
    color: var(--k-text-muted);
    border: var(--k-border-width) solid var(--k-border-strong);
    border-radius: var(--k-radius-full);
  }

  .k-e18-values {
    display: flex;
    flex-wrap: wrap;
    gap: var(--k-space-2);
    align-items: center;
    min-inline-size: 0;
  }

  .k-e18-plain {
    display: grid;
    gap: var(--k-space-1);
  }

  .k-e18-candidate {
    display: grid;
    gap: var(--k-space-2);
    padding-block-end: var(--k-space-3);
    border-block-end: var(--k-border-width) solid var(--k-border);
  }
}
</style>
