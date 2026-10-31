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

/**
 * A changeset of 60 objects of 6 changes (420 lines): virtualised, it scrolls in its place
 * and holds only the changes near the view (e2e/components/diff-list.spec.ts).
 */
const KINDS = ["changed", "added", "removed"] as const;
const long: DiffGroup[] = Array.from({ length: 60 }, (_, g) => ({
  id: `l${g}`,
  title: `Sous-réseau ${g + 1}`,
  objectKey: `192.0.2.${(g * 4) % 256}/30`,
  kind: KINDS[g % 3]!,
  changes: Array.from({ length: 6 }, (_, c) => ({
    attribute: `Attribut ${c + 1}`,
    kind: KINDS[(g + c) % 3]!,
    ...(KINDS[(g + c) % 3] !== "added" ? { before: `ancienne valeur ${c + 1}` } : {}),
    ...(KINDS[(g + c) % 3] !== "removed" ? { after: `nouvelle valeur ${c + 1}` } : {}),
  })),
}));

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
    {
      name: "long",
      description:
        "420 lignes : virtualisée, elle défile en place ; chaque objet reste atteignable au clavier",
      props: { groups: long },
      wide: true,
    },
  ],
} satisfies CatalogueEntry;
