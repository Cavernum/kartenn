<script setup lang="ts">
// E21 — Proposer des flux (spec 017, task 1; view `request` and action `request-from` of
// the plugin `flow`, REQ-017-15, 16; Q-017-2), in the shape E11 validated: the proposal is
// a table in the draft of the shell, one row per flow (source, destination, object,
// reference, justification, verdict). Here what the 017 adds: the file of the request
// (entity, on behalf of, owners, nature of the data, personal data, start and end) in the
// right panel, shared by the rows; the pre-check of the chosen row in the bottom panel, by
// the query of conformity with that file, without writing; the refusals of the server
// placed on their cell or field; the outcome of the submission — held for review (202:
// the outcome shown is the pre-check's, said so, with the decisive element and its text)
// or applied (201: the active flows and their cards). Prefilled from a flow that ended
// (`request?from=flux-0988`), as the cards of addresses and prefixes do from their object.
import {
  Button,
  Checkbox,
  CodeBadge,
  Cluster,
  DataGrid,
  IconButton,
  InlineAlert,
  KeyLabel,
  Select,
  Stack,
  StatusBadge,
  TextField,
  widthQuery,
  type CellState,
  type GridColumn,
  type ListOption,
  type TabItem,
} from "@kartenn/ui";
import { ref } from "vue";
import { RouterLink, useRoute } from "vue-router";
import AVenir from "./AVenir.vue";
import Attributs from "./EcranAttributs.vue";
import Cadre from "./EcranCadre.vue";
import Panneau from "./EcranPanneau.vue";
import { RULES, SERVICES } from "./donnees.ts";
import { CHANGESET, ENTITIES, PATTERNS, PEOPLE, SPACES_017, lien } from "./donnees017.ts";

const props = defineProps<{ vue: string }>();
const route = useRoute();
const refused = props.vue === "refus";
const sent = props.vue === "instruction" || props.vue === "appliquee";
const applied = props.vue === "appliquee";

type Verdict = "conforme" | "reserve" | "non-conforme";
const VERDICT: Record<Verdict, { state: "ok" | "warn" | "ko"; label: string }> = {
  conforme: { state: "ok", label: "Conforme" },
  reserve: { state: "warn", label: "Sous réserve" },
  "non-conforme": { state: "ko", label: "Non conforme" },
};
interface Row extends Record<string, unknown> {
  key: string;
  source: string;
  destination: string;
  service: string;
  reference: string;
  justification: string;
  verdict: Verdict;
  decisive: string;
  flow?: string;
}
const rows: Row[] = applied
  ? [
      {
        key: "l1",
        source: "srv-web-02 · eth0",
        destination: "srv-app-01 · eth0",
        service: "https",
        reference: "ticketing:CHG-1071",
        justification: "Nouveau frontal",
        verdict: "conforme",
        decisive: "app-meme-env",
        flow: "flux-1071",
      },
      {
        key: "l2",
        source: "srv-app-01 · eth0",
        destination: "192.0.2.30",
        service: "postgres",
        reference: "ticketing:CHG-1071",
        justification: "Base du frontal",
        verdict: "conforme",
        decisive: "bdd-meme-env",
        flow: "flux-1072",
      },
    ]
  : [
      {
        key: "l1",
        source: "srv-rec-04 · eth0",
        destination: "srv-app-01 · eth0",
        service: "https",
        reference: refused ? "" : "ticketing:CHG-1080",
        justification: "Jeux de contrôle de la recette, reconduits",
        verdict: "reserve",
        decisive: "rece-vers-prod",
      },
      ...(refused
        ? [
            {
              key: "l2",
              source: "192.0.2.100–192.0.2.120",
              destination: "192.0.2.30",
              service: "postgres",
              reference: "ticketing:CHG-1080",
              justification: "Plage de supervision",
              verdict: "non-conforme" as Verdict,
              decisive: "flow.endpoint.invalid",
            },
          ]
        : []),
    ];
const columns: GridColumn[] = [
  { id: "source", label: "Source", width: "md", mono: true, wrap: true },
  { id: "destination", label: "Destination", width: "md", mono: true, wrap: true },
  { id: "service", label: "Service", width: "md" },
  // Its refusal stays in sight (refus) ; else it goes first when the grid narrows.
  { id: "reference", label: "Référence", width: "md", mono: true, priority: refused ? 1 : 3 },
  { id: "justification", label: "Justification", width: "lg", wrap: true, priority: 3 },
  { id: "verdict", label: sent ? "Résultat" : "Verdict", width: "fill" },
];
const cellState = (row: Row, column: GridColumn): CellState | undefined => {
  if (!refused) return undefined;
  if (row.key === "l1" && column.id === "reference")
    return { error: "Au moins une référence externe", code: "flow.reference.missing" };
  if (row.key === "l2" && column.id === "source")
    return {
      error: "Une plage n'est pas une extrémité : une adresse, un préfixe ou une interface",
      code: "flow.endpoint.invalid",
    };
  return undefined;
};
const chosen = ref<string[]>(["l1"]);
/**
 * One sheet at a time in the compact class: the file (right panel) is open, the pre-check
 * of the row (bottom panel) comes when the row is chosen again.
 */
const compact = typeof window !== "undefined" && window.matchMedia(widthQuery("compact")).matches;
const bottomOpen = ref(!compact);
const bottom: TabItem[] = [{ value: "precheck", label: "Pré-vérification" }];
const rule = RULES.find((r) => r.key === "rece-vers-prod")!;
const pattern = PATTERNS.find((p) => p.key === "api-lecture")!;

// ——— The file of the request, shared by its rows ———

const ENTITY_CHOICES: ListOption[] = Object.entries(ENTITIES).map(([value, label]) => ({
  value,
  label,
}));
const entity = ref<string | undefined>(refused ? "sud" : "nord");
const onBehalf = ref("");
const NATURE: ListOption[] = [
  { value: "public", label: "Publique", detail: "public" },
  { value: "interne", label: "Interne", detail: "interne" },
  { value: "restreint", label: "Restreinte", detail: "restreint" },
];
const nature = ref<string | undefined>("interne");
const personal = ref(false);
const SYSTEMS: ListOption[] = [
  { value: "ticketing", label: "Gestion des demandes", detail: "ticketing" },
  { value: "cmdb", label: "Inventaire", detail: "cmdb" },
];
const system = ref<string | undefined>("ticketing");
const registerMine = lien("E11", "registre", route.query);
</script>

<template>
  <Cadre
    v-model:bottom-open="bottomOpen"
    title="Proposer des flux"
    active="flows"
    space="flows"
    :spaces="SPACES_017"
    :draft="sent ? 0 : rows.length"
    :panel="sent ? undefined : 'detail'"
    :bottom="sent ? undefined : bottom"
    bottom-label="Flux choisi"
    :crumbs="[
      { label: 'Flux', to: '/x/flow' },
      { label: 'Registre', to: '/x/flow/flows' },
      { label: sent ? 'Proposition envoyée' : 'Proposer des flux' },
    ]"
  >
    <Stack gap="3">
      <InlineAlert
        v-if="props.vue === 'instruction'"
        tone="info"
        title="Proposition envoyée : elle part à l'instruction."
        :items="[
          {
            message:
              'Un flux est sous réserve : la politique « Instruction des flux » retient la proposition entière, un instructeur en décide.',
            code: 'flow.created',
          },
        ]"
      >
        Changeset <CodeBadge :code="CHANGESET.request" /> en attente ; le flux figure dans
        <RouterLink :to="registerMine">Mes demandes</RouterLink>, état « Demandé ».
      </InlineAlert>
      <InlineAlert
        v-else-if="applied"
        tone="ok"
        title="Proposition enregistrée : les deux flux sont actifs."
      >
        Conformes à la matrice de flux, enregistrés sans instruction.
      </InlineAlert>
      <InlineAlert
        v-else-if="refused"
        tone="ko"
        title="La proposition n'a pas été envoyée : le serveur refuse 3 champs."
        :items="[
          { message: 'Ligne 1 : au moins une référence externe.', code: 'flow.reference.missing' },
          {
            message: 'Ligne 2 : une plage n\'est pas une extrémité.',
            code: 'flow.endpoint.invalid',
          },
          {
            message: 'Entité Atelier Sud : il faut prop.core.owner.write sur elle.',
            code: 'core.object.forbidden',
          },
        ]"
      >
        Votre saisie est gardée ; chaque refus est sur sa cellule ou son champ.
      </InlineAlert>
      <Panneau
        :title="sent ? 'Flux proposés' : 'Proposer des flux'"
        :detail="
          props.vue === 'dossier' ? 'pré-remplie depuis flux-0988, échu' : 'une ligne par flux'
        "
        main
        flush
      >
        <template v-if="!sent" #actions>
          <Button icon="plus" :shortcut="['n']">Ajouter un flux</Button>
        </template>
        <DataGrid
          v-model:selected="chosen"
          label="Flux proposés"
          :columns="columns"
          :rows="rows"
          row-key="key"
          :selectable="sent ? false : 'single'"
          :row-state="() => (sent ? (applied ? undefined : 'review') : 'draft')"
          :cell-state="cellState"
        >
          <template #cell="{ row, column, value }">
            <KeyLabel
              v-if="column.id === 'service'"
              :value="row.service"
              :label="SERVICES[row.service]?.label ?? null"
              show-key
            />
            <span v-else-if="column.id === 'verdict'" class="k-e21-verdict">
              <template v-if="applied">
                <StatusBadge state="ok" label="Actif" />
                <RouterLink :to="lien('E22', 'fiche', route.query)" class="k-mono">{{
                  row.flow
                }}</RouterLink>
              </template>
              <template v-else-if="sent">
                <StatusBadge state="review" label="Demandé" />
                <span class="k-ecran-muted">pré-vérification :</span>
                <StatusBadge
                  :state="VERDICT[row.verdict].state"
                  :label="VERDICT[row.verdict].label"
                />
                <CodeBadge :code="row.decisive" />
              </template>
              <template v-else>
                <StatusBadge
                  :state="VERDICT[row.verdict].state"
                  :label="VERDICT[row.verdict].label"
                />
                <CodeBadge :code="row.decisive" />
              </template>
            </span>
            <span v-else-if="!value" class="k-ecran-muted">—</span>
            <template v-else>{{ value }}</template>
          </template>
        </DataGrid>
        <template #footer>
          <span v-if="sent" class="k-num"
            >{{ rows.length }} flux · changeset {{ applied ? "appliqué" : "en attente" }}</span
          >
          <span v-else class="k-num"
            >{{ rows.length }} flux ·
            {{ rows.length === 1 ? "1 sous réserve" : "1 sous réserve · 1 refusé" }}</span
          >
          <span v-if="!sent" class="k-ecran-muted"
            >Le dossier, à droite, vaut pour toutes les lignes</span
          >
        </template>
      </Panneau>
      <Panneau
        v-if="props.vue === 'instruction'"
        title="Ce qui attend l'instructeur"
        detail="issue de la pré-vérification"
      >
        <p class="k-e21-verdict">
          <StatusBadge state="review" label="À instruire" />
          <span>La cellule Recette → Production autorise sous le motif</span>
          <CodeBadge code="api-lecture" />
        </p>
        <p>{{ pattern.text }}</p>
        <p class="k-ecran-muted">
          L'issue affichée est celle de la pré-vérification : le triage du serveur fait foi et
          remplace cette ligne s'il diffère.
        </p>
      </Panneau>
    </Stack>

    <template #bottom-precheck>
      <Stack gap="2">
        <p class="k-e21-verdict">
          <span class="k-mono">srv-rec-04 · eth0 → srv-app-01 · eth0</span>
          <span class="k-ecran-muted">· https · Applicatif</span>
        </p>
        <p class="k-e21-verdict">
          <StatusBadge state="warn" label="Sous réserve" />
          <StatusBadge state="review" label="À instruire" />
          <CodeBadge code="rece-vers-prod" label="Recette vers production" />
          <span class="k-ecran-muted k-num"
            >2 paires d'adresses · 0 conforme · 2 sous réserve · politique r42</span
          >
        </p>
        <Attributs
          :items="[
            { key: 'rule', label: 'Règle', value: rule.text },
            { key: 'pattern', label: 'Motif attendu' },
            { key: 'conditions', label: 'Conditions', value: 'justification-requise : remplie' },
          ]"
          columns
        >
          <template #pattern
            ><KeyLabel value="api-lecture" label="Lecture d'API" show-key
          /></template>
        </Attributs>
        <p class="k-ecran-muted">
          Pré-vérification de la ligne avec le dossier : aucun changeset n'est créé.
        </p>
      </Stack>
    </template>

    <template #detail>
      <Stack gap="3">
        <h2 class="k-ecran-subtitle">Dossier de la proposition</h2>
        <Select
          v-model="entity"
          label="Entité"
          :options="ENTITY_CHOICES"
          required
          :error="
            refused
              ? 'Il faut prop.core.owner.write sur Atelier Sud (core.object.forbidden)'
              : undefined
          "
          hint="Parmi les entités lisibles, ou par identifiant collé."
        />
        <TextField
          v-model="onBehalf"
          label="Pour le compte de"
          icon="search"
          placeholder="Une identité"
          hint="Facultatif : le demandeur reste vous."
        />
        <Stack gap="1">
          <span class="k-e21-label">Responsables</span>
          <Cluster gap="2">
            <span class="k-e21-chip"
              >{{ PEOPLE.lou }} <IconButton icon="x" size="sm" :label="`Retirer ${PEOPLE.lou}`"
            /></span>
            <Button size="sm" variant="ghost" icon="plus">Ajouter</Button>
          </Cluster>
        </Stack>
        <Select
          v-model="system"
          label="Système des références"
          :options="SYSTEMS"
          hint="Liste core.external_system ; chaque ligne porte sa référence."
        />
        <Select
          v-model="nature"
          label="Nature des données"
          :options="NATURE"
          hint="Valeurs de l'axe Sensibilité."
        />
        <div class="k-e21-check">
          <Checkbox v-model="personal" label="Données personnelles" />
        </div>
        <AVenir component="DateTimeField" task="020-20">
          <Cluster gap="2" :wrap="false" align="start" class="k-e21-dates">
            <TextField label="Début" model-value="au présent" />
            <TextField label="Fin" model-value="2026-12-31" mono hint="Facultative." />
          </Cluster>
        </AVenir>
      </Stack>
    </template>
  </Cadre>
</template>

<style>
@layer components {
  .k-e21-verdict {
    display: flex;
    flex-wrap: wrap;
    gap: var(--k-space-2);
    align-items: center;
    min-inline-size: 0;
  }

  .k-e21-label {
    font-size: var(--k-text-sm);
    line-height: var(--k-text-sm-line);
    font-weight: var(--k-weight-medium);
  }

  .k-e21-chip {
    display: inline-flex;
    gap: var(--k-space-1);
    align-items: center;
    padding-inline-start: var(--k-space-2);
    border: var(--k-border-width) solid var(--k-border-strong);
    border-radius: var(--k-radius-md);
  }

  /* Off the edge of the right panel: its resizing handle is a target too (2.5.8). */
  .k-e21-check {
    padding-inline-start: var(--k-space-2);
  }

  .k-e21-dates > * {
    flex: 1 1 0;
    min-inline-size: 0;
  }
}
</style>
