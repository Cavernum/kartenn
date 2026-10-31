// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { defineComponent, h, ref, type Component, type FunctionalComponent } from "vue";
import type { CatalogueEntry } from "../../catalogue.ts";
import { prefixes } from "../../fixtures/index.ts";
import Button from "../Button/Button.vue";
import Cluster from "../Cluster/Cluster.vue";
import Menu from "../Menu/Menu.vue";
import Stack from "../Stack/Stack.vue";
import TextField from "../TextField/TextField.vue";
import Dialog from "./Dialog.vue";

/** A creation form in a dialog, its actions at the end. */
const demo: FunctionalComponent = (props) =>
  h(Dialog as Component, props, {
    default: () =>
      h(Stack, { gap: "3" }, () => [
        h(TextField, { label: "Préfixe", mono: true, modelValue: prefixes[3], required: true }),
        h(TextField, { label: "Nom", hint: "Visible dans les listes." }),
      ]),
    actions: () => [
      h(Button, { variant: "ghost" }, () => "Annuler"),
      h(Button, { variant: "primary", shortcut: ["Mod", "Enter"] }, () => "Créer"),
    ],
  });

/**
 * The dialogs of the keyboard script (e2e/components/dialog.spec.ts): one opened by its
 * button (a Reka trigger), one by an entry of a menu, which is gone when it closes (the
 * focus then goes back to the menu's button).
 */
const Keyboard = defineComponent({
  setup() {
    const created = ref(false);
    const renaming = ref(false);
    const name = ref("Salle A");
    return () =>
      h(Stack, { gap: "3" }, () => [
        h(Cluster, { gap: "2" }, () => [
          h(
            Dialog as Component,
            {
              title: "Nouveau préfixe",
              description: "Le préfixe rejoint le brouillon.",
              open: created.value,
              "onUpdate:open": (open: boolean) => (created.value = open),
            },
            {
              trigger: () => h(Button, () => "Nouveau préfixe"),
              default: () =>
                h(Stack, { gap: "3" }, () => [
                  h(TextField, { label: "Préfixe", mono: true }),
                  h(TextField, { label: "Nom" }),
                ]),
              actions: () => [
                h(
                  Button,
                  { variant: "ghost", onClick: () => (created.value = false) },
                  () => "Annuler",
                ),
                h(
                  Button,
                  { variant: "primary", onClick: () => (created.value = false) },
                  () => "Créer",
                ),
              ],
            },
          ),
          h(Menu, {
            label: "Actions",
            items: [
              { id: "rename", label: "Renommer…", icon: "pencil" },
              { id: "copy", label: "Copier l'identifiant", icon: "copy" },
            ],
            onSelect: (id: string) => {
              if (id === "rename") renaming.value = true;
            },
          }),
        ]),
        h(TextField, { label: "Note" }),
        h(
          Dialog as Component,
          {
            title: "Renommer",
            size: "sm",
            open: renaming.value,
            "onUpdate:open": (open: boolean) => (renaming.value = open),
          },
          {
            default: () =>
              h(TextField, {
                label: "Nom",
                modelValue: name.value,
                "onUpdate:modelValue": (value: string) => (name.value = value),
              }),
            actions: () => [
              h(
                Button,
                { variant: "primary", onClick: () => (renaming.value = false) },
                () => "Renommer",
              ),
            ],
          },
        ),
      ]);
  },
});

export default {
  component: Dialog,
  pattern: "Dialog (Modal)",
  states: [
    {
      name: "open",
      description: "Ouvert : titre, description, formulaire, actions",
      props: {
        title: "Nouveau préfixe",
        description: "Le préfixe rejoint le brouillon ; rien n'est appliqué avant validation.",
        open: true,
      },
      demo,
      isolated: true,
    },
    {
      name: "small",
      description: "Petit, sans description",
      props: { title: "Renommer", size: "sm", open: true },
      demo,
      isolated: true,
    },
    {
      name: "keyboard",
      description:
        "Au clavier : ouvert par son bouton ou par une entrée de menu, focus sur le premier champ, piégé, Échap, focus rendu (au bouton du menu quand l'entrée n'est plus là)",
      props: {},
      demo: Keyboard,
    },
  ],
} satisfies CatalogueEntry;
