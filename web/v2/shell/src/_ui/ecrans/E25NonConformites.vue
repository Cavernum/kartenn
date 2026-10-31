<script setup lang="ts">
// E25 — Non-conformités (spec 017, task 1; view `nonconformities` of the plugin `flow`,
// REQ-017-24; 011, REQ-011-25; U6): the nonconformities (`GET /v1/flow-nonconformities`,
// cursor, task 021-29) filtered by entity, requester, rule, period, state and kind; each
// row with its state (rejected, noncompliant, waived), date, motive, the rule in cause by
// its code and label, and its pedagogical text in the language of the user — else the
// French one, the language said; a link to the flow or to the rejected changeset. A
// noncompliant flow is acknowledged through its anomaly (`flow.verdict.denied`). « Mes
// non-conformités » is the same list on `requester` = the user.
import {
  Button,
  Dialog,
  FilterPicker,
  Stack,
  StatusBadge,
  TextField,
  type Filter,
  type FilterModel,
  type GridColumn,
  type GridSort,
  type State,
  DataGrid,
  CodeBadge,
} from "@kartenn/ui";
import { ref } from "vue";
import Cadre from "./EcranCadre.vue";
import Panneau from "./EcranPanneau.vue";
import { RULES } from "./donnees.ts";
import { CHANGESET, ENTITIES, ME, PEOPLE, SPACES_017 } from "./donnees017.ts";

const props = defineProps<{ vue: string }>();
const mine = props.vue === "miennes";

type NcState = "rejected" | "noncompliant" | "waived";
const STATE: Record<NcState, { state: State; label: string }> = {
  rejected: { state: "ko", label: "Rejetée" },
  noncompliant: { state: "ko", label: "Non conforme" },
  waived: { state: "warn", label: "Dérogée" },
};
const text = (key: string) => RULES.find((r) => r.key === key)!.text;
interface Nc extends Record<string, unknown> {
  key: string;
  state: NcState;
  date: string;
  subject: string;
  requester: string;
  rule: string;
  ruleLabel: string;
  text: string;
  lang: "fr" | "en";
  motive: string;
}
const all: Nc[] = [
  {
    key: "n1",
    state: "rejected",
    date: "2026-01-14",
    subject: CHANGESET.request.slice(0, 10),
    requester: ME,
    rule: "deny-deve-prod",
    ruleLabel: "Développement vers production",
    text: text("deny-deve-prod"),
    lang: "fr",
    motive: "La recette dispose de ses propres jeux : passez par l'export anonymisé.",
  },
  {
    key: "n2",
    state: "noncompliant",
    date: "2025-12-02",
    subject: "flux-1029",
    requester: PEOPLE.alex,
    rule: "deny-admin-hors-bastion",
    ruleLabel: "Administration hors bastion",
    text: text("deny-admin-hors-bastion"),
    lang: "fr",
    motive: "Devenu non conforme : politique r39",
  },
  {
    key: "n3",
    state: "waived",
    date: "2025-09-30",
    subject: "flux-1037",
    requester: ME,
    rule: "deny-deve-prod",
    ruleLabel: "Développement vers production",
    text: text("deny-deve-prod"),
    lang: "fr",
    motive: "Dérogé jusqu'au 2026-03-31",
  },
  {
    key: "n4",
    state: "noncompliant",
    date: "2026-01-08",
    subject: "flux-1110",
    requester: PEOPLE.lou,
    rule: "deny-partenaire-sortant",
    ruleLabel: "",
    text: "Outbound flows to partner networks go through the egress relay.",
    lang: "en",
    motive: "Devenu non conforme : politique r41",
  },
];
const rows = mine ? all.filter((r) => r.requester === ME) : all;
const columns: GridColumn[] = [
  { id: "state", label: "État", width: "md", sortable: true, filterable: true },
  { id: "date", label: "Date", width: "sm", mono: true, sortable: true, priority: 2 },
  { id: "subject", label: "Flux ou changeset", width: "sm", mono: true },
  { id: "rule", label: "Règle en cause", width: "md", wrap: true, filterable: true },
  { id: "text", label: "Texte de la règle, motif", width: "fill", wrap: true },
  { id: "actions", label: "Action", width: "md", actions: true },
];
const sort = ref<GridSort | undefined>({ column: "date", direction: "descending" });
const FILTERS: FilterModel = [
  {
    key: "entity",
    label: "Entité",
    type: "enum",
    operators: ["eq"],
    options: Object.entries(ENTITIES).map(([value, label]) => ({ value, label })),
  },
  {
    key: "requester",
    label: "Demandeur",
    type: "enum",
    operators: ["eq"],
    options: [ME, ...Object.values(PEOPLE)].map((p) => ({ value: p, label: p })),
  },
  {
    key: "rule",
    label: "Règle",
    type: "enum",
    operators: ["eq"],
    options: RULES.map((r) => ({ value: r.key, label: r.key })),
  },
  { key: "period", label: "Période", type: "date", operators: ["between", "after", "before"] },
  {
    key: "state",
    label: "État",
    type: "enum",
    operators: ["eq"],
    options: Object.entries(STATE).map(([value, s]) => ({ value, label: s.label })),
  },
  {
    key: "kind",
    label: "Sorte",
    type: "enum",
    operators: ["eq"],
    options: [
      { value: "request", label: "Demande" },
      { value: "extension", label: "Prolongation" },
      { value: "flow", label: "Flux" },
    ],
  },
];
const filters = ref<Filter[]>(
  mine
    ? [{ key: "requester", operator: "eq", values: [ME] }]
    : [{ key: "state", operator: "eq", values: ["rejected", "noncompliant", "waived"] }],
);
const comment = ref("Bastion en cours de déploiement sur ce site, flux retiré le 2026-02-01.");
</script>

<template>
  <Cadre
    :title="mine ? 'Mes non-conformités' : 'Non-conformités'"
    active="nonconformities"
    space="flows"
    :spaces="SPACES_017"
    :draft="0"
    :crumbs="[
      { label: 'Flux', to: '/x/flow' },
      { label: mine ? 'Mes non-conformités' : 'Non-conformités' },
    ]"
  >
    <Panneau
      :title="mine ? 'Mes non-conformités' : 'Non-conformités'"
      :detail="mine ? `demandeur ${ME}` : 'au présent'"
      main
      flush
    >
      <template #toolbar>
        <FilterPicker
          v-model="filters"
          :model="FILTERS"
          label="Filtres des non-conformités"
          :shortcut="['/']"
        />
      </template>
      <DataGrid
        v-model:sort="sort"
        label="Non-conformités des flux"
        :columns="columns"
        :rows="rows"
        row-key="key"
        :filtered="mine ? [] : ['state']"
      >
        <template #cell="{ row, column, value }">
          <StatusBadge
            v-if="column.id === 'state'"
            :state="STATE[row.state].state"
            :label="STATE[row.state].label"
          />
          <a
            v-else-if="column.id === 'subject'"
            :href="
              row.state === 'rejected'
                ? `/changesets/${CHANGESET.request}`
                : `/x/flow/flows/${row.subject}`
            "
            >{{ row.subject }}</a
          >
          <span v-else-if="column.id === 'rule'" class="k-e25-rule">
            <CodeBadge :code="row.rule" />
            <span v-if="row.ruleLabel">{{ row.ruleLabel }}</span>
          </span>
          <span v-else-if="column.id === 'text'" class="k-e25-text">
            <span :lang="row.lang"
              >{{ row.text }}
              <span v-if="row.lang !== 'fr'" class="k-ecran-muted" lang="fr"
                >(texte en anglais : aucun texte français)</span
              ></span
            >
            <span class="k-ecran-muted">Motif : {{ row.motive }}</span>
          </span>
          <Button
            v-else-if="column.id === 'actions' && row.state === 'noncompliant' && !mine"
            size="sm"
            icon="check"
            >Acquitter…</Button
          >
          <span v-else-if="column.id === 'actions'" class="k-ecran-muted">—</span>
          <template v-else>{{ value }}</template>
        </template>
      </DataGrid>
      <template #footer>
        <span class="k-num">{{ rows.length }} non-conformités</span>
        <Button size="sm" icon="chevron-right">Suivantes</Button>
      </template>
    </Panneau>

    <Dialog
      v-if="props.vue === 'acquitter'"
      :open="true"
      title="Acquitter la non-conformité de flux-1029"
      description="L'anomalie flow.verdict.denied du flux quitte les ouvertes ; le flux reste non conforme et l'acquittement reste dans son historique."
    >
      <Stack gap="3">
        <TextField v-model="comment" label="Commentaire" hint="Ce qui est prévu pour ce flux." />
      </Stack>
      <template #actions>
        <Button variant="ghost">Annuler</Button>
        <Button variant="primary" icon="check">Acquitter</Button>
      </template>
    </Dialog>
  </Cadre>
</template>

<style>
@layer components {
  /* The text of the rule, then the motive on a line of its own. */
  .k-e25-text {
    display: grid;
    gap: var(--k-space-1);
  }

  .k-e25-rule {
    display: inline-flex;
    flex-wrap: wrap;
    gap: var(--k-space-1) var(--k-space-2);
    align-items: center;
    min-inline-size: 0;
  }
}
</style>
