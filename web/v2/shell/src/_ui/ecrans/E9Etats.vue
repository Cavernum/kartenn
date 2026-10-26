<script setup lang="ts">
// E9 — États (design « Maquettes »; skill kartenn-ui § 1.3; 021, « Édition en place »;
// 024/M12, M16): every data zone has its three states (empty with a sentence and an action,
// loading in animated skeletons in the cells of the table, error in plain language with its
// code and Request-Id next to it), and the outcomes of a quick edit: 412 with a clear
// message and the input kept, 202 « proposé, en revue » with its reason. An HTTP problem
// shows the `type` the API serves (RFC 9457, `urn:kartenn:problem:<slug>`, 006 design
// « Erreurs »); a violation or a reason shows its rule code as served.
import {
  Button,
  CodeBadge,
  DataGrid,
  Dialog,
  EmptyState,
  ErrorState,
  InlineAlert,
  KeyLabel,
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
  { id: "value", label: "Valeur", width: "md", mono: true, sortable: true },
  { id: "usage", label: "Usage", width: "sm", filterable: true },
  { id: "owner", label: "Propriétaire", width: "fill", sortable: true, filterable: true },
  { id: "site", label: "Site", width: "md", mono: true, filterable: true },
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
    <template v-if="props.vue === 'zones'">
      <Panneau title="Adresses" detail="chargement" main flush>
        <DataGrid label="Adresses" :columns="columns" :rows="[]" row-key="value" loading />
      </Panneau>
      <Panneau title="Préfixes" detail="Bureaux">
        <EmptyState title="Aucun préfixe racine" icon="prefix">
          Le domaine « bureaux » n'a aucun préfixe au présent. Créez-en un ou lancez un import.
          <template #action>
            <Button variant="primary" icon="plus" :shortcut="['c']">Créer un préfixe</Button>
          </template>
        </EmptyState>
      </Panneau>
      <Panneau title="Anomalies ouvertes">
        <ErrorState
          title="Les anomalies n'ont pas pu être lues : le service est momentanément indisponible."
          code="urn:kartenn:problem:unavailable"
          :request-id="ID.request"
          retry-label="Réessayer"
        >
          Le reste de l'écran reste utilisable. Si l'erreur persiste, transmettez ces deux codes.
        </ErrorState>
      </Panneau>
    </template>

    <Panneau v-else title="Adresses" detail="1 284" main flush>
      <template v-if="props.vue === 'revue'" #toolbar>
        <AVenir component="Toast" task="22">
          <InlineAlert
            tone="info"
            title="Proposé, en revue : votre modification de 192.0.2.12 attend une validation."
            :items="[
              {
                message:
                  'Vous n\'avez pas le droit d\'appliquer vous-même les adresses (core.address.approval.self).',
                code: 'changeset.approval.self.missing',
              },
            ]"
          >
            Changeset <CodeBadge :code="ID.changeset" /> ; la cellule garde la valeur lue d'ici là.
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
      title="L'objet a changé depuis votre lecture"
      description="Quelqu'un a modifié 192.0.2.12 après que vous l'avez lue : rien n'a été écrit, votre saisie est gardée."
    >
      <Stack gap="3">
        <InlineAlert
          tone="ko"
          title="Vous avez lu la version v4, la version actuelle est la v5."
          :items="[
            {
              message: 'Relisez la valeur actuelle, puis réappliquez ou abandonnez votre saisie.',
              code: 'urn:kartenn:problem:precondition-failed',
            },
          ]"
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
        <!-- The safe way out: a visible secondary button, not a bare text beside the primary. -->
        <Button variant="secondary">Garder la valeur actuelle</Button>
        <Button variant="primary" icon="redo-2">Réappliquer ma saisie</Button>
      </template>
    </Dialog>
  </Cadre>
</template>
