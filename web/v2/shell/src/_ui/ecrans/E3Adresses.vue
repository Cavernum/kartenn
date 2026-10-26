<script setup lang="ts">
// E3 — Liste d'adresses (design « Maquettes »; 021, « Listes des collections », « Édition
// en place », « Filtres »): typed filters as chips (one chip = one parameter of the API),
// a cell being edited, a cell refused in plain language with its code beside (024/M16),
// multiple selection and a group action. One grid (header and rows share their columns),
// sort and filter on every column, the filtered ones marked (024/M12); the actions fold
// into « … » when the header narrows.
import {
  Button,
  Cluster,
  DataGrid,
  EnvChip,
  FilterChip,
  Kbd,
  KeyLabel,
  Menu,
  Toolbar,
  type CellState,
  type GridColumn,
  type GridSort,
  type MenuItem,
  type ToolbarAction,
} from "@kartenn/ui";
import { ref } from "vue";
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
  { id: "value", label: "Valeur", width: "md", mono: true, sortable: true, filterable: true },
  { id: "usage", label: "Usage", width: "sm", sortable: true, filterable: true, priority: 2 },
  { id: "env", label: "Environnement", width: "md", sortable: true, filterable: true, priority: 3 },
  { id: "owner", label: "Propriétaire", width: "fill", sortable: true, filterable: true },
  {
    id: "site",
    label: "Site",
    width: "md",
    mono: true,
    sortable: true,
    filterable: true,
    priority: 3,
  },
  {
    id: "machine",
    label: "Machine",
    width: "md",
    mono: true,
    sortable: true,
    filterable: true,
    priority: 3,
  },
  { id: "vlan", label: "VLAN", width: "sm", align: "end", mono: true, sortable: true, priority: 2 },
];
const sort = ref<GridSort | undefined>({ column: "value", direction: "ascending" });
/** The selected rows: Space toggles one, Ctrl+A all (the grid's keys). */
const selection = ref<string[]>(selecting ? ["192.0.2.10", "192.0.2.12", "192.0.2.72"] : []);
const actions: ToolbarAction[] = [
  { id: "create", label: "Créer", icon: "plus", variant: "primary", shortcut: ["c"], priority: 3 },
  { id: "import", label: "Importer", icon: "clipboard-paste", variant: "ghost" },
];
const cellState = (row: Row, column: GridColumn): CellState | undefined => {
  if (selecting) return undefined;
  if (row.value === "192.0.2.11" && column.id === "usage") return "draft";
  if (row.value === "192.0.2.13" && column.id === "site") return "pending";
  if (row.value === "192.0.2.70" && column.id === "usage")
    return {
      error: "Une adresse d'une plage DHCP ne peut pas être un hôte fixe.",
      code: "core.address.range_usage",
    };
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
        <Toolbar label="Actions sur les adresses" align="end" :actions="actions" />
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
        <span class="k-num" role="status">{{
          selection.length === 1
            ? "1 adresse sélectionnée"
            : `${selection.length} adresses sélectionnées`
        }}</span>
        <Menu label="Action groupée" icon="layers" :items="bulk" default-open />
        <AVenir component="Checkbox" task="17" inline>
          <Button variant="ghost" size="sm">Tout sélectionner (1 284)</Button>
        </AVenir>
        <Button variant="ghost" size="sm" icon="x">Annuler la sélection</Button>
      </Cluster>
      <DataGrid
        v-model:sort="sort"
        v-model:selected="selection"
        label="Adresses, filtrées"
        :columns="columns"
        :rows="rows"
        row-key="value"
        :filtered="['value', 'usage', 'owner']"
        :selectable="selecting"
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
          <span class="k-keyboard-hint">
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
