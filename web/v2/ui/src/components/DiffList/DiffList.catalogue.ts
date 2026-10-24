// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import type { CatalogueEntry } from "../../catalogue.ts";
import { prefixes } from "../../fixtures/index.ts";
import type { DiffGroup } from "./diff.ts";
import DiffList from "./DiffList.vue";

const groups: DiffGroup[] = [
  {
    id: "g1",
    title: "Serveurs de bases",
    objectKey: prefixes[1],
    kind: "changed",
    changes: [
      { attribute: "Libellé", kind: "changed", before: "Bases", after: "Serveurs de bases" },
      { attribute: "Responsable", kind: "added", after: "Équipe réseau" },
      { attribute: "Commentaire", kind: "removed", before: "À revoir" },
    ],
  },
  {
    id: "g2",
    title: "Invités",
    objectKey: prefixes[4],
    kind: "added",
    changes: [
      { attribute: "Préfixe", kind: "added", after: prefixes[4], mono: true },
      { attribute: "Environnement", kind: "added", after: "Développement" },
    ],
  },
  {
    id: "g3",
    title: "Supervision",
    objectKey: prefixes[2],
    kind: "anomaly",
    changes: [
      {
        attribute: "Chevauchement",
        kind: "anomaly",
        before: prefixes[2],
        after: "192.0.2.96/27",
        mono: true,
      },
    ],
  },
];

export default {
  component: DiffList,
  pattern: "liste groupée par objet, Disclosure",
  states: [
    {
      name: "open",
      description: "Modification, ajout, anomalie, avant et après",
      props: { groups },
      wide: true,
    },
    {
      name: "collapsed",
      description: "Un objet replié",
      props: { groups, collapsed: ["g1"] },
      wide: true,
    },
  ],
} satisfies CatalogueEntry;
