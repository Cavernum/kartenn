// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { defineComponent, h, ref, type Component } from "vue";
import type { CatalogueEntry } from "../../catalogue.ts";
import { ids, prefixes, problems } from "../../fixtures/index.ts";
import Button from "../Button/Button.vue";
import CodeBadge from "../CodeBadge/CodeBadge.vue";
import IconButton from "../IconButton/IconButton.vue";
import Stack from "../Stack/Stack.vue";
import TextField from "../TextField/TextField.vue";
import Popover from "./Popover.vue";

/** The explanation of an inherited value (E2): where it comes from, what set it. */
const explanation = () => [
  h("p", null, [
    "Héritée de ",
    h("code", null, prefixes[0]),
    " (Salle A), où elle est posée directement depuis la version v3 ; aucune règle ne la remplace ici.",
  ]),
  h("p", null, ["Posée par le changeset ", h(CodeBadge, { code: ids.changeset }), "."]),
];

/** A popover on the explanation of a value, its trigger a quiet button in a sentence. */
const Inherited = defineComponent({
  props: { defaultOpen: Boolean },
  setup(props) {
    return () =>
      h(
        Popover as Component,
        { title: "Propriétaire : valeur héritée", defaultOpen: props.defaultOpen },
        {
          trigger: () => h(Button, { variant: "ghost", size: "sm", icon: "info" }, () => "Héritée"),
          default: explanation,
        },
      );
  },
});

/** The detail of a code, from an icon button beside it, the narrow width. */
const Code = defineComponent({
  props: { defaultOpen: Boolean },
  setup(props) {
    return () =>
      h("p", { class: "k-cat-note" }, [
        h("code", null, problems.violation),
        " ",
        h(
          Popover as Component,
          { title: "Chevauchement d'un voisin", size: "sm", defaultOpen: props.defaultOpen },
          {
            trigger: () =>
              h(IconButton, {
                icon: "info",
                label: "Détail du code",
                size: "sm",
                variant: "ghost",
              }),
            default: () =>
              h(
                "p",
                null,
                "Le préfixe recouvre un préfixe voisin du même parent : deux réseaux ne peuvent pas se partager des adresses.",
              ),
          },
        ),
      ]);
  },
});

/**
 * The popover of the keyboard script (e2e/components/popover.spec.ts): a field before
 * and after its trigger, to leave it by Tab and Shift+Tab.
 */
const Keyboard = defineComponent({
  setup() {
    const before = ref("");
    const after = ref("");
    return () =>
      h(Stack, { gap: "3" }, () => [
        h(TextField, {
          label: "Préfixe",
          modelValue: before.value,
          "onUpdate:modelValue": (value: string) => (before.value = value),
        }),
        // In a block of its own: the stack would stretch the button.
        h("div", null, [h(Inherited)]),
        h(TextField, {
          label: "Description",
          modelValue: after.value,
          "onUpdate:modelValue": (value: string) => (after.value = value),
        }),
      ]);
  },
});

export default {
  component: Popover,
  pattern: "Dialog non modal",
  states: [
    {
      name: "closed",
      description: "Fermé : le bouton dit qu'il ouvre un dialogue (aria-haspopup, aria-expanded)",
      props: {},
      demo: Inherited,
    },
    {
      name: "open",
      description:
        "Ouvert : explication d'une valeur héritée, nommée par son titre, le focus sur elle",
      props: { defaultOpen: true },
      demo: Inherited,
      isolated: true,
    },
    {
      name: "code",
      description: "Détail d'un code, ouvert d'un bouton-icône, largeur sm",
      props: { defaultOpen: true },
      demo: Code,
      isolated: true,
    },
    {
      name: "keyboard",
      description: "Au clavier : ouvrir, parcourir, fermer, sortir par Tab ou Maj+Tab",
      props: {},
      demo: Keyboard,
    },
  ],
} satisfies CatalogueEntry;
