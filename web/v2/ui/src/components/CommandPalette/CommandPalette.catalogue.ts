// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { computed, defineComponent, h, ref } from "vue";
import type { CatalogueEntry } from "../../catalogue.ts";
import { prefixes } from "../../fixtures/index.ts";
import Button from "../Button/Button.vue";
import Stack from "../Stack/Stack.vue";
import CommandPalette from "./CommandPalette.vue";
import type { PaletteGroup } from "./palette.ts";

const groups: PaletteGroup[] = [
  {
    label: "Commandes",
    items: [
      { id: "create", label: "Créer un préfixe", icon: "plus", shortcut: ["c"] },
      { id: "draft", label: "Ouvrir le brouillon", icon: "draft", shortcut: [["g"], ["b"]] },
    ],
  },
  {
    label: "Navigation",
    items: [
      { id: "prefixes", label: "Préfixes", icon: "prefix", shortcut: [["g"], ["p"]] },
      { id: "changesets", label: "Changesets", icon: "changeset", shortcut: [["g"], ["c"]] },
    ],
  },
  {
    label: "Récents",
    items: [
      { id: "p1", label: "Serveurs de bases", icon: "prefix", detail: prefixes[1] },
      { id: "p2", label: "Postes du siège", icon: "prefix", detail: prefixes[4] },
    ],
  },
];

/**
 * The palette of the keyboard script (e2e/components/command-palette.spec.ts), opened by a
 * button of the host (the shell opens it by Ctrl+K): the host filters by label and detail,
 * the command run is written under the button.
 */
const Keyboard = defineComponent({
  setup() {
    const open = ref(false);
    const query = ref("");
    const ran = ref("aucune");
    const shown = computed(() => {
      const text = query.value.trim().toLowerCase();
      return groups
        .map((group) => ({
          ...group,
          items: group.items.filter((item) =>
            `${item.label} ${item.detail ?? ""}`.toLowerCase().includes(text),
          ),
        }))
        .filter((group) => group.items.length > 0);
    });
    const label = (id: string) =>
      groups.flatMap((group) => group.items).find((item) => item.id === id)?.label ?? id;
    return () =>
      h(Stack, { gap: "3" }, () => [
        h(
          Button,
          {
            variant: "secondary",
            icon: "search",
            "aria-haspopup": "dialog",
            "aria-expanded": open.value ? "true" : "false",
            onClick: () => {
              query.value = "";
              open.value = true;
            },
          },
          () => "Ouvrir la palette",
        ),
        h("p", { class: "k-cat-note", "data-ran": "" }, `Commande lancée : ${ran.value}`),
        h(CommandPalette, {
          open: open.value,
          "onUpdate:open": (next: boolean) => (open.value = next),
          query: query.value,
          "onUpdate:query": (next: string) => (query.value = next),
          groups: shown.value,
          onSelect: (id: string) => (ran.value = label(id)),
        }),
      ]);
  },
});

export default {
  component: CommandPalette,
  pattern: "Combobox dans un Dialog modal",
  states: [
    {
      name: "open",
      description: "Ouverte : commandes, navigation, objets récents",
      props: { groups, open: true },
      isolated: true,
    },
    {
      name: "empty",
      description: "Aucune entrée",
      props: { groups: [], query: "zzz", open: true },
      isolated: true,
    },
    {
      name: "loading",
      description: "Chargement des objets",
      props: { groups: [], query: "serv", loading: true, open: true },
      isolated: true,
    },
    {
      name: "keyboard",
      description:
        "Au clavier : ouverte par un bouton, filtrée à la saisie, la première entrée mise en avant, Entrée lance, Échap ferme",
      props: {},
      demo: Keyboard,
    },
  ],
} satisfies CatalogueEntry;
