// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { h, type Component, type FunctionalComponent } from "vue";
import type { CatalogueEntry } from "../../catalogue.ts";
import { prefixes } from "../../fixtures/index.ts";
import Button from "../Button/Button.vue";
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
  ],
} satisfies CatalogueEntry;
