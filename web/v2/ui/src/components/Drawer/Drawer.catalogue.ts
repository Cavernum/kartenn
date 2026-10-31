// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { defineComponent, h, ref, type Component, type FunctionalComponent } from "vue";
import type { CatalogueEntry } from "../../catalogue.ts";
import { prefixes } from "../../fixtures/index.ts";
import Button from "../Button/Button.vue";
import Stack from "../Stack/Stack.vue";
import StatusBadge from "../StatusBadge/StatusBadge.vue";
import TextField from "../TextField/TextField.vue";
import Drawer from "./Drawer.vue";

/** The draft panel: a few operations and the actions. */
const demo: FunctionalComponent = (props) =>
  h(Drawer as Component, props, {
    default: () =>
      h(Stack, { gap: "2" }, () =>
        prefixes
          .slice(0, 3)
          .map((prefix) =>
            h("p", { class: "k-mono" }, [
              h(StatusBadge, { state: "neutral", label: "Brouillon" }),
              ` ${prefix}`,
            ]),
          ),
      ),
    actions: () => [
      h(Button, { variant: "ghost" }, () => "Abandonner"),
      h(Button, { variant: "primary" }, () => "Soumettre"),
    ],
  });

/**
 * The drawer of the keyboard script (e2e/components/dialog.spec.ts): a long form opened by
 * a button of the host (no Reka trigger), its focus on the first field.
 */
const Keyboard = defineComponent({
  setup() {
    const open = ref(false);
    return () =>
      h(Stack, { gap: "3" }, () => [
        h(Button, { icon: "pencil", onClick: () => (open.value = true) }, () => "Modifier le site"),
        h(TextField, { label: "Note" }),
        h(
          Drawer as Component,
          {
            title: "Modifier le site",
            open: open.value,
            "onUpdate:open": (value: boolean) => (open.value = value),
          },
          {
            default: () =>
              h(Stack, { gap: "3" }, () => [
                h(TextField, { label: "Nom", modelValue: "Site ouest" }),
                h(TextField, { label: "Adresse postale" }),
                h(TextField, { label: "Contact" }),
              ]),
            actions: () => [
              h(Button, { variant: "ghost", onClick: () => (open.value = false) }, () => "Annuler"),
              h(
                Button,
                { variant: "primary", onClick: () => (open.value = false) },
                () => "Enregistrer",
              ),
            ],
          },
        ),
      ]);
  },
});

export default {
  component: Drawer,
  pattern: "Dialog (modal), sur le côté de la page",
  states: [
    {
      name: "modal",
      description: "Dialogue latéral modal (formulaire long) ; le panneau non modal est SidePanel",
      props: {
        title: "Brouillon",
        description: "3 opérations, non soumises.",
        open: true,
      },
      demo,
      isolated: true,
    },
    {
      name: "keyboard",
      description:
        "Au clavier : ouvert par un bouton de l'hôte, focus sur le premier champ, piégé, Échap, focus rendu au bouton",
      props: {},
      demo: Keyboard,
    },
  ],
} satisfies CatalogueEntry;
