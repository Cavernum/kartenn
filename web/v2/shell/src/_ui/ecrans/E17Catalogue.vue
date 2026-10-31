<script setup lang="ts">
// E17 — Catalogue de la politique (spec 017, task 1; view `catalogue` of the plugin
// `policy`, REQ-017-10, 30, 31; Q-017-5), as the 027 leaves it: environments, services,
// flow types, axes and their values are settings of the policy, written in the Settings
// (E13, `policy.settings.edit`, 027/P4, P8); the rules of the policy stay apart
// (`policy.rule.admin`, 027/P7). The catalogue keeps the objects that change verdicts:
// conditions of admissibility (selector, requirement, pedagogical text required), risk
// signals (kind, parameters, weight) and exempted roles (creation and closure). The same
// shape for each: a dated list, the chosen one in the right panel, writes through the
// draft (a change of weight is simulated there: the flows whose score changes).
import {
  Button,
  CodeBadge,
  Cluster,
  DataGrid,
  DiffValue,
  ExpandableGrid,
  IconButton,
  InlineAlert,
  KeyLabel,
  Select,
  SplitButton,
  Stack,
  StatusBadge,
  Tabs,
  TextField,
  type GridColumn,
  type ListOption,
  type SplitAction,
  type TabItem,
} from "@kartenn/ui";
import { ref } from "vue";
import { RouterLink, useRoute } from "vue-router";
import Attributs from "./EcranAttributs.vue";
import Cadre from "./EcranCadre.vue";
import Panneau from "./EcranPanneau.vue";
import Soumission from "./EcranSoumission.vue";
import { SPACES_017, lien } from "./donnees017.ts";

const props = defineProps<{ vue: string }>();
const route = useRoute();
const readOnly = props.vue === "roles";
const tab = ref(
  props.vue === "signaux" ? "signals" : props.vue === "roles" ? "roles" : "conditions",
);
const tabs: TabItem[] = [
  { value: "conditions", label: "Conditions d'admissibilité", count: 3 },
  { value: "signals", label: "Signaux de risque", count: 3 },
  { value: "roles", label: "Rôles exemptés", count: 2 },
];
const settings = lien("E13", "politique", route.query);

// ——— Conditions of admissibility (/v1/admissibility-conditions) ———

interface Condition extends Record<string, unknown> {
  key: string;
  selector: string;
  requirement: string;
  text: string;
}
const conditions: Condition[] = [
  {
    key: "justification-requise",
    selector: "Environnements différents",
    requirement: "dossier.justification présent",
    text: "Un flux entre deux environnements différents porte une justification dans son dossier.",
  },
  {
    key: "donnees-personnelles",
    selector: "Données personnelles",
    requirement: "dossier.owners non vide",
    text: "Un flux de données personnelles nomme au moins un responsable.",
  },
  {
    key: "reference-externe",
    selector: "Tout flux",
    requirement: "dossier.external_refs non vide",
    text: "",
  },
];
const conditionColumns: GridColumn[] = [
  { id: "key", label: "Condition", width: "md", mono: true, sortable: true },
  { id: "selector", label: "S'applique à", width: "md", priority: 2 },
  { id: "requirement", label: "Exigence", width: "fill", mono: true },
  { id: "text", label: "Texte pédagogique", width: "lg", wrap: true, priority: 3 },
];
const SELECTORS: ListOption[] = [
  { value: "any", label: "Tout flux" },
  { value: "cross", label: "Environnements différents" },
  { value: "personal", label: "Données personnelles" },
];
const REQUIREMENTS: ListOption[] = [
  {
    value: "external_refs",
    label: "Au moins une référence externe",
    detail: "dossier.external_refs",
  },
  { value: "justification", label: "Justification présente", detail: "dossier.justification" },
  { value: "owners", label: "Au moins un responsable", detail: "dossier.owners" },
];
const selector = ref<string | undefined>("any");
const requirement = ref<string | undefined>("external_refs");
const conditionText = ref("");
const conditionActions: SplitAction[] = [
  {
    id: "draft",
    label: "Ajouter au brouillon",
    icon: "draft",
    disabled: true,
    reason: "Une condition porte un texte pédagogique en français.",
  },
];
const conditionAction = ref("draft");

// ——— Risk signals (/v1/risk-signals) ———

interface Signal extends Record<string, unknown> {
  key: string;
  kind: string;
  params: string;
  weight: string;
}
const signals: Signal[] = [
  { key: "exposition-internet", kind: "zone", params: "zone = internet", weight: "30" },
  {
    key: "donnees-restreintes",
    kind: "axis_value",
    params: "sensitivity = restreint",
    weight: "40",
  },
  { key: "ports-hauts", kind: "port_range", params: "tcp 1024–65535", weight: "10" },
];
const signalColumns: GridColumn[] = [
  { id: "key", label: "Signal", width: "lg", mono: true, sortable: true },
  { id: "kind", label: "Sorte", width: "sm", mono: true },
  { id: "params", label: "Paramètres", width: "fill", mono: true },
  { id: "weight", label: "Poids", width: "sm", align: "end", sortable: true },
];
const weight = ref("50");
interface Scored extends Record<string, unknown> {
  key: string;
  flow: string;
  before: string;
  after: string;
  outcome: string;
}
const scored: Scored[] = [
  { key: "s1", flow: "flux-1061", before: "30", after: "50", outcome: "—" },
  { key: "s2", flow: "flux-1046", before: "30", after: "50", outcome: "—" },
  { key: "s3", flow: "flux-0877", before: "45", after: "65", outcome: "devient à instruire" },
  { key: "s4", flow: "flux-0880", before: "45", after: "65", outcome: "devient à instruire" },
];
const scoredColumns: GridColumn[] = [
  { id: "flow", label: "Flux", width: "sm", mono: true },
  { id: "score", label: "Score", width: "md" },
  { id: "outcome", label: "Issue", width: "md", wrap: true },
];

// ——— Exempted roles (/v1/policy-exempt-roles) ———

interface Exempt extends Record<string, unknown> {
  key: string;
  label: string;
  from: string;
}
const exempts: Exempt[] = [
  { key: "supervision", label: "Supervision", from: "2025-04-02" },
  { key: "sauvegarde", label: "Sauvegarde", from: "2025-06-17" },
];
const exemptColumns: GridColumn[] = [
  { id: "key", label: "Rôle", width: "fill", sortable: true },
  { id: "from", label: "Exempté depuis", width: "md", mono: true, sortable: true },
  { id: "actions", label: "Actions", width: "sm", actions: true },
];
</script>

<template>
  <Cadre
    title="Catalogue de la politique"
    active="catalogue"
    space="policy"
    :spaces="SPACES_017"
    :draft="props.vue === 'signaux' ? 1 : 0"
    :panel="props.vue === 'signaux' ? 'draft' : props.vue === 'conditions' ? 'detail' : undefined"
    :crumbs="[{ label: 'Politique', to: '/x/policy' }, { label: 'Catalogue' }]"
  >
    <Panneau title="Catalogue" detail="ce qui change les verdicts, au présent" main>
      <InlineAlert
        v-if="readOnly"
        tone="warn"
        icon="lock"
        title="Lecture seule : écrire le catalogue demande policy.rule.admin."
        :items="[
          {
            message: 'Vous lisez le catalogue avec policy.catalogue.read.',
            code: 'policy.rule.admin',
          },
        ]"
      />
      <p class="k-ecran-muted">
        Environnements, services, types de flux, axes et valeurs :
        <RouterLink :to="settings">Paramètres de la politique</RouterLink>.
      </p>
      <Tabs v-model="tab" :tabs="tabs" label="Collections du catalogue">
        <template #conditions>
          <DataGrid
            label="Conditions d'admissibilité"
            :columns="conditionColumns"
            :rows="conditions"
            row-key="key"
            selectable="single"
            :selected="['reference-externe']"
          >
            <template #cell="{ column, value }">
              <span v-if="column.id === 'text' && !value" class="k-e17-missing">
                <StatusBadge state="ko" label="Texte manquant" />
              </span>
              <template v-else>{{ value }}</template>
            </template>
          </DataGrid>
        </template>
        <template #signals>
          <DataGrid
            label="Signaux de risque"
            :columns="signalColumns"
            :rows="signals"
            row-key="key"
            :cell-state="
              (row, column) =>
                row.key === 'exposition-internet' && column.id === 'weight' ? 'draft' : undefined
            "
          >
            <template #cell="{ row, column, value }">
              <DiffValue
                v-if="column.id === 'weight' && row.key === 'exposition-internet'"
                kind="changed"
              >
                <template #before>{{ value }}</template>
                {{ weight }}
              </DiffValue>
              <template v-else>{{ value }}</template>
            </template>
          </DataGrid>
        </template>
        <template #roles>
          <Stack gap="2">
            <DataGrid
              label="Rôles exemptés de la politique"
              :columns="exemptColumns"
              :rows="exempts"
              row-key="key"
            >
              <template #cell="{ row, column, value }">
                <KeyLabel v-if="column.id === 'key'" :value="row.key" :label="row.label" show-key />
                <IconButton
                  v-else-if="column.id === 'actions'"
                  icon="trash"
                  size="sm"
                  :label="`Clore l'exemption de ${row.label}`"
                  forbidden="policy.rule.admin"
                />
                <template v-else>{{ value }}</template>
              </template>
            </DataGrid>
            <div>
              <Button size="sm" icon="plus" forbidden="policy.rule.admin">Exempter un rôle</Button>
            </div>
            <p class="k-ecran-muted">
              Un rôle exempté ne déclenche pas les signaux de risque sur les flux qu'il demande.
            </p>
          </Stack>
        </template>
      </Tabs>
      <template #footer>
        <span class="k-num">3 conditions · 3 signaux · 2 rôles exemptés</span>
        <span class="k-ecran-muted">Au présent · politique r42</span>
      </template>
    </Panneau>

    <template #detail>
      <Stack gap="3">
        <h2 class="k-ecran-subtitle"><code>reference-externe</code></h2>
        <TextField label="Clé" model-value="reference-externe" mono readonly />
        <Select v-model="selector" label="S'applique à" :options="SELECTORS" />
        <Select v-model="requirement" label="Exigence" :options="REQUIREMENTS" />
        <TextField
          v-model="conditionText"
          label="Texte pédagogique (fr)"
          required
          error="Texte exigé : le serveur refuserait la condition (policy.text.missing)."
          hint="Il s'affiche à qui ne remplit pas la condition."
        />
        <TextField label="Texte pédagogique (en)" hint="Facultatif." />
      </Stack>
    </template>
    <template #detail-footer>
      <Cluster justify="end">
        <Button variant="ghost">Annuler</Button>
        <SplitButton v-model:action="conditionAction" :actions="conditionActions" />
      </Cluster>
    </template>

    <template #draft>
      <Stack gap="3">
        <Stack as="ol" gap="2" class="k-ecran-list" aria-label="Opérations du brouillon">
          <li class="k-ecran-draft-item">
            <Cluster justify="between" :wrap="false" gap="2">
              <span
                ><code>exposition-internet</code> · Poids :
                <DiffValue kind="changed"
                  ><template #before>30</template>{{ weight }}</DiffValue
                ></span
              >
              <IconButton
                icon="x"
                size="sm"
                label="Retirer le changement de poids de exposition-internet"
              />
            </Cluster>
          </li>
        </Stack>
        <Stack as="section" gap="2" aria-labelledby="k-e17-effect">
          <h2 id="k-e17-effect" class="k-ecran-subtitle">Effet simulé</h2>
          <Attributs
            :items="[
              { key: 'scores', label: 'Scores changés', value: '9', mono: true },
              { key: 'outcome', label: 'Deviennent à instruire', value: '2', mono: true },
            ]"
          />
          <ExpandableGrid
            label="Flux dont le score change avec le brouillon"
            :columns="scoredColumns"
            :rows="scored"
            row-key="key"
            :compact-columns="['flow', 'score']"
            :visible-rows="4"
          >
            <template #cell="{ row, column, value }">
              <DiffValue v-if="column.id === 'score'" kind="changed">
                <template #before>{{ row.before }}</template
                >{{ row.after }}
              </DiffValue>
              <template v-else>{{ value }}</template>
            </template>
          </ExpandableGrid>
          <p class="k-ecran-muted">Section <CodeBadge code="flow.flows" /> de la simulation.</p>
        </Stack>
      </Stack>
    </template>
    <template #draft-footer>
      <Soumission summary="Exposition à internet pèse plus" />
    </template>
  </Cadre>
</template>

<style>
@layer components {
  .k-e17-missing {
    display: inline-flex;
  }
}
</style>
