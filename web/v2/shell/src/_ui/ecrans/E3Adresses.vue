<script setup lang="ts">
// E3 — Liste d'adresses (design « Maquettes »; 021, « Listes des collections », « Édition
// en place », « Filtres »): typed filters as chips (one chip = one parameter of the API),
// a cell being edited, a cell refused with its code, multiple selection and a group action.
import {
  Button,
  Cluster,
  DataGrid,
  EnvChip,
  FilterChip,
  Kbd,
  KeyLabel,
  Menu,
  type CellState,
  type GridColumn,
  type MenuItem,
} from "@kartenn/ui";
import AVenir from "./AVenir.vue";
import Cadre from "./EcranCadre.vue";
import Panneau from "./EcranPanneau.vue";
import { DOMAIN, ENV, usageLabel, type EnvKey } from "./donnees.ts";

const props = defineProps<{ vue: string }>();
const selecting = props.vue === "selection";

interface Row extends Record<string, unknown> {
  value: string;
  usage: string;
  env: EnvKey;
  owner: string;
  site: string;
  machine: string;
  vlan: string;
}
const rows: Row[] = [
  {
    value: "192.0.2.10",
    usage: "host",
    env: "prod",
    owner: "Atelier Nord",
    site: "site-ouest",
    machine: "srv-app-01",
    vlan: "120",
  },
  {
    value: "192.0.2.11",
    usage: "vip",
    env: "prod",
    owner: "Atelier Nord",
    site: "site-ouest",
    machine: "—",
    vlan: "120",
  },
  {
    value: "192.0.2.12",
    usage: "host",
    env: "rece",
    owner: "Atelier Nord",
    site: "site-ouest",
    machine: "srv-rec-03",
    vlan: "120",
  },
  {
    value: "192.0.2.13",
    usage: "host",
    env: "rece",
    owner: "Atelier Nord",
    site: "site-ouest",
    machine: "srv-rec-04",
    vlan: "120",
  },
  {
    value: "192.0.2.70",
    usage: "host",
    env: "deve",
    owner: "Atelier Nord",
    site: "site-est",
    machine: "dev-web-01",
    vlan: "121",
  },
  {
    value: "192.0.2.71",
    usage: "vip",
    env: "prod",
    owner: "Atelier Nord",
    site: "site-est",
    machine: "—",
    vlan: "121",
  },
  {
    value: "192.0.2.72",
    usage: "host",
    env: "prod",
    owner: "Atelier Nord",
    site: "site-est",
    machine: "srv-app-07",
    vlan: "121",
  },
  {
    value: "192.0.2.80",
    usage: "host",
    env: "prod",
    owner: "Atelier Nord",
    site: "site-est",
    machine: "srv-app-08",
    vlan: "121",
  },
];
const columns: GridColumn[] = [
  { id: "value", label: "Valeur", width: "md", mono: true, sortable: true },
  { id: "usage", label: "Usage", width: "sm", filterable: true },
  { id: "env", label: "Environnement", width: "md", filterable: true },
  { id: "owner", label: "Propriétaire", width: "fill", sortable: true, filterable: true },
  { id: "site", label: "Site", width: "md", mono: true, filterable: true },
  { id: "machine", label: "Machine", width: "md", mono: true },
  { id: "vlan", label: "VLAN", width: "xs", align: "end", mono: true },
];
const cellState = (row: Row, column: GridColumn): CellState | undefined => {
  if (selecting) return undefined;
  if (row.value === "192.0.2.11" && column.id === "usage") return "draft";
  if (row.value === "192.0.2.13" && column.id === "site") return "pending";
  if (row.value === "192.0.2.70" && column.id === "usage")
    return { error: "Usage refusé dans une plage DHCP", code: "core.address.range_usage" };
  return undefined;
};
const bulk: MenuItem[] = [
  { id: "usage", label: "Changer l'usage…", icon: "pencil" },
  { id: "owner", label: "Changer le propriétaire…", icon: "users" },
  { id: "prop", label: "Poser une propriété…", icon: "tag" },
  { id: "export", label: "Copier les valeurs", icon: "copy" },
  { kind: "separator" },
  { id: "close", label: "Clore les 3 adresses…", icon: "trash", danger: true },
];
</script>

<template>
  <Cadre
    title="Adresses"
    active="addresses"
    space="referential"
    :draft="selecting ? 2 : 3"
    :crumbs="[{ label: 'Référentiel', to: '/' }, { label: 'Adresses' }]"
  >
    <Panneau title="Adresses" detail="1 284" main flush>
      <template #actions>
        <Button variant="ghost" icon="layers">Colonnes</Button>
        <Button variant="primary" icon="plus" :shortcut="['c']">Créer</Button>
      </template>
      <template #toolbar>
        <Cluster gap="2">
          <FilterChip property="Domaine" operator="=" :value="DOMAIN.label" />
          <FilterChip property="Inclus dans" operator="=" value="192.0.2.0/24" mono />
          <FilterChip property="Usage" operator="∈" value="Hôte, VIP" />
          <FilterChip
            property="Propriétaire"
            operator="="
            value="Atelier Nord"
            state="draft"
            remove-label="Retirer le filtre Propriétaire"
          />
          <FilterChip
            property="Créée après"
            operator="="
            value="2026-13-01"
            mono
            state="invalid"
            reason="Date invalide : le mois va de 01 à 12."
            remove-label="Retirer le filtre Créée après"
          />
          <AVenir component="FilterPicker" task="19" inline>
            <Button variant="ghost" size="sm" icon="funnel" :shortcut="['/']"
              >Ajouter un filtre</Button
            >
          </AVenir>
        </Cluster>
      </template>
      <Cluster v-if="selecting" gap="3" class="k-ecran-selection">
        <span class="k-num" role="status">3 adresses sélectionnées</span>
        <Menu label="Action groupée" icon="layers" :items="bulk" default-open />
        <AVenir component="Checkbox" task="17" inline>
          <Button variant="ghost" size="sm">Tout sélectionner (1 284)</Button>
        </AVenir>
        <Button variant="ghost" size="sm" icon="x">Annuler la sélection</Button>
      </Cluster>
      <DataGrid
        label="Adresses, filtrées"
        :columns="columns"
        :rows="rows"
        row-key="value"
        :sort="{ column: 'value', direction: 'ascending' }"
        :selectable="selecting"
        :selected="selecting ? ['192.0.2.10', '192.0.2.12', '192.0.2.72'] : []"
        :cell-state="cellState"
        :active="
          selecting
            ? { row: '192.0.2.72', column: 'value' }
            : { row: '192.0.2.12', column: 'owner' }
        "
        :editing="selecting ? undefined : { row: '192.0.2.12', column: 'owner' }"
      >
        <template #cell="{ row, column, value }">
          <KeyLabel
            v-if="column.id === 'usage'"
            :value="row.usage"
            :label="usageLabel(row.usage)"
          />
          <EnvChip v-else-if="column.id === 'env'" v-bind="ENV[row.env]" />
          <template v-else>{{ value }}</template>
        </template>
      </DataGrid>
      <template #footer>
        <span class="k-num">8 affichées sur 1 284</span>
        <Cluster gap="3">
          <span>
            <Kbd :keys="['Enter']" /> vers le brouillon ·
            <Kbd :keys="['Control', 'Enter']" /> appliquer avec motif ·
            <Kbd :keys="['Escape']" /> annuler
          </span>
        </Cluster>
      </template>
    </Panneau>
  </Cadre>
</template>

<style>
@layer components {
  .k-ecran-selection {
    padding: var(--k-space-2) var(--k-space-3);
    border-block-end: var(--k-border-width) solid var(--k-border);
  }
}
</style>
