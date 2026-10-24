// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { h, type Component, type FunctionalComponent } from "vue";
import type { CatalogueEntry } from "../../catalogue.ts";
import Button from "../Button/Button.vue";
import EmptyState from "./EmptyState.vue";

const withAction: FunctionalComponent = (props) =>
  h(EmptyState as Component, props, {
    action: () => h(Button, { variant: "primary", icon: "plus", shortcut: ["c"] }, () => "Créer"),
  });

const withText: FunctionalComponent = (props) =>
  h(EmptyState as Component, props, {
    default: () => "Retirez un filtre ou élargissez la période.",
  });

export default {
  component: EmptyState,
  pattern: "— (une phrase et une action)",
  states: [
    {
      name: "action",
      description: "Une phrase et une action",
      props: { title: "Aucun préfixe dans ce domaine.", icon: "prefix" },
      demo: withAction,
      wide: true,
    },
    {
      name: "filtered",
      description: "Vide par les filtres, sans action",
      props: { title: "Aucun résultat pour ces filtres.", icon: "funnel" },
      demo: withText,
      wide: true,
    },
  ],
} satisfies CatalogueEntry;
