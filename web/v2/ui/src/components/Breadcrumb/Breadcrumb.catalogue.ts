// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { h, type FunctionalComponent } from "vue";
import type { CatalogueEntry } from "../../catalogue.ts";
import DemoColumn from "../../demo/DemoColumn.vue";
import { domains, prefixes } from "../../fixtures/index.ts";
import Breadcrumb from "./Breadcrumb.vue";
import type { Crumb } from "./breadcrumb.ts";

const record: Crumb[] = [
  { label: "Référentiel", to: "/" },
  { label: domains[0].label, to: `/domains/${domains[0].key}` },
  { label: prefixes[0], to: "/prefixes/1", mono: true },
  { label: prefixes[1], mono: true },
];

/** The same path in a narrow column: the earlier crumbs give way, cut by an ellipsis. */
const Narrow: FunctionalComponent = () =>
  h(DemoColumn, { size: "sm" }, () => h(Breadcrumb, { items: record }));

export default {
  component: Breadcrumb,
  pattern: "Breadcrumb (aria-current=page)",
  states: [
    {
      name: "record",
      description: "Fiche d'un préfixe, la clé en chasse fixe",
      props: { items: record },
    },
    {
      name: "cut",
      description:
        "À l'étroit : la page courante garde son nom, les autres cèdent ; le libellé entier d'un lien coupé en infobulle au focus",
      props: {},
      demo: Narrow,
      interaction: "focus",
      target: "li:nth-child(2) a",
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
