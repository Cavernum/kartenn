<script setup lang="ts">
// E9 — États (design « Maquettes »; skill kartenn-ui § 1.3; 021, « Édition en place »):
// every data zone has its three states (empty with a sentence and an action, loading in
// fixed-size skeletons, error with its copyable code and Request-Id), and the outcomes of a
// quick edit: 412 with the input kept, 202 « proposé, en revue ».
import {
  Button,
  CodeBadge,
  DataGrid,
  Dialog,
  EmptyState,
  ErrorState,
  InlineAlert,
  KeyLabel,
  Skeleton,
  Split,
  Stack,
  TextField,
  type CellState,
  type GridColumn,
} from "@kartenn/ui";
import { ref } from "vue";
import AVenir from "./AVenir.vue";
import Attributs from "./EcranAttributs.vue";
import Cadre from "./EcranCadre.vue";
import Panneau from "./EcranPanneau.vue";
import { ID, usageLabel } from "./donnees.ts";

const props = defineProps<{ vue: string }>();
const kept = ref("Atelier Nord");

interface Row extends Record<string, unknown> {
  value: string;
  usage: string;
  owner: string;
  site: string;
}
const rows: Row[] = [
  { value: "192.0.2.10", usage: "host", owner: "Atelier Nord", site: "site-ouest" },
  { value: "192.0.2.11", usage: "vip", owner: "Atelier Nord", site: "site-ouest" },
  {
    value: "192.0.2.12",
    usage: "host",
    owner: props.vue === "conflit" ? "Atelier Sud" : "Service Pilote",
    site: "site-ouest",
  },
  { value: "192.0.2.13", usage: "host", owner: "Service Pilote", site: "site-ouest" },
];
const columns: GridColumn[] = [
  { id: "value", label: "Valeur", width: "md", mono: true },
  { id: "usage", label: "Usage", width: "sm" },
  { id: "owner", label: "Propriétaire", width: "fill" },
  { id: "site", label: "Site", width: "md", mono: true },
];
const cellState = (row: Row, column: GridColumn): CellState | undefined => {
  if (row.value !== "192.0.2.12" || column.id !== "owner") return undefined;
  return props.vue === "revue" ? "review" : undefined;
};
</script>

<template>
  <Cadre
    :title="props.vue === 'zones' ? 'Domaine Bureaux' : 'Adresses'"
    :active="props.vue === 'zones' ? 'home' : 'addresses'"
    space="referential"
    :crumbs="
      props.vue === 'zones'
        ? [{ label: 'Référentiel', to: '/' }, { label: 'Bureaux' }]
        : [{ label: 'Référentiel', to: '/' }, { label: 'Adresses' }]
    "
  >
    <Split v-if="props.vue === 'zones'" gap="3">
      <template #side>
        <Panneau title="Préfixes" detail="Bureaux">
          <EmptyState title="Aucun préfixe racine" icon="prefix">
            Le domaine « bureaux » n'a aucun préfixe au présent. Créez-en un ou lancez un import.
            <template #action>
              <Button variant="primary" icon="plus" :shortcut="['c']">Créer un préfixe</Button>
            </template>
          </EmptyState>
        </Panneau>
      </template>
      <Stack gap="3">
        <Panneau title="Adresses" main>
          <Skeleton variant="rows" :rows="6" :columns="4" />
        </Panneau>
        <Panneau title="Anomalies ouvertes">
          <ErrorState
            title="Les anomalies n'ont pas pu être lues."
            code="urn:kartenn:problem:unavailable"
            :request-id="ID.request"
            retry-label="Réessayer"
          >
            Le service a répondu 503. Le reste de l'écran reste utilisable.
          </ErrorState>
        </Panneau>
      </Stack>
    </Split>

    <Panneau v-else title="Adresses" detail="1 284" main flush>
      <template v-if="props.vue === 'revue'" #toolbar>
        <AVenir component="Toast" task="22">
          <InlineAlert tone="info" title="Proposé, en revue">
            Votre modification de <code>192.0.2.12</code> attend une validation : changeset
            <CodeBadge :code="ID.changeset" />. La cellule garde la valeur lue d'ici là.
          </InlineAlert>
        </AVenir>
      </template>
      <DataGrid
        label="Adresses"
        :columns="columns"
        :rows="rows"
        row-key="value"
        :cell-state="cellState"
        :row-state="
          (row) => (props.vue === 'revue' && row.value === '192.0.2.12' ? 'review' : undefined)
        "
      >
        <template #cell="{ row, column, value }">
          <KeyLabel
            v-if="column.id === 'usage'"
            :value="row.usage"
            :label="usageLabel(row.usage)"
          />
          <template v-else>{{ value }}</template>
        </template>
      </DataGrid>
    </Panneau>

    <Dialog
      v-if="props.vue === 'conflit'"
      :open="true"
      title="192.0.2.12 a changé entre-temps"
      description="Quelqu'un l'a modifiée depuis votre lecture : rien n'a été écrit."
    >
      <Stack gap="3">
        <InlineAlert
          tone="warn"
          title="Version lue v4, version actuelle v5"
          :items="[{ message: 'Condition If-Match non remplie', code: 'precondition-failed' }]"
        />
        <Attributs
          :items="[
            { key: 'current', label: 'Valeur actuelle', value: 'Atelier Sud' },
            { key: 'by', label: 'Modifiée par', value: 'Lou Bernard, 10:02' },
          ]"
        />
        <TextField v-model="kept" label="Votre saisie, gardée" hint="Propriétaire de 192.0.2.12" />
      </Stack>
      <template #actions>
        <Button variant="ghost">Garder la valeur actuelle</Button>
        <Button variant="primary" icon="redo-2">Réappliquer ma saisie</Button>
      </template>
    </Dialog>
  </Cadre>
</template>
