// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { h } from "vue";
import type { CatalogueEntry } from "../../catalogue.ts";
import { environments } from "../../fixtures/index.ts";
import Cluster from "../Cluster/Cluster.vue";
import EnvChip from "./EnvChip.vue";

/** The six hues of the closed palette. */
const Palette = () =>
  h(Cluster, { gap: "2" }, () =>
    ([1, 2, 3, 4, 5, 6] as const).map((hue) =>
      h(EnvChip, { value: `env-${hue}`, label: `Environnement ${hue}`, hue }),
    ),
  );

/** Hues read from the keys (`envHue`), no hue given: the same for every user. */
const FromKey = () =>
  h(Cluster, { gap: "2" }, () =>
    ["prod", "rece", "deve", "form", "preprod", "dmz"].map((key) =>
      h(EnvChip, { value: key, label: null }),
    ),
  );

export default {
  component: EnvChip,
  pattern: "Button (copier la clé) ; pastille et libellé",
  states: [
    {
      name: "labelled",
      description: "Pastille et libellé",
      props: { value: environments[0].key, label: environments[0].label, hue: environments[0].hue },
    },
    {
      name: "key-only",
      description: "Sans libellé : la clé en chasse fixe",
      props: { value: environments[3].key, label: environments[3].label, hue: environments[3].hue },
    },
    {
      name: "hover",
      description: "Survol : la clé dans la bulle",
      props: { value: environments[1].key, label: environments[1].label, hue: environments[1].hue },
      interaction: "hover",
    },
    {
      name: "focus",
      description: "Focus visible",
      props: { value: environments[1].key, label: environments[1].label, hue: environments[1].hue },
      interaction: "focus",
    },
    {
      name: "plain",
      description: "Sans bouton (dans un contrôle) : la clé toujours à côté",
      props: {
        value: environments[0].key,
        label: environments[0].label,
        hue: environments[0].hue,
        plain: true,
      },
    },
    { name: "palette", description: "Les six teintes", props: {}, demo: Palette },
    { name: "from-key", description: "Teinte déduite de la clé", props: {}, demo: FromKey },
  ],
} satisfies CatalogueEntry;
