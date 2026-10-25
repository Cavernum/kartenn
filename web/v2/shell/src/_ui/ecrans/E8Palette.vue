<script setup lang="ts">
// E8 — Palette et aide (design « Maquettes »; 021, « Raccourcis »): the palette Ctrl+K over
// the home page (commands, navigation, objects), and the help « ? » of the shortcuts, read
// from the registry, by scope.
import {
  CodeBadge,
  CommandPalette,
  DataGrid,
  Dialog,
  Grid,
  Kbd,
  Stack,
  StatusBadge,
  type GridColumn,
  type PaletteGroup,
} from "@kartenn/ui";
import AVenir from "./AVenir.vue";
import Cadre from "./EcranCadre.vue";
import Panneau from "./EcranPanneau.vue";

const props = defineProps<{ vue: string }>();

const groups: PaletteGroup[] = [
  {
    label: "Objets",
    items: [
      { id: "p1", label: "Salle A", icon: "prefix", detail: "192.0.2.0/24" },
      { id: "p2", label: "Supervision", icon: "prefix", detail: "192.0.2.64/26" },
      { id: "a1", label: "srv-app-01", icon: "address", detail: "192.0.2.10" },
    ],
  },
  {
    label: "Commandes",
    items: [
      { id: "create", label: "Créer un préfixe dans 192.0.2.0/24", icon: "plus", shortcut: ["c"] },
      { id: "free", label: "Prochain libre dans 192.0.2.0/24", icon: "search" },
      { id: "date", label: "Lire à une autre date…", icon: "calendar" },
    ],
  },
  {
    label: "Navigation",
    items: [
      { id: "prefixes", label: "Préfixes", icon: "prefix", shortcut: ["g", "p"] },
      { id: "addresses", label: "Adresses", icon: "address", shortcut: ["g", "a"] },
    ],
  },
];

interface Shortcut extends Record<string, unknown> {
  id: string;
  keys: string[][];
  action: string;
  scope: string;
}
const shortcuts: Shortcut[] = [
  { id: "palette", keys: [["Control", "k"]], action: "Palette de commandes", scope: "Partout" },
  { id: "filter", keys: [["/"]], action: "Filtrer la vue", scope: "Hors champ" },
  { id: "help", keys: [["?"]], action: "Cette aide", scope: "Hors champ" },
  { id: "gp", keys: [["g"], ["p"]], action: "Aller aux préfixes", scope: "Hors champ" },
  { id: "ga", keys: [["g"], ["a"]], action: "Aller aux adresses", scope: "Hors champ" },
  { id: "gc", keys: [["g"], ["c"]], action: "Aller aux changesets", scope: "Hors champ" },
  { id: "gb", keys: [["g"], ["b"]], action: "Ouvrir le brouillon", scope: "Hors champ" },
  { id: "gv", keys: [["g"], ["v"]], action: "Aller aux VLAN (plugin)", scope: "Hors champ" },
  { id: "c", keys: [["c"]], action: "Créer dans la collection", scope: "Présent, droit" },
  { id: "e", keys: [["e"]], action: "Modifier l'objet ou la cellule", scope: "Présent, droit" },
  {
    id: "apply",
    keys: [["Control", "Enter"]],
    action: "Action choisie du bouton scindé (Appliquer, Soumettre)",
    scope: "Brouillon",
  },
  { id: "esc", keys: [["Escape"]], action: "Fermer, annuler l'édition", scope: "Partout" },
  { id: "sidebar", keys: [["["]], action: "Replier la barre latérale", scope: "Hors champ" },
  { id: "panel", keys: [["]"]], action: "Replier le panneau droit", scope: "Hors champ" },
  {
    id: "column",
    keys: [["Alt", "ArrowDown"]],
    action: "Filtrer la colonne",
    scope: "En-tête de table",
  },
];
const shortcutColumns: GridColumn[] = [
  { id: "keys", label: "Touches", width: "md" },
  { id: "action", label: "Action", width: "fill" },
  { id: "scope", label: "Portée", width: "md" },
];
</script>

<template>
  <Cadre
    title="Accueil"
    active="home"
    space="referential"
    :crumbs="[{ label: 'Référentiel', to: '/' }, { label: 'Accueil' }]"
  >
    <Grid columns="auto" gap="3">
      <Panneau title="Mes changesets en attente" detail="2">
        <StatusBadge state="review" label="2 en revue" />
        <a href="/changesets/01JD4K8B2C">Renumérotation salle B</a>
      </Panneau>
      <Panneau title="À instruire" detail="3">
        <p>3 propositions attendent vos droits.</p>
        <a href="/proposals">Ouvrir la file</a>
      </Panneau>
      <Panneau title="Anomalies ouvertes" detail="6">
        <Stack gap="1">
          <span><CodeBadge code="vlan.site.mismatch" /> 4</span>
          <span><CodeBadge code="machines.mac.duplicate" /> 2</span>
        </Stack>
      </Panneau>
      <Panneau title="Échéances de flux" detail="plugin Flux">
        <StatusBadge state="warn" label="2 flux à J-7" />
      </Panneau>
      <Panneau title="Dernier rapport d'audit" detail="plugin Politique">
        <StatusBadge state="ok" label="Conforme au 2026-01-01" />
      </Panneau>
      <Panneau title="Groupes de VLAN" detail="plugin VLAN">
        <p class="k-num">campus-ouest : 87 % des VID utilisés</p>
      </Panneau>
    </Grid>
    <CommandPalette v-if="props.vue === 'palette'" :open="true" :groups="groups" />
    <Dialog
      v-else
      :open="true"
      size="lg"
      title="Raccourcis clavier"
      description="Lus au registre des raccourcis : ceux du shell, puis ceux des plugins chargés."
    >
      <Stack gap="3">
        <AVenir component="ShortcutHint" task="16" inline>
          <span><Kbd :keys="['g']" /> puis <Kbd :keys="['p']" /></span>
        </AVenir>
        <DataGrid label="Raccourcis" :columns="shortcutColumns" :rows="shortcuts" row-key="id">
          <template #cell="{ row, column, value }">
            <span v-if="column.id === 'keys'">
              <span class="k-visually-hidden">Touches : </span>
              <template v-for="(step, i) in row.keys" :key="i"
                ><template v-if="i > 0"> puis </template><Kbd :keys="step"
              /></template>
            </span>
            <template v-else>{{ value }}</template>
          </template>
        </DataGrid>
        <p class="k-ecran-muted">
          Les raccourcis à une touche se désactivent dans le menu du compte ; Ctrl+K, Ctrl+Entrée et
          Échap restent.
        </p>
      </Stack>
    </Dialog>
  </Cadre>
</template>
