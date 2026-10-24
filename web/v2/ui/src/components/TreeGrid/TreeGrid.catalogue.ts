// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import type { CatalogueEntry } from "../../catalogue.ts";
import type { GridColumn } from "../DataGrid/grid.ts";
import TreeGrid from "./TreeGrid.vue";
import type { TreeNode, TreeNodeState } from "./tree.ts";

const columns: GridColumn[] = [
  { id: "env", label: "Environnement", width: "md" },
  { id: "used", label: "Occupation", width: "sm", align: "end" },
];

/** An inclusion tree of documentation prefixes. */
const nodes: TreeNode[] = [
  {
    id: "d1",
    label: "Centre de données nord",
    icon: "domain",
    children: [
      {
        id: "p1",
        label: "Serveurs",
        objectKey: "192.0.2.0/24",
        icon: "prefix",
        cells: { env: "Production", used: "62 %" },
        children: [
          {
            id: "p2",
            label: "Bases",
            objectKey: "192.0.2.0/26",
            icon: "prefix",
            cells: { env: "Production", used: "12 %" },
          },
          {
            id: "p3",
            label: "Supervision",
            objectKey: "192.0.2.64/26",
            icon: "prefix",
            cells: { env: "Recette", used: "45 %" },
          },
        ],
      },
      {
        id: "p4",
        label: "Postes",
        objectKey: "198.51.100.0/24",
        icon: "prefix",
        hasChildren: true,
        cells: { env: "Production", used: "87 %" },
      },
      {
        id: "p5",
        label: "Invités",
        objectKey: "203.0.113.0/24",
        icon: "prefix",
        cells: { env: "Développement", used: "3 %" },
      },
    ],
  },
];

const base = {
  label: "Arbre des préfixes",
  treeLabel: "Préfixe",
  nodes,
  columns,
  expanded: ["d1", "p1"],
};
const stateBy =
  (states: Record<string, TreeNodeState>) =>
  (node: TreeNode): TreeNodeState | undefined =>
    states[node.id];

export default {
  component: TreeGrid,
  pattern: "Treegrid",
  states: [
    {
      name: "expanded",
      description: "Domaine et préfixe dépliés, un nœud sélectionné",
      props: { ...base, selected: "p2" },
      wide: true,
    },
    {
      name: "loading-children",
      description: "Chargement des enfants d'un nœud",
      props: {
        ...base,
        expanded: ["d1", "p4"],
        nodeState: stateBy({ p4: "loading-children" }),
      },
      wide: true,
    },
    {
      name: "draft",
      description: "Nœud en brouillon",
      props: { ...base, nodeState: stateBy({ p3: "draft" }) },
      wide: true,
    },
    {
      name: "drop-targets",
      description: "Glisser : cible acceptée, cible refusée avec motif et code",
      props: {
        ...base,
        nodeState: stateBy({
          p4: "drop-ok",
          p2: {
            drop: "refused",
            reason: "Un préfixe ne peut contenir un préfixe plus large.",
            code: "prefix.parent-narrower",
          },
        }),
      },
      wide: true,
    },
    {
      name: "move-states",
      description: "Déplacement en attente (à sa place d'origine), déplacement refusé",
      props: {
        ...base,
        nodeState: stateBy({
          p3: "move-pending",
          p5: { refused: { reason: "Parent déduit de l'inclusion.", code: "tree.derived-parent" } },
        }),
      },
      wide: true,
    },
  ],
} satisfies CatalogueEntry;
