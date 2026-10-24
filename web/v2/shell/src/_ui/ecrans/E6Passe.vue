<script setup lang="ts">
// E6 — Lecture au passé (design « Maquettes »; 021, « Cadre » : bande, « Édition en
// place » : hors présent): the amber band, the date pill open on the period picker (still to
// come: a marked stand-in), writes disabled with their reason, the chronology of versions
// with the one valid at the date read.
import {
  Button,
  Cluster,
  DataGrid,
  InlineAlert,
  Menu,
  Stack,
  TextField,
  type GridColumn,
  type MenuItem,
} from "@kartenn/ui";
import AVenir from "./AVenir.vue";
import Attributs from "./EcranAttributs.vue";
import Cadre from "./EcranCadre.vue";
import Panneau from "./EcranPanneau.vue";
import { DOMAIN, PAST } from "./donnees.ts";

const props = defineProps<{ vue: string }>();
const picker = props.vue === "selecteur";

const READ_ONLY = "Lecture datée : revenir au présent pour écrire";
const actions: MenuItem[] = [
  { id: "edit", label: "Modifier", icon: "pencil", disabled: true, reason: READ_ONLY },
  { id: "copy", label: "Copier l'identifiant", icon: "copy" },
  { id: "compare", label: "Comparer avec le présent", icon: "git-compare" },
  { kind: "separator" },
  { id: "close", label: "Clore…", icon: "trash", disabled: true, reason: READ_ONLY },
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
  { id: "version", label: "Version", width: "sm", mono: true },
  { id: "valid", label: "Valide", width: "lg", mono: true },
  { id: "known", label: "Connue du", width: "md", mono: true },
  { id: "motive", label: "Motif", width: "fill" },
];
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
    <template v-if="picker" #picker>
      <AVenir component="PeriodPicker" task="20">
        <Panneau title="Lire à une date">
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
          </Stack>
          <template #footer>
            <Button variant="ghost" icon="undo-2">Lire au présent</Button>
            <Button variant="primary" icon="calendar">Lire à cette date</Button>
          </template>
        </Panneau>
      </AVenir>
    </template>
    <Panneau title="192.0.2.64/26" detail="préfixe · réseau · Supervision" main>
      <template #actions>
        <Button icon="pencil" :shortcut="['e']" disabled>Modifier</Button>
        <Menu label="Actions" icon="ellipsis" align="end" :items="actions" />
      </template>
      <InlineAlert tone="warn" title="Lecture au 3 novembre 2025 : les écritures sont désactivées.">
        Les valeurs sont celles valides à cette date (version v3). Le brouillon peut recevoir des
        opérations datées ; l'édition en place reprend au présent.
      </InlineAlert>
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
        label="Versions de 192.0.2.64/26 ; la v3 est celle de la date lue"
        :columns="columns"
        :rows="versions"
        row-key="version"
        selectable
        :selected="['v3']"
        :sort="{ column: 'version', direction: 'descending' }"
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
  </Cadre>
</template>
