<script setup lang="ts">
// E19 — Conformité (spec 017, task 1; view `compliance` and action `check` of the plugin
// `policy`, REQ-017-11 to 13; Q-017-2; 010, REQ-010-11 to 16, 36 to 38; 011/F21): a query
// of conformity — ends by kind (address, prefix or interface: never a machine since
// 011/F21), chosen by search or by a pasted identifier; the object (service, transport and
// port, or flow type); an optional file; the entity; the dates of the context — then the
// synthesis and one row per evaluation, the chosen one explained in the right panel:
// decisive element and its text, combination, discarded rules, conditions (met, unmet, not
// evaluated), signals and score, and with `access: full` the values of each point by axis,
// their source and origin; with `reduced`, the ranks and values only, no further read. A
// port without a service, a query too large (422), an end absent or not readable (404).
import {
  Button,
  CodeBadge,
  Cluster,
  DataGrid,
  EmptyState,
  EnvChip,
  FormField,
  Grid,
  InlineAlert,
  KeyLabel,
  Select,
  Stack,
  StatusBadge,
  TextField,
  type GridColumn,
  type ListOption,
  type State,
} from "@kartenn/ui";
import { computed, ref } from "vue";
import { RouterLink, useRoute } from "vue-router";
import Attributs from "./EcranAttributs.vue";
import Cadre from "./EcranCadre.vue";
import Panneau from "./EcranPanneau.vue";
import { ENV, ID, RULES, type EnvKey } from "./donnees.ts";
import { ENTITIES, SPACES_017, VERDICT, lien, type Verdict } from "./donnees017.ts";

const props = defineProps<{ vue: string }>();
const route = useRoute();
const reduced = props.vue === "reduite";
const port = props.vue === "port";
const tooLarge = props.vue === "trop-large";
const absent = props.vue === "absente";
const answered = !tooLarge && !absent;

const KINDS: ListOption[] = [
  { value: "address", label: "Adresse", icon: "address" },
  { value: "prefix", label: "Préfixe", icon: "prefix" },
  { value: "interface", label: "Interface", icon: "interface" },
];
const sourceKind = ref<string | undefined>(tooLarge ? "prefix" : "address");
const source = ref(tooLarge ? "192.0.2.0/24" : "192.0.2.10");
const destinationKind = ref<string | undefined>(absent ? "address" : "interface");
const destination = ref(absent ? "01JD9Z0Q1R2S3T4U5V6W7X8Y9Z" : "srv-db-03 · eth0");
const objectKind = ref<"service" | "port" | "type">(port ? "port" : "service");
const SERVICE_CHOICES: ListOption[] = [
  { value: "postgres", label: "PostgreSQL", detail: "postgres" },
  { value: "ssh", label: "Shell sécurisé", detail: "ssh" },
  { value: "https", label: "Web chiffré", detail: "https" },
];
const service = ref<string | undefined>("postgres");
const portValue = ref("tcp/9999");
const entity = ref<string | undefined>(reduced ? "nord" : undefined);
const ENTITY_CHOICES: ListOption[] = Object.entries(ENTITIES).map(([value, label]) => ({
  value,
  label,
}));
const settings = lien("E13", "politique", route.query);

// ——— The answer (QUERY /v1/verdicts) ———

type Cause = "rule" | "unclassified" | "unknown_service";
interface Evaluation extends Record<string, unknown> {
  key: string;
  points: string;
  source: EnvKey | null;
  destination: EnvKey | null;
  verdict: Verdict;
  outcome: "automatic" | "review" | "noncompliant";
  decisive: string;
  cause: Cause;
  score: string;
}
const evaluations = computed<Evaluation[]>(() =>
  port
    ? [
        {
          key: "e1",
          points: reduced ? "point 1 → point 1" : "192.0.2.10 → 192.0.2.33",
          source: "prod",
          destination: "prod",
          verdict: "denied",
          outcome: "review",
          decisive: "unknown_service",
          cause: "unknown_service",
          score: "10",
        },
      ]
    : [
        {
          key: "e1",
          points: reduced ? "point 1 → point 1" : "192.0.2.10 → 192.0.2.33",
          source: "prod",
          destination: "prod",
          verdict: "allowed",
          outcome: "automatic",
          decisive: "bdd-meme-env",
          cause: "rule",
          score: "0",
        },
        {
          key: "e2",
          points: reduced ? "point 1 → point 2" : "192.0.2.10 → 192.0.2.34",
          source: "prod",
          destination: "rece",
          verdict: "denied",
          outcome: "noncompliant",
          decisive: "deny-prod-hors-prod",
          cause: "rule",
          score: "40",
        },
        {
          key: "e3",
          points: reduced ? "point 1 → point 3" : "192.0.2.10 → 2001:db8::34",
          source: "prod",
          destination: null,
          verdict: "denied",
          outcome: "review",
          decisive: "unclassified",
          cause: "unclassified",
          score: "20",
        },
      ],
);
const OUTCOME: Record<Evaluation["outcome"], { state: State; label: string }> = {
  automatic: { state: "ok", label: "Automatique" },
  review: { state: "review", label: "À instruire" },
  noncompliant: { state: "ko", label: "Non conforme" },
};
const columns: GridColumn[] = [
  { id: "points", label: reduced ? "Points" : "Paire d'adresses", width: "lg", mono: true },
  { id: "envs", label: "Environnements", width: "lg", priority: 2 },
  { id: "verdict", label: "Verdict", width: "md", sortable: true, filterable: true },
  { id: "outcome", label: "Issue", width: "md", sortable: true },
  { id: "decisive", label: "Élément décisif", width: "md", mono: true },
  { id: "score", label: "Score", width: "xs", align: "end", priority: 3 },
];
const chosen = ref<string[]>([port ? "e1" : "e2"]);
const decisiveRule = RULES.find((r) => r.key === "deny-prod-hors-prod")!;

/** The values of the two points of the chosen evaluation, by axis (full explanation). */
const POINT_VALUES = [
  {
    point: "192.0.2.10",
    axis: "Environnement",
    value: "prod" as EnvKey,
    source: "tree",
    origin: "192.0.2.0/26",
  },
  {
    point: "192.0.2.10",
    axis: "Sensibilité",
    value: null,
    label: "interne",
    source: "machine",
    origin: "srv-app-01",
  },
  {
    point: "192.0.2.34",
    axis: "Environnement",
    value: "rece" as EnvKey,
    source: "tree",
    origin: "192.0.2.34",
  },
];
</script>

<template>
  <Cadre
    title="Conformité"
    active="compliance"
    space="policy"
    :spaces="SPACES_017"
    :draft="0"
    :panel="answered ? 'detail' : undefined"
    :crumbs="[{ label: 'Politique', to: '/x/policy' }, { label: 'Conformité' }]"
  >
    <Stack gap="3">
      <Panneau title="Vérifier une connexion" detail="au présent · politique r42" main>
        <p v-if="props.vue === 'complete'" class="k-ecran-muted">
          Source pré-remplie depuis la fiche de
          <RouterLink :to="lien('E27', 'actions', route.query)" class="k-mono"
            >192.0.2.10</RouterLink
          >.
        </p>
        <Grid :columns="2" gap="3">
          <FormField label="Source" group required>
            <Cluster gap="2" align="start" class="k-e19-end">
              <Select v-model="sourceKind" label="Sorte de la source" hide-label :options="KINDS" />
              <TextField
                v-model="source"
                label="Source : recherche ou identifiant collé"
                hide-label
                mono
                icon="search"
                :error="tooLarge ? '254 adresses : 64 au plus par extrémité' : undefined"
              />
            </Cluster>
          </FormField>
          <FormField label="Destination" group required>
            <Cluster gap="2" align="start" class="k-e19-end">
              <Select
                v-model="destinationKind"
                label="Sorte de la destination"
                hide-label
                :options="KINDS"
              />
              <TextField
                v-model="destination"
                label="Destination : recherche ou identifiant collé"
                hide-label
                mono
                icon="search"
                :error="
                  absent
                    ? 'Absente ou non autorisée : aucune adresse lisible ne porte cet identifiant'
                    : undefined
                "
              />
            </Cluster>
          </FormField>
          <FormField label="Objet" group required>
            <Stack gap="2">
              <Cluster gap="1">
                <Button
                  size="sm"
                  :pressed="objectKind === 'service'"
                  @click="objectKind = 'service'"
                  >Service</Button
                >
                <Button size="sm" :pressed="objectKind === 'port'" @click="objectKind = 'port'"
                  >Transport et port</Button
                >
                <Button size="sm" :pressed="objectKind === 'type'" @click="objectKind = 'type'"
                  >Type de flux</Button
                >
              </Cluster>
              <Select
                v-if="objectKind === 'service'"
                v-model="service"
                label="Service"
                hide-label
                :options="SERVICE_CHOICES"
              />
              <TextField v-else v-model="portValue" label="Transport et port" hide-label mono />
            </Stack>
          </FormField>
          <Select
            v-model="entity"
            label="Entité"
            :options="ENTITY_CHOICES"
            :required="reduced"
            placeholder="Aucune"
            :hint="
              reduced
                ? 'Exigée pour une réponse sous policy.flow.request.'
                : 'Facultative avec policy.verdict.read.'
            "
          />
        </Grid>
        <Cluster gap="2" justify="between">
          <Button size="sm" variant="ghost" icon="plus">Ajouter un dossier</Button>
          <Cluster gap="2">
            <Button icon="copy">Copier le lien</Button>
            <Button variant="primary" icon="clipboard-check" :shortcut="['Control', 'Enter']"
              >Vérifier</Button
            >
          </Cluster>
        </Cluster>
      </Panneau>

      <Panneau
        title="Réponse"
        :detail="
          answered
            ? `${evaluations.length} évaluation${evaluations.length > 1 ? 's' : ''}`
            : undefined
        "
        flush
      >
        <template v-if="answered" #toolbar>
          <Stack gap="2">
            <p class="k-e19-summary">
              <StatusBadge v-if="port" state="ko" label="Non conforme" />
              <StatusBadge v-else state="warn" label="En partie conforme" />
              <span>Issue la plus défavorable :</span>
              <StatusBadge v-if="port" state="review" label="À instruire" />
              <StatusBadge v-else state="ko" label="Non conforme" />
              <span class="k-ecran-muted k-num"
                >{{ port ? "0 conforme · 1 non conforme" : "1 conforme · 2 non conformes" }} ·
                politique r42 · sans dossier</span
              >
            </p>
            <InlineAlert
              v-if="port"
              tone="warn"
              title="Aucun service ne déclare tcp/9999 : la politique ne peut pas la classer."
              :items="[
                {
                  message:
                    'Choisissez un service existant, ou faites nommer ce port dans les Paramètres de la politique.',
                  code: 'unknown_service',
                },
              ]"
            >
              <RouterLink :to="settings">Services, dans les Paramètres de la politique</RouterLink>
            </InlineAlert>
            <p v-if="reduced" class="k-ecran-muted">
              Explication réduite : 2 adresses non lisibles omises ; les points sont désignés par
              leur rang.
            </p>
          </Stack>
        </template>
        <InlineAlert
          v-if="tooLarge"
          tone="ko"
          title="La requête est trop large : restreignez la source."
          :items="[
            {
              message:
                '254 adresses dans 192.0.2.0/24 ; une extrémité en porte 64 au plus, et la requête 10 000 évaluations au plus.',
              code: 'policy-query-too-large',
            },
          ]"
        >
          Votre saisie est gardée. Choisissez un préfixe plus petit, une interface ou une adresse.
        </InlineAlert>
        <EmptyState
          v-else-if="absent"
          icon="search"
          title="Pas de réponse : la destination est absente ou non autorisée"
        >
          Vérifiez l'identifiant collé, ou cherchez l'adresse par sa valeur.
        </EmptyState>
        <DataGrid
          v-else
          v-model:selected="chosen"
          label="Évaluations de la requête"
          :columns="columns"
          :rows="evaluations"
          row-key="key"
          selectable="single"
        >
          <template #cell="{ row, column, value }">
            <span v-if="column.id === 'envs'" class="k-e19-envs">
              <EnvChip v-if="row.source" v-bind="ENV[row.source]" />
              <span class="k-ecran-muted">→</span>
              <EnvChip v-if="row.destination" v-bind="ENV[row.destination]" />
              <StatusBadge v-else state="warn" label="Non classé" />
            </span>
            <StatusBadge
              v-else-if="column.id === 'verdict'"
              :state="VERDICT[row.verdict].state"
              :label="VERDICT[row.verdict].label"
            />
            <StatusBadge
              v-else-if="column.id === 'outcome'"
              :state="OUTCOME[row.outcome].state"
              :label="OUTCOME[row.outcome].label"
            />
            <CodeBadge v-else-if="column.id === 'decisive'" :code="String(value)" />
            <template v-else>{{ value }}</template>
          </template>
        </DataGrid>
        <template v-if="answered" #footer>
          <span class="k-ecran-muted">Entrée sur une ligne : son explication à droite</span>
          <span class="k-num">Requête {{ ID.request }}</span>
        </template>
      </Panneau>
    </Stack>

    <!-- The chosen evaluation, explained. -->
    <template #detail>
      <Stack v-if="port" gap="3">
        <h2 class="k-ecran-subtitle k-mono">{{ evaluations[0]!.points }}</h2>
        <Attributs
          :items="[
            { key: 'verdict', label: 'Verdict' },
            { key: 'cause', label: 'Élément décisif' },
            { key: 'object', label: 'Objet', value: 'tcp/9999, aucun service', mono: true },
          ]"
        >
          <template #verdict><StatusBadge state="ko" label="Non conforme" /></template>
          <template #cause><CodeBadge code="unknown_service" /></template>
        </Attributs>
        <p>
          Sans service, le type de flux est inconnu : aucune règle ne s'applique et la demande
          partirait à l'instruction.
        </p>
      </Stack>
      <Stack v-else gap="3">
        <h2 class="k-ecran-subtitle k-mono">{{ evaluations[1]!.points }}</h2>
        <Attributs
          :items="[
            { key: 'verdict', label: 'Verdict' },
            { key: 'decisive', label: 'Élément décisif' },
            { key: 'combination', label: 'Combinaison' },
            { key: 'score', label: 'Score', value: '40 (données restreintes)', mono: true },
          ]"
        >
          <template #verdict><StatusBadge state="ko" label="Non conforme" /></template>
          <template #decisive
            ><CodeBadge code="deny-prod-hors-prod" label="Production vers hors production"
          /></template>
          <template #combination>
            <span class="k-e19-envs"
              ><EnvChip v-bind="ENV.prod" /><span class="k-ecran-muted">→</span
              ><EnvChip v-bind="ENV.rece" /><span class="k-ecran-muted">Base de données</span></span
            >
          </template>
        </Attributs>
        <Stack as="section" gap="1" aria-labelledby="k-e19-text">
          <h3 id="k-e19-text" class="k-ecran-subtitle">Pourquoi</h3>
          <p>{{ decisiveRule.text }}</p>
        </Stack>
        <Stack as="section" gap="1" aria-labelledby="k-e19-discarded">
          <h3 id="k-e19-discarded" class="k-ecran-subtitle">Règles écartées</h3>
          <ul class="k-ecran-list k-e19-list">
            <li>
              <CodeBadge code="bdd-meme-env" />
              <span class="k-ecran-muted">destination d'un autre environnement</span>
            </li>
          </ul>
        </Stack>
        <Stack as="section" gap="1" aria-labelledby="k-e19-conditions">
          <h3 id="k-e19-conditions" class="k-ecran-subtitle">Conditions</h3>
          <ul class="k-ecran-list k-e19-list">
            <li>
              <StatusBadge state="neutral" label="Non évaluée" />
              <CodeBadge code="justification-requise" />
              <span class="k-ecran-muted">sans dossier</span>
            </li>
            <li>
              <StatusBadge state="ok" label="Remplie" /><CodeBadge code="reference-externe" />
            </li>
          </ul>
        </Stack>
        <Stack as="section" gap="1" aria-labelledby="k-e19-values">
          <h3 id="k-e19-values" class="k-ecran-subtitle">Valeurs des points</h3>
          <ul class="k-ecran-list k-e19-list">
            <li v-for="v in POINT_VALUES" :key="`${v.point}-${v.axis}`" class="k-e19-value">
              <template v-if="reduced">
                <span>Point {{ v.point === "192.0.2.10" ? 1 : 2 }}</span>
                <span class="k-ecran-muted">{{ v.axis }}</span>
                <EnvChip v-if="v.value" v-bind="ENV[v.value]" />
                <KeyLabel v-else value="interne" label="Interne" />
              </template>
              <template v-else>
                <code>{{ v.point }}</code>
                <span class="k-ecran-muted">{{ v.axis }}</span>
                <EnvChip v-if="v.value" v-bind="ENV[v.value]" />
                <KeyLabel v-else value="interne" label="Interne" />
                <span class="k-ecran-muted">{{
                  v.source === "tree" ? "arbre, de" : "machine"
                }}</span>
                <a :href="`/prefixes/${v.origin}`" class="k-mono">{{ v.origin }}</a>
              </template>
            </li>
          </ul>
        </Stack>
      </Stack>
    </template>
  </Cadre>
</template>

<style>
@layer components {
  .k-e19-end > .k-select {
    flex: 0 1 var(--k-size-col-sm);
    min-inline-size: 0;
  }

  .k-e19-end > .k-text-field {
    flex: 1 1 var(--k-size-col-sm);
    min-inline-size: 0;
  }

  .k-e19-summary,
  .k-e19-envs,
  .k-e19-value {
    display: flex;
    flex-wrap: wrap;
    gap: var(--k-space-2);
    align-items: center;
    min-inline-size: 0;
  }

  .k-e19-list {
    display: grid;
    gap: var(--k-space-1);
  }

  .k-e19-list > li {
    display: flex;
    flex-wrap: wrap;
    gap: var(--k-space-2);
    align-items: center;
    min-inline-size: 0;
  }
}
</style>
