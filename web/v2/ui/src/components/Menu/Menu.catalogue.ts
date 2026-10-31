// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { defineComponent, h, ref } from "vue";
import type { CatalogueEntry } from "../../catalogue.ts";
import Stack from "../Stack/Stack.vue";
import TextField from "../TextField/TextField.vue";
import Menu from "./Menu.vue";
import type { MenuItem } from "./menu.ts";

const items: MenuItem[] = [
  { kind: "label", label: "Préfixe" },
  { id: "edit", label: "Modifier", icon: "pencil", shortcut: ["e"] },
  { id: "copy", label: "Copier l'identifiant", icon: "copy" },
  { id: "history", label: "Historique", icon: "clock" },
  {
    id: "move",
    label: "Déplacer vers…",
    icon: "move",
    disabled: true,
    reason: "Parent déduit de l'inclusion",
  },
  { kind: "separator" },
  { id: "delete", label: "Supprimer", icon: "trash", danger: true },
  { id: "purge", label: "Purger l'historique", icon: "trash", forbidden: "prefix.purge" },
];

/** A menu of the user: shortcuts of each form, as the registry of the shell gives them. */
const shortcuts: MenuItem[] = [
  { id: "palette", label: "Palette de commandes", icon: "search", shortcut: ["Mod", "k"] },
  { id: "help", label: "Raccourcis", icon: "circle-question-mark", shortcut: ["?"] },
  { kind: "separator" },
  { kind: "label", label: "Aller à" },
  { id: "prefixes", label: "Préfixes", icon: "prefix", shortcut: [["g"], ["p"]] },
  { id: "drafts", label: "Brouillons", icon: "draft", shortcut: [["g"], ["b"]] },
];

/** The columns of a grid, each on or off: the menu stays open as they are chosen. */
const columns: MenuItem[] = [
  { kind: "label", label: "Colonnes" },
  { kind: "checkbox", id: "owner", label: "Propriétaire", checked: true },
  { kind: "checkbox", id: "used", label: "Occupation", checked: false },
  { kind: "checkbox", id: "site", label: "Site", checked: true },
];

/**
 * The menu of the keyboard script (APG Menu Button, Menu; e2e/components/menu.spec.ts): a
 * field before and after it, to leave it by Tab and Shift+Tab, and what was chosen said.
 */
const Keyboard = defineComponent({
  setup() {
    const before = ref("");
    const after = ref("");
    const chosen = ref("aucune");
    const entries: MenuItem[] = [
      ...items.slice(1, 5),
      { id: "prefixes", label: "Voir les préfixes", icon: "prefix", shortcut: [["g"], ["p"]] },
      ...items.slice(5),
    ];
    return () =>
      h(Stack, { gap: "3" }, () => [
        h(TextField, {
          label: "Préfixe",
          modelValue: before.value,
          "onUpdate:modelValue": (value: string) => (before.value = value),
        }),
        // In a block of its own: the stack would stretch the button.
        h("div", null, [
          h(Menu, {
            label: "Actions",
            items: entries,
            onSelect: (id: string) => {
              const entry = entries.find((e) => "id" in e && e.id === id);
              chosen.value = entry && "label" in entry ? entry.label : id;
            },
          }),
        ]),
        h("p", { role: "status", class: "k-cat-note" }, `Action choisie : ${chosen.value}`),
        h(TextField, {
          label: "Description",
          modelValue: after.value,
          "onUpdate:modelValue": (value: string) => (after.value = value),
        }),
      ]);
  },
});

export default {
  component: Menu,
  pattern: "Menu Button, Menu",
  states: [
    { name: "closed", description: "Fermé", props: { label: "Actions", items } },
    {
      name: "icon-only",
      description: "Bouton-icône (lignes, barres d'outils)",
      props: { label: "Actions sur le préfixe", items, iconOnly: true },
    },
    {
      name: "open",
      description:
        "Ouvert : raccourci à droite, entrées désactivées (motif, permission) focalisables, entrée destructive",
      props: { label: "Actions", items, defaultOpen: true },
      isolated: true,
    },
    {
      name: "shortcuts",
      description:
        "Raccourcis à droite : une combinaison (Ctrl K), une touche (?), une séquence (g puis p : deux touches, jamais une combinaison)",
      props: { label: "Menu de l'utilisateur", icon: "user", items: shortcuts, defaultOpen: true },
      isolated: true,
    },
    {
      name: "checkbox",
      description:
        "Entrées à cocher (menuitemcheckbox) : le menu reste ouvert quand on en coche une",
      props: { label: "Colonnes", icon: "columns-3", items: columns, defaultOpen: true },
      isolated: true,
    },
    {
      name: "radio",
      description:
        "Entrées radio (menuitemradio dans un groupe) : l'une d'un ensemble est cochée, aria-checked",
      props: {
        label: "Brouillons",
        items: [
          { kind: "label", label: "Brouillons" },
          { kind: "radio", id: "d1", label: "Salle B (3)", checked: true },
          { kind: "radio", id: "d2", label: "Reprise des VIP (12)", checked: false },
        ] satisfies MenuItem[],
        defaultOpen: true,
      },
      isolated: true,
    },
    {
      name: "keyboard",
      description:
        "Au clavier : ouvrir par Entrée, Espace, Bas ou Haut, parcourir, choisir, fermer, sortir par Tab",
      props: {},
      demo: Keyboard,
    },
  ],
} satisfies CatalogueEntry;
