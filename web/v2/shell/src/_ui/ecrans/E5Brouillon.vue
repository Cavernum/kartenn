<script setup lang="ts">
// E5 — Brouillon (design « Maquettes »; 021, « Cadre » : panneau du brouillon, « Écrans » :
// Brouillons; 024/M8, M9): the draft tab of the right panel, over the end of the list it
// was filled from; its operations that can be removed; the simulation under way, then
// with coded violations, then valid but held by a policy; the footer of the tab (result
// of the simulation, CommitForm, SplitButton open on its menu, « Appliquer » disabled with
// its reason). And the page `/drafts`: the drafts of the user, to resume, create, delete.
import {
  Button,
  DataGrid,
  KeyLabel,
  Menu,
  StatusBadge,
  Toolbar,
  type GridColumn,
  type GridSort,
  type MenuItem,
  type RowState,
  type ToolbarAction,
} from "@kartenn/ui";
import { ref } from "vue";
import Brouillon from "./EcranBrouillon.vue";
import Cadre from "./EcranCadre.vue";
import Panneau from "./EcranPanneau.vue";
import Soumission from "./EcranSoumission.vue";
import { DRAFTS, usageLabel } from "./donnees.ts";

const props = defineProps<{ vue: string }>();
const list = props.vue === "liste";

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
  { id: "value", label: "Valeur", width: "md", mono: true, sortable: true },
  { id: "usage", label: "Usage", width: "sm", sortable: true, filterable: true, priority: 2 },
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
];
const sort = ref<GridSort | undefined>({ column: "value", direction: "ascending" });
const DRAFTED = ["192.0.2.11", "192.0.2.12", "192.0.2.40"];
const rowState = (row: Row): RowState | undefined =>
  DRAFTED.includes(row.value) ? "draft" : undefined;
const listActions: ToolbarAction[] = [
  { id: "create", label: "Créer", icon: "plus", variant: "primary", shortcut: ["c"], priority: 2 },
  { id: "import", label: "Importer", icon: "clipboard-paste", priority: 1 },
  { id: "copy", label: "Copier les valeurs", icon: "copy" },
];

const violations =
  props.vue === "violations"
    ? {
        "192.0.2.12": {
          message: "« Atelier Sud » n'est pas une entité connue.",
          code: "prop.assignment.invalid_value",
        },
        "192.0.2.40": {
          message: "L'adresse sort du réseau 192.0.2.0/27 qui devait la contenir.",
          code: "core.address.outside_network",
        },
      }
    : {};
const simulation =
  props.vue === "simulation" ? "running" : props.vue === "violations" ? "violations" : "valid";

// The page `/drafts`.
interface DraftRow extends Record<string, unknown> {
  id: string;
  title: string;
  operations: number;
  updated: string;
  expires: string;
}
const draftRows: DraftRow[] = DRAFTS.map((d) => ({ ...d }));
const draftColumns: GridColumn[] = [
  { id: "title", label: "Titre", width: "fill", sortable: true, filterable: true },
  { id: "operations", label: "Opérations", width: "md", align: "end", sortable: true, priority: 2 },
  { id: "updated", label: "Modifié le", width: "md", mono: true, sortable: true, priority: 3 },
  { id: "expires", label: "Suppression", width: "md", sortable: true, priority: 3 },
  { id: "actions", label: "Actions", width: "sm", actions: true },
];
const draftSort = ref<GridSort | undefined>({ column: "updated", direction: "descending" });
const draftState = (row: DraftRow): RowState | undefined =>
  row.id === DRAFTS[0]!.id ? "draft" : undefined;
const draftMenu = (row: DraftRow): MenuItem[] => [
  {
    id: "resume",
    label: "Reprendre",
    icon: "undo-2",
    ...(row.id === DRAFTS[0]!.id ? { reason: "C'est déjà le brouillon actif." } : {}),
  },
  { id: "rename", label: "Renommer…", icon: "pencil" },
  { id: "copy", label: "Copier l'identifiant", icon: "copy" },
  { kind: "separator" },
  { id: "delete", label: `Supprimer « ${row.title} »…`, icon: "trash", danger: true },
];
</script>

<template>
  <Cadre
    v-if="list"
    title="Mes brouillons"
    active="draft"
    space="changes"
    :crumbs="[{ label: 'Changesets', to: '/changesets' }, { label: 'Mes brouillons' }]"
  >
    <Panneau title="Mes brouillons" :detail="`${DRAFTS.length} sur 50`" main flush>
      <template #actions>
        <Button variant="primary" icon="plus">Nouveau brouillon</Button>
      </template>
      <DataGrid
        v-model:sort="draftSort"
        label="Mes brouillons"
        :columns="draftColumns"
        :rows="draftRows"
        row-key="id"
        :row-state="draftState"
        stack
      >
        <template #cell="{ row, column, value }">
          <template v-if="column.id === 'title'">
            {{ row.title }}
            <span v-if="row.id === DRAFTS[0]!.id" class="k-ecran-muted">(actif)</span>
          </template>
          <template v-else-if="column.id === 'expires'">
            <StatusBadge v-if="row.expires" state="warn" :label="`le ${row.expires}`" />
            <span v-else class="k-ecran-muted">—</span>
          </template>
          <span v-else-if="column.id === 'actions'" class="k-ecran-row-actions">
            <Menu
              :label="`Actions sur « ${row.title} »`"
              icon="ellipsis"
              icon-only
              variant="ghost"
              size="sm"
              align="end"
              :items="draftMenu(row)"
            />
          </span>
          <template v-else>{{ value }}</template>
        </template>
      </DataGrid>
      <template #footer>
        <span class="k-ecran-muted"
          >Un brouillon sans modification depuis 90 jours est supprimé ; la date s'annonce une
          semaine avant.</span
        >
      </template>
    </Panneau>
  </Cadre>
  <Cadre
    v-else
    title="Adresses"
    active="addresses"
    space="referential"
    panel="draft"
    :crumbs="[{ label: 'Référentiel', to: '/' }, { label: 'Adresses' }]"
  >
    <Panneau title="Adresses" detail="1 284" main flush>
      <template #actions>
        <Toolbar label="Actions sur les adresses" align="end" :actions="listActions" />
      </template>
      <DataGrid
        v-model:sort="sort"
        label="Adresses"
        :columns="columns"
        :rows="rows"
        row-key="value"
        :row-state="rowState"
        :filtered="['site']"
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
    <template #draft>
      <Brouillon :count="3" :violations="violations" />
    </template>
    <template #draft-footer>
      <Soumission
        :simulation="simulation"
        :held="props.vue === 'soumission'"
        :menu-open="props.vue === 'soumission'"
        :summary="
          props.vue === 'soumission'
            ? 'Bascule des VIP de la salle B'
            : 'Maintenance du référentiel'
        "
      />
    </template>
  </Cadre>
</template>

<style>
@layer components {
  /* The actions of a row, on one line in their cell. */
  .k-ecran-row-actions {
    display: inline-flex;
    gap: var(--k-space-1);
    align-items: center;
  }
}
</style>
