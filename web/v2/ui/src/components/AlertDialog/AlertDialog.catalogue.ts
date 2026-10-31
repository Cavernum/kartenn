// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { defineComponent, h, ref, type Component } from "vue";
import type { CatalogueEntry } from "../../catalogue.ts";
import Button from "../Button/Button.vue";
import Stack from "../Stack/Stack.vue";
import AlertDialog from "./AlertDialog.vue";

/**
 * The confirmation of the keyboard script (e2e/components/dialog.spec.ts): its button, and
 * what came of it, written under it.
 */
const Keyboard = defineComponent({
  setup() {
    const outcome = ref("Aucune réponse.");
    return () =>
      h(Stack, { gap: "3" }, () => [
        h(
          AlertDialog as Component,
          {
            title: "Abandonner le brouillon ?",
            description: "Les 12 opérations en brouillon seront perdues.",
            confirm: "Abandonner",
            danger: true,
            onConfirm: () => (outcome.value = "Brouillon abandonné."),
            onCancel: () => (outcome.value = "Abandon annulé."),
          },
          {
            trigger: () => h(Button, { icon: "trash" }, () => "Abandonner le brouillon"),
          },
        ),
        h("p", { "data-outcome": "" }, outcome.value),
      ]);
  },
});

export default {
  component: AlertDialog,
  pattern: "Alert Dialog (focus sur Annuler)",
  states: [
    {
      name: "destructive",
      description: "Confirmation destructive",
      props: {
        title: "Abandonner le brouillon ?",
        description: "Les 12 opérations en brouillon seront perdues.",
        confirm: "Abandonner",
        danger: true,
        open: true,
      },
      isolated: true,
    },
    {
      name: "confirm",
      description: "Confirmation simple",
      props: {
        title: "Appliquer le changeset ?",
        description: "Les modifications seront visibles de tous.",
        confirm: "Appliquer",
        open: true,
      },
      isolated: true,
    },
    {
      name: "keyboard",
      description:
        "Au clavier : ouverte par son bouton, focus sur « Annuler », piégé, Échap annule, focus rendu au bouton",
      props: {},
      demo: Keyboard,
    },
  ],
} satisfies CatalogueEntry;
