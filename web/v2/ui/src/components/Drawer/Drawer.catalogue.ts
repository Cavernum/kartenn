// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { h, type Component, type FunctionalComponent } from "vue";
import type { CatalogueEntry } from "../../catalogue.ts";
import { prefixes } from "../../fixtures/index.ts";
import Button from "../Button/Button.vue";
import Stack from "../Stack/Stack.vue";
import StatusBadge from "../StatusBadge/StatusBadge.vue";
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

export default {
  component: Drawer,
  pattern: "région complementary (non modal) ou Dialog (modal)",
  states: [
    {
      name: "non-modal",
      description: "Panneau du brouillon, non modal, à côté de la surface",
      props: {
        title: "Brouillon",
        description: "3 opérations, non soumises.",
        open: true,
      },
      demo,
    },
    {
      name: "modal",
      description: "Modal, sur le côté de la page",
      props: {
        title: "Brouillon",
        description: "3 opérations, non soumises.",
        open: true,
        modal: true,
      },
      demo,
      isolated: true,
    },
  ],
} satisfies CatalogueEntry;
