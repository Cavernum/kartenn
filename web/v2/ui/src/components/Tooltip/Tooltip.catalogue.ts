// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { h, type FunctionalComponent } from "vue";
import type { CatalogueEntry } from "../../catalogue.ts";
import Button from "../Button/Button.vue";
import Cluster from "../Cluster/Cluster.vue";
import IconButton from "../IconButton/IconButton.vue";
import Tooltip from "./Tooltip.vue";

/** A refused action beside an allowed one: the tooltip opens on the refused one. */
const demo: FunctionalComponent<{ kind: "button" | "icon" | "reason" }> = ({ kind }) =>
  h(Cluster, { gap: "2" }, () => [
    kind === "icon"
      ? h(IconButton, { icon: "trash", label: "Supprimer", forbidden: "prefix.delete" })
      : kind === "reason"
        ? h(
            Button,
            {
              icon: "circle-check",
              reason: "Une politique d'approbation retient ce changeset.",
            },
            () => "Appliquer",
          )
        : h(Button, { icon: "pencil", forbidden: "prefix.edit" }, () => "Modifier"),
    h(Button, { icon: "copy" }, () => "Copier"),
  ]);

export default {
  component: Tooltip,
  pattern: "Tooltip (action refusée : aria-disabled, focalisable)",
  states: [
    {
      name: "forbidden",
      description: "Bouton interdit au focus : « Il faut prefix.edit »",
      props: { kind: "button" },
      demo,
      interaction: "focus",
    },
    {
      name: "icon",
      description: "Bouton-icône interdit au focus",
      props: { kind: "icon" },
      demo,
      interaction: "focus",
    },
    {
      name: "reason",
      description: "Refusé par une politique : la phrase de l'hôte",
      props: { kind: "reason" },
      demo,
      interaction: "focus",
    },
  ],
} satisfies CatalogueEntry;
