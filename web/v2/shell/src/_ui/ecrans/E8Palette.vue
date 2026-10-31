<script setup lang="ts">
// E8 — Palette et aide (design « Maquettes »; 021, « Raccourcis »): the palette Ctrl+K over
// the home page (commands, navigation, objects), and the help « ? » of the shortcuts: the
// shell's own help (`ShortcutHelp`), read from the registry of the shell (tâche 10), as an
// administrator sees it.
import { Dialog } from "@kartenn/ui";
import { useI18n } from "vue-i18n";
import { Rights } from "../../rights/index.ts";
import { shellShortcuts } from "../../shortcuts/shell.ts";
import ShortcutHelp from "../../shortcuts/ShortcutHelp.vue";
import Accueil from "./EcranAccueil.vue";
import Cadre from "./EcranCadre.vue";

const props = defineProps<{ vue: string }>();
const { t } = useI18n();

const ADMIN = new Rights(["*"]);
const none = () => false;
const shortcuts = shellShortcuts({
  t,
  rights: () => ADMIN,
  navigate: () => {},
  palette: none,
  help: () => {},
  filter: none,
  create: none,
  edit: none,
  sidebar: none,
  panel: () => {},
});
</script>

<template>
  <Cadre
    title="Accueil"
    active="home"
    space="referential"
    :crumbs="[{ label: 'Accueil' }]"
    :palette="props.vue === 'palette'"
  >
    <!-- The home page of E0 itself under the layer, never a stale copy; the palette is the
         frame's (« Rechercher », Ctrl+K). -->
    <Accueil />
    <Dialog v-if="props.vue !== 'palette'" :open="true" size="lg" :title="t('shortcuts.title')">
      <ShortcutHelp :shortcuts="shortcuts" single-keys />
    </Dialog>
  </Cadre>
</template>
