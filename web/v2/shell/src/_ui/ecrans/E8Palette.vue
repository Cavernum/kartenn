<script setup lang="ts">
// E8 — Palette et aide (design « Maquettes »; 021, « Raccourcis »): the palette Ctrl+K over
// the home page (commands, navigation, objects), and the help « ? » of the shortcuts, read
// from the registry, by scope.
import {
  CommandPalette,
  DataGrid,
  Dialog,
  ShortcutHint,
  Stack,
  type GridColumn,
  type PaletteGroup,
} from "@kartenn/ui";
import Accueil from "./EcranAccueil.vue";
import Cadre from "./EcranCadre.vue";

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
      { id: "prefixes", label: "Préfixes", icon: "prefix", shortcut: [["g"], ["p"]] },
      { id: "addresses", label: "Adresses", icon: "address", shortcut: [["g"], ["a"]] },
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
  { id: "gv", keys: [["g"], ["v"]], action: "Aller aux VLAN", scope: "Hors champ" },
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
  <Cadre title="Accueil" active="home" space="referential" :crumbs="[{ label: 'Accueil' }]">
    <!-- The home page of E0 itself under the layer, never a stale copy. -->
    <Accueil />
    <CommandPalette v-if="props.vue === 'palette'" :open="true" :groups="groups" />
    <Dialog
      v-else
      :open="true"
      size="lg"
      title="Raccourcis clavier"
      description="Lus au registre des raccourcis : ceux du shell, puis ceux des plugins chargés."
    >
      <Stack gap="3">
        <DataGrid label="Raccourcis" :columns="shortcutColumns" :rows="shortcuts" row-key="id">
          <template #cell="{ row, column, value }">
            <span v-if="column.id === 'keys'">
              <span class="k-visually-hidden">Touches : </span>
              <ShortcutHint :keys="row.keys" />
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
