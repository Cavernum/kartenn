// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import type { CatalogueEntry } from "../../catalogue.ts";
import { domains, prefixes } from "../../fixtures/index.ts";
import Breadcrumb from "./Breadcrumb.vue";

export default {
  component: Breadcrumb,
  pattern: "Breadcrumb (aria-current=page)",
  states: [
    {
      name: "record",
      description: "Fiche d'un préfixe, la clé en chasse fixe",
      props: {
        items: [
          { label: "Référentiel", to: "/" },
          { label: domains[0].label, to: `/domains/${domains[0].key}` },
          { label: prefixes[0], to: "/prefixes/1", mono: true },
          { label: prefixes[1], mono: true },
        ],
      },
    },
    {
      name: "short",
      description: "Deux niveaux",
      props: { items: [{ label: "Changesets", to: "/changesets" }, { label: "À instruire" }] },
    },
    {
      name: "hover",
      description: "Survol d'un lien",
      props: { items: [{ label: "Changesets", to: "/changesets" }, { label: "À instruire" }] },
      interaction: "hover",
      target: "a",
    },
  ],
} satisfies CatalogueEntry;
