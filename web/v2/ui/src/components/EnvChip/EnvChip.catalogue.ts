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

export default {
  component: EnvChip,
  pattern: "— (pastille et libellé)",
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
    { name: "palette", description: "Les six teintes", props: {}, demo: Palette },
  ],
} satisfies CatalogueEntry;
