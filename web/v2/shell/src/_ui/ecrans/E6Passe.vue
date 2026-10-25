<script setup lang="ts">
// E6 — Lecture au passé (design « Maquettes »; 021, « Cadre » : bande, « Édition en
// place » : hors présent; 024/M14, M15): the amber band, the date pill open on the period
// picker, laid over the page (a non-modal dialog in the `picker` slot of the frame, out of
// the flow under the date pill: the page does not move; the picker itself is still to
// come, a marked stand-in), writes greyed with
// their reason in their tooltip, the chronology of versions with the one valid at the date
// read.
import {
  Button,
  Cluster,
  DataGrid,
  Stack,
  TextField,
  type GridColumn,
  type GridSort,
  type ToolbarAction,
} from "@kartenn/ui";
import { ref } from "vue";
import AVenir from "./AVenir.vue";
import Attributs from "./EcranAttributs.vue";
import Barre from "./EcranBarre.vue";
import Cadre from "./EcranCadre.vue";
import Panneau from "./EcranPanneau.vue";
import { DOMAIN, PAST } from "./donnees.ts";

const props = defineProps<{ vue: string }>();
const picker = props.vue === "selecteur";

const READ_ONLY = "Lecture datée : revenir au présent pour écrire.";
const actions: ToolbarAction[] = [
  { id: "present", label: "Lire au présent", icon: "undo-2", priority: 3 },
  { id: "compare", label: "Comparer avec le présent", icon: "git-compare", priority: 2 },
  {
    id: "edit",
    label: "Modifier",
    icon: "pencil",
    shortcut: ["e"],
    priority: 1,
    reason: READ_ONLY,
  },
  { id: "copy", label: "Copier l'identifiant", icon: "copy" },
  { id: "close", label: "Clore…", icon: "trash", reason: READ_ONLY },
];

interface Version extends Record<string, unknown> {
  version: string;
  valid: string;
  known: string;
  motive: string;
}
const versions: Version[] = [
  {
    version: "v4",
    valid: "2025-12-02 → …",
    known: "2025-12-02",
    motive: "Bascule des VIP salle B",
  },
  {
    version: "v3",
    valid: "2025-09-12 → 2025-12-02",
    known: "2025-09-12",
    motive: "Rattachement au site ouest",
  },
  {
    version: "v2",
    valid: "2025-06-01 → 2025-09-12",
    known: "2025-06-03",
    motive: "Correction rétroactive du propriétaire",
  },
  {
    version: "v1",
    valid: "2025-03-17 → 2025-06-01",
    known: "2025-03-17",
    motive: "Import initial",
  },
];
const columns: GridColumn[] = [
  { id: "version", label: "Version", width: "sm", mono: true, sortable: true },
  { id: "valid", label: "Valide", width: "lg", mono: true, sortable: true },
  { id: "known", label: "Connue du", width: "md", mono: true, sortable: true },
  { id: "motive", label: "Motif", width: "fill", filterable: true },
];
const sort = ref<GridSort>({ column: "version", direction: "descending" });
</script>

<template>
  <Cadre
    title="Préfixe 192.0.2.64/26, lu au 3 novembre 2025"
    active="prefixes"
    space="referential"
    :at="PAST"
    :picker="picker"
    :crumbs="[
      { label: 'Référentiel', to: '/' },
      { label: DOMAIN.label, to: `/domains/${DOMAIN.key}` },
      { label: 'Préfixes', to: '/prefixes' },
      { label: '192.0.2.64/26', mono: true },
    ]"
  >
    <Panneau title="192.0.2.64/26" detail="préfixe · réseau · Supervision" main>
      <template #toolbar>
        <Barre label="Actions du préfixe" :actions="actions" />
      </template>
      <p>
        Valeurs valides au 3 novembre 2025 (version <span class="k-mono">v3</span>). Les écritures
        reprennent au présent ; le brouillon peut recevoir des opérations datées.
      </p>
      <Attributs
        :items="[
          { key: 'owner', label: 'Propriétaire', value: 'Atelier Nord' },
          { key: 'site', label: 'Site', value: 'site-ouest', mono: true },
          { key: 'version', label: 'Version lue', value: 'v3', mono: true },
        ]"
      />
    </Panneau>
    <Panneau title="Chronologie des versions" detail="4" flush>
      <DataGrid
        v-model:sort="sort"
        label="Versions de 192.0.2.64/26 ; la v3 est celle de la date lue"
        :columns="columns"
        :rows="versions"
        row-key="version"
        selectable
        :selected="['v3']"
      >
        <template #cell="{ row, column, value }">
          <template v-if="column.id === 'version'"
            >{{ row.version
            }}<span v-if="row.version === 'v3'" class="k-ecran-muted"> · lue</span></template
          >
          <template v-else>{{ value }}</template>
        </template>
      </DataGrid>
    </Panneau>
    <!-- The picker lies over the page, under the date pill: the page keeps its layout. -->
    <template v-if="picker" #picker>
      <div role="dialog" aria-labelledby="k-ecran-periode" class="k-ecran-surimpression">
        <AVenir component="PeriodPicker" task="20">
          <section class="k-ecran-periode">
            <h2 id="k-ecran-periode" class="k-ecran-subtitle">Lire à une date</h2>
            <Stack gap="3">
              <Cluster gap="2" role="group" aria-label="Axe du temps">
                <Button size="sm" :pressed="true">Valide au</Button>
                <Button size="sm" :pressed="false">Connu au</Button>
              </Cluster>
              <Cluster gap="3" align="end">
                <TextField
                  label="Date et heure"
                  mono
                  model-value="2025-11-03 09:30"
                  hint="AAAA-MM-JJ hh:mm"
                />
                <TextField label="Fuseau" readonly model-value="Europe/Paris" />
              </Cluster>
              <Cluster gap="2">
                <Button size="sm" variant="ghost">Il y a une semaine</Button>
                <Button size="sm" variant="ghost">Début du mois</Button>
                <Button size="sm" variant="ghost">Fin 2025</Button>
              </Cluster>
              <Cluster gap="2" justify="end">
                <Button variant="ghost" icon="undo-2">Lire au présent</Button>
                <Button variant="primary" icon="calendar">Lire à cette date</Button>
              </Cluster>
            </Stack>
          </section>
        </AVenir>
      </div>
    </template>
  </Cadre>
</template>

<style>
@layer components {
  /* A non-modal layer: the frame lays it under the date pill, over the band and the page. */
  .k-ecran-surimpression {
    inline-size: var(--k-size-panel-lg);
    max-inline-size: 100%;
    background-color: var(--k-surface);
    border-radius: var(--k-radius-md);
  }

  .k-ecran-periode {
    display: grid;
    gap: var(--k-space-3);
    padding: var(--k-space-2);
  }
}
</style>
