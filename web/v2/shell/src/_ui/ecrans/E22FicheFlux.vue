<script setup lang="ts">
// E22 — Fiche d'un flux (spec 017, task 1; view `flows/<id>` of the plugin `flow`,
// REQ-017-18, 22, 25; Q-017-4; 011/F19, F20): a flow at the date of the context — its
// ends, service, period, derived state, synthesis and outcome, file, waiver (end, motive,
// accepted risk), the evaluation of its registration and the current ones by pair of
// addresses (verdict, outcome, decisive element, unmet conditions, score, revision of the
// policy), its history. Reduced for a requester without `flow.read` (points by rank, no
// decisive element, nothing read besides). Actions: edit the file, bring the end forward,
// extend (pre-check of the added portion with the file of the flow, then 200 or 202),
// renew the waiver, close, new prefilled request; changing an end or the service is not
// offered. A flow that ended offers the new request only; a flow without any pair of
// addresses at the date says so; a link to a flow that does not exist or cannot be read
// is « absent ou non autorisé ».
import {
  Button,
  Cluster,
  CodeBadge,
  DataGrid,
  DateTimeField,
  Dialog,
  EmptyState,
  ErrorState,
  InlineAlert,
  KeyLabel,
  Menu,
  Split,
  Stack,
  StatusBadge,
  Tabs,
  type GridColumn,
  type MenuItem,
  type State,
  type TabItem,
} from "@kartenn/ui";
import { ref } from "vue";
import { RouterLink, useRoute } from "vue-router";
import Attributs from "./EcranAttributs.vue";
import Cadre from "./EcranCadre.vue";
import Panneau from "./EcranPanneau.vue";
import { ID, RULES } from "./donnees.ts";
import { CHANGESET, ENTITIES, ME, PEOPLE, SPACES_017, lien } from "./donnees017.ts";

const props = defineProps<{ vue: string }>();
const route = useRoute();
const reduced = props.vue === "reduite";
const extension = props.vue === "prolonger" || props.vue === "prolongation";
const ended = props.vue === "echu";
const absent = props.vue === "absent";
const tab = ref("evaluations");

interface Shown {
  key: string;
  source: string;
  destination: string;
  service: string;
  serviceLabel: string;
  flowType: string;
  state: { state: State; label: string };
  synthesis: { state: State; label: string };
  period: string;
  requester: string;
  reference: string;
  justification: string;
}
const SHOWN: Shown = extension
  ? {
      key: "flux-1043",
      source: "bastion-01 · eth0",
      destination: "srv-app-01 · eth1",
      service: "ssh",
      serviceLabel: "Shell sécurisé",
      flowType: "Administratif",
      state: { state: "ok", label: "Actif" },
      synthesis: { state: "ok", label: "Conforme" },
      period: "2025-07-01 → 2026-06-30",
      requester: PEOPLE.alex,
      reference: "ticketing:CHG-1043",
      justification: "Administration des serveurs d'application",
    }
  : ended
    ? {
        key: "flux-0988",
        source: "srv-rec-04 · eth0",
        destination: "srv-app-01 · eth0",
        service: "https",
        serviceLabel: "Web chiffré",
        flowType: "Applicatif",
        state: { state: "neutral", label: "Échu" },
        synthesis: { state: "ko", label: "Non conforme" },
        period: "2025-01-06 → 2025-12-31",
        requester: ME,
        reference: "ticketing:CHG-0988",
        justification: "Jeux de contrôle de la recette",
      }
    : {
        key: "flux-1037",
        source: "dev-web-01 · eth0",
        destination: "srv-db-02 · eth0",
        service: "postgres",
        serviceLabel: "PostgreSQL",
        flowType: "Base de données",
        state: { state: "warn", label: "Dérogé" },
        synthesis: { state: "ko", label: "Non conforme" },
        period: "2025-09-30 → 2026-03-31",
        requester: ME,
        reference: "ticketing:CHG-1037",
        justification: "Reprise des données de test anonymisées",
      };

const menu: MenuItem[] = ended
  ? [
      { id: "copy", label: "Copier l'identifiant", icon: "copy" },
      { id: "date", label: "Lire à une autre date…", icon: "calendar" },
    ]
  : [
      {
        id: "extend",
        label: "Prolonger…",
        icon: "clock",
        ...(props.vue === "prolongation"
          ? { reason: "Une prolongation jusqu'au 2026-12-31 attend déjà l'instruction." }
          : {}),
      },
      { id: "shorten", label: "Avancer la fin…", icon: "calendar" },
      ...(!extension
        ? [
            {
              id: "waiver",
              label: "Recertifier la dérogation…",
              icon: "shield",
              ...(reduced ? { forbidden: "flow.waiver.grant" } : {}),
            } as MenuItem,
          ]
        : []),
      { id: "renew", label: "Nouvelle demande pré-remplie", icon: "redo-2" },
      { id: "copy", label: "Copier l'identifiant", icon: "copy" },
      { kind: "separator" },
      {
        id: "close",
        label: "Clore…",
        icon: "trash",
        danger: true,
        ...(reduced ? { forbidden: "flow.write" } : {}),
      },
    ];

const tabs: TabItem[] = [
  { value: "evaluations", label: "Évaluations", count: ended ? 0 : 1 },
  { value: "history", label: "Historique", count: 3 },
];
interface Evaluation extends Record<string, unknown> {
  key: string;
  pair: string;
  verdict: { state: State; label: string };
  outcome: { state: State; label: string };
  decisive: string | null;
  unmet: string;
  score: string;
}
const DENIED = { state: "ko" as State, label: "Non conforme" };
const OK = { state: "ok" as State, label: "Conforme" };
const NONCOMPLIANT = { state: "ko" as State, label: "Non conforme" };
const AUTO = { state: "ok" as State, label: "Automatique" };
const evaluations: Evaluation[] = extension
  ? [
      {
        key: "p1",
        pair: "198.51.100.2 → 198.51.100.5",
        verdict: OK,
        outcome: AUTO,
        decisive: "admin-par-bastion",
        unmet: "—",
        score: "0",
      },
    ]
  : [
      {
        key: "p1",
        pair: reduced ? "point 1 → point 1" : "192.0.2.70 → 192.0.2.30",
        verdict: DENIED,
        outcome: NONCOMPLIANT,
        decisive: reduced ? null : "deny-deve-prod",
        unmet: "justification-requise",
        score: "40",
      },
    ];
const evaluationColumns: GridColumn[] = [
  { id: "pair", label: reduced ? "Points" : "Paire d'adresses", width: "lg", mono: true },
  { id: "verdict", label: "Verdict", width: "md" },
  { id: "outcome", label: "Issue", width: "md", priority: 3 },
  { id: "decisive", label: "Élément décisif", width: "fill", mono: true },
  { id: "unmet", label: "Conditions non remplies", width: "md", mono: true, priority: 3 },
  { id: "score", label: "Score", width: "xs", align: "end", priority: 3 },
];
interface Version extends Record<string, unknown> {
  version: string;
  at: string;
  change: string;
  author: string;
}
const versions: Version[] = [
  {
    version: "v3",
    at: "2025-12-02",
    change: "Dérogation recertifiée jusqu'au 2026-03-31",
    author: PEOPLE.noa,
  },
  {
    version: "v2",
    at: "2025-10-01",
    change: "Devenu non conforme : règle deny-deve-prod",
    author: "politique r39",
  },
  {
    version: "v1",
    at: "2025-09-30",
    change: "Enregistré, dérogé à l'instruction",
    author: PEOPLE.noa,
  },
];
const versionColumns: GridColumn[] = [
  { id: "version", label: "Version", width: "sm", mono: true },
  { id: "at", label: "Le", width: "md", mono: true },
  { id: "change", label: "Changement", width: "fill", wrap: true },
  { id: "author", label: "Par", width: "md", priority: 2 },
];
const denyText = RULES.find((r) => r.key === "deny-deve-prod")!.text;
const newEnd = ref<string | null>("2026-12-31");
const changesetLink = lien("E23", "decision", route.query);
</script>

<template>
  <Cadre
    :title="absent ? 'Flux introuvable' : `Flux ${SHOWN.key}`"
    active="flows"
    space="flows"
    :spaces="SPACES_017"
    :draft="0"
    :crumbs="[
      { label: 'Flux', to: '/x/flow' },
      { label: 'Registre', to: '/x/flow/flows' },
      { label: absent ? 'flux-9999' : SHOWN.key, mono: true },
    ]"
  >
    <Panneau v-if="absent" title="flux-9999" main>
      <ErrorState
        title="Ce flux est absent ou vous n'avez pas le droit de le lire."
        code="urn:kartenn:problem:not-found"
        :request-id="ID.request"
      >
        Le lien peut viser un flux qui n'existe pas, ou un flux dont aucune extrémité ne vous est
        lisible. Les deux cas se lisent de la même façon.
      </ErrorState>
    </Panneau>
    <Panneau
      v-else
      :title="SHOWN.key"
      :detail="`flux · ${SHOWN.state.label.toLowerCase()} · ${SHOWN.flowType}`"
      main
    >
      <template #actions>
        <Button v-if="ended" variant="primary" icon="redo-2">Nouvelle demande pré-remplie</Button>
        <Button
          v-else
          icon="pencil"
          :shortcut="['e']"
          v-bind="reduced ? { forbidden: 'flow.write' } : {}"
          >Modifier</Button
        >
        <Menu label="Actions" icon="ellipsis" align="end" :items="menu" />
      </template>
      <InlineAlert
        v-if="props.vue === 'prolongation'"
        tone="info"
        title="Prolongation jusqu'au 2026-12-31 en attente d'instruction."
        :items="[
          {
            message:
              'La fiche garde la fin du 2026-06-30 tant que la prolongation n\'est pas approuvée.',
            code: 'flow.created',
          },
        ]"
      >
        Changeset
        <RouterLink :to="changesetLink" class="k-mono">{{
          CHANGESET.extension.slice(0, 10)
        }}</RouterLink
        >, demandé par {{ ME }} le 2026-01-15.
      </InlineAlert>
      <InlineAlert
        v-if="ended"
        tone="info"
        title="Ce flux a pris fin le 2025-12-31 : il ne se prolonge plus."
        :items="[
          {
            message: 'Une nouvelle demande pré-remplie le renouvelle, avec son dossier.',
            code: 'flow.update.ended',
          },
        ]"
      />
      <Split side="end" side-size="md" gap="3">
        <Tabs v-model="tab" :tabs="tabs" label="Sections de la fiche">
          <template #evaluations>
            <Stack gap="3">
              <Cluster as="p" gap="2">
                <span class="k-ecran-muted">Au présent :</span>
                <StatusBadge :state="SHOWN.synthesis.state" :label="SHOWN.synthesis.label" />
                <StatusBadge
                  v-if="!extension && !ended"
                  state="warn"
                  label="Dérogé jusqu'au 2026-03-31"
                />
                <span class="k-ecran-muted k-num">politique r42</span>
              </Cluster>
              <EmptyState v-if="ended" icon="address" title="Aucune adresse à la date">
                srv-rec-04 · eth0 ne porte plus d'adresse : le flux est non conforme faute de paire
                à évaluer.
              </EmptyState>
              <DataGrid
                v-else
                label="Évaluations courantes par paire"
                :columns="evaluationColumns"
                :rows="evaluations"
                row-key="key"
              >
                <template #cell="{ row, column, value }">
                  <StatusBadge
                    v-if="column.id === 'verdict'"
                    :state="row.verdict.state"
                    :label="row.verdict.label"
                  />
                  <StatusBadge
                    v-else-if="column.id === 'outcome'"
                    :state="row.outcome.state"
                    :label="row.outcome.label"
                  />
                  <span v-else-if="column.id === 'decisive' && !value" class="k-ecran-muted"
                    >non communiqué</span
                  >
                  <template v-else>{{ value }}</template>
                </template>
              </DataGrid>
              <p v-if="reduced" class="k-ecran-muted">
                Explication réduite : vous lisez ce flux comme demandeur (flow.request), sans
                flow.read ; les adresses et l'élément décisif ne sont pas communiqués.
              </p>
              <Stack
                v-else-if="!extension && !ended"
                as="section"
                gap="1"
                aria-labelledby="k-e22-why"
              >
                <h3 id="k-e22-why" class="k-ecran-subtitle">Pourquoi non conforme</h3>
                <CodeBadge code="deny-deve-prod" label="Développement vers production" />
                <p>{{ denyText }}</p>
              </Stack>
              <Attributs
                :items="[
                  {
                    key: 'registered',
                    label: 'À l\'enregistrement',
                    value: extension
                      ? '2025-07-01 : conforme, automatique'
                      : ended
                        ? '2025-01-06 : sous réserve, instruit'
                        : '2025-09-30 : non conforme, dérogé à l\'instruction',
                  },
                ]"
              />
            </Stack>
          </template>
          <template #history>
            <DataGrid
              label="Versions du flux"
              :columns="versionColumns"
              :rows="versions"
              row-key="version"
            />
          </template>
        </Tabs>
        <template #side>
          <Stack gap="3">
            <Panneau title="Flux">
              <Attributs
                :items="[
                  { key: 'source', label: 'Source', value: SHOWN.source, mono: true },
                  {
                    key: 'destination',
                    label: 'Destination',
                    value: SHOWN.destination,
                    mono: true,
                  },
                  { key: 'service', label: 'Service' },
                  { key: 'state', label: 'État' },
                  { key: 'period', label: 'Période', mono: true },
                ]"
              >
                <!-- A date never breaks; the period wraps at its arrow. -->
                <template #period>
                  <template v-for="(d, i) in SHOWN.period.split(' → ')" :key="d"
                    ><template v-if="i"> → </template
                    ><span class="k-e22-date">{{ d }}</span></template
                  >
                </template>
                <template #service
                  ><KeyLabel :value="SHOWN.service" :label="SHOWN.serviceLabel" show-key
                /></template>
                <template #state
                  ><StatusBadge :state="SHOWN.state.state" :label="SHOWN.state.label"
                /></template>
              </Attributs>
            </Panneau>
            <Panneau title="Dossier">
              <Attributs
                :items="[
                  { key: 'entity', label: 'Entité', value: ENTITIES.nord },
                  { key: 'requester', label: 'Demandeur', value: SHOWN.requester },
                  { key: 'reference', label: 'Référence', value: SHOWN.reference, mono: true },
                  { key: 'justification', label: 'Justification', value: SHOWN.justification },
                  { key: 'owners', label: 'Responsables', value: PEOPLE.lou },
                  { key: 'nature', label: 'Nature des données', value: 'Interne' },
                ]"
              />
            </Panneau>
            <Panneau v-if="!extension && !ended" title="Dérogation" detail="en cours">
              <Attributs
                :items="[
                  { key: 'end', label: 'Fin', value: '2026-03-31', mono: true },
                  {
                    key: 'reason',
                    label: 'Motif',
                    value: 'Migration des jeux de test, fin prévue en mars',
                  },
                  {
                    key: 'risk',
                    label: 'Risque accepté',
                    value: 'Données anonymisées, accès en lecture',
                  },
                  { key: 'by', label: 'Accordée par', value: PEOPLE.noa },
                ]"
              />
            </Panneau>
          </Stack>
        </template>
      </Split>
    </Panneau>

    <Dialog
      v-if="props.vue === 'prolonger'"
      :open="true"
      title="Prolonger flux-1043"
      description="Une prolongation repasse par l'instruction quand la portion ajoutée n'est pas conforme sans réserve ; rien ne se prolonge seul."
    >
      <Stack gap="3">
        <DateTimeField
          v-model="newEnd"
          label="Nouvelle fin"
          granularity="day"
          required
          hint="Fin actuelle : 2026-06-30. Au plus 12 mois après la décision."
        />
        <Stack as="section" gap="2" aria-labelledby="k-e22-portion">
          <h3 id="k-e22-portion" class="k-ecran-subtitle">
            Pré-vérification de la portion ajoutée
          </h3>
          <Cluster as="p" gap="2">
            <span class="k-mono">2026-06-30 → 2026-12-31</span>
            <StatusBadge state="warn" label="Sous réserve" />
            <StatusBadge state="review" label="À instruire" />
            <CodeBadge code="admin-par-bastion" label="Administration par le bastion" />
          </Cluster>
          <p>
            Dès le 2026-07-01, la règle admin-par-bastion demande le motif
            <code>bastion-admin</code> : la session part du bastion, nominative et enregistrée.
          </p>
          <p class="k-ecran-muted">Vérifiée avec le dossier du flux ; rien n'est écrit.</p>
        </Stack>
      </Stack>
      <template #actions>
        <Button variant="ghost">Annuler</Button>
        <Button variant="primary" icon="send">Demander la prolongation</Button>
      </template>
    </Dialog>
  </Cadre>
</template>

<style>
@layer components {
  .k-e22-date {
    white-space: nowrap;
  }
}
</style>
