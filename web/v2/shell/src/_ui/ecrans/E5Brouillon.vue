<script setup lang="ts">
// E5 — Brouillon (design « Maquettes »; 021, « Cadre » : panneau du brouillon, « Écrans » :
// Brouillon): the draft panel, non modal, pushing the list it was filled from; operations
// that can be removed; the simulation under way, then with coded violations, then valid;
// submission with a motive.
import {
  Button,
  Cluster,
  DataGrid,
  Drawer,
  Icon,
  IconButton,
  InlineAlert,
  KeyLabel,
  Skeleton,
  StatusBadge,
  Stack,
  TextField,
  type GridColumn,
  type RowState,
} from "@kartenn/ui";
import { ref } from "vue";
import Attributs from "./EcranAttributs.vue";
import Cadre from "./EcranCadre.vue";
import Panneau from "./EcranPanneau.vue";
import { usageLabel } from "./donnees.ts";

const props = defineProps<{ vue: string }>();
const submitting = props.vue === "soumission";
const motive = ref(submitting ? "Bascule des VIP de la salle B" : "");

interface Row extends Record<string, unknown> {
  value: string;
  usage: string;
  owner: string;
  site: string;
}
const rows: Row[] = [
  { value: "192.0.2.10", usage: "host", owner: "Atelier Nord", site: "site-ouest" },
  { value: "192.0.2.11", usage: "vip", owner: "Atelier Nord", site: "site-ouest" },
  { value: "192.0.2.12", usage: "host", owner: "Atelier Sud", site: "site-ouest" },
  { value: "192.0.2.13", usage: "host", owner: "Service Pilote", site: "site-ouest" },
  { value: "192.0.2.40", usage: "host", owner: "Service Pilote", site: "site-ouest" },
  { value: "192.0.2.41", usage: "reserved", owner: "Service Pilote", site: "site-ouest" },
];
const columns: GridColumn[] = [
  { id: "value", label: "Valeur", width: "md", mono: true },
  { id: "usage", label: "Usage", width: "sm" },
  { id: "owner", label: "Propriétaire", width: "fill" },
  { id: "site", label: "Site", width: "md", mono: true },
];
const DRAFTED = ["192.0.2.11", "192.0.2.12", "192.0.2.40"];
const rowState = (row: Row): RowState | undefined =>
  DRAFTED.includes(row.value) ? "draft" : undefined;

/** The operations of the draft, as the panel lists them. */
const operations = [
  { key: "192.0.2.11", what: "Usage", before: "Hôte", after: "VIP", op: "modifier" },
  { key: "192.0.2.12", what: "Propriétaire", before: "Atelier Nord", after: "Atelier Sud" },
  { key: "192.0.2.40", what: "Créer l'adresse", before: "", after: "Hôte, Service Pilote" },
];
const violations =
  props.vue === "violations"
    ? {
        "192.0.2.12": {
          message: "« Atelier Sud » n'est pas une entité connue.",
          code: "prop.assignment.invalid_value",
        },
        "192.0.2.40": {
          message: "Hors du réseau 192.0.2.0/27 qui la contient.",
          code: "core.address.outside_network",
        },
      }
    : ({} as Record<string, { message: string; code: string }>);
</script>

<template>
  <Cadre
    title="Adresses"
    active="addresses"
    space="referential"
    :draft="3"
    draft-open
    :crumbs="[{ label: 'Référentiel', to: '/' }, { label: 'Adresses' }]"
  >
    <Panneau title="Adresses" detail="1 284" main flush>
      <template #actions>
        <Button variant="primary" icon="plus" :shortcut="['c']">Créer</Button>
      </template>
      <DataGrid
        label="Adresses"
        :columns="columns"
        :rows="rows"
        row-key="value"
        :row-state="rowState"
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
    <template #drawer>
      <Drawer :open="true" title="Brouillon" description="3 opérations, non soumises.">
        <Stack gap="4">
          <Stack as="ol" gap="2" class="k-ecran-list">
            <li v-for="op in operations" :key="op.key">
              <Stack gap="1">
                <Cluster justify="between" :wrap="false" gap="2">
                  <Cluster gap="2" :wrap="false">
                    <Icon name="draft" />
                    <code>{{ op.key }}</code>
                    <span>
                      {{ op.what }}<template v-if="op.before"> : </template>
                      <del v-if="op.before">{{ op.before }}</del>
                      <template v-if="op.before"> → </template>
                      <ins v-if="op.before">{{ op.after }}</ins>
                      <span v-else class="k-ecran-muted"> ({{ op.after }})</span>
                    </span>
                  </Cluster>
                  <IconButton
                    icon="x"
                    size="sm"
                    :label="`Retirer l'opération « ${op.what} » sur ${op.key}`"
                  />
                </Cluster>
                <InlineAlert
                  v-if="violations[op.key]"
                  tone="ko"
                  :title="`${op.key} : refusée par la simulation`"
                  :items="[violations[op.key]!]"
                />
              </Stack>
            </li>
          </Stack>
          <section aria-labelledby="k-ecran-simulation">
            <Stack gap="2">
              <h3 id="k-ecran-simulation" class="k-ecran-subtitle">Simulation</h3>
              <template v-if="props.vue === 'simulation'">
                <Cluster gap="2" :wrap="false">
                  <Icon name="loader-circle" class="k-spinner" />
                  <span>Simulation en cours des 3 opérations…</span>
                </Cluster>
                <Skeleton variant="text" :rows="2" />
              </template>
              <p v-else-if="props.vue === 'violations'">
                <StatusBadge state="ko" label="2 violations" /> Corriger ou retirer les opérations
                en cause.
              </p>
              <InlineAlert v-else tone="ok" title="Simulation valide · 0 violation" />
            </Stack>
          </section>
          <TextField
            v-model="motive"
            label="Motif"
            required
            hint="Reporté dans l'historique de chaque objet."
          />
          <Attributs
            :items="[
              { key: 'effect', label: 'Effet au', value: 'présent' },
              { key: 'review', label: 'Revue', value: 'non requise' },
            ]"
          />
        </Stack>
        <template #actions>
          <span v-if="!submitting" class="k-ecran-muted">{{
            props.vue === "simulation" ? "Simulation en cours" : "2 violations à corriger"
          }}</span>
          <Button variant="ghost" icon="trash">Tout abandonner</Button>
          <Button
            variant="primary"
            icon="send"
            :shortcut="['Control', 'Enter']"
            :disabled="!submitting"
            >Soumettre</Button
          >
        </template>
      </Drawer>
    </template>
  </Cadre>
</template>
