// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { defineComponent, h, ref, type Component, type FunctionalComponent } from "vue";
import type { CatalogueEntry } from "../../catalogue.ts";
import { ids, prefixes, problems } from "../../fixtures/index.ts";
import Button from "../Button/Button.vue";
import CodeBadge from "../CodeBadge/CodeBadge.vue";
import Dialog from "../Dialog/Dialog.vue";
import IconButton from "../IconButton/IconButton.vue";
import Menu from "../Menu/Menu.vue";
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

/** The versions an inherited value went through: a body longer than the room it has. */
const versions = [
  ["v1", "2025-03-17", "import initial, sans propriétaire"],
  ["v2", "2025-06-01", "propriétaire posé sur 192.0.2.0/24 (Atelier Sud)"],
  ["v3", "2025-09-12", "propriétaire corrigé sur 192.0.2.0/24 (Atelier Nord)"],
  ["v4", "2025-12-02", "192.0.2.64/26 découpé, la valeur héritée sans changement"],
  ["v5", "2026-02-10", "règle env-par-site ajoutée, elle ne touche pas au propriétaire"],
  ["v6", "2026-05-04", "192.0.2.0/24 rattaché au site ouest"],
] as const;

/** A long explanation: the body scrolls in the room left by the viewport, a tab stop then. */
const Long = defineComponent({
  props: { defaultOpen: Boolean },
  setup(props) {
    return () =>
      h(
        Popover as Component,
        { title: "Propriétaire : historique de la valeur", defaultOpen: props.defaultOpen },
        {
          trigger: () =>
            h(Button, { variant: "ghost", size: "sm", icon: "clock" }, () => "Historique"),
          default: () =>
            versions.map(([version, date, what]) =>
              h("p", null, [h("code", null, version), ` (${date}) : ${what}.`]),
            ),
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

/**
 * A popover and a menu in a modal dialog, the menu its last control (keyboard scripts of
 * Popover and Menu): leaving either by Tab stays in the dialog and goes round, as its trap.
 */
const InDialog: FunctionalComponent = () =>
  h(
    Dialog as Component,
    { title: "Propriétés du préfixe", open: true },
    {
      default: () =>
        h(Stack, { gap: "3" }, () => [
          h(TextField, { label: "Nom" }),
          h("div", null, [h(Inherited)]),
          h("div", null, [
            h(Menu, {
              label: "Actions",
              items: [
                { id: "copy", label: "Copier l'identifiant", icon: "copy" },
                { id: "history", label: "Historique", icon: "clock" },
              ],
            }),
          ]),
        ]),
    },
  );

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
      name: "long",
      description:
        "Corps long : dans une fenêtre basse (zoom à 400 %), il défile, et devient alors un arrêt de tabulation nommé par le titre",
      props: { defaultOpen: true },
      demo: Long,
      isolated: true,
    },
    {
      name: "keyboard",
      description: "Au clavier : ouvrir, parcourir, fermer, sortir par Tab ou Maj+Tab",
      props: {},
      demo: Keyboard,
    },
    {
      name: "in-dialog",
      description:
        "Dans un dialogue modal, avec un menu en dernier contrôle : Tab qui en sort reste dans le dialogue",
      props: {},
      demo: InDialog,
      isolated: true,
    },
  ],
} satisfies CatalogueEntry;
