<script setup lang="ts">
// E23 — Instruction (spec 017, task 1; view `review` and action `review-in-flow` of the
// plugin `flow`, REQ-017-20, 21; Q-017-3; 011/F19, F20; 021/X37): the queue of the
// requests and extensions pending (`GET /v1/flows?review=pending`, a page of 100), each row
// with the evaluation of its triage read as is (no read per changeset); the card of
// review (`review/<changeset>`): the triage, the file, the decision in the right panel —
// approve with an end at most 12 months after the decision and, the verdict being
// refused, a waiver (motive, accepted risk, end at most 6 months); reject with a motive;
// the refusals of the server shown without losing the input; no decision on one's own
// request (or one made on one's behalf: `flow.separation`). In « À instruire » of the
// shell, a changeset that comes from a request or an extension of a flow carries
// « Instruire dans Flux » instead of « Valider ».
import {
  Button,
  CodeBadge,
  Cluster,
  DataGrid,
  DateTimeField,
  Dialog,
  EmptyState,
  FilterPicker,
  InlineAlert,
  KeyLabel,
  Stack,
  StatusBadge,
  TextField,
  type Filter,
  type FilterModel,
  type GridColumn,
  type GridSort,
  type State,
} from "@kartenn/ui";
import { ref } from "vue";
import { RouterLink, useRoute } from "vue-router";
import Attributs from "./EcranAttributs.vue";
import Cadre from "./EcranCadre.vue";
import Panneau from "./EcranPanneau.vue";
import { RULES } from "./donnees.ts";
import { CHANGESET, ENTITIES, ME, PEOPLE, SPACES_017, lien } from "./donnees017.ts";

const props = defineProps<{ vue: string }>();
const route = useRoute();
const card = ["decision", "rejet", "refus", "demandeur"].includes(props.vue);
const own = props.vue === "demandeur";
const refused = props.vue === "refus";
const decisionLink = lien("E23", "decision", route.query);

// ——— The queue ———

type Kind = "request" | "extension";
const KIND: Record<Kind, string> = { request: "Demande", extension: "Prolongation" };
interface Pending extends Record<string, unknown> {
  key: string;
  kind: Kind;
  requester: string;
  entity: string;
  source: string;
  destination: string;
  service: string;
  outcome: { state: State; label: string };
  decisive: string;
  score: string;
  warnings: string;
  mine: boolean;
}
const REVIEW = { state: "review" as State, label: "À instruire" };
const NONCOMPLIANT = { state: "ko" as State, label: "Non conforme" };
const pending: Pending[] = [
  {
    key: "q1",
    kind: "request",
    requester: PEOPLE.alex,
    entity: ENTITIES.nord,
    source: "dev-web-01 · eth0",
    destination: "srv-db-02 · eth0",
    service: "postgres",
    outcome: NONCOMPLIANT,
    decisive: "deny-deve-prod",
    score: "40",
    warnings: "données restreintes",
    mine: false,
  },
  {
    key: "q2",
    kind: "extension",
    requester: ME,
    entity: ENTITIES.nord,
    source: "bastion-01 · eth0",
    destination: "srv-app-01 · eth1",
    service: "ssh",
    outcome: REVIEW,
    decisive: "admin-par-bastion",
    score: "10",
    warnings: "—",
    mine: true,
  },
  {
    key: "q3",
    kind: "request",
    requester: PEOPLE.sacha,
    entity: ENTITIES.pilote,
    source: "srv-rec-03 · eth0",
    destination: "srv-app-01 · eth0",
    service: "https",
    outcome: REVIEW,
    decisive: "rece-vers-prod",
    score: "0",
    warnings: "—",
    mine: false,
  },
  {
    key: "q4",
    kind: "request",
    requester: PEOPLE.lou,
    entity: ENTITIES.sud,
    source: "203.0.113.50",
    destination: "srv-app-01 · eth0",
    service: "api",
    outcome: REVIEW,
    decisive: "unclassified",
    score: "20",
    warnings: "source non classée",
    mine: false,
  },
];
const pendingColumns: GridColumn[] = [
  { id: "kind", label: "Nature", width: "sm", sortable: true, filterable: true },
  { id: "requester", label: "Demandeur, entité", width: "md", wrap: true, sortable: true },
  { id: "ends", label: "Extrémités, service", width: "lg", mono: true, wrap: true },
  { id: "outcome", label: "Issue du triage", width: "md", sortable: true, filterable: true },
  { id: "decisive", label: "Élément décisif", width: "md", mono: true, priority: 2 },
  { id: "score", label: "Score", width: "xs", align: "end", sortable: true, priority: 3 },
  { id: "actions", label: "Actions", width: "md", actions: true },
];
const pendingSort = ref<GridSort | undefined>({ column: "score", direction: "descending" });
const QUEUE_FILTERS: FilterModel = [
  {
    key: "kind",
    label: "Nature",
    type: "enum",
    operators: ["eq"],
    options: [
      { value: "request", label: "Demande" },
      { value: "extension", label: "Prolongation" },
    ],
  },
  {
    key: "entity",
    label: "Entité",
    type: "enum",
    operators: ["eq"],
    options: Object.entries(ENTITIES).map(([value, label]) => ({ value, label })),
  },
  {
    key: "outcome",
    label: "Issue",
    type: "enum",
    operators: ["eq"],
    options: [
      { value: "review", label: "À instruire" },
      { value: "noncompliant", label: "Non conforme" },
    ],
  },
];
const queueFilters = ref<Filter[]>([]);
/** The empty queue: no counter on the entry « Instruction ». */
const EMPTY_SPACES = SPACES_017.map((space) => ({
  ...space,
  entries: space.entries.map((entry) =>
    entry.id === "flow-review"
      ? { id: entry.id, label: entry.label, icon: entry.icon, to: entry.to }
      : entry,
  ),
}));

// ——— The card of review ———

const denyText = RULES.find((r) => r.key === "deny-deve-prod")!.text;
const flowEnd = ref<string | null>(refused ? "2027-02-15" : "2026-12-31");
const waiverReason = ref("Reprise ponctuelle des jeux de test anonymisés");
const waiverRisk = ref("Données anonymisées, lecture seule, fin en juin");
const waiverEnd = ref<string | null>("2026-06-30");
const motive = ref("La recette dispose de ses propres jeux : passez par l'export anonymisé.");

// ——— « À instruire » of the shell ———

interface Proposal extends Record<string, unknown> {
  key: string;
  title: string;
  origin: string;
  author: string;
  submitted: string;
  flow: boolean;
}
const proposals: Proposal[] = [
  {
    key: "a1",
    title: "Demande de flux postgres",
    origin: "Flux : demande",
    author: PEOPLE.alex,
    submitted: "2026-01-15 09:41",
    flow: true,
  },
  {
    key: "a2",
    title: "Prolongation de flux-1043",
    origin: "Flux : prolongation",
    author: ME,
    submitted: "2026-01-15 09:20",
    flow: true,
  },
  {
    key: "a3",
    title: "Renumérotation salle B",
    origin: "Référentiel",
    author: PEOPLE.lou,
    submitted: "2026-01-14 16:02",
    flow: false,
  },
  {
    key: "a4",
    title: "Recette vers production sous motif",
    origin: "Politique",
    author: PEOPLE.sacha,
    submitted: "2026-01-15 09:58",
    flow: false,
  },
];
const proposalColumns: GridColumn[] = [
  { id: "title", label: "Changeset", width: "fill", sortable: true },
  { id: "origin", label: "Origine", width: "md", filterable: true },
  { id: "author", label: "Auteur", width: "md", sortable: true, priority: 2 },
  { id: "submitted", label: "Soumis", width: "md", mono: true, sortable: true, priority: 3 },
  { id: "actions", label: "Actions", width: "lg", actions: true },
];
</script>

<template>
  <!-- « À instruire » of the shell. -->
  <Cadre
    v-if="props.vue === 'a-instruire'"
    title="À instruire"
    active="proposals"
    space="changes"
    :spaces="SPACES_017"
    :draft="0"
    :crumbs="[{ label: 'Changesets', to: '/changesets' }, { label: 'À instruire' }]"
  >
    <Panneau title="À instruire" detail="changesets d'autres auteurs, et les vôtres" main flush>
      <DataGrid
        label="Changesets à instruire"
        :columns="proposalColumns"
        :rows="proposals"
        row-key="key"
      >
        <template #cell="{ row, column, value }">
          <RouterLink
            v-if="column.id === 'title'"
            :to="row.flow ? decisionLink : lien('E4', 'relecteur', route.query)"
            >{{ row.title }}</RouterLink
          >
          <Cluster v-else-if="column.id === 'actions'" gap="1" :wrap="false">
            <template v-if="row.flow">
              <Button
                size="sm"
                icon="flow"
                v-bind="
                  row.author === ME
                    ? {
                        reason:
                          'Vous en êtes le demandeur : un autre instructeur décide (flow.separation).',
                      }
                    : {}
                "
                >Instruire dans Flux</Button
              >
            </template>
            <template v-else>
              <Button size="sm" icon="check">Valider</Button>
              <Button size="sm" variant="ghost" icon="x">Rejeter…</Button>
            </template>
          </Cluster>
          <template v-else>{{ value }}</template>
        </template>
      </DataGrid>
      <template #footer>
        <span class="k-num">4 changesets · 2 de flux</span>
        <span class="k-ecran-muted"
          >« Valider » n'ajoute pas la revue d'un flux : sa demande s'instruit dans Flux</span
        >
      </template>
    </Panneau>
  </Cadre>

  <!-- The queue of the flow plugin. -->
  <Cadre
    v-else-if="!card"
    title="Instruction"
    active="flow-review"
    space="flows"
    :spaces="props.vue === 'vide' ? EMPTY_SPACES : SPACES_017"
    :draft="0"
    :crumbs="[{ label: 'Flux', to: '/x/flow' }, { label: 'Instruction' }]"
  >
    <Panneau v-if="props.vue === 'vide'" title="Instruction" main>
      <EmptyState icon="send" title="Rien à instruire">
        Aucune demande ni prolongation n'attend une décision que vous pouvez prendre.
        <template #action>
          <Button icon="clock">Voir les échéances</Button>
        </template>
      </EmptyState>
    </Panneau>
    <Panneau v-else title="Instruction" detail="demandes et prolongations en attente" main flush>
      <template #toolbar>
        <FilterPicker
          v-model="queueFilters"
          :model="QUEUE_FILTERS"
          label="Filtres de la file"
          :shortcut="['/']"
        />
      </template>
      <DataGrid
        v-model:sort="pendingSort"
        label="Demandes et prolongations à instruire"
        :columns="pendingColumns"
        :rows="pending"
        row-key="key"
      >
        <template #cell="{ row, column, value }">
          <template v-if="column.id === 'ends'"
            >{{ row.source }} → {{ row.destination }} · {{ row.service }}</template
          >
          <template v-else-if="column.id === 'requester'"
            >{{ row.requester }} <span class="k-ecran-muted">· {{ row.entity }}</span></template
          >
          <StatusBadge
            v-else-if="column.id === 'outcome'"
            :state="row.outcome.state"
            :label="row.outcome.label"
          />
          <span v-else-if="column.id === 'actions' && row.mine" class="k-ecran-muted"
            >Votre demande</span
          >
          <Button v-else-if="column.id === 'actions'" size="sm" icon="send">Instruire</Button>
          <template v-else-if="column.id === 'kind'">{{ KIND[row.kind] }}</template>
          <template v-else>{{ value }}</template>
        </template>
      </DataGrid>
      <template #footer>
        <span class="k-num">4 en attente · 3 demandes · 1 prolongation</span>
        <span class="k-ecran-muted">Évaluation du triage, telle qu'enregistrée · page de 100</span>
      </template>
    </Panneau>
  </Cadre>

  <!-- The card of review. -->
  <Cadre
    v-else
    title="Instruire une demande de flux"
    active="flow-review"
    space="flows"
    :spaces="SPACES_017"
    :draft="0"
    panel="detail"
    :crumbs="[
      { label: 'Flux', to: '/x/flow' },
      { label: 'Instruction', to: '/x/flow/review' },
      { label: CHANGESET.request.slice(0, 10), mono: true },
    ]"
  >
    <Stack gap="3">
      <Panneau
        title="Demande de flux postgres"
        :detail="own ? `de ${PEOPLE.lou}, pour votre compte` : `de ${PEOPLE.alex}`"
        main
      >
        <Cluster as="p" gap="2">
          <span class="k-mono">dev-web-01 · eth0 → srv-db-02 · eth0</span>
          <KeyLabel value="postgres" label="PostgreSQL" show-key />
          <span class="k-ecran-muted">Base de données</span>
        </Cluster>
        <Cluster as="p" gap="2">
          <span class="k-ecran-muted">Triage :</span>
          <StatusBadge state="ko" label="Non conforme" />
          <StatusBadge state="ko" label="Issue : non conforme" />
          <CodeBadge code="deny-deve-prod" label="Développement vers production" />
          <span class="k-ecran-muted k-num">score 40 · politique r42</span>
        </Cluster>
        <p>{{ denyText }}</p>
        <Attributs
          columns
          :items="[
            {
              key: 'pairs',
              label: 'Paires',
              value: '192.0.2.70 → 192.0.2.30 : non conforme',
              mono: true,
            },
            {
              key: 'unmet',
              label: 'Conditions non remplies',
              value: 'justification-requise',
              mono: true,
            },
            {
              key: 'warnings',
              label: 'Avertissements',
              value: 'données restreintes (signal donnees-restreintes)',
            },
            { key: 'entity', label: 'Entité', value: ENTITIES.nord },
            { key: 'reference', label: 'Référence', value: 'ticketing:CHG-1090', mono: true },
            { key: 'period', label: 'Période demandée' },
            { key: 'author', label: 'Auteur du changeset', value: own ? PEOPLE.lou : PEOPLE.alex },
            { key: 'behalf', label: 'Pour le compte de', value: own ? ME : '—' },
          ]"
        >
          <template #period
            >au présent → <span class="k-mono k-e23-date">2026-12-31</span></template
          >
        </Attributs>
        <template #footer>
          <span class="k-ecran-muted">Motif du changeset : « Reprise des jeux de test »</span>
          <CodeBadge :code="CHANGESET.request" />
        </template>
      </Panneau>
    </Stack>

    <template #detail>
      <Stack v-if="own" gap="3">
        <h2 class="k-ecran-subtitle">Décision</h2>
        <InlineAlert
          tone="info"
          title="Cette demande est faite pour votre compte : un autre instructeur en décide."
          :items="[
            {
              message:
                'L\'instructeur est distinct du demandeur et de celui pour le compte duquel il demande.',
              code: 'flow.separation',
            },
          ]"
        />
      </Stack>
      <Stack v-else gap="3">
        <h2 class="k-ecran-subtitle">Décision</h2>
        <InlineAlert
          v-if="refused"
          tone="ko"
          title="L'approbation n'a pas été écrite : la fin dépasse le plafond."
          :items="[
            {
              message: 'La fin d\'un flux instruit tombe au plus 12 mois après la décision.',
              code: 'flow.review.end_required',
            },
          ]"
        >
          Votre saisie est gardée.
        </InlineAlert>
        <DateTimeField
          v-model="flowEnd"
          label="Fin du flux"
          granularity="day"
          required
          :error="refused ? 'Au plus le 2027-01-15, 12 mois après la décision' : undefined"
          hint="Au plus 12 mois après la décision : le 2027-01-15."
        />
        <Stack as="section" gap="2" aria-labelledby="k-e23-waiver">
          <h3 id="k-e23-waiver" class="k-ecran-subtitle">Dérogation exigée</h3>
          <p class="k-ecran-muted">Le triage est non conforme : approuver, c'est déroger.</p>
          <TextField v-model="waiverReason" label="Motif" required />
          <TextField v-model="waiverRisk" label="Risque accepté" required />
          <DateTimeField
            v-model="waiverEnd"
            label="Fin de la dérogation"
            granularity="day"
            required
            hint="Au plus tôt de la fin du flux et du 2026-07-15 (6 mois)."
          />
        </Stack>
      </Stack>
    </template>
    <template #detail-footer>
      <Cluster v-if="!own" justify="end">
        <Button variant="ghost" icon="x">Rejeter…</Button>
        <Button
          variant="primary"
          icon="check"
          v-bind="refused ? { reason: 'La fin du flux dépasse le plafond de 12 mois.' } : {}"
          >Approuver avec dérogation</Button
        >
      </Cluster>
    </template>

    <Dialog
      v-if="props.vue === 'rejet'"
      :open="true"
      title="Rejeter la demande de flux postgres"
      :description="`Le motif est envoyé à ${PEOPLE.alex} ; le rejet devient une non-conformité de sa demande.`"
    >
      <Stack gap="3">
        <TextField
          v-model="motive"
          label="Motif du rejet"
          required
          hint="Une phrase : ce qu'il faut faire à la place."
        />
        <Attributs :items="[{ key: 'rule', label: 'Règle en cause' }]">
          <template #rule
            ><CodeBadge code="deny-deve-prod" label="Développement vers production"
          /></template>
        </Attributs>
        <p class="k-ecran-muted">
          Reprise de l'évaluation du triage ; son texte accompagne la non-conformité.
        </p>
      </Stack>
      <template #actions>
        <Button variant="ghost">Annuler</Button>
        <Button variant="danger" icon="x">Rejeter</Button>
      </template>
    </Dialog>
  </Cadre>
</template>

<style>
@layer components {
  .k-e23-date {
    white-space: nowrap;
  }
}
</style>
