// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { h } from "vue";
import type { CatalogueEntry } from "../../catalogue.ts";
import { STATES } from "../../tokens/index.ts";
import Cluster from "../Cluster/Cluster.vue";
import StatusBadge from "./StatusBadge.vue";

const LABELS = {
  ok: "Conforme",
  warn: "À vérifier",
  ko: "En échec",
  review: "En revue",
  info: "Planifié",
  neutral: "Brouillon",
} as const;

/** The six states side by side. */
const All = () =>
  h(Cluster, { gap: "2" }, () =>
    STATES.map((state) => h(StatusBadge, { state, label: LABELS[state] })),
  );

export default {
  component: StatusBadge,
  pattern: "— (couleur, icône et libellé)",
  states: [
    { name: "all", description: "Les six états côte à côte", props: {}, demo: All },
    ...STATES.map((state) => ({
      name: state,
      description: `État ${state}`,
      props: { state, label: LABELS[state] },
    })),
  ],
} satisfies CatalogueEntry;
